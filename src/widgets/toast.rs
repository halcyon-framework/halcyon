// SPEC-HAL-269A: Toast Stack Interaction Model — Widget
//
// Self-rendering overlay widget. No modal. No scrim. No pointer capture.
// All bg fills use TextBackgroundSpan (INV-BG-FILL).
// All timing via cx.frame_time_ms() (INV-ANIM-TIMER).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_OVERLAY, ShadowMode, TextBackgroundSpan};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, OverlayHitRegion};
use crate::model::toast::{DismissState, ToastId, ToastManager};
use crate::utils::id::{TreeWidgetId, WidgetInternalId};

// ============================================================================
// Layer constants
// ============================================================================

// Position 2 (furthest back peek) — lowest band
const TOAST_P2_SHADOW_LAYER: u32 = LAYER_OVERLAY + 3;
const TOAST_P2_BG_LAYER: u32 = LAYER_OVERLAY + 4;
const TOAST_P2_TEXT_LAYER: u32 = LAYER_OVERLAY + 5;

// Position 1 (middle peek)
const TOAST_P1_SHADOW_LAYER: u32 = LAYER_OVERLAY + 6;
const TOAST_P1_BG_LAYER: u32 = LAYER_OVERLAY + 7;
const TOAST_P1_TEXT_LAYER: u32 = LAYER_OVERLAY + 8;

// Position 0 (primary / front) — highest band, occludes all peers
const TOAST_P0_SHADOW_LAYER: u32 = LAYER_OVERLAY + 9;
const TOAST_P0_BG_LAYER: u32 = LAYER_OVERLAY + 10;
const TOAST_P0_TEXT_LAYER: u32 = LAYER_OVERLAY + 11;

// Position 3+ (expanded overflow) — same as P1 band, only visible when expanded
const TOAST_PN_SHADOW_LAYER: u32 = LAYER_OVERLAY + 6;
const TOAST_PN_BG_LAYER: u32 = LAYER_OVERLAY + 7;
const TOAST_PN_TEXT_LAYER: u32 = LAYER_OVERLAY + 8;

// ============================================================================
// Layout constants (logical pixels)
// ============================================================================

const TOAST_MARGIN_BOTTOM: f32 = 32.0;
const TOAST_PADDING_H: f32 = 20.0;
const TOAST_PADDING_V: f32 = 12.0;
const TOAST_MAX_WIDTH: f32 = 480.0;
const TOAST_MIN_WIDTH: f32 = 200.0;
const TOAST_ITEM_GAP: f32 = 8.0;
const ACTION_GAP: f32 = 16.0;
const ACTION_PADDING_X: f32 = 8.0;
const ACTION_PADDING_Y: f32 = 3.0;
const ANIM_ENTRY_MS: f32 = 220.0;
const ANIM_EXIT_MS: f32 = 180.0;
const TOAST_RADIUS: f32 = 8.0;
const ACCENT_STRIPE_W: f32 = 3.0;
const STACK_PEEK_OFFSET: f32 = 8.0;
const STACK_SCALE_1: f32 = 0.95;
const STACK_SCALE_2: f32 = 0.90;
const STACK_OPACITY_1: f32 = 0.70;
const STACK_OPACITY_2: f32 = 0.45;
const EXPAND_MS: f32 = 260.0;
const COLLAPSE_MS: f32 = 200.0;
const GAP_CLOSE_MS: f32 = 200.0;
const CLEAR_ALL_GAP: f32 = 6.0;
const CLEAR_ALL_HEIGHT: f32 = 24.0;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackState {
    Collapsed,
    Expanding,
    Expanded,
    Collapsing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastControlTarget {
    Action(ToastId),
    ClearAll,
}

/// Per-entry gap-close animation state.
/// Tracks how far a toast needs to slide down after a peer is dismissed.
struct GapCloseEntry {
    id: ToastId,
    start_canonical_y: f32,
    offset_px: f32,
    start_ms: u64,
}

// ============================================================================
// Easing functions
// ============================================================================

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

fn ease_in(t: f32) -> f32 {
    t * t
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ============================================================================
// ToastWidget
// ============================================================================

pub struct ToastWidget {
    id: WidgetInternalId,
    manager: Rc<RefCell<ToastManager>>,
    /// Canonical rects per entry (in expanded layout). Rebuilt every layout pass.
    /// (ToastId, toast_rect, action_rect, toast_h)
    rects: RefCell<Vec<(ToastId, RectF, Option<RectF>, f32)>>,
    /// Clear All button canonical rect (above topmost toast in expanded).
    clear_all_rect: Cell<RectF>,
    /// Stack bounding rect for hover detection.
    stack_rect: Cell<RectF>,
    /// Current stack interaction state.
    stack_state: Cell<StackState>,
    /// When the current expand/collapse animation began (in engine ms).
    transition_start_ms: Cell<Option<u64>>,
    /// Gap-close animations for entries above a dismissed peer.
    gap_close: RefCell<Vec<GapCloseEntry>>,
    hover_target: Cell<Option<ToastControlTarget>>,
    press_target: Cell<Option<ToastControlTarget>>,
}

impl ToastWidget {
    pub fn new(manager: Rc<RefCell<ToastManager>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            manager,
            rects: RefCell::new(Vec::new()),
            clear_all_rect: Cell::new(RectF::zero()),
            stack_rect: Cell::new(RectF::zero()),
            stack_state: Cell::new(StackState::Collapsed),
            transition_start_ms: Cell::new(None),
            gap_close: RefCell::new(Vec::new()),
            hover_target: Cell::new(None),
            press_target: Cell::new(None),
        }
    }

    fn start_gap_close(
        &self,
        dismissed_id: ToastId,
        dismissed_h: f32,
        rects: &[(ToastId, RectF, Option<RectF>, f32)],
        now_ms: u64,
    ) {
        let dismissed_pos = rects.iter().position(|r| r.0 == dismissed_id);
        let Some(pos) = dismissed_pos else { return };

        let gap_offset = dismissed_h + TOAST_ITEM_GAP;
        let mut gc = self.gap_close.borrow_mut();
        // Entries at positions > pos are above the dismissed toast — they need to slide down
        for entry in rects.iter().skip(pos + 1) {
            let canonical_y = entry.1.y;
            if let Some(existing) = gc.iter_mut().find(|g| g.id == entry.0) {
                existing.offset_px += gap_offset;
                existing.start_ms = now_ms;
            } else {
                gc.push(GapCloseEntry {
                    id: entry.0,
                    start_canonical_y: canonical_y,
                    offset_px: gap_offset,
                    start_ms: now_ms,
                });
            }
        }
    }

    /// Returns the animated y for a given toast during gap-close, or canonical_y
    /// if no gap-close is active for this toast.
    fn gc_animated_y(&self, id: ToastId, canonical_y: f32, now_ms: u64) -> f32 {
        let gc = self.gap_close.borrow();
        if let Some(g) = gc.iter().find(|g| g.id == id) {
            let elapsed = (now_ms - g.start_ms) as f32;
            let t = (elapsed / GAP_CLOSE_MS).min(1.0);
            g.start_canonical_y + g.offset_px * ease_out(t)
        } else {
            canonical_y
        }
    }

    fn target_at(&self, px: f32, py: f32, now_ms: u64) -> Option<ToastControlTarget> {
        let rects = self.rects.borrow();
        let state = self.stack_state.get();

        if matches!(state, StackState::Expanded | StackState::Expanding) {
            let ca = self.clear_all_rect.get();
            if ca.width > 0.0
                && px >= ca.x
                && px < ca.x + ca.width
                && py >= ca.y
                && py < ca.y + ca.height
            {
                return Some(ToastControlTarget::ClearAll);
            }
        }

        let mgr = self.manager.borrow();
        for (id, toast_rect, action_rect, _) in rects.iter() {
            let entry = match mgr.entry_by_id(*id) {
                Some(e) => e,
                None => continue,
            };
            if entry.dismiss_state.get() != DismissState::Visible {
                continue;
            }
            if matches!(state, StackState::Collapsed | StackState::Collapsing)
                && *id != rects.first().map(|r| r.0).unwrap_or(*id)
            {
                continue;
            }

            let dy = self.gc_animated_y(*id, toast_rect.y, now_ms) - toast_rect.y;
            if let Some(act_rect) = action_rect {
                if px >= act_rect.x
                    && px < act_rect.x + act_rect.width
                    && py >= act_rect.y + dy
                    && py < act_rect.y + dy + act_rect.height
                {
                    return Some(ToastControlTarget::Action(*id));
                }
            }
        }

        None
    }
}

impl WidgetDowncast for ToastWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ToastWidget {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let screen_size = Size::new(constraints.max_width, constraints.max_height);
        let font_size = cx.theme.typography.font_size;

        let mgr = self.manager.borrow();
        let entries = mgr.active_entries();
        let mut result_rects = Vec::with_capacity(entries.len());

        // Stack bottom-up from screen bottom
        let mut cursor_y = screen_size.height - TOAST_MARGIN_BOTTOM;
        let mut topmost_toast_y = cursor_y;
        let mut topmost_toast_x = 0.0_f32;
        let mut topmost_toast_w = 0.0_f32;

        for entry in &entries {
            let (msg_w, msg_h) = cx.text_engine.measure(&entry.message, font_size);

            let mut content_w = msg_w;
            let mut action_rect = None;
            let mut action_w = 0.0;
            let mut action_h = 0.0;

            if let Some(ref action) = entry.action {
                let (act_text_w, act_text_h) = cx.text_engine.measure(&action.label, font_size);
                action_w = act_text_w + ACTION_PADDING_X * 2.0;
                action_h = act_text_h + ACTION_PADDING_Y * 2.0;
                content_w += ACTION_GAP + action_w;
            }
            let toast_w =
                (content_w + TOAST_PADDING_H * 2.0).clamp(TOAST_MIN_WIDTH, TOAST_MAX_WIDTH);
            let toast_h = msg_h.max(action_h) + TOAST_PADDING_V * 2.0;
            let toast_x = (screen_size.width - toast_w) / 2.0;
            cursor_y -= toast_h;
            let toast_y = cursor_y;
            cursor_y -= TOAST_ITEM_GAP;

            let toast_rect = RectF::new(toast_x, toast_y, toast_w, toast_h);

            if entry.action.is_some() {
                let act_x = toast_x + toast_w - TOAST_PADDING_H - action_w;
                let act_y = toast_y + (toast_h - action_h) / 2.0;
                action_rect = Some(RectF::new(act_x, act_y, action_w, action_h));
            }

            topmost_toast_y = toast_y;
            topmost_toast_x = toast_x;
            topmost_toast_w = toast_w;

            result_rects.push((entry.id, toast_rect, action_rect, toast_h));
        }

        if entries.len() >= 2 {
            let clear_rect = RectF::new(
                topmost_toast_x,
                topmost_toast_y - CLEAR_ALL_GAP - CLEAR_ALL_HEIGHT,
                topmost_toast_w,
                CLEAR_ALL_HEIGHT,
            );
            self.clear_all_rect.set(clear_rect);
            // stack_rect = union of all toast rects + clear_all_rect
            let mut min_x = clear_rect.x;
            let mut min_y = clear_rect.y;
            let mut max_x = clear_rect.x + clear_rect.width;
            let mut max_y = clear_rect.y + clear_rect.height;
            for (_, r, _, _) in &result_rects {
                min_x = min_x.min(r.x);
                min_y = min_y.min(r.y);
                max_x = max_x.max(r.x + r.width);
                max_y = max_y.max(r.y + r.height);
            }
            self.stack_rect
                .set(RectF::new(min_x, min_y, max_x - min_x, max_y - min_y));
        } else {
            self.clear_all_rect.set(RectF::zero());
            if let Some((_, r, _, _)) = result_rects.first() {
                self.stack_rect.set(*r);
            } else {
                self.stack_rect.set(RectF::zero());
            }
        }

        // INV-SS-SINGLE-ENTRY: if <= 1 entry, force Collapsed
        if entries.len() <= 1 {
            self.stack_state.set(StackState::Collapsed);
        }

        *self.rects.borrow_mut() = result_rects;
        LayoutResult::fit(screen_size)
    }

    fn paint(&self, cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        let now_ms = cx.frame_time_ms();
        let font_size = cx.theme.typography.font_size;

        // Step 1 — Advance stack transition
        let stack_state = self.stack_state.get();
        let t_transition: f32 = match stack_state {
            StackState::Collapsed => 0.0,
            StackState::Expanded => 1.0,
            StackState::Expanding => {
                let start = self.transition_start_ms.get().unwrap_or(now_ms);
                let t = (now_ms - start) as f32 / EXPAND_MS;
                if t >= 1.0 {
                    self.stack_state.set(StackState::Expanded);
                    self.transition_start_ms.set(None);
                    1.0
                } else {
                    ease_out(t)
                }
            }
            StackState::Collapsing => {
                let start = self.transition_start_ms.get().unwrap_or(now_ms);
                let t = (now_ms - start) as f32 / COLLAPSE_MS;
                if t >= 1.0 {
                    self.stack_state.set(StackState::Collapsed);
                    self.transition_start_ms.set(None);
                    0.0
                } else {
                    1.0 - ease_in(t)
                }
            }
        };

        // Step 2 — Per-entry lifecycle (timer, show_at, exit_start)
        let mgr = self.manager.borrow();
        let rects = self.rects.borrow();
        let is_paused = matches!(
            self.stack_state.get(),
            StackState::Expanded | StackState::Expanding
        );
        mgr.advance_lifecycle(now_ms, is_paused, 16, ANIM_EXIT_MS as u64);

        // Step 3 — Compute gap-close animated y overrides.
        // gc_offsets: (ToastId, animated_y) — absolute y to use instead of toast_rect.y.
        // Animation goes 0 → offset_px so the toast slides downward to its new slot.
        let mut gc_offsets: Vec<(ToastId, f32)> = Vec::new();
        {
            let rects_slice: &[(ToastId, RectF, Option<RectF>, f32)] = &rects;
            let mut gc = self.gap_close.borrow_mut();
            gc.retain(|g| {
                let elapsed = (now_ms - g.start_ms) as f32;
                let t = elapsed / GAP_CLOSE_MS;
                if t < 1.0 {
                    return true; // animation still running
                }
                // t >= 1.0: only remove once layout has settled at the expected new position.
                // While layout still shows the old y (dismissed entry still Exiting in this
                // frame's layout pass), keep the gc entry alive so base_y stays at
                // start_canonical_y + offset_px instead of snapping to the stale toast_rect.y.
                let expected_new_y = g.start_canonical_y + g.offset_px;
                let current_layout_y = rects_slice
                    .iter()
                    .find(|(id, _, _, _)| *id == g.id)
                    .map(|(_, r, _, _)| r.y)
                    .unwrap_or(expected_new_y); // toast gone from layout = settled
                // Keep alive while layout y hasn't moved to expected position yet.
                (current_layout_y - expected_new_y).abs() > 0.5
            });
            for g in gc.iter() {
                let elapsed = (now_ms - g.start_ms) as f32;
                let t = (elapsed / GAP_CLOSE_MS).min(1.0);
                let animated_y = g.start_canonical_y + g.offset_px * ease_out(t);
                gc_offsets.push((g.id, animated_y));
            }
        }

        // Step 4 — Paint each toast
        let active_count = rects
            .iter()
            .filter(|(id, _, _, _)| {
                mgr.entry_by_id(*id)
                    .map(|e| e.dismiss_state.get() != DismissState::Dismissed)
                    .unwrap_or(false)
            })
            .count();
        if active_count == 0 {
            self.clear_all_rect.set(RectF::zero());
            self.stack_rect.set(RectF::zero());
            self.stack_state.set(StackState::Collapsed);
            self.transition_start_ms.set(None);
            self.gap_close.borrow_mut().clear();
            return;
        }
        let mut pos0_rect = RectF::zero();

        for (position, (id, toast_rect, action_rect, toast_h)) in rects.iter().enumerate() {
            let entry = match mgr.entry_by_id(*id) {
                Some(e) => e,
                None => continue,
            };

            let state = entry.dismiss_state.get();
            if state == DismissState::Dismissed {
                continue;
            }

            // Entry/exit animation offset and opacity
            let show_at = entry.show_at_ms.get().unwrap_or(now_ms);
            let (entry_y_offset, anim_opacity) = match state {
                DismissState::Visible => {
                    let elapsed = (now_ms - show_at) as f32;
                    let t_entry = (elapsed / ANIM_ENTRY_MS).min(1.0);
                    if t_entry < 1.0 {
                        let offset = toast_h * (1.0 - ease_out(t_entry));
                        (offset, t_entry)
                    } else {
                        (0.0, 1.0)
                    }
                }
                DismissState::Exiting => {
                    let exit_start = entry.exit_start_ms.get().unwrap_or(now_ms);
                    let elapsed = (now_ms - exit_start) as f32;
                    let t_exit = (elapsed / ANIM_EXIT_MS).min(1.0);
                    if t_exit >= 1.0 {
                        continue;
                    }
                    let offset = toast_h * ease_in(t_exit);
                    (offset, 1.0 - t_exit)
                }
                _ => continue,
            };

            // Gap-close override: use animated y when in progress, else canonical y.
            let base_y = gc_offsets
                .iter()
                .find(|(gid, _)| *gid == *id)
                .map(|(_, y)| *y)
                .unwrap_or(toast_rect.y);

            // Stack visual (collapsed vs expanded)
            let (visual_x, visual_y, visual_w, visual_h, visual_opacity) = match position {
                0 => {
                    // Primary: always at canonical position, full opacity
                    pos0_rect = RectF::new(
                        toast_rect.x,
                        base_y + entry_y_offset,
                        toast_rect.width,
                        toast_rect.height,
                    );
                    (
                        toast_rect.x,
                        base_y + entry_y_offset,
                        toast_rect.width,
                        toast_rect.height,
                        anim_opacity,
                    )
                }
                1 => {
                    // Collapsed: peek strip above pos 0, scaled.
                    // Expanded: full size at canonical position.
                    let collapsed_y = pos0_rect.y - STACK_PEEK_OFFSET;
                    let collapsed_w = pos0_rect.width * STACK_SCALE_1;
                    let collapsed_x = pos0_rect.x + (pos0_rect.width - collapsed_w) / 2.0;
                    let collapsed_h = toast_rect.height * STACK_SCALE_1;
                    let collapsed_opacity = STACK_OPACITY_1;

                    let exp_y = base_y + entry_y_offset;

                    let y = lerp(collapsed_y, exp_y, t_transition);
                    let x = lerp(collapsed_x, toast_rect.x, t_transition);
                    let w = lerp(collapsed_w, toast_rect.width, t_transition);
                    let h = lerp(collapsed_h, toast_rect.height, t_transition);
                    let alpha = lerp(collapsed_opacity, 1.0, t_transition) * anim_opacity;
                    (x, y, w, h, alpha)
                }
                2 => {
                    // Same as position 1 but with SCALE_2 / OPACITY_2 values.
                    let collapsed_y = pos0_rect.y - STACK_PEEK_OFFSET * 2.0;
                    let collapsed_w = pos0_rect.width * STACK_SCALE_2;
                    let collapsed_x = pos0_rect.x + (pos0_rect.width - collapsed_w) / 2.0;
                    let collapsed_h = toast_rect.height * STACK_SCALE_2;
                    let collapsed_opacity = STACK_OPACITY_2;

                    let exp_y = base_y + entry_y_offset;

                    let y = lerp(collapsed_y, exp_y, t_transition);
                    let x = lerp(collapsed_x, toast_rect.x, t_transition);
                    let w = lerp(collapsed_w, toast_rect.width, t_transition);
                    let h = lerp(collapsed_h, toast_rect.height, t_transition);
                    let alpha = lerp(collapsed_opacity, 1.0, t_transition) * anim_opacity;
                    (x, y, w, h, alpha)
                }
                _ => {
                    // Position 3+: only visible in expanded state.
                    if t_transition == 0.0 {
                        continue;
                    }
                    let exp_y = base_y + entry_y_offset;
                    let start_y = pos0_rect.y + pos0_rect.height; // slide up from below
                    let y = lerp(start_y, exp_y, t_transition);
                    let alpha = t_transition * anim_opacity;
                    (toast_rect.x, y, toast_rect.width, toast_rect.height, alpha)
                }
            };

            let draw_rect = RectF::new(visual_x, visual_y, visual_w, visual_h);

            let (shadow_layer, bg_layer, text_layer) = match position {
                0 => (
                    TOAST_P0_SHADOW_LAYER,
                    TOAST_P0_BG_LAYER,
                    TOAST_P0_TEXT_LAYER,
                ),
                1 => (
                    TOAST_P1_SHADOW_LAYER,
                    TOAST_P1_BG_LAYER,
                    TOAST_P1_TEXT_LAYER,
                ),
                2 => (
                    TOAST_P2_SHADOW_LAYER,
                    TOAST_P2_BG_LAYER,
                    TOAST_P2_TEXT_LAYER,
                ),
                _ => (
                    TOAST_PN_SHADOW_LAYER,
                    TOAST_PN_BG_LAYER,
                    TOAST_PN_TEXT_LAYER,
                ),
            };

            // Clip peek draw_rect to the strip visually above pos0_rect.
            // The overlap region [pos0_rect.y .. draw_rect.bottom] bleeds through
            // pos0's background regardless of z-layer, since they cover different areas.
            let collapsed_peek = position > 0
                && matches!(
                    self.stack_state.get(),
                    StackState::Collapsed | StackState::Collapsing,
                );

            let draw_rect = if collapsed_peek && pos0_rect.y > draw_rect.y {
                let clipped_h = (pos0_rect.y - draw_rect.y).max(0.0);
                RectF::new(draw_rect.x, draw_rect.y, draw_rect.width, clipped_h)
            } else {
                draw_rect
            };

            // bg is always fully opaque — depth cue comes from scale/position, not transparency
            let bg_opacity: f32 = anim_opacity;
            let fg_opacity: f32 = visual_opacity; // text, border, shadow use full stack opacity

            let colors = cx.theme.colors;
            let is_dark = cx.theme.is_dark();

            // Shadow
            let shadow_alpha = if is_dark { 0.35 } else { 0.18 } * fg_opacity;
            cx.scene.push_shadow(
                draw_rect,
                TOAST_RADIUS,
                ShadowMode::Drop,
                (0.0, 3.0),
                0.0,
                12.0,
                Color::from_srgb(0.0, 0.0, 0.0, shadow_alpha),
                shadow_layer,
                None,
            );

            // Background fill — INV-BG-FILL: must use TextBackgroundSpan to register text region.
            let bg_color = if is_dark {
                colors.surfaces.level3
            } else {
                colors.surfaces.card
            }
            .with_alpha(bg_opacity);
            let bg_run = cx.text_engine.shape(" ", font_size);
            cx.scene.push_text_with_backgrounds(
                &bg_run,
                draw_rect.x,
                draw_rect.y,
                bg_color,
                vec![TextBackgroundSpan {
                    x_offset: 0.0,
                    y_offset: 0.0,
                    width: draw_rect.width,
                    height: draw_rect.height,
                    color: bg_color,
                }],
                bg_layer,
                None,
            );

            // Subtle gradient overlay on top of flat fill: darken bottom edge slightly.
            use crate::core::paint::{ColorStop, GradientDef, GradientKind};
            let overlay_top = if is_dark {
                Color::from_srgb(1.0, 1.0, 1.0, 0.04 * bg_opacity)
            } else {
                Color::from_srgb(1.0, 1.0, 1.0, 0.18 * bg_opacity)
            };
            let overlay_bottom = if is_dark {
                Color::from_srgb(0.0, 0.0, 0.0, 0.08 * bg_opacity)
            } else {
                Color::from_srgb(0.0, 0.0, 0.0, 0.035 * bg_opacity)
            };
            let grad = GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop {
                        offset: 0.0,
                        color: overlay_top,
                    },
                    ColorStop {
                        offset: 1.0,
                        color: overlay_bottom,
                    },
                ],
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
            };
            cx.scene
                .push_gradient_rect(draw_rect, TOAST_RADIUS, &grad, bg_layer, None);

            // Top edge highlight
            let highlight = RectF::new(
                draw_rect.x + TOAST_RADIUS,
                draw_rect.y + 0.5,
                draw_rect.width - TOAST_RADIUS * 2.0,
                1.0,
            );
            cx.scene.push_rect(
                highlight,
                colors.borders.highlight.with_alpha(if is_dark {
                    0.12 * fg_opacity
                } else {
                    0.65 * fg_opacity
                }),
                bg_layer,
                None,
            );

            // Border
            cx.scene.push_border_rect(
                draw_rect,
                if is_dark {
                    colors.borders.strong.with_alpha(0.55 * fg_opacity)
                } else {
                    colors.borders.default.with_alpha(0.85 * fg_opacity)
                },
                1.0,
                bg_layer,
                None,
            );

            if collapsed_peek {
                continue;
            }

            // Left accent stripe
            let accent_color_base = if entry.action.is_some() {
                colors.semantic.success.with_alpha(fg_opacity)
            } else {
                colors.accents.primary.with_alpha(fg_opacity)
            };
            // Inset top/bottom by 4px so the stripe floats as an accent bar, not a full bleed.
            let stripe_inset = 4.0;
            let stripe_rect = RectF::new(
                draw_rect.x + 1.0,
                draw_rect.y + stripe_inset,
                ACCENT_STRIPE_W,
                (draw_rect.height - stripe_inset * 2.0).max(0.0),
            );
            cx.scene
                .push_rect(stripe_rect, accent_color_base, text_layer, None);

            // Message text
            let msg_run = cx.text_engine.shape(&entry.message, font_size);
            let text_x = draw_rect.x + ACCENT_STRIPE_W + TOAST_PADDING_H;
            let text_y = draw_rect.y + (draw_rect.height - msg_run.height) * 0.5;
            let text_color = colors.typography.primary.with_alpha(fg_opacity);
            cx.scene.push_text_with_backgrounds(
                &msg_run,
                text_x,
                text_y,
                text_color,
                vec![],
                text_layer,
                None,
            );

            // Action label (modulated by visual_opacity)
            if entry.action.is_some() {
                if let Some(act_layout_rect) = action_rect {
                    let act_x = act_layout_rect.x + (visual_x - toast_rect.x);
                    let act_y = act_layout_rect.y + (visual_y - toast_rect.y);
                    let act_rect =
                        RectF::new(act_x, act_y, act_layout_rect.width, act_layout_rect.height);
                    let action = entry.action.as_ref().unwrap();
                    let act_run = cx.text_engine.shape(&action.label, font_size);
                    let target = ToastControlTarget::Action(*id);
                    let hovered = self.hover_target.get() == Some(target);
                    let pressed = self.press_target.get() == Some(target) && hovered;
                    let act_text_color = if pressed {
                        colors.accents.hover.darken(0.12).with_alpha(fg_opacity)
                    } else if hovered {
                        colors.accents.hover.with_alpha(fg_opacity)
                    } else {
                        colors.accents.primary.with_alpha(fg_opacity)
                    };
                    let text_x = act_rect.x + (act_rect.width - act_run.width) * 0.5;
                    let text_y = act_rect.y
                        + (act_rect.height - act_run.height) * 0.5
                        + if pressed { 1.0 } else { 0.0 };
                    cx.scene.push_text_with_backgrounds(
                        &act_run,
                        text_x,
                        text_y,
                        act_text_color,
                        vec![],
                        text_layer,
                        None,
                    );
                    if hovered || pressed {
                        cx.scene.push_rect(
                            RectF::new(text_x, text_y + act_run.height - 1.0, act_run.width, 1.0),
                            act_text_color,
                            text_layer,
                            None,
                        );
                    }
                }
            }

            // Count badge (Collapsed only, N >= 3)
            if position == 0 && t_transition < 0.3 && active_count > 3 {
                let badge_text = format!("+{}", active_count - 1);
                let badge_run = cx.text_engine.shape(&badge_text, font_size);
                let badge_opacity = (1.0 - t_transition / 0.3) * fg_opacity;
                let badge_x = draw_rect.x + draw_rect.width - TOAST_PADDING_H - badge_run.width;
                let badge_y = draw_rect.y + TOAST_PADDING_V;
                cx.scene.push_text_with_backgrounds(
                    &badge_run,
                    badge_x,
                    badge_y,
                    colors.typography.secondary.with_alpha(badge_opacity),
                    vec![],
                    text_layer,
                    None,
                );
            }
        }

        // Clear All (outside per-entry loop)
        if t_transition > 0.0 && active_count >= 2 {
            let clear_opacity = t_transition * 0.8;
            let ca = self.clear_all_rect.get();
            if ca.width > 0.0 {
                let clear_run = cx.text_engine.shape("Clear all", font_size);
                // Right-aligned in clear_all_rect
                let clear_x = ca.x + ca.width - clear_run.width;
                let clear_y = ca.y + (ca.height - clear_run.height) / 2.0;
                cx.scene.push_text_with_backgrounds(
                    &clear_run,
                    clear_x,
                    clear_y,
                    cx.theme
                        .colors
                        .typography
                        .secondary
                        .with_alpha(clear_opacity),
                    vec![],
                    TOAST_P0_TEXT_LAYER,
                    None,
                );
            }
        }
    }

    fn paint_overlay(&self, cx: &mut PaintContext, _rect: RectF, tree_widget_id: TreeWidgetId) {
        let mgr = self.manager.borrow();
        let rects = self.rects.borrow();
        let state = self.stack_state.get();

        match state {
            StackState::Collapsed | StackState::Collapsing => {
                // Register hit region only for position-0 toast (canonical rect)
                if let Some((id, toast_rect, _, _)) = rects.first() {
                    let entry = mgr.entry_by_id(*id);
                    if let Some(e) = entry {
                        // INV-ANIM-EXIT-HIT: Hit region only for Visible, not Exiting
                        if e.dismiss_state.get() == DismissState::Visible {
                            cx.register_overlay_hit_region(OverlayHitRegion {
                                abs_rect: *toast_rect,
                                owner_abs_rect: *toast_rect,
                                widget_id: tree_widget_id,
                            });
                        }
                    }
                }
            }
            StackState::Expanded | StackState::Expanding => {
                // Register hit region for each Visible entry's animated rect
                let now_ms = cx.frame_time_ms();
                for (id, toast_rect, _, _) in rects.iter() {
                    let entry = match mgr.entry_by_id(*id) {
                        Some(e) => e,
                        None => continue,
                    };
                    // INV-ANIM-EXIT-HIT: never register Exiting entries
                    if entry.dismiss_state.get() != DismissState::Visible {
                        continue;
                    }
                    let animated_y = self.gc_animated_y(*id, toast_rect.y, now_ms);
                    let animated_rect = RectF::new(
                        toast_rect.x,
                        animated_y,
                        toast_rect.width,
                        toast_rect.height,
                    );
                    cx.register_overlay_hit_region(OverlayHitRegion {
                        abs_rect: animated_rect,
                        owner_abs_rect: animated_rect,
                        widget_id: tree_widget_id,
                    });
                }
                // Register clear_all_rect hit region if N >= 2
                let active_n = rects
                    .iter()
                    .filter(|(id, _, _, _)| {
                        mgr.entry_by_id(*id)
                            .map(|e| e.dismiss_state.get() == DismissState::Visible)
                            .unwrap_or(false)
                    })
                    .count();
                if active_n >= 2 {
                    let ca = self.clear_all_rect.get();
                    if ca.width > 0.0 {
                        cx.register_overlay_hit_region(OverlayHitRegion {
                            abs_rect: ca,
                            owner_abs_rect: ca,
                            widget_id: tree_widget_id,
                        });
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, _phase: EventPhase) -> bool {
        match event {
            Event::Pointer(pe) if pe.kind == PointerEventKind::Move => {
                let (px, py) = pe.local_pos;
                let now_ms = cx.frame_time_ms();
                let stack_rect = self.stack_rect.get();
                let inside = px >= stack_rect.x
                    && px < stack_rect.x + stack_rect.width
                    && py >= stack_rect.y
                    && py < stack_rect.y + stack_rect.height;
                let next_target = self.target_at(px, py, now_ms);
                if self.hover_target.get() != next_target {
                    self.hover_target.set(next_target);
                    cx.invalidate_layout();
                }

                let mgr = self.manager.borrow();
                let n = mgr.active_entries().len();
                drop(mgr);

                let state = self.stack_state.get();
                match (inside, n >= 2, state) {
                    (true, true, StackState::Collapsed) => {
                        self.stack_state.set(StackState::Expanding);
                        self.transition_start_ms.set(Some(cx.frame_time_ms()));
                    }
                    (false, _, StackState::Expanded | StackState::Expanding) => {
                        if self.gap_close.borrow().is_empty() {
                            self.stack_state.set(StackState::Collapsing);
                            self.transition_start_ms.set(Some(cx.frame_time_ms()));
                        }
                    }
                    _ => {}
                }
                false // hover never consumes the event
            }
            Event::Pointer(pe)
                if pe.kind == PointerEventKind::Down
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let (px, py) = pe.local_pos;
                let now_ms = cx.frame_time_ms();
                let state = self.stack_state.get();

                if let Some(target) = self.target_at(px, py, now_ms) {
                    self.hover_target.set(Some(target));
                    self.press_target.set(Some(target));
                    cx.capture_pointer(PointerButton::Primary);
                    cx.invalidate_layout();
                    return true;
                }

                let rects = self.rects.borrow();
                let mgr = self.manager.borrow();

                for (id, toast_rect, _, toast_h) in rects.iter() {
                    let entry = match mgr.entry_by_id(*id) {
                        Some(e) => e,
                        None => continue,
                    };
                    if entry.dismiss_state.get() != DismissState::Visible {
                        continue;
                    }

                    let hit_rect = match state {
                        StackState::Collapsed | StackState::Collapsing => {
                            // Only position 0 is hittable in collapsed
                            if *id != rects.first().map(|r| r.0).unwrap_or(*id) {
                                continue;
                            }
                            *toast_rect
                        }
                        _ => *toast_rect,
                    };

                    // Body click
                    let dy = self.gc_animated_y(*id, toast_rect.y, now_ms) - toast_rect.y;
                    let animated_hit_rect =
                        RectF::new(hit_rect.x, hit_rect.y + dy, hit_rect.width, hit_rect.height);
                    if px >= animated_hit_rect.x
                        && px < animated_hit_rect.x + animated_hit_rect.width
                        && py >= animated_hit_rect.y
                        && py < animated_hit_rect.y + animated_hit_rect.height
                    {
                        self.start_gap_close(*id, *toast_h, &rects, now_ms);
                        entry.dismiss_state.set(DismissState::Exiting);
                        return true;
                    }
                }
                false
            }
            Event::Pointer(pe)
                if pe.kind == PointerEventKind::Up && pe.button == Some(PointerButton::Primary) =>
            {
                let (px, py) = pe.local_pos;
                let now_ms = cx.frame_time_ms();
                let release_target = self.target_at(px, py, now_ms);
                let armed_target = self.press_target.take();
                self.hover_target.set(release_target);
                cx.invalidate_layout();

                let Some(target) = release_target else {
                    return true;
                };
                if armed_target != Some(target) {
                    return true;
                }

                match target {
                    ToastControlTarget::ClearAll => {
                        self.manager.borrow_mut().dismiss_all();
                        true
                    }
                    ToastControlTarget::Action(id) => {
                        let rects = self.rects.borrow();
                        let mgr = self.manager.borrow();
                        if let Some((_, _, _, toast_h)) =
                            rects.iter().find(|(entry_id, _, _, _)| *entry_id == id)
                        {
                            self.start_gap_close(id, *toast_h, &rects, now_ms);
                        }
                        if let Some(entry) = mgr.entry_by_id(id) {
                            if entry.dismiss_state.get() == DismissState::Visible {
                                entry.dismiss_state.set(DismissState::Exiting);
                            }
                        }
                        true
                    }
                }
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn needs_animation(&self) -> bool {
        if self.manager.borrow().has_active() {
            return true;
        }
        // Also animate during stack transitions or gap closes in progress
        !matches!(
            self.stack_state.get(),
            StackState::Collapsed | StackState::Expanded
        ) || !self.gap_close.borrow().is_empty()
    }

    fn debug_type_name(&self) -> &'static str {
        "ToastWidget"
    }

    fn hit_testable(&self) -> bool {
        // Toast input is routed through overlay hit regions. The widget lays out
        // full-screen for overlay placement and must not block base widgets.
        false
    }
}
