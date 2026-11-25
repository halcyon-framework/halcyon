// SPEC-HAL-131 Stage 1: RadioButton Projection Widget
// SPEC-HAL-208: Integrated label support — label is owned, not a child widget.
// Pure stateless view over RadioGroupState. No behavior, no input, no reducer logic.

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{LAYER_CONTENT, RoundedCorners};
use crate::core::style::{BorderStyle, DecorationStack, DecorationState};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::radio_group::{RadioGroupState, RadioId};

// ============================================================================
// Projected State (SPEC-HAL-131 Stage 1)
// ============================================================================

/// Projected state for a single RadioButton member.
///
/// This is a pure derivation from RadioGroupState — no persistent storage.
#[derive(Clone, Debug, PartialEq)]
pub struct RadioButtonState {
    pub checked: bool,
    pub focused: bool,
    pub active: bool,
    pub disabled: bool,
}

// ============================================================================
// RadioButton Widget (SPEC-HAL-131 Stage 1 + SPEC-HAL-208)
// ============================================================================

/// RadioButton projection widget.
///
/// Stores the member ID and optional label. All semantic state is projected
/// from RadioGroupState. This widget is a pure, stateless view — it never
/// owns checked/focused/active/disabled.
///
/// SPEC-HAL-208: Label is owned directly by this widget (INV-A1).
pub struct RadioButton {
    internal_id: WidgetInternalId,
    id: RadioId,
    /// SPEC-HAL-208 INV-A1: Owned label text. None = indicator-only.
    label: Option<String>,
    hover_t: f32,
    hover_target: f32,
    hover_anim: Option<TransitionHandle>,
    hover_retiring: Option<TransitionHandle>,
}

impl RadioButton {
    /// Create a new RadioButton with the given member ID.
    pub fn new(id: RadioId) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            id,
            label: None,
            hover_t: 0.0,
            hover_target: 0.0,
            hover_anim: None,
            hover_retiring: None,
        }
    }

    /// SPEC-HAL-208: Create a RadioButton with an integrated label.
    pub fn with_label(id: RadioId, label: impl Into<String>) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            id,
            label: Some(label.into()),
            hover_t: 0.0,
            hover_target: 0.0,
            hover_anim: None,
            hover_retiring: None,
        }
    }

    /// Get the RadioId of this button.
    pub fn radio_id(&self) -> RadioId {
        self.id
    }

    /// Get the integrated label text, if any.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Project state from the group.
    ///
    /// This is the sole source of semantic state for this widget.
    /// Projection rules (exact):
    /// - checked  = group.selected == Some(id)
    /// - focused  = group.focused  == Some(id)
    /// - active   = group.active   == Some(id)
    /// - disabled = group.disabled
    pub fn project(&self, group: &RadioGroupState) -> RadioButtonState {
        project_radio_button(self.id, group)
    }

    /// Paint using projected state from group.
    ///
    /// This is the primary paint entry point when group state is available.
    pub fn paint_with_group(&self, cx: &mut PaintContext, rect: RectF, group: &RadioGroupState) {
        let state = self.project(group);
        self.paint_projected(cx, rect, &state);
    }

    /// Paint using pre-projected state.
    ///
    /// Internal helper — uses only the projected state, never mutates anything.
    fn paint_projected(&self, cx: &mut PaintContext, rect: RectF, state: &RadioButtonState) {
        paint_radio_button(cx, rect, self.label(), state, self.hover_t, 0.0);
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
}

pub(crate) fn project_radio_button(id: RadioId, group: &RadioGroupState) -> RadioButtonState {
    RadioButtonState {
        checked: group.selected == Some(id),
        focused: group.focused == Some(id),
        active: group.active == Some(id),
        disabled: group.disabled,
    }
}

pub(crate) fn paint_radio_button(
    cx: &mut PaintContext,
    rect: RectF,
    label: Option<&str>,
    state: &RadioButtonState,
    hover_t: f32,
    _focus_t: f32,
) {
    let theme = cx.theme;
    let geom = &theme.geometry.checkbox; // Reuse checkbox geometry for radio

    // Radio circle is square-bounded, centered vertically in rect
    let circle_size = geom.size;
    let circle_y = rect.y + (rect.height - circle_size) / 2.0;
    let circle_rect = RectF::new(rect.x, circle_y, circle_size, circle_size);

    let focused = !state.disabled && state.focused && cx.has_focus_cue();

    // Pressed tint remains independent of the framework's focus-cue eligibility.
    let (bg_color, border_color) = if state.disabled {
        (theme.colors.surfaces.content, theme.colors.borders.default)
    } else if state.active || focused {
        (theme.colors.surfaces.content, theme.colors.borders.focus)
    } else {
        let hover_border = theme
            .colors
            .borders
            .default
            .lerp(theme.colors.borders.focus, hover_t);
        (theme.colors.surfaces.content, hover_border)
    };

    // Circular radius = half the size
    let outer_radius = circle_size * geom.checkmark_leg_ratio;
    let border_thickness = geom.border_thickness;

    // The existing circle border carries the qualified indicator tint.
    let stack = DecorationStack::background(bg_color, outer_radius).with_border(BorderStyle::new(
        border_thickness,
        border_color,
        outer_radius,
    ));

    let deco_state = DecorationState {
        focused: false,
        focus_t: 0.0,
        hover_t: 0.0,
        draw_hover: false,
    };

    // BorderStyle emits the outer border and inset fill at base_z=2.
    stack.paint(cx.scene, circle_rect, deco_state, 2);

    // Draw selection dot if checked (widget-specific glyph)
    if state.checked {
        let dot_color = if state.disabled {
            theme.colors.typography.muted
        } else {
            theme.colors.accents.primary
        };

        let inset = geom.checkmark_inset;
        let dot_size = circle_size - inset - inset;
        let dot_rect = RectF::new(
            circle_rect.x + inset,
            circle_rect.y + inset,
            dot_size,
            dot_size,
        );
        let dot_radius = dot_size * geom.checkmark_leg_ratio;
        cx.scene.push_rounded_rect(
            dot_rect,
            dot_color,
            RoundedCorners::uniform(dot_radius),
            4,
            None,
        );
    }

    // SPEC-HAL-208 INV-C1: Paint label text after indicator
    if let Some(label_text) = label {
        let font_size = theme.typography.font_size;
        let run = cx.text_engine.shape(label_text, font_size);

        let label_x = rect.x + circle_size + geom.label_gap;

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

        let text_color = if state.disabled {
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

impl Default for RadioButton {
    fn default() -> Self {
        Self::new(RadioId(0))
    }
}

impl WidgetDowncast for RadioButton {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for RadioButton {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Height = control_height for consistent alignment with TextField/SpinBox/etc.
        let width = constraints.max_width;
        let height = cx.theme.geometry.text_field.height;
        let height = height.clamp(constraints.min_height, constraints.max_height);
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Default paint without group state — paint as unchecked, unfocused, inactive, enabled
        let default_state = RadioButtonState {
            checked: false,
            focused: false,
            active: false,
            disabled: false,
        };
        self.paint_projected(cx, rect, &default_state);
    }

    fn id(&self) -> WidgetInternalId {
        self.internal_id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        self.animate_hover(cx, 1.0, Easing::EaseOut);
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.animate_hover(cx, 0.0, Easing::EaseIn);
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        if let Some(handle) = self.hover_retiring {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::TypeMismatch) => {
                    panic!("RadioButton presentation handle must contain f32");
                }
                Some(_) => {
                    cx.cancel_animation(handle);
                    self.hover_retiring = None;
                }
                None => {}
            }
        }
        let Some(handle) = self.hover_anim else {
            return;
        };
        let value = match cx.sample_animation::<f32>(handle) {
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
                panic!("RadioButton presentation handle must contain f32");
            }
        };
        if self.hover_t != value {
            self.hover_t = value;
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.hover_anim.is_some() || self.hover_retiring.is_some()
    }

    fn on_mount(&mut self) {
        self.hover_t = 0.0;
        self.hover_target = 0.0;
        self.hover_anim = None;
        self.hover_retiring = None;
    }

    fn on_unmount(&mut self) {
        self.hover_t = 0.0;
        self.hover_target = 0.0;
        self.hover_anim = None;
        self.hover_retiring = None;
    }

    fn debug_type_name(&self) -> &'static str {
        "RadioButton"
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::animation::{AnimationContext, Timeline};
    use crate::core::event::KeyboardAuthorityMode;
    use crate::core::icon::IconRegistry;
    use crate::core::rect_authority::{PaintAuthorityGuard, UpdateAuthorityGuard};
    use crate::core::scene::{SceneBuilder, SceneNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::utils::id::TreeWidgetId;

    pub(crate) fn radio_event(timeline: Option<&mut Timeline>) -> EventContext<'_> {
        EventContext::new(
            TreeWidgetId::from_raw(7),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            None,
            timeline.map(AnimationContext::new),
            false,
            false,
            KeyboardAuthorityMode::Navigation,
        )
    }

    pub(crate) fn radio_paint(
        timeline: &Timeline,
        theme: &Theme,
        paint: impl FnOnce(&mut PaintContext),
    ) -> Vec<SceneNode> {
        let tree = WidgetTree::new();
        let mut scene = SceneBuilder::new();
        let mut text = FontManagerTextEngine::new();
        let icons = IconRegistry::new(&[]);
        let mut hits = Vec::new();
        let mut tooltips = Vec::new();
        let rects = Default::default();
        {
            let mut cx = PaintContext::new(
                &tree,
                &mut scene,
                theme,
                &mut text,
                &icons,
                0,
                timeline,
                &rects,
                &mut hits,
                &mut tooltips,
                1.0,
                None,
                KeyboardAuthorityMode::Navigation,
            );
            let _paint = PaintAuthorityGuard::enter();
            paint(&mut cx);
        }
        assert!(hits.is_empty() && tooltips.is_empty());
        scene.build().root
    }

    fn update(button: &mut RadioButton, timeline: Option<&mut Timeline>) -> (bool, bool, bool) {
        let _update = UpdateAuthorityGuard::enter();
        let mut cx = UpdateContext::new(timeline.map(AnimationContext::new));
        button.update(0.0, &mut cx);
        cx.take_widget_invalidations()
    }

    #[test]
    fn radio_hover_update_publishes_samples_and_paint_is_read_only() {
        let mut button = RadioButton::with_label(RadioId(1), "Choice");
        let mut timeline = Timeline::new();
        timeline.update(0.0).unwrap();
        button.on_hover_enter(&mut radio_event(Some(&mut timeline)));
        let handle = button.hover_anim.unwrap();
        assert!(button.needs_update());
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Active(0.0));
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (false, false, false)
        );
        assert_eq!(button.hover_t, 0.0);

        timeline.update(0.04).unwrap();
        let AnimationSample::Active(value) = timeline.sample::<f32>(handle) else {
            panic!()
        };
        assert!(value > 0.0 && value < 1.0);
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (true, false, false)
        );
        assert_eq!(button.hover_t, value);
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (false, false, false)
        );

        timeline.update(0.06).unwrap();
        let theme = Theme::default();
        let identity = button.id();
        for _ in 0..2 {
            radio_paint(&timeline, &theme, |cx| {
                button.paint(cx, RectF::new(0.0, 0.0, 100.0, 24.0), &[]);
                button.paint_with_group(
                    cx,
                    RectF::new(0.0, 30.0, 100.0, 24.0),
                    &RadioGroupState::default(),
                );
            });
            assert_eq!(button.hover_t, value);
            assert_eq!(button.hover_anim, Some(handle));
            assert_eq!(button.id(), identity);
        }

        button.on_hover_leave(&mut radio_event(Some(&mut timeline)));
        let replacement = button.hover_anim.unwrap();
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Cancelled);
        assert_ne!(replacement, handle);
        assert_eq!(
            timeline.sample::<f32>(replacement),
            AnimationSample::Active(value)
        );
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (false, false, false)
        );
        timeline.update(0.15).unwrap();
        assert_eq!(
            timeline.sample::<f32>(replacement),
            AnimationSample::Completed(0.0)
        );
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (true, false, false)
        );
        assert_eq!(button.hover_t, 0.0);
        assert!(!button.needs_update());
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (false, false, false)
        );
    }

    #[test]
    fn radio_hover_cancel_missing_and_unavailable_capability_settle_explicitly() {
        let mut button = RadioButton::new(RadioId(1));
        let mut timeline = Timeline::new();
        timeline.update(0.0).unwrap();
        button.on_hover_enter(&mut radio_event(Some(&mut timeline)));
        timeline.update(0.04).unwrap();
        update(&mut button, Some(&mut timeline));
        let value = button.hover_t;
        timeline.cancel(button.hover_anim.unwrap());
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (false, false, false)
        );
        assert_eq!(button.hover_t, value);
        assert!(!button.needs_update());

        button.on_hover_enter(&mut radio_event(Some(&mut timeline)));
        let missed = button.hover_anim.unwrap();
        timeline.update(0.13).unwrap();
        timeline.update(0.14).unwrap();
        assert_eq!(timeline.sample::<f32>(missed), AnimationSample::Missing);
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (true, false, false)
        );
        assert_eq!(button.hover_t, 1.0);
        assert!(!button.needs_update());

        button.on_hover_leave(&mut radio_event(Some(&mut timeline)));
        let unavailable = button.hover_anim.unwrap();
        assert_eq!(
            timeline.sample::<f32>(unavailable),
            AnimationSample::Active(1.0)
        );
        assert_eq!(update(&mut button, None), (true, false, false));
        assert_eq!(button.hover_t, 0.0);
        assert_eq!(
            timeline.sample::<f32>(unavailable),
            AnimationSample::Active(1.0)
        );
        assert!(
            button.needs_update(),
            "unavailable Update must retain custody while the admitted Timeline ID is still Active"
        );
        assert_eq!(update(&mut button, None), (false, false, false));
        assert!(button.needs_update());
        assert_eq!(button.hover_t, 0.0);
        assert_eq!(
            timeline.sample::<f32>(unavailable),
            AnimationSample::Active(1.0)
        );
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (false, false, false)
        );
        assert_eq!(
            timeline.sample::<f32>(unavailable),
            AnimationSample::Cancelled
        );
        assert!(!button.needs_update());

        button.on_hover_enter(&mut radio_event(None));
        assert_eq!(button.hover_t, 1.0);
        assert!(!button.needs_update());

        button.on_hover_leave(&mut radio_event(Some(&mut timeline)));
        let event_retiring = button.hover_anim.unwrap();
        button.on_hover_enter(&mut radio_event(None));
        assert_eq!(button.hover_t, 1.0);
        assert!(button.hover_anim.is_none());
        assert!(button.needs_update());
        button.on_hover_leave(&mut radio_event(None));
        assert_eq!(button.hover_t, 0.0);
        button.on_hover_enter(&mut radio_event(None));
        assert_eq!(button.hover_t, 1.0);
        assert_eq!(update(&mut button, None), (false, false, false));
        assert_eq!(
            timeline.sample::<f32>(event_retiring),
            AnimationSample::Active(1.0)
        );
        assert!(button.needs_update());
        button.on_hover_leave(&mut radio_event(Some(&mut timeline)));
        let replacement = button.hover_anim.unwrap();
        assert_ne!(replacement, event_retiring);
        assert_eq!(
            timeline.sample::<f32>(event_retiring),
            AnimationSample::Cancelled
        );
        assert_eq!(
            timeline.sample::<f32>(replacement),
            AnimationSample::Active(1.0)
        );
        assert_eq!(update(&mut button, None), (true, false, false));
        assert_eq!(button.hover_t, 0.0);
        assert!(button.needs_update());
        assert_eq!(
            timeline.sample::<f32>(replacement),
            AnimationSample::Active(1.0)
        );
        assert_eq!(
            update(&mut button, Some(&mut timeline)),
            (false, false, false)
        );
        assert_eq!(
            timeline.sample::<f32>(replacement),
            AnimationSample::Cancelled
        );
        assert!(!button.needs_update());
        button.on_unmount();
        assert_eq!(
            (button.hover_t, button.hover_target, button.hover_anim),
            (0.0, 0.0, None)
        );
        button.on_mount();
        assert!(!button.needs_update());
    }

    #[test]
    fn radio_projection_preserves_indicator_label_dot_and_focus_primitives() {
        let theme = Theme::default();
        let timeline = Timeline::new();
        let rect = RectF::new(10.0, 20.0, 160.0, 24.0);
        let id = RadioId(9);
        let states = [
            (None, false, false, false),
            (Some("Choice"), true, false, false),
            (Some("Choice"), false, true, false),
            (Some(""), true, true, false),
            (Some("Disabled"), true, false, true),
        ];
        for (label, checked, focused, disabled) in states {
            let button = match label {
                Some(label) => RadioButton::with_label(id, label),
                None => RadioButton::new(id),
            };
            let group = RadioGroupState {
                selected: checked.then_some(id),
                disabled,
                focused: focused.then_some(id),
                active: None,
            };
            assert_eq!(button.project(&group), project_radio_button(id, &group));
            let snapshot = group.clone();
            let nodes = radio_paint(&timeline, &theme, |cx| {
                button.paint_with_group(cx, rect, &group)
            });
            assert_eq!(group, snapshot);
            assert_eq!(button.label(), label);
            let size = theme.geometry.checkbox.size;
            let circle = RectF::new(rect.x, rect.y + (rect.height - size) / 2.0, size, size);
            let SceneNode::RoundedRect(border) = &nodes[0] else {
                panic!()
            };
            assert_eq!(border.rect, circle);
            assert_eq!(border.z_index, 2);
            assert_eq!(border.color, theme.colors.borders.default.into());
            assert_eq!(
                border.radius,
                size * theme.geometry.checkbox.checkmark_leg_ratio
            );
            let SceneNode::RoundedRect(fill) = &nodes[1] else {
                panic!()
            };
            assert_eq!(fill.z_index, 2);
            let thickness = theme.geometry.checkbox.border_thickness;
            assert_eq!(
                fill.rect,
                RectF::new(
                    circle.x + thickness,
                    circle.y + thickness,
                    (size - 2.0 * thickness).max(0.0),
                    (size - 2.0 * thickness).max(0.0),
                )
            );
            assert_eq!(fill.color, theme.colors.surfaces.content.into());
            let dots: Vec<_> = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::RoundedRect(dot) if dot.z_index == 4 => Some(dot),
                    _ => None,
                })
                .collect();
            assert_eq!(dots.len(), usize::from(checked));
            if let Some(dot) = dots.first() {
                let inset = theme.geometry.checkbox.checkmark_inset;
                assert_eq!(
                    dot.rect,
                    RectF::new(
                        circle.x + inset,
                        circle.y + inset,
                        size - 2.0 * inset,
                        size - 2.0 * inset
                    )
                );
                assert_eq!(
                    dot.color,
                    if disabled {
                        theme.colors.typography.muted
                    } else {
                        theme.colors.accents.primary
                    }
                    .into()
                );
            }
            let focus: Vec<_> = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border)
                        if border.z_index == crate::core::paint::LAYER_FOCUS =>
                    {
                        Some(border)
                    }
                    _ => None,
                })
                .collect();
            assert!(
                focus.is_empty(),
                "projected member focus requires a current framework owner"
            );
            if let Some(label) = label.filter(|label| !label.is_empty()) {
                let text = nodes
                    .iter()
                    .find_map(|node| match node {
                        SceneNode::Text(text) => Some(text),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(text.text, label);
                assert_eq!(
                    text.position.x,
                    rect.x + size + theme.geometry.checkbox.label_gap
                );
                assert_eq!(text.z_index, LAYER_CONTENT + 2);
            } else if label.is_none() {
                assert!(!nodes.iter().any(|node| matches!(node, SceneNode::Text(_))));
            }
        }
    }
}
