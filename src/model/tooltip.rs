// SPEC-HAL-226: Tooltip Payload Model
// SPEC-HAL-226A: Tooltip Framework — Manager, Placement, Trigger Registration

use crate::core::math::{Point, RectF, Size};
use crate::utils::id::TreeWidgetId;

/// Tooltip content payload.
/// Plain data only — no callbacks, no behavior (INV-226-A1).
#[derive(Debug, Clone, PartialEq)]
pub enum TooltipPayload {
    /// Plain text tooltip.
    Text(String),
}

// ============================================================================
// SPEC-HAL-226A: Tooltip trigger registration
// ============================================================================

/// A tooltip trigger region registered by a widget during paint.
#[derive(Debug, Clone)]
pub struct TooltipTrigger {
    pub widget_id: TreeWidgetId,
    pub trigger_rect: RectF,
    pub content: TooltipPayload,
    pub placement: TooltipPlacement,
}

// ============================================================================
// SPEC-HAL-226A: Tooltip timing configuration
// ============================================================================

/// Tooltip timing configuration (from theme).
#[derive(Debug, Clone, Copy)]
pub struct TooltipTiming {
    /// Delay before showing tooltip (ms). Default: 500.
    pub show_delay_ms: u32,
    /// Delay before hiding after pointer leaves (ms). Default: 0 (instant).
    pub hide_delay_ms: u32,
    /// If a tooltip was dismissed within this window and the pointer enters
    /// a new trigger, show immediately (no delay). Default: 100ms.
    pub reshow_window_ms: u32,
}

impl Default for TooltipTiming {
    fn default() -> Self {
        Self {
            show_delay_ms: 500,
            hide_delay_ms: 0,
            reshow_window_ms: 100,
        }
    }
}

// ============================================================================
// SPEC-HAL-226A: Tooltip placement
// ============================================================================

/// Preferred tooltip placement relative to trigger rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPlacement {
    Above,
    Below,
    Left,
    Right,
    Auto,
}

/// Compute tooltip anchor position.
/// Explicit placements stay on their orientation axis; Auto may use any side.
/// Clamps to viewport in all cases.
pub fn compute_tooltip_anchor(
    trigger_rect: RectF,
    tooltip_size: Size,
    viewport: RectF,
    gap: f32,
    preferred: TooltipPlacement,
) -> Point {
    let order: &[TooltipPlacement] = match preferred {
        TooltipPlacement::Below => &[TooltipPlacement::Below, TooltipPlacement::Above],
        TooltipPlacement::Above => &[TooltipPlacement::Above, TooltipPlacement::Below],
        TooltipPlacement::Right => &[TooltipPlacement::Right, TooltipPlacement::Left],
        TooltipPlacement::Left => &[TooltipPlacement::Left, TooltipPlacement::Right],
        TooltipPlacement::Auto => &[
            TooltipPlacement::Below,
            TooltipPlacement::Above,
            TooltipPlacement::Right,
            TooltipPlacement::Left,
        ],
    };

    for placement in order {
        let (x, y) = tooltip_anchor_for_placement(*placement, trigger_rect, tooltip_size, gap);

        if tooltip_fits_viewport(x, y, tooltip_size, viewport) {
            return Point::new(x, y);
        }
    }

    let fallback = match preferred {
        TooltipPlacement::Auto => TooltipPlacement::Below,
        placement => placement,
    };
    let (x, y) = tooltip_anchor_for_placement(fallback, trigger_rect, tooltip_size, gap);
    Point::new(
        x.clamp(
            viewport.x,
            (viewport.x + viewport.width - tooltip_size.width).max(viewport.x),
        ),
        y.clamp(
            viewport.y,
            (viewport.y + viewport.height - tooltip_size.height).max(viewport.y),
        ),
    )
}

fn tooltip_anchor_for_placement(
    placement: TooltipPlacement,
    trigger_rect: RectF,
    tooltip_size: Size,
    gap: f32,
) -> (f32, f32) {
    match placement {
        TooltipPlacement::Below => (
            trigger_rect.x + (trigger_rect.width - tooltip_size.width) * 0.5,
            trigger_rect.y + trigger_rect.height + gap,
        ),
        TooltipPlacement::Above => (
            trigger_rect.x + (trigger_rect.width - tooltip_size.width) * 0.5,
            trigger_rect.y - tooltip_size.height - gap,
        ),
        TooltipPlacement::Right => (
            trigger_rect.x + trigger_rect.width + gap,
            trigger_rect.y + (trigger_rect.height - tooltip_size.height) * 0.5,
        ),
        TooltipPlacement::Left => (
            trigger_rect.x - tooltip_size.width - gap,
            trigger_rect.y + (trigger_rect.height - tooltip_size.height) * 0.5,
        ),
        TooltipPlacement::Auto => unreachable!(),
    }
}

fn tooltip_fits_viewport(x: f32, y: f32, tooltip_size: Size, viewport: RectF) -> bool {
    x >= viewport.x
        && y >= viewport.y
        && x + tooltip_size.width <= viewport.x + viewport.width
        && y + tooltip_size.height <= viewport.y + viewport.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_above_never_falls_through_to_right() {
        let anchor = compute_tooltip_anchor(
            RectF::new(50.0, 16.0, 20.0, 12.0),
            Size::new(40.0, 16.0),
            RectF::new(0.0, 5.0, 200.0, 30.0),
            3.0,
            TooltipPlacement::Above,
        );

        assert_eq!(anchor, Point::new(40.0, 5.0));
    }

    #[test]
    fn explicit_right_never_falls_through_to_below() {
        let anchor = compute_tooltip_anchor(
            RectF::new(65.0, 40.0, 20.0, 20.0),
            Size::new(40.0, 20.0),
            RectF::new(50.0, 0.0, 70.0, 200.0),
            4.0,
            TooltipPlacement::Right,
        );

        assert_eq!(anchor, Point::new(80.0, 40.0));
    }

    #[test]
    fn auto_can_use_right_when_vertical_does_not_fit() {
        let anchor = compute_tooltip_anchor(
            RectF::new(50.0, 16.0, 20.0, 12.0),
            Size::new(40.0, 16.0),
            RectF::new(0.0, 5.0, 200.0, 30.0),
            3.0,
            TooltipPlacement::Auto,
        );

        assert_eq!(anchor, Point::new(73.0, 14.0));
    }
}

// ============================================================================
// SPEC-HAL-226A: Tooltip lifecycle manager
// ============================================================================

struct TooltipHoverTarget {
    widget_id: TreeWidgetId,
    trigger_rect: RectF,
    content: TooltipPayload,
    placement: TooltipPlacement,
}

enum TooltipState {
    /// No tooltip activity.
    Idle,
    /// Pointer entered trigger region, waiting for show delay.
    WaitingToShow { elapsed_ms: f32 },
    /// Tooltip is visible.
    Visible,
    /// Pointer left, waiting for hide delay.
    WaitingToHide { elapsed_ms: f32 },
}

/// Framework-level tooltip lifecycle manager.
/// Lives in AppRoot, not in individual widgets.
pub struct TooltipManager {
    /// Current hover target: widget ID + trigger rect + content.
    hover_target: Option<TooltipHoverTarget>,
    /// Timer state.
    state: TooltipState,
    /// Timing configuration (synced from theme).
    timing: TooltipTiming,
    /// Timestamp of last dismiss (for reshow shortcut).
    last_dismiss_ms: u64,
    /// Computed anchor when visible.
    anchor: Point,
    /// Tooltip triggers registered during the current paint frame.
    triggers: Vec<TooltipTrigger>,
}

impl TooltipManager {
    pub fn new(timing: TooltipTiming) -> Self {
        Self {
            hover_target: None,
            state: TooltipState::Idle,
            timing,
            last_dismiss_ms: 0,
            anchor: Point::zero(),
            triggers: Vec::new(),
        }
    }

    /// Called when pointer enters a widget that has a registered tooltip.
    pub fn hover_enter(
        &mut self,
        widget_id: TreeWidgetId,
        trigger_rect: RectF,
        content: TooltipPayload,
        placement: TooltipPlacement,
        frame_time_ms: u64,
    ) {
        // Keep the lifecycle only when the same widget-owned trigger remains
        // active. Composite display widgets may register several item regions.
        if let Some(ref mut target) = self.hover_target {
            if target.widget_id == widget_id && target.trigger_rect == trigger_rect {
                target.content = content;
                target.placement = placement;
                return;
            }
        }

        self.hover_target = Some(TooltipHoverTarget {
            widget_id,
            trigger_rect,
            content,
            placement,
        });

        // Reshow shortcut: if dismissed recently, show immediately
        if frame_time_ms.saturating_sub(self.last_dismiss_ms) <= self.timing.reshow_window_ms as u64
        {
            self.state = TooltipState::WaitingToShow {
                elapsed_ms: self.timing.show_delay_ms as f32,
            };
        } else {
            self.state = TooltipState::WaitingToShow { elapsed_ms: 0.0 };
        }
    }

    /// Called when pointer leaves the current tooltip trigger region.
    pub fn hover_leave(&mut self) {
        if self.timing.hide_delay_ms == 0 {
            self.dismiss();
        } else {
            self.state = TooltipState::WaitingToHide { elapsed_ms: 0.0 };
        }
    }

    /// Called on any pointer-down event (dismiss active tooltip).
    pub fn pointer_down(&mut self) {
        self.dismiss();
    }

    /// Called on Escape key (dismiss active tooltip).
    pub fn escape_pressed(&mut self) {
        self.dismiss();
    }

    /// Suppress tooltip display while another interaction owns attention.
    pub fn suppress_interaction(&mut self) {
        self.dismiss();
    }

    /// Advance timer by dt seconds. Returns true if tooltip visibility changed.
    pub fn tick(&mut self, dt: f32, frame_time_ms: u64) -> bool {
        match &mut self.state {
            TooltipState::Idle => false,
            TooltipState::WaitingToShow { elapsed_ms } => {
                *elapsed_ms += dt * 1000.0;
                if *elapsed_ms >= self.timing.show_delay_ms as f32 {
                    self.state = TooltipState::Visible;
                    true
                } else {
                    false
                }
            }
            TooltipState::Visible => false,
            TooltipState::WaitingToHide { elapsed_ms } => {
                *elapsed_ms += dt * 1000.0;
                if *elapsed_ms >= self.timing.hide_delay_ms as f32 {
                    self.last_dismiss_ms = frame_time_ms;
                    self.hover_target = None;
                    self.state = TooltipState::Idle;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Is the tooltip currently visible?
    pub fn is_visible(&self) -> bool {
        matches!(self.state, TooltipState::Visible)
    }

    /// Whether the manager needs continuous frames (timer running).
    pub fn needs_tick(&self) -> bool {
        matches!(
            self.state,
            TooltipState::WaitingToShow { .. } | TooltipState::WaitingToHide { .. }
        )
    }

    /// Get current tooltip content (if visible).
    pub fn content(&self) -> Option<&TooltipPayload> {
        if self.is_visible() {
            self.hover_target.as_ref().map(|t| &t.content)
        } else {
            None
        }
    }

    /// Get the trigger rect (if hover target exists).
    pub fn trigger_rect(&self) -> Option<RectF> {
        self.hover_target.as_ref().map(|t| t.trigger_rect)
    }

    pub(crate) fn hover_target_snapshot(&self) -> Option<TooltipTrigger> {
        self.hover_target.as_ref().map(|target| TooltipTrigger {
            widget_id: target.widget_id,
            trigger_rect: target.trigger_rect,
            content: target.content.clone(),
            placement: target.placement,
        })
    }

    pub fn placement(&self) -> TooltipPlacement {
        self.hover_target
            .as_ref()
            .map(|t| t.placement)
            .unwrap_or(TooltipPlacement::Auto)
    }

    /// Get computed anchor position (if visible).
    pub fn anchor(&self) -> Point {
        self.anchor
    }

    /// Set computed anchor.
    pub fn set_anchor(&mut self, anchor: Point) {
        self.anchor = anchor;
    }

    /// Get mutable access to the trigger collection for paint-frame registration.
    pub fn triggers_mut(&mut self) -> &mut Vec<TooltipTrigger> {
        &mut self.triggers
    }

    /// Clear triggers (called at start of each paint frame).
    pub fn clear_triggers(&mut self) {
        self.triggers.clear();
    }

    /// Find a trigger for the given pointer position.
    pub fn find_trigger_at(&self, x: f32, y: f32) -> Option<&TooltipTrigger> {
        self.triggers
            .iter()
            .find(|t| t.trigger_rect.contains_point(x, y))
    }

    /// Update hover state from pointer position against registered triggers.
    /// Consumes the latest published trigger snapshot at the lifecycle checkpoint.
    pub fn update_from_pointer(&mut self, pointer_x: f32, pointer_y: f32, frame_time_ms: u64) {
        if let Some(trigger) = self
            .triggers
            .iter()
            .find(|t| t.trigger_rect.contains_point(pointer_x, pointer_y))
        {
            self.hover_enter(
                trigger.widget_id,
                trigger.trigger_rect,
                trigger.content.clone(),
                trigger.placement,
                frame_time_ms,
            );
        } else if self.hover_target.is_some() {
            self.hover_leave();
        }
    }

    fn dismiss(&mut self) {
        if matches!(
            self.state,
            TooltipState::Visible | TooltipState::WaitingToShow { .. }
        ) {
            self.last_dismiss_ms = 0; // Will be set properly with frame_time_ms from tick
        }
        self.hover_target = None;
        self.state = TooltipState::Idle;
    }
}
