// =============================================================================
// DisclosureToggle: Interactive expand/collapse control for tree nodes
// =============================================================================
//
// Real child widget reconciled by stable node_id during TreeViewBody layout.
// Paints a [+]/[-] box indicator (collapsed: +, expanded: -).
// Handles pointer interaction with Button-mirrored state machine:
//   Down → Pressed + capture, Up inside → toggle + callback.
//
// INV-UI-IDENTITY-STABLE: Widget identity derived ONLY from VisibleRow.node_id.
// INV-NO-INDEX-IDENTITY: No row_index in identity or interaction paths.
// INV-CHILD-BOUNDED: Stale nodes removed when they leave the viewport.
// INV-COORD-CONTRACT: Rect set by parent in content-local coordinate space.
// =============================================================================

use std::any::Any;
use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{Color, LAYER_CONTENT};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::utils::id::WidgetInternalId;

/// Inset from box border to +/- sign strokes (px).
const SIGN_INSET: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToggleState {
    Idle,
    Hovered,
    Pressed,
}

/// Disclosure toggle: interactive expand/collapse indicator for tree nodes.
///
/// Interaction model mirrors Button (SPEC-HAL-011/021):
///   - Down → Pressed + pointer capture
///   - Move during capture → visual feedback based on inside/outside
///   - Up inside → toggle + callback
///   - Up outside → cancel
///   - Hover enter/leave → animated overlay
pub(crate) struct DisclosureToggle {
    id: WidgetInternalId,
    expanded: bool,
    on_toggle: Option<Box<dyn FnMut()>>,
    state: ToggleState,
    /// Layout-computed bounds for hit-testing (mirrors Button pattern).
    layout_bounds: Size,
    /// Cached hover animation value [0.0, 1.0] (Cell for paint-phase interior mutability).
    hover_t: Cell<f32>,
    hover_anim: Cell<Option<TransitionHandle>>,
    /// Cached pressed animation value [0.0, 1.0].
    pressed_t: Cell<f32>,
    pressed_anim: Cell<Option<TransitionHandle>>,
}

impl DisclosureToggle {
    pub fn new(expanded: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            expanded,
            on_toggle: None,
            state: ToggleState::Idle,
            layout_bounds: Size::zero(),
            hover_t: Cell::new(0.0),
            hover_anim: Cell::new(None),
            pressed_t: Cell::new(0.0),
            pressed_anim: Cell::new(None),
        }
    }

    /// Set toggle callback. Fires on pointer-up inside bounds.
    /// HTS is sole topology authority — callback requests toggle, parent reconciles expanded state.
    pub fn on_toggle<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }
}

impl Widget for DisclosureToggle {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[crate::core::widget_tree::NodeId],
    ) -> LayoutResult {
        self.layout_bounds = Size::new(constraints.max_width, constraints.max_height);
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _children: &[crate::core::widget_tree::NodeId],
    ) {
        // Consume animation values (paint-phase interior mutability).
        {
            let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
            crate::core::rect_authority::assert_paint_mutation_authorized(
                "DisclosureToggle::paint: hover_t/pressed_t",
            );

            if let Some(handle) = self.hover_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.hover_t.set(value);
                } else {
                    self.hover_anim.set(None);
                }
            }

            if let Some(handle) = self.pressed_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.pressed_t.set(value);
                } else {
                    self.pressed_anim.set(None);
                }
            }
        }

        // Box border + sign: stronger than guide lines (interactive control, not passive structure).
        // 75% blend toward text vs guides at 55%. Same color family, more prominent.
        let bg = cx.theme.colors.list.row_background;
        let fg = cx.theme.colors.list.text;
        let t = 0.75_f32;
        let color = Color {
            r: bg.r + (fg.r - bg.r) * t,
            g: bg.g + (fg.g - bg.g) * t,
            b: bg.b + (fg.b - bg.b) * t,
            a: 1.0,
        };

        // Derive box size from row height; force odd for centered + sign.
        let raw = (rect.height * 0.5).floor().clamp(9.0, 13.0);
        let box_size = if raw as i32 % 2 == 0 { raw - 1.0 } else { raw };

        // Center box in widget rect, snap to integer grid for crisp 1px borders.
        let sf = cx.scene.scale_factor();
        let bx = snap_to_physical(rect.x + (rect.width - box_size) * 0.5, sf);
        let by = snap_to_physical(rect.y + (rect.height - box_size) * 0.5, sf);

        // --- Background fill (sharp rectangle, masks guide lines behind box) ---
        let bg_color = cx.theme.colors.list.row_background;
        cx.scene.push_rect(
            RectF::new(bx, by, box_size, box_size),
            bg_color,
            LAYER_CONTENT,
            None,
        );

        // --- Sharp border (4 full edges, 1px) ---
        let bw = 1.0;
        // Top edge.
        cx.scene
            .push_rect(RectF::new(bx, by, box_size, bw), color, LAYER_CONTENT, None);
        // Bottom edge.
        cx.scene.push_rect(
            RectF::new(bx, by + box_size - bw, box_size, bw),
            color,
            LAYER_CONTENT,
            None,
        );
        // Left edge.
        cx.scene
            .push_rect(RectF::new(bx, by, bw, box_size), color, LAYER_CONTENT, None);
        // Right edge.
        cx.scene.push_rect(
            RectF::new(bx + box_size - bw, by, bw, box_size),
            color,
            LAYER_CONTENT,
            None,
        );

        // --- Pressed overlay (full box) ---
        let pressed_alpha = self.pressed_t.get();
        if pressed_alpha > 0.001 {
            let alpha = pressed_alpha * pressed_alpha * cx.theme.styles.pressed.max_alpha;
            let oc = Color::from_srgb(0.0, 0.0, 0.0, alpha);
            cx.scene.push_rect(
                RectF::new(bx, by, box_size, box_size),
                oc,
                LAYER_CONTENT,
                None,
            );
        }

        // --- Hover overlay (full box, suppressed during Pressed) ---
        let hover_alpha = self.hover_t.get();
        if hover_alpha > 0.001 && self.state != ToggleState::Pressed {
            let alpha = hover_alpha * hover_alpha * cx.theme.styles.hover.max_alpha;
            let oc = Color::from_srgb(1.0, 1.0, 1.0, alpha);
            cx.scene.push_rect(
                RectF::new(bx, by, box_size, box_size),
                oc,
                LAYER_CONTENT,
                None,
            );
        }

        // --- Horizontal bar (both states: minus sign). Pixel-snapped centering. ---
        let center = (box_size * 0.5).floor();
        let h_x = bx + SIGN_INSET;
        let h_y = snap_to_physical(by + center, sf);
        let h_w = box_size - SIGN_INSET * 2.0;
        cx.scene
            .push_rect(RectF::new(h_x, h_y, h_w, 1.0), color, LAYER_CONTENT, None);

        // --- Vertical bar (collapsed only: plus sign). Pixel-snapped centering. ---
        if !self.expanded {
            let v_x = snap_to_physical(bx + center, sf);
            let v_y = by + SIGN_INSET;
            let v_h = box_size - SIGN_INSET * 2.0;
            cx.scene
                .push_rect(RectF::new(v_x, v_y, 1.0, v_h), color, LAYER_CONTENT, None);
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down
                | PointerEventKind::DoubleClick
                | PointerEventKind::TripleClick => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }

                    self.state = ToggleState::Pressed;
                    if let Some(button) = pe.button {
                        cx.capture_pointer(button);
                    }

                    // Pressed-in animation: 35ms EaseOut (mirrors Button).
                    let anim = Animation {
                        from: self.pressed_t.get(),
                        to: 1.0,
                        duration: 0.035,
                        easing: Easing::EaseOut,
                    };
                    if let Some(handle) = cx.start_animation(anim) {
                        self.pressed_anim.set(Some(handle));
                    } else {
                        self.pressed_t.set(1.0);
                        self.pressed_anim.set(None);
                    }

                    true
                }
                PointerEventKind::Move => {
                    if self.state == ToggleState::Pressed {
                        let (local_x, local_y) = pe.local_pos;
                        let inside = local_x >= 0.0
                            && local_x <= self.layout_bounds.width
                            && local_y >= 0.0
                            && local_y <= self.layout_bounds.height;

                        if inside {
                            self.state = ToggleState::Pressed;
                        } else {
                            self.state = ToggleState::Idle;
                            // Pressed fade-out on leave: 50ms EaseIn (tightened cancel).
                            let anim = Animation {
                                from: self.pressed_t.get(),
                                to: 0.0,
                                duration: 0.05,
                                easing: Easing::EaseIn,
                            };
                            if let Some(handle) = cx.start_animation(anim) {
                                self.pressed_anim.set(Some(handle));
                            } else {
                                self.pressed_t.set(0.0);
                                self.pressed_anim.set(None);
                            }
                        }
                        return true;
                    }
                    false
                }
                PointerEventKind::Up => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }

                    let (local_x, local_y) = pe.local_pos;
                    let inside = local_x >= 0.0
                        && local_x <= self.layout_bounds.width
                        && local_y >= 0.0
                        && local_y <= self.layout_bounds.height;

                    if inside {
                        // Request toggle — HTS is sole authority, parent reconciles expanded.
                        if let Some(cb) = &mut self.on_toggle {
                            cb();
                        }
                        cx.invalidate_layout();
                    }

                    self.state = ToggleState::Idle;

                    // Pressed-out animation: 70ms EaseIn (tightened release).
                    let anim = Animation {
                        from: self.pressed_t.get(),
                        to: 0.0,
                        duration: 0.07,
                        easing: Easing::EaseIn,
                    };
                    if let Some(handle) = cx.start_animation(anim) {
                        self.pressed_anim.set(Some(handle));
                    } else {
                        self.pressed_t.set(0.0);
                        self.pressed_anim.set(None);
                    }

                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    // Hover lifecycle (mirrors Button: SPEC-HAL-038 Stage 7 / SPEC-HAL-021).

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if self.state != ToggleState::Pressed {
            self.state = ToggleState::Hovered;

            // Hover fade-in: 120ms EaseOut (mirrors Button).
            let anim = Animation {
                from: self.hover_t.get(),
                to: 1.0,
                duration: 0.12,
                easing: Easing::EaseOut,
            };
            if let Some(handle) = cx.start_animation(anim) {
                self.hover_anim.set(Some(handle));
            } else {
                self.hover_t.set(1.0);
                self.hover_anim.set(None);
            }
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.state != ToggleState::Pressed {
            self.state = ToggleState::Idle;

            // Hover fade-out: 120ms EaseIn (mirrors Button).
            let anim = Animation {
                from: self.hover_t.get(),
                to: 0.0,
                duration: 0.12,
                easing: Easing::EaseIn,
            };
            if let Some(handle) = cx.start_animation(anim) {
                self.hover_anim.set(Some(handle));
            } else {
                self.hover_t.set(0.0);
                self.hover_anim.set(None);
            }
        }
    }

    // Mount-time animation reset (ANIM-INV-10, mirrors Button).
    fn on_mount(&mut self) {
        self.hover_t.set(0.0);
        self.hover_anim.set(None);
        self.pressed_t.set(0.0);
        self.pressed_anim.set(None);
    }

    fn debug_type_name(&self) -> &'static str {
        "DisclosureToggle"
    }
}

impl WidgetDowncast for DisclosureToggle {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
