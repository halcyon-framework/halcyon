// SPEC-HAL-209: Switch / Toggle Widget
// SPEC-HAL-208: Integrated label support — label is owned, not a child widget.
// Semantic sibling of Checkbox. Binary state, continuous visual affordance (track + thumb).

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::LAYER_CONTENT;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::form_binder::{BindingKey, ChangeWriter};
use crate::utils::id::WidgetInternalId;

// ============================================================================
// State & Events (SPEC-HAL-209 INV-A1: Binary State Only)
// ============================================================================

/// Switch state vector. Mirrors CheckboxState semantics exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchState {
    pub on: bool,
    pub disabled: bool,
    pub focused: bool,
    pub active: bool,
}

impl Default for SwitchState {
    fn default() -> Self {
        Self {
            on: false,
            disabled: false,
            focused: false,
            active: false,
        }
    }
}

/// Semantic events — closed set, identical to Checkbox semantics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SwitchEvent {
    Activate,
    Cancel,
    FocusGain,
    FocusLoss,
}

/// Pure reducer for Switch state.
///
/// Mutates `state` synchronously.
/// Returns `Some(new_on)` only when `Activate` toggles `on`.
/// Returns `None` otherwise.
pub fn reduce_switch(state: &mut SwitchState, event: SwitchEvent) -> Option<bool> {
    // SPEC-HAL-209 INV-E1: Disabled is absolute
    if state.disabled {
        return None;
    }

    match event {
        SwitchEvent::Activate => {
            state.on = !state.on;
            state.active = false;
            Some(state.on)
        }
        SwitchEvent::Cancel => {
            state.active = false;
            None
        }
        SwitchEvent::FocusGain => {
            state.focused = true;
            None
        }
        SwitchEvent::FocusLoss => {
            state.focused = false;
            state.active = false;
            None
        }
    }
}

// ============================================================================
// Widget (SPEC-HAL-209 + SPEC-HAL-208)
// ============================================================================

/// Switch / Toggle widget.
///
/// Binary state with continuous visual affordance (track + thumb).
/// Semantically identical to Checkbox — different visual language only.
///
/// SPEC-HAL-208 INV-A3: Label is owned directly by this widget.
/// SPEC-HAL-209 INV-B1: State changes only on explicit commit (pointer release / keyboard).
/// SPEC-HAL-208A: which side of the indicator the owned label occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LabelSide {
    /// Label before the indicator in flow order (left in LTR).
    Leading,
    /// Label after the indicator in flow order (right in LTR). Default.
    #[default]
    Trailing,
}

pub struct Switch {
    id: WidgetInternalId,
    state: SwitchState,
    /// SPEC-HAL-208 INV-A3: Owned label text. None = track-only.
    label: Option<String>,
    /// SPEC-HAL-208A INV-H1: which side of the track the label occupies.
    label_side: LabelSide,
    /// Callback invoked when on/off state changes.
    on_toggle_changed: Option<Box<dyn FnMut(bool)>>,
    /// SPEC-HAL-207: Binding key + writer for change store emission.
    binding: Option<(BindingKey, ChangeWriter)>,
    /// Layout-computed bounds for hit-testing.
    layout_bounds: Size,
    /// Natural indicator, gap and owned-label span, before allocation clipping.
    cached_content_width: f32,
    hover_t: f32,
    hover_target: f32,
    hover_anim: Option<TransitionHandle>,
    hover_retiring: Option<TransitionHandle>,
    toggle_t: f32,
    toggle_target: f32,
    toggle_anim: Option<TransitionHandle>,
    toggle_retiring: Option<TransitionHandle>,
}

impl Switch {
    /// Create a new switch in the off state.
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: SwitchState::default(),
            label: None,
            label_side: LabelSide::default(),
            on_toggle_changed: None,
            binding: None,
            layout_bounds: Size::zero(),
            cached_content_width: 0.0,
            hover_t: 0.0,
            hover_target: 0.0,
            hover_anim: None,
            hover_retiring: None,
            toggle_t: 0.0,
            toggle_target: 0.0,
            toggle_anim: None,
            toggle_retiring: None,
        }
    }

    /// Create a switch with initial on/off state.
    pub fn on(on: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: SwitchState {
                on,
                ..Default::default()
            },
            label: None,
            label_side: LabelSide::default(),
            on_toggle_changed: None,
            binding: None,
            layout_bounds: Size::zero(),
            cached_content_width: 0.0,
            hover_t: 0.0,
            hover_target: 0.0,
            hover_anim: None,
            hover_retiring: None,
            toggle_t: if on { 1.0 } else { 0.0 },
            toggle_target: if on { 1.0 } else { 0.0 },
            toggle_anim: None,
            toggle_retiring: None,
        }
    }

    /// SPEC-HAL-208: Set label (builder pattern).
    pub fn with_label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// SPEC-HAL-208: Set or replace label via mutable reference.
    pub fn set_label(&mut self, text: impl Into<String>) {
        self.label = Some(text.into());
    }

    /// SPEC-HAL-208A: Set label side (builder pattern). Default is Trailing.
    pub fn with_label_side(mut self, side: LabelSide) -> Self {
        self.label_side = side;
        self
    }

    /// SPEC-HAL-208A: Set label side via mutable reference.
    pub fn set_label_side(&mut self, side: LabelSide) {
        self.label_side = side;
    }

    /// Set disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.state.disabled = disabled;
    }

    /// Get current on/off state.
    pub fn is_on(&self) -> bool {
        self.state.on
    }

    /// Get current disabled state.
    pub fn is_disabled(&self) -> bool {
        self.state.disabled
    }

    /// Set the toggle changed callback (builder pattern).
    pub fn on_toggle_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle_changed = Some(Box::new(callback));
        self
    }

    /// Set callback via mutable reference.
    pub fn set_on_toggle_changed<F>(&mut self, callback: F)
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle_changed = Some(Box::new(callback));
    }

    /// SPEC-HAL-207 INV-F1: Configure binding for change store emission.
    pub fn set_binding(&mut self, key: &str, writer: ChangeWriter) {
        let binding_key = writer.intern(key);
        self.binding = Some((binding_key, writer));
    }

    /// Process a semantic event through the reducer.
    ///
    /// Returns `Some(new_on)` only when `Activate` toggles state.
    /// Invokes callback and emits binding when state changes.
    pub fn on_semantic_event(&mut self, event: SwitchEvent) -> Option<bool> {
        let result = reduce_switch(&mut self.state, event);

        if let Some(new_on) = result {
            if let Some(callback) = &mut self.on_toggle_changed {
                callback(new_on);
            }
            // SPEC-HAL-209 INV-F1: Emit via ChangeWriter::set_bool
            if let Some((key, writer)) = &self.binding {
                writer.set_bool(*key, new_on);
            }
        }

        result
    }

    /// Get immutable reference to state.
    pub fn state(&self) -> &SwitchState {
        &self.state
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
            // Retain cancellation ownership until Update has the capability.
            self.hover_retiring = Some(handle);
        }
        if self.hover_anim.is_none() && self.hover_t != target {
            self.hover_t = target;
            cx.invalidate_paint();
        }
    }

    fn animate_toggle(&mut self, cx: &mut EventContext, on: bool) {
        let target = if on { 1.0 } else { 0.0 };
        self.toggle_target = target;
        if cx.has_animation() {
            for handle in [self.toggle_retiring.take(), self.toggle_anim.take()]
                .into_iter()
                .flatten()
            {
                cx.cancel_animation(handle);
            }
            self.toggle_anim = cx.start_animation(Animation {
                from: self.toggle_t,
                to: target,
                duration: 0.15,
                easing: Easing::EaseOut,
            });
        } else if let Some(handle) = self.toggle_anim.take() {
            self.toggle_retiring = Some(handle);
        }
        if self.toggle_anim.is_none() && self.toggle_t != target {
            self.toggle_t = target;
        }
        // The semantic endpoint also selects the thumb color at the start sample.
        cx.invalidate_paint();
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for Switch {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Switch {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.switch;
        let track_width = geom.track_width;

        // SPEC-HAL-209 INV-C1: Layout accounts for track + label gap + label text
        let content_width = if let Some(label_text) = &self.label {
            let font_size = cx.theme.typography.font_size;
            let (text_width, _) = cx.text_engine.measure(label_text, font_size);
            track_width + geom.label_gap + text_width
        } else {
            track_width
        };
        self.cached_content_width = content_width;
        let width = content_width.min(constraints.max_width);

        // Height = control_height for consistent alignment with TextField/SpinBox/etc.
        let height = cx.theme.geometry.text_field.height;
        let height = height.clamp(constraints.min_height, constraints.max_height);
        self.layout_bounds = Size::new(width, height);
        LayoutResult::fit(self.layout_bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let theme = cx.theme;
        let geom = &theme.geometry.switch;

        let track_width = geom.track_width;
        let track_height = geom.track_height;

        // SPEC-HAL-208A INV-H1/H4: shape the owned label once; its width sets the
        // leading offset. Trailing (default) keeps the track at rect.x, so the
        // reported width stays side-symmetric and layout is unchanged. label_x is
        // meaningful only when a label exists.
        let label_font_size = theme.typography.font_size;
        let label_run = self
            .label
            .as_ref()
            .map(|t| cx.text_engine.shape(t, label_font_size));
        let (track_x, label_x) = match (&label_run, self.label_side) {
            (Some(run), LabelSide::Leading) => (rect.x + run.width + geom.label_gap, rect.x),
            (Some(_), LabelSide::Trailing) => (rect.x, rect.x + track_width + geom.label_gap),
            (None, _) => (rect.x, rect.x),
        };

        // Track centered vertically in rect
        let track_y = rect.y + (rect.height - track_height) * 0.5;

        let hover_t = if self.state.disabled {
            0.0
        } else {
            self.hover_t
        };

        // Track colors. Hover lifts the recessed track without changing the
        // binary state signal, which stays on the thumb.
        let track_bg = theme
            .colors
            .surfaces
            .content
            .lerp(theme.colors.surfaces.level2, hover_t);

        let scale = cx.scale_factor;
        let one_px = cx.one_px();
        let bx = snap_to_physical(track_x, scale);
        let by = snap_to_physical(track_y, scale);
        let br = snap_to_physical(track_x + track_width, scale);
        let bb = snap_to_physical(track_y + track_height, scale);
        let bw = (br - bx).max(0.0);
        let bh = (bb - by).max(0.0);
        let snapped_track = RectF::new(bx, by, bw, bh);

        if bw > 0.0 && bh > 0.0 {
            cx.scene
                .push_rect(snapped_track, track_bg, LAYER_CONTENT, cx.scene_key);

            let shadow = theme.colors.borders.shadow;
            let highlight = theme.colors.borders.highlight;
            cx.scene.push_rect(
                RectF::new(bx, by, bw, one_px),
                shadow,
                LAYER_CONTENT + 1,
                None,
            );
            cx.scene.push_rect(
                RectF::new(bx, by + one_px, one_px, bh - 2.0 * one_px),
                shadow,
                LAYER_CONTENT + 1,
                None,
            );
            cx.scene.push_rect(
                RectF::new(bx, bb - one_px, bw, one_px),
                highlight,
                LAYER_CONTENT + 1,
                None,
            );
            cx.scene.push_rect(
                RectF::new(br - one_px, by + one_px, one_px, bh - 2.0 * one_px),
                highlight,
                LAYER_CONTENT + 1,
                None,
            );
        }

        // The filled half-track slides without changing owner bounds.
        let t = self.toggle_t;
        let thumb_width = track_width / 2.0;
        let thumb_height = track_height;
        let thumb_travel = track_width - thumb_width;
        let thumb_x_logical = track_x + thumb_travel * t;
        let thumb_y_logical = track_y;
        let thumb_x_snapped = snap_to_physical(thumb_x_logical, scale);
        let thumb_y_snapped = snap_to_physical(thumb_y_logical, scale);
        let thumb_w_snapped =
            snap_to_physical(thumb_x_logical + thumb_width, scale) - thumb_x_snapped;
        let thumb_h_snapped =
            snap_to_physical(thumb_y_logical + thumb_height, scale) - thumb_y_snapped;
        let thumb_rect = RectF::new(
            thumb_x_snapped,
            thumb_y_snapped,
            thumb_w_snapped,
            thumb_h_snapped,
        );

        let thumb_color = if self.state.disabled {
            theme.colors.typography.muted
        } else if self.state.on {
            theme
                .colors
                .accents
                .primary
                .lerp(theme.colors.accents.hover, hover_t)
        } else {
            theme
                .colors
                .borders
                .strong
                .lerp(theme.colors.controls.border_hover, hover_t)
        };

        if thumb_w_snapped > 0.0 && thumb_h_snapped > 0.0 {
            cx.scene
                .push_rect(thumb_rect, thumb_color, LAYER_CONTENT + 2, cx.scene_key);
        }

        // SPEC-HAL-208/208A INV-C1/H4: paint label after track (paint order is
        // side-independent). Position (label_x) is computed above per side.
        if let Some(run) = &label_run {
            let font_size = label_font_size;

            // SPEC-HAL-092/208: Baseline-aligned label text
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
                run,
                label_x,
                label_y,
                text_color,
                LAYER_CONTENT + 3,
                cx.scene,
            );
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        if self.state.disabled {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                // SPEC-HAL-209 INV-C3: Full-area hit testing (track + label)
                let (local_x, local_y) = pe.local_pos;
                let inside = local_x >= 0.0
                    && local_x <= self.layout_bounds.width
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
                            self.on_semantic_event(SwitchEvent::FocusGain);
                            // Toggle immediately on press — no PointerUp delay.
                            let result = self.on_semantic_event(SwitchEvent::Activate);
                            if let Some(new_on) = result {
                                self.animate_toggle(cx, new_on);
                            }
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Up => false,
                    _ => false,
                }
            }
            // SPEC-HAL-209 INV-B3: Keyboard activation handled by on_keyboard_activate.
            Event::Key(ke) if ke.is_down => match ke.key_code {
                KeyCode::Escape => {
                    if self.state.active {
                        self.on_semantic_event(SwitchEvent::Cancel);
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn focusable(&self) -> bool {
        !self.state.disabled
    }

    fn touch_target(&self, _hit: crate::core::event::TouchHit) -> Option<crate::core::event::TouchTarget> {
        (!self.state.disabled).then_some(crate::core::event::TouchTarget::Control)
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, _tap: crate::core::event::TouchTap) -> bool {
        if let Some(value) = self.on_semantic_event(SwitchEvent::Activate) {
            self.animate_toggle(cx, value);
            cx.invalidate_paint();
        }
        true
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        for retiring in [&mut self.hover_retiring, &mut self.toggle_retiring] {
            if let Some(handle) = *retiring {
                match cx.sample_animation::<f32>(handle) {
                    Some(AnimationSample::TypeMismatch) => {
                        panic!("Switch presentation handle must contain f32");
                    }
                    Some(_) => {
                        cx.cancel_animation(handle);
                        *retiring = None;
                    }
                    None => {}
                }
            }
        }

        let previous = (self.hover_t, self.toggle_t);
        let endpoint = if self.state.on { 1.0 } else { 0.0 };
        let semantic_changed = self.toggle_target != endpoint;
        if semantic_changed {
            if let Some(handle) = self.toggle_anim.take() {
                match cx.sample_animation::<f32>(handle) {
                    Some(AnimationSample::TypeMismatch) => {
                        panic!("Switch toggle handle must contain f32");
                    }
                    Some(_) => cx.cancel_animation(handle),
                    None => self.toggle_retiring = Some(handle),
                }
            }
            self.toggle_target = endpoint;
            self.toggle_t = endpoint;
        }

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
                    panic!("Switch hover handle must contain f32");
                }
            };
        }
        self.toggle_t = if let Some(handle) = self.toggle_anim {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::Active(value)) => value,
                Some(AnimationSample::Completed(value)) => {
                    self.toggle_anim = None;
                    value
                }
                Some(AnimationSample::Cancelled | AnimationSample::Missing) => {
                    self.toggle_anim = None;
                    endpoint
                }
                None => {
                    self.toggle_anim = None;
                    self.toggle_retiring = Some(handle);
                    endpoint
                }
                Some(AnimationSample::TypeMismatch) => {
                    panic!("Switch toggle handle must contain f32");
                }
            }
        } else {
            endpoint
        };
        if semantic_changed || previous != (self.hover_t, self.toggle_t) {
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        let endpoint = if self.state.on { 1.0 } else { 0.0 };
        self.hover_anim.is_some()
            || self.hover_retiring.is_some()
            || self.toggle_anim.is_some()
            || self.toggle_retiring.is_some()
            || self.toggle_target != endpoint
            || self.toggle_t != endpoint
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.on_semantic_event(SwitchEvent::FocusGain);
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.on_semantic_event(SwitchEvent::FocusLoss);
    }

    fn accepts_keyboard_activation(&self) -> bool {
        !self.state.disabled
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        let result = self.on_semantic_event(SwitchEvent::Activate);
        if let Some(new_on) = result {
            self.animate_toggle(cx, new_on);
        }
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if self.state.disabled {
            return;
        }
        self.animate_hover(cx, 1.0, Easing::EaseOut);
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.animate_hover(cx, 0.0, Easing::EaseIn);
    }

    fn on_mount(&mut self) {
        self.hover_t = 0.0;
        self.hover_target = 0.0;
        self.hover_anim = None;
        self.hover_retiring = None;
        self.toggle_t = if self.state.on { 1.0 } else { 0.0 };
        self.toggle_target = self.toggle_t;
        self.toggle_anim = None;
        self.toggle_retiring = None;
    }

    fn on_unmount(&mut self) {
        self.on_mount();
    }

    fn debug_type_name(&self) -> &'static str {
        "Switch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::scene::SceneNode;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn switch_presentation_update_and_retained_settlement() {
        use crate::core::animation::{AnimationContext, Timeline};
        use crate::core::app::{AppContext, ModalStack, ScreenContext, ScreenStack};
        use crate::core::event::{InteractionDomains, WindowId};
        use crate::core::event_router::EventRouter;
        use crate::core::rect_authority::UpdateAuthorityGuard;
        use crate::model::form_binder::FormBinder;
        use crate::widgets::flex::Flex;
        use crate::widgets::label::Label;
        use crate::widgets::radio_button::tests::radio_paint;
        use std::sync::Arc;

        fn switch(tree: &WidgetTree, node: NodeId) -> &Switch {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn switch_mut(tree: &mut WidgetTree, node: NodeId) -> &mut Switch {
            tree.node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut()
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
            switch_mut(tree, node).update(0.0, &mut cx);
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
        let mut control = Switch::new()
            .with_label("Dark")
            .with_label_side(LabelSide::Leading)
            .on_toggle_changed(move |_| observed.set(observed.get() + 1));
        control.set_binding("dark", binder.writer());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let owner = tree.node(node).unwrap().id;
        let peer_id = tree.node(peer).unwrap().id;
        let root_id = tree.node(root).unwrap().id;
        let geometry = |tree: &WidgetTree| {
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
            let value = switch(tree, node);
            (
                value.hover_t,
                value.hover_target,
                value.hover_anim,
                value.hover_retiring,
                value.toggle_t,
                value.toggle_target,
                value.toggle_anim,
                value.toggle_retiring,
            )
        };
        let mut app = AppContext::new(theme.clone());
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
        let take_binding = |binder: &mut FormBinder| {
            let mut value = None;
            binder.bind(&screen, |binding| value = binding.bool("dark").take());
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
        let primitive_geometry = |nodes: &[SceneNode]| {
            // JC's 25 Oct restoration superseded the circle-only observations.
            let track = nodes
                .iter()
                .find_map(|node| match node {
                    SceneNode::Rect(track) if track.z_index == LAYER_CONTENT => Some(track.rect),
                    _ => None,
                })
                .unwrap();
            let thumb = nodes
                .iter()
                .find_map(|node| match node {
                    SceneNode::Rect(thumb) if thumb.z_index == LAYER_CONTENT + 2 => {
                        Some(thumb.rect)
                    }
                    _ => None,
                })
                .unwrap();
            let label = nodes
                .iter()
                .find_map(|node| match node {
                    SceneNode::Text(label) => {
                        Some((label.text.clone(), label.position, label.font_size))
                    }
                    _ => None,
                })
                .unwrap();
            (track, thumb, label)
        };

        harness.advance_transition_time(0.0);
        assert!(frame(&mut tree, &mut harness, &mut text, &domains).scene_built);
        let bounds = geometry(&tree);
        let initial = primitive_geometry(&harness.local_fragment(owner).unwrap());
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let hit = tree
            .hit_test(bounds[1].2.x + 1.0, bounds[1].2.y + 1.0)
            .unwrap();
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        router.update_hover(
            &mut tree,
            Some(owner),
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        let (first_hover, first_toggle) = (
            switch(&tree, node).hover_anim.unwrap(),
            switch(&tree, node).toggle_anim.unwrap(),
        );
        for handle in [first_hover, first_toggle] {
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Active(0.0)
            );
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(take_binding(&mut binder), Some(true));
        frame(&mut tree, &mut harness, &mut text, &domains);
        assert_eq!(
            (switch(&tree, node).hover_t, switch(&tree, node).toggle_t),
            (0.0, 0.0)
        );
        assert!(switch(&tree, node).needs_update());
        assert!(!switch(&tree, node).needs_animation());

        harness.advance_transition_time(0.04);
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        let intermediate = presentation(&tree);
        assert!(intermediate.0 > 0.0 && intermediate.0 < 1.0);
        assert!(intermediate.4 > 0.0 && intermediate.4 < 1.0);
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        let semantic = switch(&tree, node).state.clone();
        for _ in 0..2 {
            let nodes = radio_paint(&Timeline::new(), &theme, |cx| {
                switch(&tree, node).paint(cx, bounds[1].2, &[]);
            });
            let actual = primitive_geometry(&nodes);
            assert_eq!((actual.0, actual.2), (initial.0, initial.2.clone()));
            assert!(actual.1.x > initial.1.x);
            assert_eq!(presentation(&tree), intermediate);
            assert_eq!(switch(&tree, node).state, semantic);
        }
        assert_eq!(take_binding(&mut binder), None);
        assert_eq!(calls.get(), 1);
        frame(&mut tree, &mut harness, &mut text, &domains);

        // Retarget both live effects from the last Update publication.
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        for handle in [first_hover, first_toggle] {
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Cancelled
            );
        }
        for (handle, value) in [
            (switch(&tree, node).hover_anim.unwrap(), intermediate.0),
            (switch(&tree, node).toggle_anim.unwrap(), intermediate.4),
        ] {
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Active(value)
            );
        }
        assert_eq!(take_binding(&mut binder), Some(false));
        let completing = [
            switch(&tree, node).hover_anim.unwrap(),
            switch(&tree, node).toggle_anim.unwrap(),
        ];
        harness.advance_transition_time(0.20);
        for handle in completing {
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Completed(0.0)
            );
        }
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            (switch(&tree, node).hover_t, switch(&tree, node).toggle_t),
            (0.0, 0.0)
        );
        assert!(
            switch(&tree, node).hover_anim.is_none() && switch(&tree, node).toggle_anim.is_none()
        );
        assert!(!switch(&tree, node).needs_update());
        let completed = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(completed.scene_built && !completed.needs_next_frame);
        let settled_owner = harness.local_fragment(owner).unwrap();
        assert_eq!(primitive_geometry(&settled_owner).1, initial.1);
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &settled_owner,
            &harness.local_fragment(owner).unwrap()
        ));

        // Cancellation retains hover's publication, but the thumb reaches its boolean endpoint.
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        router.update_hover(
            &mut tree,
            Some(owner),
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        assert_eq!(take_binding(&mut binder), Some(true));
        harness.advance_transition_time(0.24);
        frame(&mut tree, &mut harness, &mut text, &domains);
        let hover_sample = switch(&tree, node).hover_t;
        for handle in [
            switch(&tree, node).hover_anim.unwrap(),
            switch(&tree, node).toggle_anim.unwrap(),
        ] {
            harness.animation_context().cancel(handle);
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Cancelled
            );
        }
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            (switch(&tree, node).hover_t, switch(&tree, node).toggle_t),
            (hover_sample, 1.0)
        );
        assert!(!switch(&tree, node).needs_update());
        assert_eq!(take_binding(&mut binder), None);
        assert_eq!(calls.get(), 3);
        frame(&mut tree, &mut harness, &mut text, &domains);

        // Skip the completion observation window: Missing is a target fallback, not Completed.
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        let missed = [
            switch(&tree, node).hover_anim.unwrap(),
            switch(&tree, node).toggle_anim.unwrap(),
        ];
        assert_eq!(take_binding(&mut binder), Some(false));
        harness.advance_transition_time(0.50);
        harness.advance_transition_time(0.51);
        for handle in missed {
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Missing
            );
        }
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            (switch(&tree, node).hover_t, switch(&tree, node).toggle_t),
            (0.0, 0.0)
        );
        assert!(!switch(&tree, node).needs_update());
        assert_eq!(take_binding(&mut binder), None);
        frame(&mut tree, &mut harness, &mut text, &domains);

        // Capability-free starts settle statically and emit each semantic change once.
        assert!(router.activate_focused_widget(&mut tree, domains.primary_state_mut(), None));
        router.update_hover(&mut tree, Some(owner), None, domains.primary_state_mut());
        assert_eq!(
            (switch(&tree, node).hover_t, switch(&tree, node).toggle_t),
            (1.0, 1.0)
        );
        assert!(!switch(&tree, node).needs_update());
        assert_eq!(take_binding(&mut binder), Some(true));
        frame(&mut tree, &mut harness, &mut text, &domains);

        // Capability-free replacement retains the admitted IDs until a real Update cancels them.
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        assert_eq!(take_binding(&mut binder), Some(false));
        let retiring = [
            switch(&tree, node).hover_anim.unwrap(),
            switch(&tree, node).toggle_anim.unwrap(),
        ];
        assert!(router.activate_focused_widget(&mut tree, domains.primary_state_mut(), None));
        router.update_hover(&mut tree, Some(owner), None, domains.primary_state_mut());
        assert_eq!(take_binding(&mut binder), Some(true));
        assert_eq!(
            (
                switch(&tree, node).hover_retiring,
                switch(&tree, node).toggle_retiring
            ),
            (Some(retiring[0]), Some(retiring[1]))
        );
        assert!(!publish(&mut tree, node, None));
        for handle in retiring {
            assert!(matches!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Active(_)
            ));
        }
        assert!(switch(&tree, node).needs_update());
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        for handle in retiring {
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Cancelled
            );
        }
        assert!(!switch(&tree, node).needs_update());
        frame(&mut tree, &mut harness, &mut text, &domains);

        // The normal harness always supplies capability; explicitly exercise Update None here.
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        router.update_hover(
            &mut tree,
            None,
            Some(harness.animation_context()),
            domains.primary_state_mut(),
        );
        assert_eq!(take_binding(&mut binder), Some(false));
        let unavailable = [
            switch(&tree, node).hover_anim.unwrap(),
            switch(&tree, node).toggle_anim.unwrap(),
        ];
        assert!(publish(&mut tree, node, None));
        assert_eq!(
            (switch(&tree, node).hover_t, switch(&tree, node).toggle_t),
            (0.0, 0.0)
        );
        assert!(
            switch(&tree, node).hover_anim.is_none() && switch(&tree, node).toggle_anim.is_none()
        );
        for handle in unavailable {
            assert!(matches!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Active(_)
            ));
        }
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        for handle in unavailable {
            assert_eq!(
                harness.animation_context().sample::<f32>(handle),
                AnimationSample::Cancelled
            );
        }
        frame(&mut tree, &mut harness, &mut text, &domains);

        // A direct semantic change supersedes an older opposite transition before it can publish.
        assert!(router.activate_focused_widget(
            &mut tree,
            domains.primary_state_mut(),
            Some(harness.animation_context())
        ));
        let obsolete = switch(&tree, node).toggle_anim.unwrap();
        assert_eq!(take_binding(&mut binder), Some(true));
        assert_eq!(
            switch_mut(&mut tree, node).on_semantic_event(SwitchEvent::Activate),
            Some(false)
        );
        assert_eq!(take_binding(&mut binder), Some(false));
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(obsolete),
            AnimationSample::Cancelled
        );
        assert_eq!(switch(&tree, node).toggle_t, 0.0);
        assert!(!switch(&tree, node).needs_update());
        frame(&mut tree, &mut harness, &mut text, &domains);
        let final_owner = harness.local_fragment(owner).unwrap();
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
        assert_eq!(geometry(&tree), bounds);
        let final_geometry = primitive_geometry(&final_owner);
        assert_eq!(final_geometry, initial);
        let final_hit = tree
            .hit_test(bounds[1].2.x + 1.0, bounds[1].2.y + 1.0)
            .unwrap();
        assert_eq!(
            (
                final_hit.widget_id,
                final_hit.local_pos,
                final_hit.absolute_bounds
            ),
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        );
        assert_eq!(calls.get(), 10);
        assert_eq!(take_binding(&mut binder), None);
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));

        // Local lifecycle reset is distinct from native App's owner cancellation on teardown.
        let value = switch_mut(&mut tree, node);
        assert_eq!(value.on_semantic_event(SwitchEvent::Activate), Some(true));
        let semantic = value.state.clone();
        value.on_unmount();
        value.on_mount();
        assert_eq!(value.state, semantic);
        assert_eq!(value.label.as_deref(), Some("Dark"));
        assert_eq!((value.hover_t, value.toggle_t), (0.0, 1.0));
        assert!(!value.needs_update() && !value.needs_animation());
        assert_eq!(calls.get(), 11);
        assert_eq!(take_binding(&mut binder), Some(true));
    }

    #[test]
    fn switch_thumb_geometry_follows_theme_and_preserves_label() {
        use crate::core::text_engine::TextEngine;

        // JC's 25 Oct filled-half restoration superseded circle diameter/inset geometry.
        let mut custom = Theme::default();
        custom.geometry.switch.track_width = 46.0;
        custom.geometry.switch.track_height = 22.0;
        custom.geometry.switch.label_gap = 10.0;

        for (theme, side) in [
            (Theme::default(), LabelSide::Trailing),
            (custom, LabelSide::Leading),
        ] {
            let callbacks = Rc::new(Cell::new(0));
            let callback_count = callbacks.clone();
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(
                    Switch::new()
                        .with_label("Dark")
                        .with_label_side(side)
                        .on_toggle_changed(move |_| callback_count.set(callback_count.get() + 1)),
                ),
                None,
                0,
            );
            let widget_id = tree.node(root).unwrap().id;
            let constraints = Constraints::new(0.0, 300.0, 0.0, 100.0);
            let mut text = FontManagerTextEngine::new();
            let icons = IconRegistry::new(&[]);
            let mut harness = RetainedSceneTestHarness::new();
            harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
            let owner_rect = tree.get_rect(root).unwrap();
            let mut off_geometry = None;
            let mut off_fill: Option<RectF> = None;

            let geom = &theme.geometry.switch;
            let font_size = theme.typography.font_size;
            let label_run = text.shape("Dark", font_size);
            let metrics = text.font_metrics(font_size);
            let (track_x, label_x) = match side {
                LabelSide::Leading => (
                    owner_rect.x + label_run.width + geom.label_gap,
                    owner_rect.x,
                ),
                LabelSide::Trailing => (
                    owner_rect.x,
                    owner_rect.x + geom.track_width + geom.label_gap,
                ),
            };
            let track_y = owner_rect.y + (owner_rect.height - geom.track_height) * 0.5;
            // This existing retained fixture has no custom target and uses scale 1.0.
            let scale = 1.0;
            let one_px = 1.0 / scale;
            let left = snap_to_physical(track_x, scale);
            let top = snap_to_physical(track_y, scale);
            let right = snap_to_physical(track_x + geom.track_width, scale);
            let bottom = snap_to_physical(track_y + geom.track_height, scale);
            let expected_track = RectF::new(left, top, right - left, bottom - top);
            let expected_edges = [
                RectF::new(left, top, right - left, one_px),
                RectF::new(left, top + one_px, one_px, bottom - top - 2.0 * one_px),
                RectF::new(left, bottom - one_px, right - left, one_px),
                RectF::new(
                    right - one_px,
                    top + one_px,
                    one_px,
                    bottom - top - 2.0 * one_px,
                ),
            ];
            let expected_label_y = owner_rect.y
                + compute_control_text_baseline_y(
                    owner_rect.height,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                )
                - metrics.ascent;

            for on in [false, true] {
                if on {
                    let switch = tree
                        .node_mut(root)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<Switch>()
                        .unwrap();
                    assert_eq!(switch.on_semantic_event(SwitchEvent::Activate), Some(true));
                }
                let switch = tree
                    .node(root)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<Switch>()
                    .unwrap();
                let before = (
                    switch.id,
                    switch.state.clone(),
                    switch.layout_bounds,
                    switch.label.clone(),
                );
                let calls_before = callbacks.get();
                tree.mark_paint_dirty(root);
                let frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
                assert!(frame.scene_built);
                let switch = tree
                    .node(root)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<Switch>()
                    .unwrap();
                assert_eq!(
                    (
                        switch.id,
                        switch.state.clone(),
                        switch.layout_bounds,
                        switch.label.clone()
                    ),
                    before
                );
                assert_eq!(switch.is_on(), on);
                assert_eq!(callbacks.get(), calls_before);
                assert_eq!(tree.node(root).unwrap().id, widget_id);
                assert_eq!(tree.get_rect(root), Some(owner_rect));
                assert_eq!(
                    switch.layout_bounds,
                    Size::new(owner_rect.width, owner_rect.height)
                );
                assert!(tree.children(root).is_empty());

                let nodes = harness.local_fragment(widget_id).unwrap();
                let (track_index, track) = nodes
                    .iter()
                    .enumerate()
                    .find_map(|(i, node)| match node {
                        SceneNode::Rect(track) if track.z_index == LAYER_CONTENT => {
                            Some((i, track))
                        }
                        _ => None,
                    })
                    .unwrap();
                let edges: Vec<_> = nodes
                    .iter()
                    .enumerate()
                    .filter_map(|(i, node)| match node {
                        SceneNode::Rect(edge) if edge.z_index == LAYER_CONTENT + 1 => {
                            Some((i, edge))
                        }
                        _ => None,
                    })
                    .collect();
                let (thumb_index, thumb) = nodes
                    .iter()
                    .enumerate()
                    .find_map(|(i, node)| match node {
                        SceneNode::Rect(thumb) if thumb.z_index == LAYER_CONTENT + 2 => {
                            Some((i, thumb))
                        }
                        _ => None,
                    })
                    .unwrap();
                let (label_index, label) = nodes
                    .iter()
                    .enumerate()
                    .find_map(|(i, node)| match node {
                        SceneNode::Text(label) if label.text == "Dark" => Some((i, label)),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(track.rect, expected_track);
                assert_eq!(
                    track.rect.y + track.rect.height * 0.5,
                    top + (bottom - top) * 0.5
                );
                let half_width = geom.track_width * 0.5;
                let fill_x = track_x + if on { half_width } else { 0.0 };
                let expected_fill = RectF::new(
                    snap_to_physical(fill_x, scale),
                    top,
                    snap_to_physical(fill_x + half_width, scale) - snap_to_physical(fill_x, scale),
                    bottom - top,
                );
                assert_eq!(thumb.rect, expected_fill);
                assert_eq!(thumb.rect.y, track.rect.y);
                assert_eq!(thumb.rect.height, track.rect.height);
                if on {
                    assert_eq!(thumb.rect.x + thumb.rect.width, right);
                } else {
                    assert_eq!(thumb.rect.x, left);
                }
                assert_eq!(edges.len(), expected_edges.len());
                for ((edge_index, edge), expected) in edges.iter().zip(expected_edges) {
                    assert_eq!(edge.rect, expected);
                    assert!(track_index < *edge_index && *edge_index < thumb_index);
                    assert!(track.z_index < edge.z_index && edge.z_index < thumb.z_index);
                }
                assert!(thumb_index < label_index && thumb.z_index < label.z_index);
                assert_eq!(label.z_index, LAYER_CONTENT + 3);
                assert_eq!(label.position.x, label_x);
                assert_eq!(label.position.y, expected_label_y);
                assert_eq!(label.font_size, font_size);
                if side == LabelSide::Leading {
                    assert_eq!(label.position.x, owner_rect.x);
                    assert!(label.position.x < track.rect.x);
                } else {
                    assert_eq!(
                        label.position.x,
                        owner_rect.x + geom.track_width + geom.label_gap
                    );
                }
                let geometry = (
                    track.rect,
                    label.position,
                    label.font_size,
                    label.text.clone(),
                );
                if on {
                    assert_eq!(Some(geometry), off_geometry);
                    let off = off_fill.unwrap();
                    assert!(thumb.rect.x > off.x);
                    assert_eq!(
                        thumb.rect.x - off.x,
                        snap_to_physical(track_x + half_width, scale)
                            - snap_to_physical(track_x, scale)
                    );
                } else {
                    off_geometry = Some(geometry);
                    off_fill = Some(thumb.rect);
                }
            }
        }
    }
}
