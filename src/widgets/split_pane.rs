// SPEC-HAL-267: SplitPane Container and Resize Invariants

use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::event::{
    CursorIcon, Event, EventPhase, KeyCode, KeyEvent, PointerButton, PointerEvent, PointerEventKind,
};
use crate::core::layout::{Axis, Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::style::{DecorationStack, DecorationState, FocusStyle};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// Layout gap between panels.
const HANDLE_THICKNESS: f32 = 3.0;

/// Visual thickness of the painted handle bar, centered within the gutter.
const HANDLE_VISUAL_THICKNESS: f32 = 1.0;

/// Interactive hit band for drag initiation, centered on the layout handle.
/// Wider than the visible separator so users do not need pixel-perfect aim.
const HANDLE_HIT_THICKNESS: f32 = 9.0;

/// Keyboard step size in logical pixels.
const KB_STEP: f32 = 8.0;

/// When the SplitPane paints its divider.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SplitPaneDividerVisibility {
    #[default]
    OnInteraction,
    Always,
}

/// Construction-time presentation policy for one SplitPane.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SplitPaneInteractionPolicy {
    pub divider_visibility: SplitPaneDividerVisibility,
}

/// A two-panel resizable container with a draggable handle.
///
/// Horizontal = side-by-side panels, Vertical = stacked panels.
/// The handle is an internal interactive affordance — not a child widget.
pub struct SplitPane {
    id: WidgetInternalId,
    /// First child (left or top)
    pub leading: NodeId,
    /// Second child (right or bottom)
    pub trailing: NodeId,
    /// Horizontal = side-by-side, Vertical = stacked
    orientation: Axis,
    interaction_policy: SplitPaneInteractionPolicy,
    /// Current split ratio [0.0, 1.0] — fraction of available space to leading panel
    split_ratio: f32,
    /// Ratio to restore on double-click reset
    initial_ratio: f32,
    /// Min size for leading panel (logical px along split axis)
    leading_min: f32,
    /// Min size for trailing panel (logical px along split axis)
    trailing_min: f32,
    /// Drag state: Some during active drag, None otherwise
    drag_state: Option<DragState>,
    /// Whether the handle currently has keyboard focus
    focused: bool,
    /// Whether the pointer is currently hovering the handle hit zone
    handle_hovered: bool,
    /// Cached layout bounds (set during layout, read during paint/event)
    layout_bounds: Size,
    /// Callback fired when split_ratio changes. Advisory — SplitPane owns the ratio.
    pub on_ratio_changed: Option<Box<dyn FnMut(f32)>>,
    focus_t: Cell<f32>,
    focus_anim: Cell<Option<TransitionHandle>>,
}

struct DragState {
    /// Split ratio at drag start (for cancel/restore)
    start_ratio: f32,
    /// Pointer position along split axis at drag start
    start_pos: f32,
    /// Available space at drag start (for delta→ratio conversion)
    available: f32,
}

impl SplitPane {
    pub fn horizontal(leading: NodeId, trailing: NodeId) -> Self {
        Self {
            id: WidgetInternalId::new(),
            leading,
            trailing,
            orientation: Axis::Horizontal,
            interaction_policy: SplitPaneInteractionPolicy::default(),
            split_ratio: 0.5,
            initial_ratio: 0.5,
            leading_min: 0.0,
            trailing_min: 0.0,
            drag_state: None,
            focused: false,
            handle_hovered: false,
            layout_bounds: Size::new(0.0, 0.0),
            on_ratio_changed: None,
            focus_t: Cell::new(0.0),
            focus_anim: Cell::new(None),
        }
    }

    pub fn vertical(leading: NodeId, trailing: NodeId) -> Self {
        Self {
            id: WidgetInternalId::new(),
            leading,
            trailing,
            orientation: Axis::Vertical,
            interaction_policy: SplitPaneInteractionPolicy::default(),
            split_ratio: 0.5,
            initial_ratio: 0.5,
            leading_min: 0.0,
            trailing_min: 0.0,
            drag_state: None,
            focused: false,
            handle_hovered: false,
            layout_bounds: Size::new(0.0, 0.0),
            on_ratio_changed: None,
            focus_t: Cell::new(0.0),
            focus_anim: Cell::new(None),
        }
    }

    pub fn with_interaction_policy(mut self, policy: SplitPaneInteractionPolicy) -> Self {
        self.interaction_policy = policy;
        self
    }

    pub fn initial_ratio(mut self, ratio: f32) -> Self {
        let r = ratio.clamp(0.0, 1.0);
        self.split_ratio = r;
        self.initial_ratio = r;
        self
    }

    pub fn leading_min(mut self, min: f32) -> Self {
        self.leading_min = min;
        self
    }

    pub fn trailing_min(mut self, min: f32) -> Self {
        self.trailing_min = min;
        self
    }

    pub fn on_ratio_changed<F: FnMut(f32) + 'static>(mut self, f: F) -> Self {
        self.on_ratio_changed = Some(Box::new(f));
        self
    }

    /// Total space along the split axis from cached layout bounds.
    fn axis_total(&self) -> f32 {
        match self.orientation {
            Axis::Horizontal => self.layout_bounds.width,
            Axis::Vertical => self.layout_bounds.height,
        }
    }

    /// Available space for panels (total minus handle thickness).
    fn available(&self) -> f32 {
        (self.axis_total() - HANDLE_THICKNESS).max(0.0)
    }

    fn normalized_min_size(min: f32) -> f32 {
        if min.is_finite() { min.max(0.0) } else { 0.0 }
    }

    fn leading_size_bounds_for_available(&self, available: f32) -> (f32, f32) {
        if !available.is_finite() || available <= 0.0 {
            return (0.0, 0.0);
        }

        let leading_min = Self::normalized_min_size(self.leading_min);
        let trailing_min = Self::normalized_min_size(self.trailing_min);
        let combined_min = f64::from(leading_min) + f64::from(trailing_min);

        if combined_min <= f64::from(available) {
            return (leading_min, available - trailing_min);
        }

        if combined_min > 0.0 {
            let leading_share = f64::from(leading_min) / combined_min;
            let leading_size = (f64::from(available) * leading_share)
                .round()
                .clamp(0.0, f64::from(available)) as f32;
            return (leading_size, leading_size);
        }

        (0.0, available)
    }

    fn ratio_bounds_for_available(&self, available: f32) -> (f32, f32) {
        if !available.is_finite() || available <= 0.0 {
            let ratio = self.split_ratio.max(0.0).min(1.0);
            return (ratio, ratio);
        }

        let (min_size, max_size) = self.leading_size_bounds_for_available(available);
        let min_ratio = (min_size / available).max(0.0).min(1.0);
        let max_ratio = (max_size / available).max(min_ratio).min(1.0);
        (min_ratio, max_ratio)
    }

    fn leading_size_for_available(&self, available: f32, ratio: f32) -> f32 {
        if !available.is_finite() || available <= 0.0 {
            return 0.0;
        }

        let ratio = if ratio.is_finite() {
            ratio.max(0.0).min(1.0)
        } else {
            self.split_ratio.max(0.0).min(1.0)
        };
        let raw = (available * ratio).round();
        let (min_size, max_size) = self.leading_size_bounds_for_available(available);
        raw.max(min_size).min(max_size)
    }

    /// Leading panel size from current ratio and available space.
    fn leading_size(&self) -> f32 {
        let available = self.available();
        self.leading_size_for_available(available, self.split_ratio)
    }

    /// Resize cursor for current orientation.
    fn resize_cursor(&self) -> CursorIcon {
        match self.orientation {
            Axis::Horizontal => CursorIcon::ColResize,
            Axis::Vertical => CursorIcon::RowResize,
        }
    }

    /// Extract pointer position along split axis (widget-local).
    fn axis_pos(&self, pe: &PointerEvent) -> f32 {
        match self.orientation {
            Axis::Horizontal => pe.local_pos.0,
            Axis::Vertical => pe.local_pos.1,
        }
    }

    fn handle_hit_bounds(&self) -> (f32, f32) {
        let leading = self.leading_size();
        let expansion = ((HANDLE_HIT_THICKNESS - HANDLE_THICKNESS) / 2.0).max(0.0);
        let start = (leading - expansion).max(0.0);
        let end = (leading + HANDLE_THICKNESS + expansion).min(self.axis_total());
        (start, end)
    }

    /// Check if a position along the split axis is within the handle hit zone.
    fn in_handle_hit_zone(&self, axis_pos: f32) -> bool {
        let (start, end) = self.handle_hit_bounds();
        axis_pos >= start && axis_pos <= end
    }

    /// Clamp ratio to respect min constraints.
    fn clamp_ratio(&self, ratio: f32) -> f32 {
        let available = self.available();
        let (min_ratio, max_ratio) = self.ratio_bounds_for_available(available);
        let ratio = if ratio.is_finite() {
            ratio
        } else {
            self.split_ratio
        };
        ratio.max(min_ratio).min(max_ratio)
    }

    /// Set ratio, clamp, and fire callback.
    fn set_ratio(&mut self, ratio: f32) {
        self.split_ratio = self.clamp_ratio(ratio);
        if let Some(cb) = &mut self.on_ratio_changed {
            cb(self.split_ratio);
        }
    }

    fn handle_pointer(&mut self, cx: &mut EventContext, pe: &PointerEvent) -> bool {
        let pos = self.axis_pos(pe);

        match pe.kind {
            PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                if self.in_handle_hit_zone(pos) {
                    let available = self.available();
                    self.drag_state = Some(DragState {
                        start_ratio: self.split_ratio,
                        start_pos: pos,
                        available,
                    });
                    cx.capture_pointer(PointerButton::Primary);
                    cx.request_cursor(self.resize_cursor());
                    cx.request_focus();
                    return true;
                }
                false
            }
            PointerEventKind::Move if self.drag_state.is_some() => {
                let drag = self.drag_state.as_ref().unwrap();
                if drag.available > 0.0 {
                    let delta = pos - drag.start_pos;
                    let proposed = drag.start_ratio + delta / drag.available;
                    self.set_ratio(proposed);
                }
                cx.request_cursor(self.resize_cursor());
                cx.invalidate_layout();
                true
            }
            PointerEventKind::Up
                if self.drag_state.is_some() && pe.button == Some(PointerButton::Primary) =>
            {
                self.drag_state = None;
                cx.invalidate_layout();
                true
            }
            PointerEventKind::DoubleClick => {
                if self.in_handle_hit_zone(pos) {
                    self.set_ratio(self.initial_ratio);
                    cx.invalidate_layout();
                    return true;
                }
                false
            }
            PointerEventKind::Move => {
                // Hover detection
                let in_zone = self.in_handle_hit_zone(pos);
                if in_zone {
                    if !self.handle_hovered {
                        self.handle_hovered = true;
                    }
                    cx.request_cursor(self.resize_cursor());
                    true
                } else if !in_zone && self.handle_hovered {
                    self.handle_hovered = false;
                    false
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn handle_key(&mut self, cx: &mut EventContext, ke: &KeyEvent) -> bool {
        if !ke.is_down || !self.focused {
            return false;
        }

        // Escape during drag: cancel
        if ke.key_code == KeyCode::Escape && self.drag_state.is_some() {
            let start = self.drag_state.as_ref().unwrap().start_ratio;
            self.set_ratio(start);
            self.drag_state = None;
            cx.invalidate_layout();
            return true;
        }

        let available = self.available();
        if available <= 0.0 {
            return false;
        }

        let step_ratio = KB_STEP / available;

        match (&self.orientation, &ke.key_code) {
            // Horizontal: Right grows leading, Left shrinks
            (Axis::Horizontal, KeyCode::Right) => {
                self.set_ratio(self.split_ratio + step_ratio);
                cx.invalidate_layout();
                true
            }
            (Axis::Horizontal, KeyCode::Left) => {
                self.set_ratio(self.split_ratio - step_ratio);
                cx.invalidate_layout();
                true
            }
            // Vertical: Down grows leading, Up shrinks
            (Axis::Vertical, KeyCode::Down) => {
                self.set_ratio(self.split_ratio + step_ratio);
                cx.invalidate_layout();
                true
            }
            (Axis::Vertical, KeyCode::Up) => {
                self.set_ratio(self.split_ratio - step_ratio);
                cx.invalidate_layout();
                true
            }
            // Home/End for any orientation
            (_, KeyCode::Home) => {
                let (min_ratio, _) = self.ratio_bounds_for_available(available);
                self.set_ratio(min_ratio);
                cx.invalidate_layout();
                true
            }
            (_, KeyCode::End) => {
                let (_, max_ratio) = self.ratio_bounds_for_available(available);
                self.set_ratio(max_ratio);
                cx.invalidate_layout();
                true
            }
            _ => false,
        }
    }
}

impl WidgetDowncast for SplitPane {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SplitPane {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // INV-SPLITPANE-UNBOUNDED-MAIN: SplitPane cannot perform a ratio split
        // when the main axis is unbounded (inf × ratio has no allocation
        // semantics). The prior fall-through to `constraints.min_*` produced
        // total=0, which forced leading_size to clamp to leading_min and
        // trailing_size to (0 - leading_min) < 0. Those values were then
        // passed verbatim into cx.layout_child(), silently corrupting
        // descendant state every measurement pass.
        //
        // Correct behavior under unbounded main axis: propagate the unbounded
        // main-axis constraint to children unchanged (cross axis stays as
        // received), and report preferred main = leading_min + HANDLE_GUTTER
        // + trailing_min. The parent will re-layout with bounded constraints
        // in the next pass and the ratio split runs correctly there.
        //
        // Matches Flex's INV-FLEX-MEASURE-UNBOUNDED pattern: children always
        // receive a real constraint, never arithmetic-derived garbage.
        let main_unbounded = match self.orientation {
            Axis::Horizontal => !constraints.max_width.is_finite(),
            Axis::Vertical => !constraints.max_height.is_finite(),
        };

        if main_unbounded {
            let cross_min = match self.orientation {
                Axis::Horizontal => constraints.min_height,
                Axis::Vertical => constraints.min_width,
            };
            let cross_max = match self.orientation {
                Axis::Horizontal => constraints.max_height,
                Axis::Vertical => constraints.max_width,
            };

            // Propagate unbounded main axis to both children. Cross stays as
            // received from the parent. No ratio-derived values touch the
            // child constraints; pure pass-through.
            let child_c = match self.orientation {
                Axis::Horizontal => Constraints::new(0.0, f32::INFINITY, cross_min, cross_max),
                Axis::Vertical => Constraints::new(cross_min, cross_max, 0.0, f32::INFINITY),
            };

            let leading_result = cx.layout_child(self.leading, child_c);
            cx.set_child_rect(
                self.leading,
                RectF::new(0.0, 0.0, leading_result.width, leading_result.height),
            );
            let trailing_result = cx.layout_child(self.trailing, child_c);
            cx.set_child_rect(
                self.trailing,
                RectF::new(0.0, 0.0, trailing_result.width, trailing_result.height),
            );

            // Preferred main = minimum viable total split.
            let preferred_main = Self::normalized_min_size(self.leading_min)
                + HANDLE_THICKNESS
                + Self::normalized_min_size(self.trailing_min);

            // Cross extent: if the parent bounded cross, use that; otherwise
            // synthesize from children's reported cross sizes (mirrors the
            // bounded path's cross-axis logic below).
            let cross = if cross_max.is_finite() {
                cross_max
            } else {
                let lead_cross = match self.orientation {
                    Axis::Horizontal => leading_result.height,
                    Axis::Vertical => leading_result.width,
                };
                let trail_cross = match self.orientation {
                    Axis::Horizontal => trailing_result.height,
                    Axis::Vertical => trailing_result.width,
                };
                lead_cross.max(trail_cross).max(cross_min)
            };
            let size = match self.orientation {
                Axis::Horizontal => Size::new(preferred_main, cross),
                Axis::Vertical => Size::new(cross, preferred_main),
            };
            self.layout_bounds = size;
            return LayoutResult::fit(constraints.constrain(size));
        }

        // Bounded main axis: ratio split. The unbounded case has returned
        // above, so `max_{width,height}` on the main axis is guaranteed finite.
        let total = match self.orientation {
            Axis::Horizontal => constraints.max_width,
            Axis::Vertical => constraints.max_height,
        };
        debug_assert!(
            total.is_finite(),
            "INV-SPLITPANE-UNBOUNDED-MAIN: bounded path entered with non-finite total"
        );

        let cross_max = match self.orientation {
            Axis::Horizontal => constraints.max_height,
            Axis::Vertical => constraints.max_width,
        };
        let cross_min = match self.orientation {
            Axis::Horizontal => constraints.min_height,
            Axis::Vertical => constraints.min_width,
        };

        let available = (total - HANDLE_THICKNESS).max(0.0);
        let leading_size = self.leading_size_for_available(available, self.split_ratio);
        let trailing_size = (available - leading_size).max(0.0);

        // Build child constraints
        let (leading_c, trailing_c) = match self.orientation {
            Axis::Horizontal => (
                Constraints::new(leading_size, leading_size, cross_min, cross_max),
                Constraints::new(trailing_size, trailing_size, cross_min, cross_max),
            ),
            Axis::Vertical => (
                Constraints::new(cross_min, cross_max, leading_size, leading_size),
                Constraints::new(cross_min, cross_max, trailing_size, trailing_size),
            ),
        };

        // Layout leading child
        let leading_result_size = cx.layout_child(self.leading, leading_c);
        cx.set_child_rect(
            self.leading,
            RectF::new(
                0.0,
                0.0,
                leading_result_size.width,
                leading_result_size.height,
            ),
        );

        // Layout trailing child
        let trailing_result_size = cx.layout_child(self.trailing, trailing_c);
        let trailing_rect = match self.orientation {
            Axis::Horizontal => RectF::new(
                leading_size + HANDLE_THICKNESS,
                0.0,
                trailing_result_size.width,
                trailing_result_size.height,
            ),
            Axis::Vertical => RectF::new(
                0.0,
                leading_size + HANDLE_THICKNESS,
                trailing_result_size.width,
                trailing_result_size.height,
            ),
        };
        cx.set_child_rect(self.trailing, trailing_rect);

        // Final size — use max of children's cross sizes when cross-axis is unbounded
        let cross = if cross_max.is_finite() {
            cross_max
        } else {
            let leading_cross = match self.orientation {
                Axis::Horizontal => leading_result_size.height,
                Axis::Vertical => leading_result_size.width,
            };
            let trailing_cross = match self.orientation {
                Axis::Horizontal => trailing_result_size.height,
                Axis::Vertical => trailing_result_size.width,
            };
            leading_cross.max(trailing_cross).max(cross_min)
        };
        let final_size = match self.orientation {
            Axis::Horizontal => Size::new(total, cross),
            Axis::Vertical => Size::new(cross, total),
        };
        let final_size = constraints.constrain(final_size);

        self.layout_bounds = final_size;
        LayoutResult::fit(final_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let leading = self.leading_size();

        let visual_offset = ((HANDLE_THICKNESS - HANDLE_VISUAL_THICKNESS) / 2.0).floor();
        let handle_rect = match self.orientation {
            Axis::Horizontal => RectF::new(
                rect.x + leading + visual_offset,
                rect.y,
                HANDLE_VISUAL_THICKNESS,
                rect.height,
            ),
            Axis::Vertical => RectF::new(
                rect.x,
                rect.y + leading + visual_offset,
                rect.width,
                HANDLE_VISUAL_THICKNESS,
            ),
        };

        // Consume focus animation
        {
            let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
            if let Some(handle) = self.focus_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.focus_t.set(value);
                } else {
                    self.focus_anim.set(None);
                }
            }
        }

        let focus_val = self.focus_t.get();
        let active_or_discoverable =
            self.drag_state.is_some() || self.handle_hovered || focus_val > 0.001;
        if self.interaction_policy.divider_visibility == SplitPaneDividerVisibility::OnInteraction
            && !active_or_discoverable
        {
            return;
        }

        // Pick color based on state — follows border color progression.
        let color = if self.drag_state.is_some() {
            cx.theme.colors.accents.primary
        } else if self.handle_hovered {
            cx.theme.colors.borders.strong
        } else {
            cx.theme.colors.borders.default
        };

        // Build decoration stack
        let mut stack = DecorationStack::background(color, 0.0);
        if focus_val > 0.001 {
            let fr = &cx.theme.focus_ring;
            stack = stack.with_focus(FocusStyle::shadow_ring(
                fr.spread,
                fr.softness,
                cx.theme.colors.borders.focus.with_alpha(fr.alpha),
                0.0,
            ));
        }

        let state = DecorationState {
            focused: false,
            focus_t: focus_val,
            hover_t: 0.0,
            draw_hover: false,
        };
        stack.paint(cx.scene, handle_rect, state, 2);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        match event {
            Event::Pointer(pe) if phase == EventPhase::Capture || phase == EventPhase::Target => {
                self.handle_pointer(cx, pe)
            }
            Event::Key(ke) if phase == EventPhase::Target => self.handle_key(cx, ke),
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "SplitPane"
    }

    fn clip_bounds(&self) -> Option<RectF> {
        Some(RectF::new(
            0.0,
            0.0,
            self.layout_bounds.width,
            self.layout_bounds.height,
        ))
    }

    /// SPEC-PM-005 §7 / SPEC-HAL-267: returns `false`.
    ///
    /// SplitPane partitions whatever main-axis space its parent supplies
    /// and has no intrinsic main-axis size. Per SPEC-HAL-267, SplitPane's
    /// unbounded-main path returns `leading_min + HANDLE_THICKNESS +
    /// trailing_min` as its preferred main extent — a proper MIN-viable
    /// measurement.
    ///
    /// Returning `true` here would trigger the SPEC-PM-005 §7 exception
    /// in `Flex::layout_no_wrap` (flex.rs:213-235), which forces a
    /// bounded pass-1 measurement. SplitPane's bounded path returns
    /// `Size(total, cross)`, claiming the entire parent allocation as
    /// its preferred main. When SplitPane is a flex child of a Flex with
    /// any non-flex siblings, the sum `splitpane_base + non_flex_bases`
    /// exceeds `main_max_space`, driving Flex into the overflow path
    /// (flex.rs:307-319) which shrinks every child including SplitPane
    /// at default `shrink = 1.0`. Pass-2 re-layout then calls
    /// `SplitPane::layout` a second time with the shrunk tight
    /// constraint, propagating two different `cross_max` values down
    /// through any nested layout (e.g. another SplitPane) and producing
    /// alternating viewport heights at the leaves.
    ///
    /// Returning `false` makes Flex measure SplitPane unbounded.
    /// SplitPane reports MIN-viable. `total_base + non_flex_bases <
    /// main_max_space`. Flex's grow path allocates the correct leftover.
    /// Pass-2 re-layout calls SplitPane once with the correct tight
    /// allocation. Single coherent layout per parent invocation.
    fn needs_viewport_constraint(&self) -> bool {
        false
    }

    fn focusable(&self) -> bool {
        // false: prevents ancestor-walk focus when clicking panel content.
        // Focus obtained explicitly via cx.request_focus() on handle click.
        false
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.focused = true;
        let anim = Animation {
            from: self.focus_t.get(),
            to: 1.0,
            duration: 0.08,
            easing: Easing::EaseOut,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.focus_anim.set(Some(handle));
        } else {
            self.focus_t.set(1.0);
            self.focus_anim.set(None);
        }
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.focused = false;
        let anim = Animation {
            from: self.focus_t.get(),
            to: 0.0,
            duration: 0.08,
            easing: Easing::EaseIn,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.focus_anim.set(Some(handle));
        } else {
            self.focus_t.set(0.0);
            self.focus_anim.set(None);
        }
    }

    fn on_mount(&mut self) {
        self.focus_t.set(0.0);
        self.focus_anim.set(None);
    }
}
