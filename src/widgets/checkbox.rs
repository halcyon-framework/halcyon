// SPEC-HAL-111 Checkbox Widget
// SPEC-HAL-208: Integrated label support — label is owned, not a child widget.
// Core reducer + widget shell + input mapping + action emission.

use std::cell::Cell;

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, PointerButton, PointerEvent, PointerEventKind,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
#[cfg(test)]
use crate::core::paint::LAYER_FOCUS;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::form_binder::{BindingKey, ChangeWriter};
use crate::utils::id::WidgetInternalId;

// ============================================================================
// Core Reducer (SPEC-HAL-111 Stage 7 + Stage 8)
// ============================================================================

/// Checkbox state vector (SPEC-HAL-111 Stage 1)
#[derive(Clone, Debug, PartialEq)]
pub struct CheckboxState {
    pub checked: bool,
    pub disabled: bool,
    pub focused: bool,
    pub active: bool,
}

/// Semantic events (SPEC-HAL-111 Stage 2) — CLOSED
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CheckboxEvent {
    Activate,
    Cancel,
    FocusGain,
    FocusLoss,
}

/// Pure reducer implementing SPEC-HAL-111 Stage 7 + Stage 8.
///
/// Mutates `state` synchronously.
/// Returns `Some(new_checked)` only when `Activate` toggles `checked`.
/// Returns `None` otherwise.
pub fn reduce_checkbox(state: &mut CheckboxState, event: CheckboxEvent) -> Option<bool> {
    // INV-D1: Disabled means inert — all events are no-ops
    if state.disabled {
        return None;
    }

    match event {
        CheckboxEvent::Activate => {
            // Stage 7: Activate toggles checked, clears active
            state.checked = !state.checked;
            state.active = false;
            // Stage 8: Emit ToggleChanged only when checked changes (it always does here)
            Some(state.checked)
        }
        CheckboxEvent::Cancel => {
            // Stage 7: Cancel clears active, never toggles
            state.active = false;
            None
        }
        CheckboxEvent::FocusGain => {
            // Stage 7: FocusGain sets focused = true
            state.focused = true;
            None
        }
        CheckboxEvent::FocusLoss => {
            // Stage 7: FocusLoss sets focused = false, clears active
            state.focused = false;
            state.active = false;
            None
        }
    }
}

// ============================================================================
// Widget Shell (SPEC-HAL-111 + SPEC-HAL-208)
// ============================================================================

impl Default for CheckboxState {
    fn default() -> Self {
        Self {
            checked: false,
            disabled: false,
            focused: false,
            active: false,
        }
    }
}

/// Checkbox widget wrapping the pure reducer.
/// Includes input mapping (pointer/keyboard) and action emission.
///
/// SPEC-HAL-208: Owns its label directly. The label is not a child widget,
/// not a sibling, not externally composed. Text is rendered by this control.
pub struct Checkbox {
    id: WidgetInternalId,
    state: CheckboxState,
    /// SPEC-HAL-208 INV-A1: Owned label text. None = indicator-only (legacy).
    label: Option<String>,
    /// Callback invoked when checked state changes.
    /// Called only when reducer returns Some(new_checked).
    on_toggle_changed: Option<Box<dyn FnMut(bool)>>,
    /// SPEC-HAL-207: Binding key + writer for change store emission.
    binding: Option<(BindingKey, ChangeWriter)>,
    /// SPEC-HAL-141: Layout-computed bounds for hit-testing.
    /// NOT authoritative geometry - WidgetNode owns that.
    /// Written: layout phase only. Read: event handlers.
    layout_bounds: Size,
    /// Cached content width (indicator + gap + label).
    /// Used for x hit-testing — clicks beyond this width are ignored.
    cached_content_width: f32,
    hover_t: f32,
    hover_target: f32,
    hover_anim: Option<TransitionHandle>,
    hover_retiring: Option<TransitionHandle>,
    /// Two-phase press animation [0.0, 1.0]. Visual depress on Down, release on Up.
    /// Pattern: Button::pressed_t (SPEC-HAL-021-A).
    press_t: Cell<f32>,
    press_anim: Cell<Option<TransitionHandle>>,
    press_retiring: Option<TransitionHandle>,
    /// Checked state animation [0.0, 1.0]. Drives background color lerp + checkmark appearance.
    /// Pattern: Switch::toggle_t.
    check_t: f32,
    check_target: f32,
    check_anim: Option<TransitionHandle>,
    check_retiring: Option<TransitionHandle>,
}

impl Checkbox {
    pub(crate) fn cell_editor_visual_facts(&self) -> crate::core::appearance::CellEditorVisualFacts {
        crate::core::appearance::CellEditorVisualFacts {
            disabled: self.state.disabled,
            hovered: self.hover_target != 0.0,
        }
    }

    /// Create a new unchecked, enabled checkbox.
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: CheckboxState::default(),
            label: None,
            on_toggle_changed: None,
            binding: None,
            layout_bounds: Size::zero(),
            cached_content_width: 0.0,
            hover_t: 0.0,
            hover_target: 0.0,
            hover_anim: None,
            hover_retiring: None,
            press_t: Cell::new(0.0),
            press_anim: Cell::new(None),
            press_retiring: None,
            check_t: 0.0,
            check_target: 0.0,
            check_anim: None,
            check_retiring: None,
        }
    }

    /// Create a checkbox with initial checked state.
    pub fn checked(checked: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: CheckboxState {
                checked,
                ..Default::default()
            },
            label: None,
            on_toggle_changed: None,
            binding: None,
            layout_bounds: Size::zero(),
            cached_content_width: 0.0,
            hover_t: 0.0,
            hover_target: 0.0,
            hover_anim: None,
            hover_retiring: None,
            press_t: Cell::new(0.0),
            press_anim: Cell::new(None),
            press_retiring: None,
            check_t: if checked { 1.0 } else { 0.0 },
            check_target: if checked { 1.0 } else { 0.0 },
            check_anim: None,
            check_retiring: None,
        }
    }

    /// SPEC-HAL-208: Create a checkbox with an integrated label (builder pattern).
    ///
    /// The label is owned by this widget — not a child, not a sibling.
    /// Clicking anywhere on the label toggles the checkbox.
    pub fn with_label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// SPEC-HAL-208: Set or replace the label via mutable reference.
    pub fn set_label(&mut self, text: impl Into<String>) {
        self.label = Some(text.into());
    }

    /// Set checked state without firing `on_toggle_changed`.
    ///
    /// For external state sync (e.g., parent widget re-reading a data
    /// source after a revision bump and mirroring the new value into a
    /// pooled Checkbox). User-initiated toggles still route through
    /// `on_semantic_event`, which DOES fire the callback. Animation
    /// Update publishes the new endpoint without replacing an active pointer
    /// preview. Displaced playback remains owned until cancellation is available.
    pub fn set_checked(&mut self, checked: bool) {
        if self.state.checked == checked {
            return;
        }
        self.state.checked = checked;
        if !self.state.active {
            self.check_target = if checked { 1.0 } else { 0.0 };
            if let Some(handle) = self.check_anim.take() {
                self.check_retiring = Some(handle);
            }
        }
    }

    /// Set disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.state.disabled = disabled;
    }

    /// Get current checked state.
    pub fn is_checked(&self) -> bool {
        self.state.checked
    }

    #[cfg(test)]
    pub(crate) fn check_visual_progress_for_test(&self) -> f32 {
        self.check_t
    }

    #[cfg(test)]
    pub(crate) fn hover_visual_progress_for_test(&self) -> f32 {
        self.hover_t
    }

    /// Get current disabled state.
    pub fn is_disabled(&self) -> bool {
        self.state.disabled
    }

    /// Get current focused state.
    pub fn is_focused(&self) -> bool {
        self.state.focused
    }

    /// Set the toggle changed callback (builder pattern).
    pub fn on_toggle_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle_changed = Some(Box::new(callback));
        self
    }

    /// Set callback via mutable reference (for testing).
    pub fn set_on_toggle_changed<F>(&mut self, callback: F)
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle_changed = Some(Box::new(callback));
    }

    /// SPEC-HAL-207: Configure binding for change store emission.
    ///
    /// On state change, emits the new checked value into the change store.
    pub fn set_binding(&mut self, key: &str, writer: ChangeWriter) {
        let binding_key = writer.intern(key);
        self.binding = Some((binding_key, writer));
    }

    /// Process a semantic event through the reducer.
    ///
    /// Returns `Some(new_checked)` only when `Activate` toggles `checked`.
    /// Invokes `on_toggle_changed` callback when checked changes.
    /// SPEC-HAL-207: Emits into change store when binding is configured.
    pub fn on_semantic_event(&mut self, event: CheckboxEvent) -> Option<bool> {
        let result = reduce_checkbox(&mut self.state, event);

        // Emit action only when checked actually changed
        if let Some(new_checked) = result {
            if let Some(callback) = &mut self.on_toggle_changed {
                callback(new_checked);
            }
            // SPEC-HAL-207: Emit into change store
            if let Some((key, writer)) = &self.binding {
                writer.set_bool(*key, new_checked);
            }
        }

        result
    }

    /// Get immutable reference to state (for testing).
    pub fn state(&self) -> &CheckboxState {
        &self.state
    }

    fn retire_press(&mut self, cx: &mut EventContext) {
        if cx.has_animation() {
            for handle in [self.press_retiring.take(), self.press_anim.take()]
                .into_iter()
                .flatten()
            {
                cx.cancel_animation(handle);
            }
        } else if let Some(handle) = self.press_anim.take() {
            self.press_retiring = Some(handle);
        }
    }

    fn animate_check_to(
        &mut self,
        cx: &mut EventContext,
        target: f32,
        duration: f32,
        easing: Easing,
    ) {
        self.check_target = target;
        if cx.has_animation() {
            for handle in [self.check_retiring.take(), self.check_anim.take()]
                .into_iter()
                .flatten()
            {
                cx.cancel_animation(handle);
            }
            self.check_anim = cx.start_animation(Animation {
                from: self.check_t,
                to: target,
                duration,
                easing,
            });
        } else if let Some(handle) = self.check_anim.take() {
            self.check_retiring = Some(handle);
        }
        if self.check_anim.is_none() && self.check_t != target {
            self.check_t = target;
            cx.invalidate_paint();
        }
    }

    fn snap_check_to(&mut self, cx: &mut EventContext, target: f32) {
        self.check_target = target;
        if cx.has_animation() {
            for handle in [self.check_retiring.take(), self.check_anim.take()]
                .into_iter()
                .flatten()
            {
                cx.cancel_animation(handle);
            }
        } else if let Some(handle) = self.check_anim.take() {
            self.check_retiring = Some(handle);
        }
        if self.check_t != target {
            self.check_t = target;
            cx.invalidate_paint();
        }
    }

    fn animate_hover(&mut self, cx: &mut EventContext, target: f32, easing: Easing) {
        self.hover_target = target;
        if cx.has_animation() {
            for handle in [self.hover_retiring.take(), self.hover_anim.take()]
                .into_iter()
                .flatten()
            {
                cx.cancel_animation(handle);
            }
            self.hover_anim = cx.start_animation(Animation {
                from: self.hover_t,
                to: target,
                duration: 0.08,
                easing,
            });
        } else if let Some(handle) = self.hover_anim.take() {
            self.hover_retiring = Some(handle);
        }
        if self.hover_anim.is_none() && self.hover_t != target {
            self.hover_t = target;
            cx.invalidate_paint();
        }
    }

    fn pin_hover_visual(&mut self, cx: &mut EventContext) {
        self.hover_target = 1.0;
        if cx.has_animation() {
            for handle in [self.hover_retiring.take(), self.hover_anim.take()]
                .into_iter()
                .flatten()
            {
                cx.cancel_animation(handle);
            }
        } else if let Some(handle) = self.hover_anim.take() {
            self.hover_retiring = Some(handle);
        }
        if self.hover_t != 1.0 {
            self.hover_t = 1.0;
            cx.invalidate_paint();
        }
    }

    // ========================================================================
    // Input Mapping
    // ========================================================================

    /// Handle an input event and translate to semantic events.
    ///
    /// Returns true if the event was handled.
    pub fn handle_event(&mut self, event: &Event, rect: RectF) -> bool {
        if self.state.disabled {
            return false;
        }

        match event {
            Event::Pointer(ptr) => self.handle_pointer(ptr, rect),
            Event::Key(key) => self.handle_keyboard(key),
            _ => false,
        }
    }

    /// Handle pointer events.
    fn handle_pointer(&mut self, event: &PointerEvent, rect: RectF) -> bool {
        // Hit test: check if pointer is within bounds
        let x = event.local_pos.0;
        let y = event.local_pos.1;
        let in_bounds =
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height;

        match event.kind {
            PointerEventKind::Down => {
                if in_bounds {
                    self.on_semantic_event(CheckboxEvent::FocusGain);
                    self.state.active = true; // Set active for visual feedback
                    true
                } else {
                    false
                }
            }
            PointerEventKind::Up => {
                if self.state.active && in_bounds {
                    self.on_semantic_event(CheckboxEvent::Activate);
                    true
                } else if self.state.active {
                    // Pointer up outside bounds = cancel
                    self.on_semantic_event(CheckboxEvent::Cancel);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Handle keyboard events.
    fn handle_keyboard(&mut self, event: &KeyEvent) -> bool {
        if !event.is_down {
            return false;
        }

        match event.key_code {
            KeyCode::Space | KeyCode::Enter => {
                if self.state.focused {
                    self.on_semantic_event(CheckboxEvent::Activate);
                    true
                } else {
                    false
                }
            }
            KeyCode::Escape => {
                if self.state.active {
                    self.on_semantic_event(CheckboxEvent::Cancel);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for Checkbox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Checkbox {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.checkbox;
        let indicator_size = geom.size;

        // SPEC-HAL-208 INV-B1: Width accounts for indicator + gap + label
        let content_width = if let Some(label_text) = &self.label {
            let font_size = cx.theme.typography.font_size;
            let (text_width, _) = cx.text_engine.measure(label_text, font_size);
            indicator_size + geom.label_gap + text_width
        } else {
            indicator_size
        };
        self.cached_content_width = content_width;
        let width = if self.label.is_some() {
            content_width.min(constraints.max_width)
        } else {
            constraints.max_width
        };

        // Height = control_height for consistent alignment with TextField/SpinBox/etc.
        let height = cx.theme.geometry.text_field.height;
        let height = height.clamp(constraints.min_height, constraints.max_height);
        self.layout_bounds = Size::new(width, height);
        LayoutResult::fit(self.layout_bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Consume the legacy press animation; check and hover publish in Update.
        {
            let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
            if let Some(handle) = self.press_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.press_t.set(value);
                } else {
                    self.press_anim.set(None);
                    self.press_t.set(0.0);
                }
            }
        }

        let theme = cx.theme;
        let geom = &theme.geometry.checkbox;

        // Checkbox box is square, centered vertically in rect.
        // SPEC-HAL-093 v3: derived rect — snap so the 1px border is consistent.
        let box_size = geom.size;
        let box_y = rect.y + (rect.height - box_size) / 2.0;
        let box_rect = RectF::new(rect.x, box_y, box_size, box_size);

        let check_t = self.check_t;
        let hover_t = if self.state.disabled {
            0.0
        } else {
            self.hover_t
        };

        // Colors: lerp surface→accent via check_t for smooth check transition.
        let bg_color = if self.state.disabled {
            if self.state.checked {
                theme.colors.borders.default
            } else {
                theme.colors.surfaces.content
            }
        } else {
            let unchecked_fill = theme
                .colors
                .surfaces
                .content
                .lerp(theme.colors.surfaces.level2, hover_t);
            let checked_fill = theme
                .colors
                .accents
                .primary
                .lerp(theme.colors.accents.hover, hover_t);
            unchecked_fill.lerp(checked_fill, check_t)
        };
        // Visible 1px border so the checkbox box reads against any container
        // surface (page, GroupBox, or elevated Card background). Hover shifts
        // the edge toward the control hover token without using a focus ring.
        let border_color = if self.state.disabled {
            theme.colors.borders.default
        } else {
            theme
                .colors
                .borders
                .default
                .lerp(theme.colors.controls.border_hover, hover_t)
        };

        // SPEC-HAL-093 v3: per-coordinate snap to physical pixel grid;
        // 1-physical-pixel border thickness regardless of DPI.
        let scale = cx.scale_factor;
        let one_px = cx.one_px();
        let bx = crate::core::math::snap_to_physical(box_rect.x, scale);
        let by = crate::core::math::snap_to_physical(box_rect.y, scale);
        let br = crate::core::math::snap_to_physical(box_rect.x + box_rect.width, scale);
        let bb = crate::core::math::snap_to_physical(box_rect.y + box_rect.height, scale);
        let bw = br - bx;
        let bh = bb - by;
        let snapped_box = RectF::new(bx, by, bw, bh);

        // Background fill
        cx.scene.push_rect(snapped_box, bg_color, 2, cx.scene_key);

        // Flat 1px border (borders.default) on all four edges.
        cx.scene
            .push_rect(RectF::new(bx, by, bw, one_px), border_color, 3, None);
        cx.scene.push_rect(
            RectF::new(bx, by + one_px, one_px, bh - 2.0 * one_px),
            border_color,
            3,
            None,
        );
        cx.scene.push_rect(
            RectF::new(bx, bb - one_px, bw, one_px),
            border_color,
            3,
            None,
        );
        cx.scene.push_rect(
            RectF::new(br - one_px, by + one_px, one_px, bh - 2.0 * one_px),
            border_color,
            3,
            None,
        );

        // Press overlay: positive = darken (press-in), negative = brighten (release flash)
        let press_val = self.press_t.get();
        if press_val > 0.001 {
            let alpha = press_val * press_val * 0.15;
            let overlay_color = crate::core::paint::Color::from_srgb(0.0, 0.0, 0.0, alpha);
            cx.scene.push_rect(snapped_box, overlay_color, 3, None);
        } else if press_val < -0.001 {
            let alpha = press_val.abs() * 0.5;
            let overlay_color = crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, alpha);
            cx.scene.push_rect(snapped_box, overlay_color, 3, None);
        }

        // V-shaped checkmark, alpha driven by check_t.
        if check_t > 0.001 {
            let geom_cb = &cx.theme.geometry.checkbox;
            let mark_color = if self.state.disabled {
                theme.colors.surfaces.content
            } else {
                crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, check_t)
            };

            let inset = geom_cb.checkmark_inset;
            let thickness = geom_cb.checkmark_thickness;
            let leg_ratio = geom_cb.checkmark_leg_ratio;

            let left_x = box_rect.x + inset;
            let right_x = box_rect.x + box_size - inset;
            let top_y = box_rect.y + inset;
            let bottom_y = box_rect.y + box_size - inset;

            let mid_x = left_x + (right_x - left_x) * leg_ratio;
            let mid_y = bottom_y;

            use crate::core::math::Point;
            cx.scene.push_line(
                Point::new(left_x, top_y + (bottom_y - top_y) * 0.4),
                Point::new(mid_x, mid_y),
                thickness,
                mark_color,
                4,
                None,
            );
            cx.scene.push_line(
                Point::new(mid_x, mid_y),
                Point::new(right_x, top_y),
                thickness,
                mark_color,
                4,
                None,
            );
        }

        // SPEC-HAL-208 INV-C1: Paint label text after indicator
        if let Some(label_text) = &self.label {
            let font_size = theme.typography.font_size;
            let run = cx.text_engine.shape(label_text, font_size);

            // Label X = indicator right edge + gap
            let label_x = rect.x + box_size + geom.label_gap;

            // SPEC-HAL-092/208 INV-B3: Baseline-aligned label text
            let metrics = cx.text_engine.font_metrics(font_size);
            let baseline_y = rect.y
                + compute_control_text_baseline_y(
                    rect.height,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                );
            let label_y = baseline_y - metrics.ascent;

            let text_color = if self.state.disabled {
                theme.colors.typography.muted
            } else {
                theme.colors.typography.primary
            };
            cx.text_engine.draw(
                &run,
                label_x,
                label_y,
                text_color,
                LAYER_CONTENT + 2,
                cx.scene,
            );
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        // Checkbox is hit-testable for input mapping
        true
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Only handle events in Target phase
        if phase != EventPhase::Target {
            return false;
        }

        if self.state.disabled {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                // SPEC-HAL-208 INV-D1: Hit test uses content width (indicator + gap + label)
                let (local_x, local_y) = pe.local_pos;
                let inside = local_x >= 0.0
                    && local_x <= self.cached_content_width
                    && local_y >= 0.0
                    && local_y <= self.layout_bounds.height;

                match pe.kind {
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if inside {
                            self.on_semantic_event(CheckboxEvent::FocusGain);
                            self.state.active = true;
                            cx.capture_pointer(PointerButton::Primary);
                            self.pin_hover_visual(cx);
                            // Press animation: depress visual
                            self.retire_press(cx);
                            let anim = Animation {
                                from: self.press_t.get(),
                                to: 1.0,
                                duration: 0.035,
                                easing: Easing::EaseOut,
                            };
                            if let Some(handle) = cx.start_animation(anim) {
                                self.press_anim.set(Some(handle));
                            } else {
                                self.press_t.set(1.0);
                                self.press_anim.set(None);
                            }
                            let anticipated_target = if self.state.checked { 0.0 } else { 1.0 };
                            self.snap_check_to(cx, anticipated_target);
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Up => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if self.state.active && inside {
                            if let Some(new_checked) =
                                self.on_semantic_event(CheckboxEvent::Activate)
                            {
                                self.animate_check_to(
                                    cx,
                                    if new_checked { 1.0 } else { 0.0 },
                                    0.12,
                                    Easing::EaseOut,
                                );
                            }
                            // Release press with overshoot (back-easing) for "click landed" flash.
                            self.retire_press(cx);
                            let press_anim = Animation {
                                from: -0.35,
                                to: 0.0,
                                duration: 0.15,
                                easing: Easing::EaseOut,
                            };
                            if let Some(handle) = cx.start_animation(press_anim) {
                                self.press_anim.set(Some(handle));
                            } else {
                                self.press_t.set(0.0);
                                self.press_anim.set(None);
                            }
                            true
                        } else if self.state.active {
                            self.on_semantic_event(CheckboxEvent::Cancel);
                            self.animate_check_to(
                                cx,
                                if self.state.checked { 1.0 } else { 0.0 },
                                0.06,
                                Easing::EaseIn,
                            );
                            // Release press
                            self.retire_press(cx);
                            let press_anim = Animation {
                                from: self.press_t.get(),
                                to: 0.0,
                                duration: 0.04,
                                easing: Easing::EaseIn,
                            };
                            if let Some(handle) = cx.start_animation(press_anim) {
                                self.press_anim.set(Some(handle));
                            } else {
                                self.press_t.set(0.0);
                                self.press_anim.set(None);
                            }
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if !ke.is_down => {
                use crate::core::event::KeyCode;
                if matches!(ke.key_code, KeyCode::Enter | KeyCode::Space) {
                    if self.press_t.get() > 0.001 {
                        self.retire_press(cx);
                        self.press_t.set(0.0);
                        self.press_anim.set(None);
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn focusable(&self) -> bool {
        !self.state.disabled
    }

    fn touch_target(&self, hit: crate::core::event::TouchHit) -> Option<crate::core::event::TouchTarget> {
        (!self.state.disabled && RectF::new(0.0, 0.0, self.cached_content_width, self.layout_bounds.height)
            .contains(hit.local_position))
            .then_some(crate::core::event::TouchTarget::Control)
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, _tap: crate::core::event::TouchTap) -> bool {
        let changed = self.on_semantic_event(CheckboxEvent::Activate).is_some();
        self.animate_check_to(cx, if self.state.checked { 1.0 } else { 0.0 }, 0.12, Easing::EaseOut);
        if changed {
            cx.invalidate_paint();
        }
        true
    }

    fn needs_animation(&self) -> bool {
        self.press_anim.get().is_some()
    }

    fn needs_update(&self) -> bool {
        self.hover_anim.is_some()
            || self.hover_retiring.is_some()
            || self.check_anim.is_some()
            || self.check_retiring.is_some()
            || self.press_retiring.is_some()
            || (!self.state.active && self.check_t != if self.state.checked { 1.0 } else { 0.0 })
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        if let Some(handle) = self.press_retiring {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::TypeMismatch) => {
                    panic!("Checkbox press handle must contain f32");
                }
                Some(_) => {
                    cx.cancel_animation(handle);
                    self.press_retiring = None;
                }
                None => {}
            }
        }

        if let Some(handle) = self.hover_retiring {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::TypeMismatch) => {
                    panic!("Checkbox hover handle must contain f32");
                }
                Some(_) => {
                    cx.cancel_animation(handle);
                    self.hover_retiring = None;
                }
                None => {}
            }
        }

        let previous = self.hover_t;
        if let Some(handle) = self.hover_anim {
            self.hover_t = match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::Active(value)) => value,
                Some(AnimationSample::Completed(value)) => {
                    self.hover_anim = None;
                    value
                }
                Some(AnimationSample::Cancelled) => {
                    self.hover_anim = None;
                    self.hover_t
                }
                Some(AnimationSample::Missing) => {
                    self.hover_anim = None;
                    self.hover_target
                }
                None => {
                    self.hover_anim = None;
                    self.hover_retiring = Some(handle);
                    self.hover_target
                }
                Some(AnimationSample::TypeMismatch) => {
                    panic!("Checkbox hover handle must contain f32");
                }
            };
        }
        if self.hover_t != previous {
            cx.invalidate_paint();
        }

        if let Some(handle) = self.check_retiring {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::TypeMismatch) => {
                    panic!("Checkbox check handle must contain f32");
                }
                Some(_) => {
                    cx.cancel_animation(handle);
                    self.check_retiring = None;
                }
                None => {}
            }
        }
        let endpoint = if self.state.active {
            self.check_target
        } else if self.state.checked {
            1.0
        } else {
            0.0
        };
        let previous = self.check_t;
        if let Some(handle) = self.check_anim {
            self.check_t = match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::Active(value)) => value,
                Some(AnimationSample::Completed(value)) => {
                    self.check_anim = None;
                    value
                }
                Some(AnimationSample::Cancelled) => {
                    self.check_anim = None;
                    endpoint
                }
                Some(AnimationSample::Missing) => {
                    self.check_anim = None;
                    endpoint
                }
                None => {
                    self.check_anim = None;
                    self.check_retiring = Some(handle);
                    endpoint
                }
                Some(AnimationSample::TypeMismatch) => {
                    panic!("Checkbox check handle must contain f32");
                }
            };
        }
        if self.check_anim.is_none() && !self.state.active {
            self.check_target = endpoint;
            self.check_t = endpoint;
        }
        if self.check_t != previous {
            cx.invalidate_paint();
        }
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.on_semantic_event(CheckboxEvent::FocusGain);
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.on_semantic_event(CheckboxEvent::FocusLoss);
    }

    fn accepts_keyboard_activation(&self) -> bool {
        !self.state.disabled
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        // Action system routes ACTIVATE here on KeyDown.
        // Keyboard activation keeps immediate visuals; capability controls cancellation.
        let result = self.on_semantic_event(CheckboxEvent::Activate);
        // Snap press visual (KeyUp handler will clear it)
        self.retire_press(cx);
        self.press_t.set(1.0);
        self.press_anim.set(None);
        // Snap check visual to match new state
        let target = if self.state.checked { 1.0 } else { 0.0 };
        self.snap_check_to(cx, target);
        if result.is_some() {
            cx.invalidate_paint();
        }
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if self.state.disabled {
            return;
        }
        self.animate_hover(cx, 1.0, Easing::EaseOut);
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.state.active {
            self.pin_hover_visual(cx);
            return;
        }

        self.animate_hover(cx, 0.0, Easing::EaseIn);
    }

    fn on_mount(&mut self) {
        self.hover_t = 0.0;
        self.hover_target = 0.0;
        self.hover_anim = None;
        self.hover_retiring = None;
        self.press_t.set(0.0);
        self.press_anim.set(None);
        self.press_retiring = None;
        self.check_t = if self.state.checked { 1.0 } else { 0.0 };
        self.check_target = self.check_t;
        self.check_anim = None;
        self.check_retiring = None;
    }

    fn debug_type_name(&self) -> &'static str {
        "Checkbox"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::animation::{AnimationContext, Timeline};
    use crate::core::app::{
        AppContext, ModalStack, RetainedSceneTestHarness, ScreenContext, ScreenStack,
    };
    use crate::core::event::{InteractionDomains, KeyboardAuthorityMode, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::core::icon::IconRegistry;
    use crate::core::rect_authority::{PaintAuthorityGuard, UpdateAuthorityGuard};
    use crate::core::scene::{SceneBuilder, SceneNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::model::form_binder::FormBinder;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::{flex::Flex, label::Label};
    use std::rc::Rc;
    use std::sync::Arc;

    #[test]
    fn checkbox_press_handle_custody_survives_keyboard_and_capability_changes() {
        use crate::core::event::{
            FusedInputEvent, Modifiers, PhysicalKeyCode, PointerDevice, PointerId,
        };
        use crate::core::math::Point;
        use crate::runtime::binding_registry::BindingRegistry;
        use std::time::Instant;

        fn checkbox(tree: &WidgetTree, node: NodeId) -> &Checkbox {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn publish(
            tree: &mut WidgetTree,
            node: NodeId,
            animation: Option<AnimationContext<'_>>,
        ) -> bool {
            let _update = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(animation);
            cx.begin_widget_update_for_owner(tree.node(node).unwrap().id);
            tree.node_mut(node).unwrap().widget.update(0.0, &mut cx);
            let (paint, subtree, layout) = cx.take_widget_invalidations();
            assert!(!subtree && !layout);
            if paint {
                tree.mark_paint_dirty(node);
            }
            paint
        }
        let theme = Theme::default();
        let mut binder = FormBinder::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut control = Checkbox::new()
            .with_label("Owned press label")
            .on_toggle_changed(move |_| observed.set(observed.get() + 1));
        control.set_binding("checked", binder.writer());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let bounds = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let mut app = AppContext::new(theme.clone());
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
        let take_binding = |binder: &mut FormBinder| {
            let mut value = None;
            binder.bind(&screen, |binding| value = binding.bool("checked").take());
            value
        };
        let mut text = FontManagerTextEngine::new();
        let icons = IconRegistry::new(&[]);
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        macro_rules! frame {
            () => {
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(300.0, 100.0)),
                    &theme,
                    &mut text,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                )
            };
        }
        macro_rules! pointer {
            ($down:expr, $pos:expr, $capable:expr) => {{
                let pos = $pos;
                let event = if $down {
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    }
                } else {
                    FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    }
                };
                let animation = if $capable {
                    Some(harness.animation_context())
                } else {
                    None
                };
                router.route_input_event(
                    &mut tree,
                    event,
                    Some(root),
                    &mut text,
                    animation,
                    domains.primary_state_mut(),
                    &mut bindings,
                );
            }};
        }
        macro_rules! key_up {
            ($capable:expr) => {{
                let event = FusedInputEvent::KeyUp {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code: KeyCode::Space,
                    physical_key: PhysicalKeyCode::Space,
                    reservation: None,
                    modifiers: Modifiers::default(),
                };
                let animation = if $capable {
                    Some(harness.animation_context())
                } else {
                    None
                };
                router.route_input_event(
                    &mut tree,
                    event,
                    Some(root),
                    &mut text,
                    animation,
                    domains.primary_state_mut(),
                    &mut bindings,
                );
            }};
        }
        harness.advance_transition_time(0.0);
        frame!();
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        frame!();
        router.reset_continuous_frames_request();
        let original_bounds = bounds(&tree);
        let rect = original_bounds[1].2;
        let inside = Point::new(rect.x + 2.0, rect.y + rect.height * 0.5);
        let outside = Point::new(rect.x + rect.width + 10.0, inside.y);
        let hit = |tree: &WidgetTree| {
            let hit = tree.hit_test(inside.x, inside.y).unwrap();
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        };
        let original_hit = hit(&tree);
        let ordinary = |nodes: &[SceneNode]| {
            let edges = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) => Some((rect.rect, rect.z_index)),
                    _ => None,
                })
                .take(5)
                .collect::<Vec<_>>();
            let text = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Text(text) => Some((
                        text.text.clone(),
                        text.position,
                        text.font_size,
                        text.color,
                        text.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let focus = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) => Some((
                        border.rect,
                        border.fill_color,
                        border.border_color,
                        border.border_width,
                        border.radius,
                        border.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (edges, text, focus)
        };
        let initial = harness.local_fragment(owner).unwrap();
        let geometry = ordinary(&initial);
        assert_eq!(geometry.2.len(), 1);
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let stable = |tree: &WidgetTree, harness: &RetainedSceneTestHarness| {
            assert_eq!(bounds(tree), original_bounds);
            assert_eq!(hit(tree), original_hit);
            assert_eq!(ordinary(&harness.local_fragment(owner).unwrap()), geometry);
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(root_id).unwrap()
            ));
            assert!(!tree.node(node).unwrap().dirty_layout);
        };

        pointer!(true, inside, true);
        assert_eq!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        frame!();
        harness.advance_transition_time(0.02);
        frame!();
        let progress = checkbox(&tree, node).press_t.get();
        assert!(progress > 0.0 && progress < 1.0);
        pointer!(false, outside, true);
        assert!(!checkbox(&tree, node).is_checked());
        assert_eq!(calls.get(), 0);
        assert_eq!(take_binding(&mut binder), None);
        let displaced = checkbox(&tree, node).press_anim.get().unwrap();
        assert_eq!(
            harness.animation_context().sample::<f32>(displaced),
            AnimationSample::Active(progress)
        );
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        assert!(checkbox(&tree, node).is_checked());
        assert_eq!(calls.get(), 1);
        assert_eq!(take_binding(&mut binder), Some(true));
        let old_sample = harness.animation_context().sample::<f32>(displaced);
        eprintln!(
            "press after capable keyboard snap: old_sample={old_sample:?} current={:?}",
            checkbox(&tree, node).press_anim.get()
        );
        assert_eq!(
            old_sample,
            AnimationSample::Cancelled,
            "capable keyboard activation must cancel displaced admitted press playback before clearing its ID"
        );
        assert_eq!(checkbox(&tree, node).press_t.get(), 1.0);
        assert!(checkbox(&tree, node).press_anim.get().is_none());
        frame!();
        stable(&tree, &harness);

        pointer!(true, inside, true);
        let pending = checkbox(&tree, node).press_anim.get().unwrap();
        pointer!(false, inside, false);
        assert!(!checkbox(&tree, node).is_checked());
        assert_eq!(calls.get(), 2);
        assert_eq!(take_binding(&mut binder), Some(false));
        assert_eq!(checkbox(&tree, node).press_t.get(), 0.0);
        assert!(checkbox(&tree, node).press_anim.get().is_none());
        assert!(matches!(
            harness.animation_context().sample::<f32>(pending),
            AnimationSample::Active(_)
        ));
        assert!(router.activate_focused_widget(&mut tree, domains.primary_state_mut(), None));
        assert_eq!(calls.get(), 3);
        assert_eq!(take_binding(&mut binder), Some(true));
        key_up!(false);
        assert_eq!(checkbox(&tree, node).press_t.get(), 0.0);
        key_up!(false);
        let value = checkbox(&tree, node);
        assert!(value.hover_anim.is_none() && value.hover_retiring.is_none());
        assert!(value.check_anim.is_none() && value.check_retiring.is_none());
        assert_eq!(value.check_t, 1.0);
        assert!(value.needs_update());
        assert!(!value.needs_animation());
        assert!(!publish(&mut tree, node, None));
        assert!(!publish(&mut tree, node, None));
        assert!(matches!(
            harness.animation_context().sample::<f32>(pending),
            AnimationSample::Active(_)
        ));
        assert!(checkbox(&tree, node).needs_update());
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(pending),
            AnimationSample::Cancelled
        );
        assert!(!checkbox(&tree, node).needs_update());
        assert_eq!(calls.get(), 3);
        assert_eq!(take_binding(&mut binder), None);
        frame!();
        stable(&tree, &harness);
        let settled = harness.local_fragment(owner).unwrap();
        let idle = frame!();
        assert!(!idle.scene_built && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &settled,
            &harness.local_fragment(owner).unwrap()
        ));

        pointer!(true, inside, true);
        let event_retiring = checkbox(&tree, node).press_anim.get().unwrap();
        pointer!(false, outside, false);
        assert!(router.activate_focused_widget(&mut tree, domains.primary_state_mut(), None));
        assert_eq!(calls.get(), 4);
        assert_eq!(take_binding(&mut binder), Some(false));
        assert!(matches!(
            harness.animation_context().sample::<f32>(event_retiring),
            AnimationSample::Active(_)
        ));
        pointer!(true, inside, true);
        assert_eq!(
            harness.animation_context().sample::<f32>(event_retiring),
            AnimationSample::Cancelled
        );
        let replacement = checkbox(&tree, node).press_anim.get().unwrap();
        assert_ne!(replacement, event_retiring);
        assert_eq!(
            harness.animation_context().sample::<f32>(replacement),
            AnimationSample::Active(1.0)
        );
        pointer!(false, outside, true);
        assert_eq!(
            harness.animation_context().sample::<f32>(replacement),
            AnimationSample::Cancelled
        );
        let released = checkbox(&tree, node).press_anim.get().unwrap();
        assert_ne!(released, replacement);
        assert_eq!(
            harness.animation_context().sample::<f32>(released),
            AnimationSample::Active(1.0)
        );
        key_up!(true);
        assert_eq!(
            harness.animation_context().sample::<f32>(released),
            AnimationSample::Cancelled
        );
        assert_eq!(checkbox(&tree, node).press_t.get(), 0.0);
        assert!(checkbox(&tree, node).press_anim.get().is_none());
        key_up!(true);
        assert_eq!(calls.get(), 4);
        assert_eq!(take_binding(&mut binder), None);
        harness.advance_transition_time(0.20);
        frame!();
        harness.advance_transition_time(0.21);
        frame!();
        stable(&tree, &harness);
        assert!(!checkbox(&tree, node).is_checked());
        assert!(!checkbox(&tree, node).state.active);
        assert!(!checkbox(&tree, node).needs_update() && !checkbox(&tree, node).needs_animation());
        let settled = harness.local_fragment(owner).unwrap();
        let idle = frame!();
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &settled,
            &harness.local_fragment(owner).unwrap()
        ));
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
        assert!(!router.continuous_frames_requested());
        assert_eq!(calls.get(), 4);
        assert_eq!(take_binding(&mut binder), None);
    }

    #[test]
    fn checkbox_check_presentation_updates_before_retained_paint() {
        use crate::core::event::{FusedInputEvent, PointerDevice, PointerId};
        use crate::core::math::Point;
        use crate::runtime::binding_registry::BindingRegistry;
        use std::time::Instant;

        fn checkbox(tree: &WidgetTree, node: NodeId) -> &Checkbox {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn checkbox_mut(tree: &mut WidgetTree, node: NodeId) -> &mut Checkbox {
            tree.node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut()
                .unwrap()
        }
        fn published(tree: &WidgetTree, node: NodeId) -> (f32, Option<TransitionHandle>) {
            let value = checkbox(tree, node);
            (value.check_visual_progress_for_test(), value.check_anim)
        }
        fn publish(
            tree: &mut WidgetTree,
            node: NodeId,
            animation: Option<AnimationContext<'_>>,
        ) -> bool {
            let _update = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(animation);
            cx.begin_widget_update_for_owner(tree.node(node).unwrap().id);
            tree.node_mut(node).unwrap().widget.update(0.0, &mut cx);
            let (paint, subtree, layout) = cx.take_widget_invalidations();
            assert!(!subtree && !layout);
            if paint {
                tree.mark_paint_dirty(node);
            }
            paint
        }
        let theme = Theme::default();
        let mut binder = FormBinder::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut control = Checkbox::new()
            .with_label("Owned check label")
            .on_toggle_changed(move |_| observed.set(observed.get() + 1));
        control.set_binding("checked", binder.writer());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let bounds = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let mut app = AppContext::new(theme.clone());
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
        let take_binding = |binder: &mut FormBinder| {
            let mut value = None;
            binder.bind(&screen, |binding| value = binding.bool("checked").take());
            value
        };
        let mut text = FontManagerTextEngine::new();
        let icons = IconRegistry::new(&[]);
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        macro_rules! frame {
            () => {
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(300.0, 100.0)),
                    &theme,
                    &mut text,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                )
            };
        }
        macro_rules! pointer {
            ($down:expr, $pos:expr, $capable:expr) => {{
                let pos = $pos;
                let event = if $down {
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    }
                } else {
                    FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    }
                };
                let animation = if $capable {
                    Some(harness.animation_context())
                } else {
                    None
                };
                router.route_input_event(
                    &mut tree,
                    event,
                    Some(root),
                    &mut text,
                    animation,
                    domains.primary_state_mut(),
                    &mut bindings,
                );
            }};
        }
        harness.advance_transition_time(0.0);
        frame!();
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        frame!();
        router.reset_continuous_frames_request();
        let original_bounds = bounds(&tree);
        let rect = original_bounds[1].2;
        let inside = Point::new(rect.x + 2.0, rect.y + rect.height * 0.5);
        let outside = Point::new(rect.x + rect.width + 10.0, inside.y);
        let hit = |tree: &WidgetTree| {
            let result = tree.hit_test(inside.x, inside.y).unwrap();
            (result.widget_id, result.local_pos, result.absolute_bounds)
        };
        let original_hit = hit(&tree);
        let ordinary = |nodes: &[SceneNode]| {
            let edges = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) => Some((rect.rect, rect.z_index)),
                    _ => None,
                })
                .take(5)
                .collect::<Vec<_>>();
            let labels = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Text(text) => Some((
                        text.text.clone(),
                        text.position,
                        text.font_size,
                        text.color,
                        text.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let focus = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) => Some((
                        border.rect,
                        border.fill_color,
                        border.border_color,
                        border.border_width,
                        border.radius,
                        border.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (edges, labels, focus)
        };
        let original = harness.local_fragment(owner).unwrap();
        let geometry = ordinary(&original);
        assert_eq!(geometry.2.len(), 1);
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let check_scene = |nodes: &[SceneNode], progress: f32, hover: f32| {
            let fill = theme
                .colors
                .surfaces
                .content
                .lerp(theme.colors.surfaces.level2, hover)
                .lerp(
                    theme
                        .colors
                        .accents
                        .primary
                        .lerp(theme.colors.accents.hover, hover),
                    progress,
                );
            let fills = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) if rect.z_index == 2 => Some(rect.color),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(fills, vec![fill.into()]);
            let lines = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Line(line) => Some(line),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(lines.len(), if progress > 0.001 { 2 } else { 0 });
            if !lines.is_empty() {
                let geom = &theme.geometry.checkbox;
                let x = rect.x + geom.checkmark_inset;
                let right = rect.x + geom.size - geom.checkmark_inset;
                let top = rect.y + (rect.height - geom.size) * 0.5 + geom.checkmark_inset;
                let bottom = rect.y + (rect.height + geom.size) * 0.5 - geom.checkmark_inset;
                let middle = Point::new(x + (right - x) * geom.checkmark_leg_ratio, bottom);
                assert_eq!(
                    (lines[0].a, lines[0].b),
                    (Point::new(x, top + (bottom - top) * 0.4), middle)
                );
                assert_eq!((lines[1].a, lines[1].b), (middle, Point::new(right, top)));
                for line in lines {
                    assert_eq!(line.width, geom.checkmark_thickness);
                    assert_eq!(
                        line.color,
                        crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, progress).into()
                    );
                    assert_eq!(line.z_index, 4);
                }
            }
        };
        let stable = |tree: &WidgetTree, harness: &RetainedSceneTestHarness| {
            assert_eq!(bounds(tree), original_bounds);
            assert_eq!(hit(tree), original_hit);
            assert_eq!(ordinary(&harness.local_fragment(owner).unwrap()), geometry);
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(root_id).unwrap()
            ));
            assert!(!tree.node(node).unwrap().dirty_layout);
        };

        pointer!(true, inside, true);
        assert!(!checkbox(&tree, node).is_checked());
        assert!(checkbox(&tree, node).state.active);
        assert_eq!(published(&tree, node).0, 1.0);
        assert_eq!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(take_binding(&mut binder), None);
        frame!();
        harness.advance_transition_time(0.04);
        frame!();
        pointer!(false, outside, true);
        assert!(!checkbox(&tree, node).state.active && !checkbox(&tree, node).is_checked());
        assert!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
        let cancel_preview = published(&tree, node).1.unwrap();
        assert_eq!(
            harness.animation_context().sample::<f32>(cancel_preview),
            AnimationSample::Active(1.0)
        );
        frame!();
        let before_update = harness.local_fragment(owner).unwrap();
        harness.advance_transition_time(0.07);
        let AnimationSample::Active(sample) =
            harness.animation_context().sample::<f32>(cancel_preview)
        else {
            panic!()
        };
        assert!(sample > 0.0 && sample < 1.0);
        let dirty = publish(&mut tree, node, Some(harness.animation_context()));
        eprintln!(
            "check before Paint: typed_sample={sample} published={} paint_invalidated={dirty}",
            published(&tree, node).0
        );
        assert_eq!(
            published(&tree, node).0,
            sample,
            "ordinary Widget Update must publish the admitted check sample before retained Paint"
        );
        assert!(dirty);
        assert!(Arc::ptr_eq(
            &before_update,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        frame!();
        let progressed = harness.local_fragment(owner).unwrap();
        assert!(!Arc::ptr_eq(&before_update, &progressed));
        check_scene(&progressed, sample, checkbox(&tree, node).hover_t);
        stable(&tree, &harness);
        harness.advance_transition_time(0.09);
        frame!();
        let AnimationSample::Active(later) =
            harness.animation_context().sample::<f32>(cancel_preview)
        else {
            panic!()
        };
        assert_eq!(published(&tree, node).0, later);
        assert!(later < sample);
        check_scene(
            &harness.local_fragment(owner).unwrap(),
            later,
            checkbox(&tree, node).hover_t,
        );
        harness.advance_transition_time(0.11);
        assert_eq!(
            harness.animation_context().sample::<f32>(cancel_preview),
            AnimationSample::Completed(0.0)
        );
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(published(&tree, node), (0.0, None));
        frame!();
        assert!(!checkbox(&tree, node).needs_update() && !checkbox(&tree, node).needs_animation());
        let idle = frame!();
        assert!(!idle.scene_built && !idle.needs_next_frame);

        pointer!(true, inside, true);
        pointer!(false, outside, true);
        let cancelled = published(&tree, node).1.unwrap();
        harness.advance_transition_time(0.155);
        frame!();
        harness.advance_transition_time(0.16);
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        frame!();
        assert!(checkbox(&tree, node).press_anim.get().is_none());
        let before_paint = published(&tree, node);
        for _ in 0..2 {
            let mut scene = SceneBuilder::new();
            let timeline = Timeline::new();
            let rects = Default::default();
            let mut hits = Vec::new();
            let mut tooltips = Vec::new();
            {
                let mut cx = PaintContext::new(
                    &tree,
                    &mut scene,
                    &theme,
                    &mut text,
                    &icons,
                    0,
                    &timeline,
                    &rects,
                    &mut hits,
                    &mut tooltips,
                    1.0,
                    None,
                    KeyboardAuthorityMode::Navigation,
                );
                let _paint = PaintAuthorityGuard::enter();
                checkbox(&tree, node).paint(&mut cx, rect, &[]);
            }
            assert_eq!(published(&tree, node), before_paint);
            check_scene(
                &scene.build().root,
                before_paint.0,
                checkbox(&tree, node).hover_t,
            );
        }
        harness.animation_context().cancel(cancelled);
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(published(&tree, node), (0.0, None));
        assert_eq!(
            harness.animation_context().sample::<f32>(cancelled),
            AnimationSample::Cancelled
        );
        frame!();

        pointer!(true, inside, true);
        pointer!(false, outside, true);
        let missed = published(&tree, node).1.unwrap();
        harness.advance_transition_time(0.24);
        harness.advance_transition_time(0.25);
        assert_eq!(
            harness.animation_context().sample::<f32>(missed),
            AnimationSample::Missing
        );
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(published(&tree, node), (0.0, None));
        frame!();

        pointer!(true, inside, true);
        pointer!(false, outside, true);
        let event_retiring = published(&tree, node).1.unwrap();
        pointer!(true, inside, false);
        pointer!(false, outside, false);
        pointer!(true, inside, false);
        assert_eq!(published(&tree, node), (1.0, None));
        checkbox_mut(&mut tree, node).set_checked(true);
        assert!(!publish(&mut tree, node, None));
        checkbox_mut(&mut tree, node).set_checked(false);
        assert!(!publish(&mut tree, node, None));
        assert_eq!(published(&tree, node).0, 1.0);
        assert!(checkbox(&tree, node).state.active);
        pointer!(false, outside, false);
        assert_eq!(published(&tree, node), (0.0, None));
        assert!(checkbox(&tree, node).needs_update());
        assert!(!publish(&mut tree, node, None));
        assert_eq!(
            harness.animation_context().sample::<f32>(event_retiring),
            AnimationSample::Active(1.0)
        );
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(event_retiring),
            AnimationSample::Cancelled
        );

        pointer!(true, inside, true);
        pointer!(false, outside, true);
        let update_retiring = published(&tree, node).1.unwrap();
        assert!(publish(&mut tree, node, None));
        assert_eq!(published(&tree, node), (0.0, None));
        assert!(!publish(&mut tree, node, None));
        assert_eq!(
            harness.animation_context().sample::<f32>(update_retiring),
            AnimationSample::Active(1.0)
        );
        assert!(checkbox(&tree, node).needs_update());
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(update_retiring),
            AnimationSample::Cancelled
        );

        pointer!(true, inside, true);
        pointer!(false, outside, true);
        let setter_retiring = published(&tree, node).1.unwrap();
        checkbox_mut(&mut tree, node).set_checked(true);
        assert!(checkbox(&tree, node).needs_update());
        assert!(!publish(&mut tree, node, None));
        checkbox_mut(&mut tree, node).set_checked(false);
        checkbox_mut(&mut tree, node).set_checked(true);
        assert!(!publish(&mut tree, node, None));
        assert_eq!(
            harness.animation_context().sample::<f32>(setter_retiring),
            AnimationSample::Active(1.0)
        );
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(setter_retiring),
            AnimationSample::Cancelled
        );
        checkbox_mut(&mut tree, node).set_checked(false);
        assert!(checkbox(&tree, node).needs_update());
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(published(&tree, node), (0.0, None));
        assert_eq!(calls.get(), 0);
        assert_eq!(take_binding(&mut binder), None);

        pointer!(true, inside, true);
        pointer!(false, outside, true);
        let keyboard_retiring = published(&tree, node).1.unwrap();
        assert!(router.activate_focused_widget(&mut tree, domains.primary_state_mut(), None));
        assert_eq!(published(&tree, node), (1.0, None));
        assert!(checkbox(&tree, node).is_checked());
        assert_eq!(calls.get(), 1);
        assert_eq!(take_binding(&mut binder), Some(true));
        assert!(!publish(&mut tree, node, None));
        assert_eq!(
            harness.animation_context().sample::<f32>(keyboard_retiring),
            AnimationSample::Active(1.0)
        );
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(keyboard_retiring),
            AnimationSample::Cancelled
        );
        pointer!(true, inside, true);
        pointer!(false, outside, true);
        let keyboard_cancelled = published(&tree, node).1.unwrap();
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        assert_eq!(
            harness
                .animation_context()
                .sample::<f32>(keyboard_cancelled),
            AnimationSample::Cancelled
        );
        assert_eq!(published(&tree, node), (0.0, None));
        assert!(!checkbox(&tree, node).is_checked());
        assert_eq!(calls.get(), 2);
        assert_eq!(take_binding(&mut binder), Some(false));
        harness.advance_transition_time(0.50);
        frame!();
        let settled = harness.local_fragment(owner).unwrap();
        check_scene(&settled, 0.0, checkbox(&tree, node).hover_t);
        stable(&tree, &harness);
        let idle = frame!();
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &settled,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(!checkbox(&tree, node).needs_update() && !checkbox(&tree, node).needs_animation());
        assert_eq!(calls.get(), 2);
        assert_eq!(take_binding(&mut binder), None);
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
        assert!(!router.continuous_frames_requested());
    }

    #[test]
    fn checkbox_hover_update_and_retained_settlement() {
        fn checkbox(tree: &WidgetTree, node: NodeId) -> &Checkbox {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn publish(
            tree: &mut WidgetTree,
            node: NodeId,
            animation: Option<AnimationContext<'_>>,
        ) -> bool {
            let _update = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(animation);
            cx.begin_widget_update_for_owner(tree.node(node).unwrap().id);
            tree.node_mut(node).unwrap().widget.update(0.0, &mut cx);
            let (paint, subtree, layout) = cx.take_widget_invalidations();
            assert!(!subtree && !layout);
            if paint {
                tree.mark_paint_dirty(node);
            }
            paint
        }
        fn geometry(nodes: &[SceneNode]) -> Vec<String> {
            nodes
                .iter()
                .map(|node| match node {
                    SceneNode::Rect(rect) => format!("rect {:?} {}", rect.rect, rect.z_index),
                    SceneNode::Line(line) => format!(
                        "line {:?} {:?} {} {:?} {}",
                        line.a, line.b, line.width, line.color, line.z_index
                    ),
                    SceneNode::Text(text) => format!(
                        "text {:?} {:?} {} {:?} {}",
                        text.text, text.position, text.font_size, text.color, text.z_index
                    ),
                    SceneNode::BorderRect(border) => format!(
                        "focus {:?} {:?} {:?} {} {} {}",
                        border.rect,
                        border.fill_color,
                        border.border_color,
                        border.border_width,
                        border.radius,
                        border.z_index
                    ),
                    other => panic!("unexpected Checkbox primitive: {other:?}"),
                })
                .collect()
        }
        let theme = Theme::default();
        let mut binder = FormBinder::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut control = Checkbox::checked(true)
            .with_label("Owned label")
            .on_toggle_changed(move |_| observed.set(observed.get() + 1));
        control.set_binding("checked", binder.writer());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let bounds = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let presentation = |tree: &WidgetTree| {
            let value = checkbox(tree, node);
            (
                value.hover_t,
                value.hover_target,
                value.hover_anim,
                value.hover_retiring,
            )
        };
        let other_state = |tree: &WidgetTree| {
            let value = checkbox(tree, node);
            (
                value.state.clone(),
                value.label.clone(),
                value.layout_bounds,
                value.cached_content_width,
                value.press_t.get(),
                value.press_anim.get(),
                value.check_t,
                value.check_anim,
            )
        };
        let mut app = AppContext::new(theme.clone());
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
        let take_binding = |binder: &mut FormBinder| {
            let mut value = None;
            binder.bind(&screen, |binding| value = binding.bool("checked").take());
            value
        };
        let mut text = FontManagerTextEngine::new();
        let icons = IconRegistry::new(&[]);
        let mut harness = RetainedSceneTestHarness::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let frame = |tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness,
                     text: &mut FontManagerTextEngine,
                     domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(300.0, 100.0)),
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            )
        };
        let check_colors = |nodes: &[SceneNode], hover: f32| {
            let checked_fill = theme
                .colors
                .accents
                .primary
                .lerp(theme.colors.accents.hover, hover);
            let fill = theme
                .colors
                .surfaces
                .content
                .lerp(theme.colors.surfaces.level2, hover)
                .lerp(checked_fill, 1.0);
            let border = theme
                .colors
                .borders
                .default
                .lerp(theme.colors.controls.border_hover, hover);
            let fills = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) if rect.z_index == 2 => {
                        Some((rect.color.r, rect.color.g, rect.color.b, rect.color.a))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(fills, vec![(fill.r, fill.g, fill.b, fill.a)]);
            let borders = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) if rect.z_index == 3 => {
                        Some((rect.color.r, rect.color.g, rect.color.b, rect.color.a))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(borders, vec![(border.r, border.g, border.b, border.a); 4]);
        };

        harness.advance_transition_time(0.0);
        frame(&mut tree, &mut harness, &mut text, &domains);
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        router.reset_continuous_frames_request();
        frame(&mut tree, &mut harness, &mut text, &domains);
        let original_bounds = bounds(&tree);
        let semantic = other_state(&tree);
        let original = harness.local_fragment(owner).unwrap();
        let original_geometry = geometry(&original);
        assert_eq!(
            original
                .iter()
                .filter(|n| matches!(n, SceneNode::Line(_)))
                .count(),
            2
        );
        assert_eq!(
            original
                .iter()
                .filter(|n| matches!(n, SceneNode::BorderRect(b) if b.z_index == LAYER_FOCUS))
                .count(),
            0,
            "default checkbox emits no enclosing rectangle"
        );
        check_colors(&original, 0.0);
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let hit = tree
            .hit_test(original_bounds[1].2.x + 1.0, original_bounds[1].2.y + 1.0)
            .unwrap();

        router.update_hover(
            &mut tree,
            Some(owner),
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        let entering = checkbox(&tree, node).hover_anim.unwrap();
        assert_eq!(
            harness.animation_context().sample::<f32>(entering),
            AnimationSample::Active(0.0)
        );
        frame(&mut tree, &mut harness, &mut text, &domains);
        assert_eq!(checkbox(&tree, node).hover_t, 0.0);
        assert!(checkbox(&tree, node).needs_update());
        assert!(!checkbox(&tree, node).needs_animation());
        let starting_fragment = harness.local_fragment(owner).unwrap();
        harness.advance_transition_time(0.04);
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        let intermediate = presentation(&tree);
        assert!(intermediate.0 > 0.0 && intermediate.0 < 1.0);
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));

        // Direct Paint only checks hover immutability; real ownership is observed in retained frames.
        for _ in 0..2 {
            let mut scene = SceneBuilder::new();
            let mut hits = Vec::new();
            let mut tooltips = Vec::new();
            let rects = Default::default();
            let timeline = Timeline::new();
            {
                let mut cx = PaintContext::new(
                    &tree,
                    &mut scene,
                    &theme,
                    &mut text,
                    &icons,
                    0,
                    &timeline,
                    &rects,
                    &mut hits,
                    &mut tooltips,
                    1.0,
                    None,
                    KeyboardAuthorityMode::Navigation,
                );
                let _paint = PaintAuthorityGuard::enter();
                checkbox(&tree, node).paint(&mut cx, original_bounds[1].2, &[]);
            }
            check_colors(&scene.build().root, intermediate.0);
            assert_eq!(presentation(&tree), intermediate);
            assert_eq!(other_state(&tree), semantic);
        }
        assert!(frame(&mut tree, &mut harness, &mut text, &domains).scene_built);
        let changed = harness.local_fragment(owner).unwrap();
        assert!(!Arc::ptr_eq(&starting_fragment, &changed));
        assert_eq!(geometry(&changed), original_geometry);
        check_colors(&changed, intermediate.0);
        assert!(Arc::ptr_eq(
            &peer_fragment,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_fragment,
            &harness.local_fragment(root_id).unwrap()
        ));

        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        assert_eq!(
            harness.animation_context().sample::<f32>(entering),
            AnimationSample::Cancelled
        );
        let leaving = checkbox(&tree, node).hover_anim.unwrap();
        assert_eq!(
            harness.animation_context().sample::<f32>(leaving),
            AnimationSample::Active(intermediate.0)
        );
        harness.advance_transition_time(0.20);
        assert_eq!(
            harness.animation_context().sample::<f32>(leaving),
            AnimationSample::Completed(0.0)
        );
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(checkbox(&tree, node).hover_t, 0.0);
        assert!(!checkbox(&tree, node).needs_update());
        let completed = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(completed.scene_built && !completed.needs_next_frame);
        let settled = harness.local_fragment(owner).unwrap();
        check_colors(&settled, 0.0);
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &settled,
            &harness.local_fragment(owner).unwrap()
        ));

        router.update_hover(
            &mut tree,
            Some(owner),
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        harness.advance_transition_time(0.24);
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        let cancelled_value = checkbox(&tree, node).hover_t;
        let cancelled = checkbox(&tree, node).hover_anim.unwrap();
        harness.animation_context().cancel(cancelled);
        assert_eq!(
            harness.animation_context().sample::<f32>(cancelled),
            AnimationSample::Cancelled
        );
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(checkbox(&tree, node).hover_t, cancelled_value);
        assert!(!checkbox(&tree, node).needs_update());
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_colors(&harness.local_fragment(owner).unwrap(), cancelled_value);

        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        let missed = checkbox(&tree, node).hover_anim.unwrap();
        harness.advance_transition_time(0.50);
        harness.advance_transition_time(0.51);
        assert_eq!(
            harness.animation_context().sample::<f32>(missed),
            AnimationSample::Missing
        );
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(checkbox(&tree, node).hover_t, 0.0);
        assert!(!checkbox(&tree, node).needs_update());
        frame(&mut tree, &mut harness, &mut text, &domains);

        router.update_hover(&mut tree, Some(owner), None, domains.primary_state_mut());
        assert_eq!(checkbox(&tree, node).hover_t, 1.0);
        assert!(!checkbox(&tree, node).needs_update());
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_colors(&harness.local_fragment(owner).unwrap(), 1.0);
        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        let retiring = checkbox(&tree, node).hover_anim.unwrap();
        router.update_hover(&mut tree, Some(owner), None, domains.primary_state_mut());
        router.update_hover(&mut tree, None, None, domains.primary_state_mut());
        router.update_hover(&mut tree, Some(owner), None, domains.primary_state_mut());
        assert_eq!(presentation(&tree), (1.0, 1.0, None, Some(retiring)));
        assert!(!publish(&mut tree, node, None));
        assert_eq!(checkbox(&tree, node).hover_retiring, Some(retiring));
        assert!(matches!(
            harness.animation_context().sample::<f32>(retiring),
            AnimationSample::Active(_)
        ));
        assert!(checkbox(&tree, node).needs_update());
        // Event capability cancels retirement before admitting the next effect.
        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        assert_eq!(
            harness.animation_context().sample::<f32>(retiring),
            AnimationSample::Cancelled
        );
        assert!(checkbox(&tree, node).hover_retiring.is_none());
        let unavailable = checkbox(&tree, node).hover_anim.unwrap();
        assert!(publish(&mut tree, node, None));
        assert_eq!(presentation(&tree), (0.0, 0.0, None, Some(unavailable)));
        assert!(!publish(&mut tree, node, None));
        assert!(matches!(
            harness.animation_context().sample::<f32>(unavailable),
            AnimationSample::Active(_)
        ));
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(unavailable),
            AnimationSample::Cancelled
        );
        assert!(!checkbox(&tree, node).needs_update());
        assert!(!checkbox(&tree, node).needs_animation());
        frame(&mut tree, &mut harness, &mut text, &domains);
        let final_owner = harness.local_fragment(owner).unwrap();
        check_colors(&final_owner, 0.0);
        assert_eq!(geometry(&final_owner), original_geometry);
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &final_owner,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_fragment,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_fragment,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert_eq!(bounds(&tree), original_bounds);
        let final_hit = tree
            .hit_test(original_bounds[1].2.x + 1.0, original_bounds[1].2.y + 1.0)
            .unwrap();
        assert_eq!(
            (
                final_hit.widget_id,
                final_hit.local_pos,
                final_hit.absolute_bounds
            ),
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        );
        assert_eq!(other_state(&tree), semantic);
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(calls.get(), 0);
        assert_eq!(take_binding(&mut binder), None);
        assert!(!router.continuous_frames_requested());
    }
}
