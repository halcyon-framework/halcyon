// SPEC-HAL-210: Slider Control
// SPEC-HAL-208: Integrated label support — label is owned, not a child widget.
// Scalar value emitter with continuous drag and keyboard adjustment.
// Binding choice: f32 via ChangeWriter::set_f32.

use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::composite_text_edit::{
    CompositeEditOperation, CompositeEditReply, CompositeEditorId,
};
use crate::core::event::{
    Event, EventPhase, KeyCode, KeyboardAuthorityMode, PointerButton, PointerEventKind,
};
use crate::core::layout::{
    Constraints, ControlPartRequirement, ControlPartsRequest, ControlPartsRequirements,
    ControlSideRequirements, LayoutResult, ResolvedControlParts,
};
use crate::core::math::{Point, RectF, Size, snap_to_physical};
use crate::core::paint::{
    Color, ColorStop, GradientDef, GradientKind, LAYER_CONTENT, RoundedCorners, TrackDisplayStyle,
    ZIndex,
};
use crate::core::paragraph::TextAlign;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::theme::{ControlChromeCategory, ControlChromeState, ControlValidationState};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::form_binder::{BindingKey, ChangeWriter};
use crate::utils::id::WidgetInternalId;
use crate::widgets::composite_edit_host::{
    CompositeEditorFilter, CompositeEditorSeed, CompositeEditorState, CompositeHostContinuation,
    CompositeHostStep,
};

// Shared paint-only material for the scalar and range controls. All coverage is
// derived from their existing geometry; these gradients use the normal GPU path.
pub(crate) fn paint_track_well(cx: &mut PaintContext, rect: RectF, radius: f32, z: ZIndex) {
    let colors = &cx.theme.colors;
    cx.scene.push_gradient_rect(
        rect,
        radius,
        &GradientDef {
            kind: GradientKind::Linear,
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: colors.borders.shadow,
                },
                ColorStop {
                    offset: 1.0,
                    color: colors.borders.highlight,
                },
            ],
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
        },
        z,
        cx.scene_key,
    );
    let edge = cx.one_px().min(rect.height * 0.5).min(rect.width * 0.5);
    let inner = RectF::new(
        rect.x + edge,
        rect.y + edge,
        (rect.width - 2.0 * edge).max(0.0),
        (rect.height - 2.0 * edge).max(0.0),
    );
    if inner.width > 0.0 && inner.height > 0.0 {
        cx.scene.push_gradient_rect(
            inner,
            (radius - edge).max(0.0),
            &GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop {
                        offset: 0.0,
                        color: colors.surfaces.content.lerp(colors.borders.shadow, 0.12),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: colors.surfaces.content.lerp(colors.borders.highlight, 0.22),
                    },
                ],
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
            },
            z + 1,
            None,
        );
    }
}

pub(crate) fn paint_track_fill(
    cx: &mut PaintContext,
    rect: RectF,
    radius: f32,
    color: Color,
    disabled: bool,
    z: ZIndex,
) {
    if disabled {
        cx.scene
            .push_rounded_rect(rect, color, RoundedCorners::uniform(radius), z, None);
        return;
    }
    // Put the lighting ridge inside the rounded coverage rather than on its
    // anti-aliased edge. Derive every stop from the current color so hover cannot
    // flatten the highlight into the core.
    let highlight = (cx.one_px() / rect.height).min(0.25);
    cx.scene.push_gradient_rect(
        rect,
        radius,
        &GradientDef {
            kind: GradientKind::Linear,
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: color.darken(0.08),
                },
                ColorStop {
                    offset: highlight,
                    color: color.lighten(0.28),
                },
                ColorStop {
                    offset: 0.58,
                    color,
                },
                ColorStop {
                    offset: 1.0,
                    color: color.darken(0.25),
                },
            ],
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
        },
        z,
        None,
    );
}

pub(crate) fn paint_track_thumb(
    cx: &mut PaintContext,
    rect: RectF,
    color: Color,
    pressed: bool,
    disabled: bool,
    z: ZIndex,
) {
    let radius = rect.height * 0.5;
    if disabled {
        cx.scene
            .push_rounded_rect(rect, color, RoundedCorners::uniform(radius), z, None);
        return;
    }
    cx.scene.push_gradient_rect(
        rect,
        radius,
        &GradientDef {
            kind: GradientKind::Linear,
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: color.darken(0.12),
                },
                ColorStop {
                    offset: 1.0,
                    color: color.darken(0.32),
                },
            ],
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
        },
        z,
        None,
    );
    let edge = cx.one_px().min(radius);
    let inner = RectF::new(
        rect.x + edge,
        rect.y + edge,
        (rect.width - 2.0 * edge).max(0.0),
        (rect.height - 2.0 * edge).max(0.0),
    );
    if inner.width > 0.0 && inner.height > 0.0 {
        cx.scene.push_gradient_rect(
            inner,
            (radius - edge).max(0.0),
            &GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop {
                        offset: 0.0,
                        color: if pressed {
                            color.darken(0.10)
                        } else {
                            color.lighten(0.08)
                        },
                    },
                    ColorStop {
                        offset: 0.45,
                        color,
                    },
                    ColorStop {
                        offset: 1.0,
                        color: if pressed {
                            color.lighten(0.12)
                        } else {
                            color.darken(0.10)
                        },
                    },
                ],
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
            },
            z + 1,
            None,
        );
    }
}

// ============================================================================
// State & Events (SPEC-HAL-210)
// ============================================================================

/// Slider state vector.
#[derive(Clone, Debug, PartialEq)]
pub struct SliderState {
    pub value: f32,
    pub disabled: bool,
    pub focused: bool,
    pub active: bool,
}

/// Semantic events for the slider — closed set.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SliderEvent {
    /// Set value to exact position (from pointer drag or click).
    SetValue(f32),
    /// Adjust value by delta (from keyboard).
    Adjust(f32),
    FocusGain,
    FocusLoss,
}

/// Domain specification for the slider.
#[derive(Copy, Clone, Debug)]
pub struct SliderDomain {
    pub min: f32,
    pub max: f32,
    /// Optional step. When set, values snap to multiples of step relative to min.
    pub step: Option<f32>,
}

impl SliderDomain {
    /// Clamp and optionally snap a value to the domain.
    pub(crate) fn constrain(&self, value: f32) -> f32 {
        let clamped = value.clamp(self.min, self.max);
        if let Some(step) = self.step {
            if step > 0.0 {
                let steps = ((clamped - self.min) / step).round();
                (self.min + steps * step).clamp(self.min, self.max)
            } else {
                clamped
            }
        } else {
            clamped
        }
    }

    /// Default keyboard delta: step if set, otherwise 1% of range.
    pub(crate) fn small_delta(&self) -> f32 {
        self.step.unwrap_or((self.max - self.min) * 0.01)
    }

    /// Page delta: 10x small delta, clamped to range.
    pub(crate) fn large_delta(&self) -> f32 {
        (self.small_delta() * 10.0).min(self.max - self.min)
    }

    /// Normalized position [0, 1] for a given value.
    pub(crate) fn normalize(&self, value: f32) -> f32 {
        let range = self.max - self.min;
        if range <= 0.0 {
            0.0
        } else {
            ((value - self.min) / range).clamp(0.0, 1.0)
        }
    }

    /// Denormalize a [0, 1] position to a domain value.
    pub(crate) fn denormalize(&self, t: f32) -> f32 {
        self.min + t.clamp(0.0, 1.0) * (self.max - self.min)
    }
}

/// Which side the editable value field appears on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValueFieldSide {
    /// Value field appears to the left of the track.
    Left,
    /// Value field appears to the right of the track.
    #[default]
    Right,
}

/// Side of a horizontal Slider's owned caption; numeric-field side is independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderLabelSide {
    Left,
    #[default]
    Right,
}

/// Pure reducer for Slider state.
///
/// Returns `Some(new_value)` when value actually changes.
/// Returns `None` otherwise.
pub fn reduce_slider(
    state: &mut SliderState,
    event: SliderEvent,
    domain: &SliderDomain,
) -> Option<f32> {
    // SPEC-HAL-210 INV-G1: Disabled is absolute
    if state.disabled {
        return None;
    }

    match event {
        SliderEvent::SetValue(raw) => {
            let new_value = domain.constrain(raw);
            if (new_value - state.value).abs() > f32::EPSILON {
                state.value = new_value;
                Some(new_value)
            } else {
                None
            }
        }
        SliderEvent::Adjust(delta) => {
            let new_value = domain.constrain(state.value + delta);
            if (new_value - state.value).abs() > f32::EPSILON {
                state.value = new_value;
                Some(new_value)
            } else {
                None
            }
        }
        SliderEvent::FocusGain => {
            state.focused = true;
            None
        }
        SliderEvent::FocusLoss => {
            state.focused = false;
            state.active = false;
            None
        }
    }
}

// ============================================================================
// Widget (SPEC-HAL-210 + SPEC-HAL-208)
// ============================================================================

/// Slider widget.
///
/// Scalar value emitter within a fixed domain. Supports continuous pointer
/// drag and keyboard adjustment. Value label is optional and owned.
///
/// SPEC-HAL-210 INV-A2: Single scalar value (f32).
/// SPEC-HAL-210 INV-F1: Emits via ChangeWriter::set_f32.
pub struct Slider {
    id: WidgetInternalId,
    state: SliderState,
    domain: SliderDomain,
    /// SPEC-HAL-210 INV-C3: Owned value label. None = no value display.
    label: Option<String>,
    label_side: SliderLabelSide,
    /// Pre-formatted value text buffer. Updated on value change to avoid per-frame allocation.
    value_text: String,
    /// Format function for value display. Default: "{value:.1}".
    value_formatter: Option<Box<dyn Fn(f32) -> String>>,
    /// Show value label.
    show_value: bool,
    /// Callback invoked on value change.
    on_value_changed: Option<Box<dyn FnMut(f32)>>,
    /// SPEC-HAL-207: Binding key + writer for change store emission.
    binding: Option<(BindingKey, ChangeWriter)>,
    /// Layout-computed bounds for hit-testing.
    layout_bounds: Cell<Size>,
    /// Cached geometry from layout for use in handle_event.
    cached_track_length: f32,
    /// One Layout-owned geometry used by the editor, painter and input gate.
    layout_parts: Option<ResolvedControlParts>,
    aligned_allocation: bool,
    cached_thumb_diameter: f32,
    /// Layout-measured label rect relative to the track end and control center.
    cached_label_rect: Option<RectF>,
    /// Layout-reserved width for the label, value field, and their gaps.
    cached_part_width: f32,
    /// Actual rendered track width from last paint.
    cached_paint_width: Cell<f32>,
    cached_thumb_snapped: Cell<f32>,
    hover_t: Cell<f32>,
    hover_anim: Cell<Option<TransitionHandle>>,
    drag_anim: Cell<Option<TransitionHandle>>,
    thumb_render_t: Cell<f32>,
    thumb_anim: Cell<Option<TransitionHandle>>,
    display_style: TrackDisplayStyle,
    /// Tree-owned direct editor relation and source-qualified draft projection.
    value_field: CompositeEditorState,
    /// Whether value field is in edit mode.
    value_editing: bool,
    value_entry_pending: bool,
    /// Value when editing started (for revert on Escape).
    value_edit_start: f32,
    /// Which side the value field appears on (Left or Right).
    value_field_side: ValueFieldSide,
    /// Cached widget-local x where the track starts. Set in paint, read by
    /// x_to_value/x_to_value_bar so hit-testing matches the side-aware paint.
    cached_track_offset_x: Cell<f32>,
    /// Cached from layout for enter_value_edit (EventContext has no theme).
    cached_font_size: f32,
    /// Cached from layout for enter_value_edit (EventContext has no theme).
    cached_text_field_padding_x: f32,
}

pub(crate) enum CompositeSliderContinuation {
    Entry,
    Input,
    Commit,
    Retire,
}

impl Slider {
    /// Select shared text-entry presentation before mounting this control.
    pub fn with_smooth_caret_prototype(mut self, enabled: bool) -> Self {
        self.value_field
            .set_initial_motion(enabled)
            .expect("composite presentation is a construction property");
        self
    }
    pub(crate) fn composite_seed(&self) -> CompositeEditorSeed {
        let mut seed = self.value_field.construction_seed();
        seed.text = self.value_text.clone();
        seed.palette = crate::widgets::composite_edit_host::CompositeEditorPalette::InputChrome;
        seed
    }
    pub(crate) fn composite_id(&self) -> Option<&CompositeEditorId> {
        self.value_field.id.as_ref()
    }
    pub(crate) fn composite_bind(&mut self, id: CompositeEditorId, child: NodeId) -> bool {
        self.value_field.bind(id, child)
    }
    pub(crate) fn composite_active(&self) -> bool {
        self.show_value && self.value_editing && !self.state.disabled
    }
    pub(crate) fn composite_visible(&self) -> bool {
        self.show_value && self.value_editing
    }
    pub(crate) fn composite_policy(&self) -> crate::core::text_edit_policy::TextEditPolicy {
        crate::core::text_edit_policy::TextEditPolicy {
            enabled: !self.state.disabled,
            read_only: false,
        }
    }
    pub(crate) fn composite_observe(&mut self, reply: &CompositeEditReply) -> bool {
        self.value_field.absorb(reply)
    }
    pub(crate) fn resume_composite(
        &mut self,
        cx: &mut EventContext,
        continuation: CompositeSliderContinuation,
        reply: CompositeEditReply,
    ) -> CompositeHostStep {
        if matches!(continuation, CompositeSliderContinuation::Retire) {
            return CompositeHostStep::Done { handled: true };
        }
        if !self.value_field.absorb(&reply) {
            if matches!(continuation, CompositeSliderContinuation::Entry) {
                self.value_editing = false;
                self.value_entry_pending = false;
            }
            return CompositeHostStep::Done {
                handled: reply.handled,
            };
        }
        if matches!(continuation, CompositeSliderContinuation::Commit) {
            let text = self.value_field.text().to_owned();
            self.commit_value_edit_text(&text);
            cx.invalidate_layout();
            return CompositeHostStep::Done { handled: true };
        }
        if matches!(
            continuation,
            CompositeSliderContinuation::Entry | CompositeSliderContinuation::Input
        ) {
            // A combined entry/current-pointer operation keeps the Input
            // continuation; its applied reply also completes the entry seed.
            self.value_entry_pending = false;
        }
        CompositeHostStep::Done {
            handled: matches!(continuation, CompositeSliderContinuation::Entry) || reply.handled,
        }
    }
    /// Create a new slider with the given domain.
    pub fn new(min: f32, max: f32) -> Self {
        let domain = SliderDomain {
            min,
            max,
            step: None,
        };
        let value = domain.constrain(min);
        Self {
            id: WidgetInternalId::new(),
            state: SliderState {
                value,
                disabled: false,
                focused: false,
                active: false,
            },
            domain,
            label: None,
            label_side: SliderLabelSide::Right,
            value_text: String::new(),
            value_formatter: None,
            show_value: false,
            on_value_changed: None,
            binding: None,
            layout_bounds: Cell::new(Size::zero()),
            cached_track_length: 0.0,
            layout_parts: None,
            aligned_allocation: false,
            cached_thumb_diameter: 0.0,
            cached_label_rect: None,
            cached_part_width: 0.0,
            cached_paint_width: Cell::new(0.0),
            cached_thumb_snapped: Cell::new(0.0),
            hover_t: Cell::new(0.0),
            hover_anim: Cell::new(None),
            drag_anim: Cell::new(None),
            thumb_render_t: Cell::new(0.0),
            thumb_anim: Cell::new(None),
            display_style: TrackDisplayStyle::Track,
            value_field: CompositeEditorState::new(
                TextAlign::Center,
                CompositeEditorFilter::Decimal,
            ),
            value_editing: false,
            value_entry_pending: false,
            value_edit_start: 0.0,
            value_field_side: ValueFieldSide::Right,
            cached_track_offset_x: Cell::new(0.0),
            // INV-CONSTRUCTOR-FALLBACK: synced from theme in layout()
            cached_font_size: 13.0,
            cached_text_field_padding_x: 0.0,
        }
    }

    /// Place the editable value field on the given side of the track.
    pub fn with_value_field_side(mut self, side: ValueFieldSide) -> Self {
        self.value_field_side = side;
        self
    }

    /// Place the owned caption on a horizontal track's side. Default: Right.
    /// Left is supported by Track/Bar/Pill; a labeled Radial configuration is rejected.
    /// This does not move the numeric field or change input/value semantics.
    pub fn with_label_side(mut self, side: SliderLabelSide) -> Self {
        self.label_side = side;
        self
    }

    /// Create a slider with a step value.
    pub fn with_step(mut self, step: f32) -> Self {
        self.domain.step = Some(step);
        self.state.value = self.domain.constrain(self.state.value);
        self
    }

    /// Set the initial value.
    pub fn with_value(mut self, value: f32) -> Self {
        self.state.value = self.domain.constrain(value);
        self.update_value_text();
        self
    }

    /// SPEC-HAL-208: Set label (builder pattern).
    pub fn with_label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Enable value display.
    pub fn with_value_display(mut self) -> Self {
        self.show_value = true;
        self.update_value_text();
        self
    }

    /// Set a custom value formatter.
    pub fn with_value_formatter<F>(mut self, f: F) -> Self
    where
        F: Fn(f32) -> String + 'static,
    {
        self.value_formatter = Some(Box::new(f));
        self.update_value_text();
        self
    }

    /// Set disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.state.disabled = disabled;
    }

    /// Get current value.
    pub fn value(&self) -> f32 {
        self.state.value
    }

    /// SPEC-HAL-319: Read display style (used by tree snapshot for diff reporting).
    pub fn display_style(&self) -> TrackDisplayStyle {
        self.display_style
    }

    /// Set the value changed callback (builder pattern).
    pub fn on_value_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(f32) + 'static,
    {
        self.on_value_changed = Some(Box::new(callback));
        self
    }

    /// SPEC-HAL-207 INV-F1: Configure binding for change store emission.
    pub fn set_binding(&mut self, key: &str, writer: ChangeWriter) {
        let binding_key = writer.intern(key);
        self.binding = Some((binding_key, writer));
    }

    /// Get immutable reference to state.
    pub fn state(&self) -> &SliderState {
        &self.state
    }

    /// Process a semantic event through the reducer.
    fn on_semantic_event(&mut self, event: SliderEvent) -> Option<f32> {
        let result = reduce_slider(&mut self.state, event, &self.domain);

        if let Some(new_value) = result {
            // Update value text (no allocation if show_value is false)
            if self.show_value {
                self.update_value_text();
            }
            if let Some(callback) = &mut self.on_value_changed {
                callback(new_value);
            }
            // SPEC-HAL-210 INV-F1: Emit via ChangeWriter::set_f32
            if let Some((key, writer)) = &self.binding {
                writer.set_f32(*key, new_value);
            }
        }

        result
    }

    /// Update the cached value text from current state.
    fn update_value_text(&mut self) {
        if self.show_value {
            self.value_text.clear();
            if let Some(formatter) = &self.value_formatter {
                self.value_text = formatter(self.state.value);
            } else {
                use std::fmt::Write;
                let _ = write!(self.value_text, "{:.1}", self.state.value);
            }
        }
    }

    fn aligned_parts(&self) -> Option<ResolvedControlParts> {
        self.layout_parts.filter(|_| self.aligned_allocation)
    }

    /// Convert a local X position to a domain value.
    /// Convert a local X position to a domain value using cached geometry.
    fn x_to_value(&self, local_x: f32) -> f32 {
        let track_w = self
            .aligned_parts()
            .map_or(self.cached_paint_width.get(), |p| p.body.width);
        let thumb_d = if self.aligned_allocation {
            self.cached_thumb_diameter
        } else {
            self.cached_thumb_snapped.get()
        };
        let thumb_radius = thumb_d * 0.5;
        let usable = track_w - thumb_d;
        // Translate widget-local x to track-local x (track may be offset when
        // value field is on the left).
        let track_local_x = local_x
            - self
                .aligned_parts()
                .map_or(self.cached_track_offset_x.get(), |p| p.body.x);
        let t = if usable > 0.0 {
            ((track_local_x - thumb_radius) / usable).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.domain.denormalize(t)
    }

    /// Set the display style (builder pattern).
    pub fn with_display_style(mut self, style: TrackDisplayStyle) -> Self {
        self.display_style = style;
        self
    }

    /// SPEC-HAL-319: Mutate display style in place (used by ConfigureWidget).
    pub fn set_display_style(&mut self, style: TrackDisplayStyle) {
        self.display_style = style;
    }

    /// SPEC-HAL-319: Mutate value in place (used by ConfigureWidget).
    pub fn set_value(&mut self, value: f32) {
        self.state.value = self.domain.constrain(value);
        self.update_value_text();
    }

    /// Convert a local X position to a domain value for bar style (no thumb offset).
    fn x_to_value_bar(&self, local_x: f32) -> f32 {
        let track_local_x = local_x
            - self
                .aligned_parts()
                .map_or(self.cached_track_offset_x.get(), |p| p.body.x);
        let width = self
            .aligned_parts()
            .map_or(self.cached_paint_width.get(), |p| p.body.width);
        let t = (track_local_x / width).clamp(0.0, 1.0);
        self.domain.denormalize(t)
    }

    fn value_field_rect(&self) -> RectF {
        self.layout_parts
            .and_then(|p| p.value)
            .unwrap_or(RectF::zero())
    }

    fn set_touch_track_value(&mut self, cx: &mut EventContext, position: Point) {
        if !self.aligned_body_has_travel() {
            return;
        }
        if self.value_editing {
            self.cancel_value_edit();
        }
        let value = match self.display_style {
            TrackDisplayStyle::Track => self.x_to_value(position.x),
            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => self.x_to_value_bar(position.x),
            TrackDisplayStyle::Radial => {
                self.angle_to_value(position.x, position.y, self.cached_track_length)
            }
        };
        self.on_semantic_event(SliderEvent::SetValue(value));
        cx.invalidate_paint();
    }

    fn aligned_body_has_travel(&self) -> bool {
        self.aligned_parts().is_none_or(|p| {
            p.body.width
                > if self.display_style == TrackDisplayStyle::Track {
                    self.cached_thumb_diameter
                } else {
                    0.0
                }
        })
    }

    fn label_rect(&self) -> Option<RectF> {
        self.layout_parts.and_then(|p| p.label)
    }

    fn paint_label(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        if let (Some(label_text), Some(label_rect)) = (&self.label, self.label_rect()) {
            let run = cx.text_engine.shape(label_text, theme.typography.font_size);
            let text_color = if self.state.disabled {
                theme.colors.typography.muted
            } else {
                theme.colors.typography.primary
            };
            cx.text_engine.draw(
                &run,
                rect.x + label_rect.x,
                rect.y + label_rect.y,
                text_color,
                LAYER_CONTENT,
                cx.scene,
            );
        }

        if let Some(value) = self.layout_parts.and_then(|p| p.value) {
            let font_size = theme.typography.input_font_size;
            let value_rect = RectF::new(
                rect.x + value.x,
                rect.y + value.y,
                value.width,
                value.height,
            );
            let field_state = if self.state.disabled {
                ControlChromeState::Disabled
            } else if self.value_editing && cx.has_capture() {
                ControlChromeState::Captured
            } else {
                ControlChromeState::Idle
            };
            crate::widgets::text_field::paint_input_surface(
                cx,
                value_rect,
                &theme.input_chrome(field_state),
                field_state == ControlChromeState::Captured,
            );

            if !self.value_editing && !self.value_text.is_empty() {
                let (text_w, _) = cx.text_engine.measure(&self.value_text, font_size);
                let centered_x = value_rect.x + (value_rect.width - text_w) / 2.0;
                let metrics = cx.text_engine.font_metrics(font_size);
                let baseline_y = rect.y
                    + compute_control_text_baseline_y(
                        rect.height,
                        metrics.ascent,
                        metrics.descent,
                        theme.typography.baseline_offset,
                    );
                let run = cx.text_engine.shape(&self.value_text, font_size);
                let text_color = if self.state.disabled {
                    theme.colors.typography.muted
                } else {
                    theme.colors.typography.primary
                };
                cx.text_engine.draw(
                    &run,
                    centered_x,
                    baseline_y - metrics.ascent,
                    text_color,
                    LAYER_CONTENT + 3,
                    cx.scene,
                );
            }
        }
    }

    fn focus_cue_width(cx: &PaintContext, captured: bool) -> f32 {
        let chrome = cx.theme.control_chrome(
            ControlChromeCategory::TrackRange,
            if captured {
                ControlChromeState::Captured
            } else {
                ControlChromeState::Focused
            },
            ControlValidationState::Neutral,
        );
        if captured && chrome.focus_width == cx.theme.focus_ring.focused_width {
            chrome.focus_width + cx.one_px()
        } else {
            chrome.focus_width
        }
    }

    fn track_is_active(&self, cx: &PaintContext) -> bool {
        !self.state.disabled && !self.value_editing && (self.state.active || cx.has_capture())
    }

    fn paint_focus_cue(&self, cx: &mut PaintContext, target: RectF, bounds: RectF) {
        if self.state.disabled
            || self.value_editing
            || self.track_is_active(cx)
            || !cx.has_focus_cue()
        {
            return;
        }
        let chrome = cx.theme.control_chrome(
            ControlChromeCategory::TrackRange,
            ControlChromeState::Focused,
            ControlValidationState::Neutral,
        );
        let width = chrome.focus_width;
        // Keep the Track cue separate from the thumb fill.
        let gap = if self.display_style == TrackDisplayStyle::Track {
            2.0 * cx.one_px()
        } else {
            0.0
        };
        let outset = width + gap;
        let expanded = target.expand(outset);
        let cue_rect = if expanded.left() >= bounds.left()
            && expanded.right() <= bounds.right()
            && expanded.top() >= bounds.top()
            && expanded.bottom() <= bounds.bottom()
        {
            expanded
        } else {
            let Some(clipped) = expanded.intersection(&bounds) else {
                return;
            };
            clipped
        };
        let radius = match self.display_style {
            TrackDisplayStyle::Track => target.height * 0.5,
            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => 0.0,
            TrackDisplayStyle::Radial => chrome.radius,
        };
        let cue_radius = (radius + outset).min(cue_rect.width.min(cue_rect.height) * 0.5);
        cx.scene.push_border_rounded_rect(
            cue_rect,
            chrome.focus_color,
            width,
            RoundedCorners::uniform(cue_radius),
            crate::core::paint::LAYER_FOCUS + 1,
            None,
        );
    }

    fn paint_track_style(&self, cx: &mut PaintContext, rect: RectF) {
        if !self.aligned_body_has_travel() {
            self.paint_label(cx, rect);
            return;
        }
        let theme = cx.theme;
        let geom = &theme.geometry.slider;
        let (accent, accent_hover) =
            theme.track_accent_colors(cx.has_capture() && !self.value_editing);

        let _track_length = geom.track_length;
        let track_thickness = geom.track_thickness;
        let thumb_diameter = geom.thumb_diameter;

        // Track centered vertically
        let track_y = rect.y + (rect.height - track_thickness) * 0.5;

        // SPEC-HAL-093 v3: per-coordinate snap to physical pixel grid;
        // 1-physical-pixel sunken sculpted border (input control).
        let scale = cx.scale_factor;
        let one_px = cx.one_px();

        let parts = self
            .layout_parts
            .expect("Slider must be laid out before Paint");
        let actual_track_w = parts.body.width;
        let track_x = rect.x + parts.body.x;

        let slot_left = snap_to_physical(track_x, scale);
        let by = snap_to_physical(track_y, scale);
        let slot_right = snap_to_physical(track_x + actual_track_w, scale);
        let bb = snap_to_physical(track_y + track_thickness, scale);
        let thumb_y_logical = rect.y + (rect.height - thumb_diameter) * 0.5;
        let thumb_y_snapped = snap_to_physical(thumb_y_logical, scale);
        let thumb_diameter_snapped =
            snap_to_physical(rect.y + (rect.height + thumb_diameter) * 0.5, scale)
                - thumb_y_snapped;
        // Keep the cue outside the visible track, with the thumb touching
        // each visible endpoint. Label/value placement retains the outer slot.
        let cue_outset =
            Self::focus_cue_width(cx, false).max(Self::focus_cue_width(cx, true)) + 2.0 * one_px;
        let inset = if self.aligned_allocation {
            0.0
        } else {
            ((cue_outset / one_px).ceil() * one_px)
                .min(((slot_right - slot_left - thumb_diameter_snapped) * 0.5).max(0.0))
        };
        let bx = slot_left + inset;
        let br = slot_right - inset;
        let bw = br - bx;
        let bh = bb - by;
        let snapped_track = RectF::new(bx, by, bw, bh);

        // Cache the actual rendered track width and offset for handle_event pointer mapping.
        if !self.aligned_allocation {
            self.cached_paint_width.set(bw);
            self.cached_track_offset_x.set(bx - rect.x);
            self.cached_thumb_snapped.set(thumb_diameter_snapped);
        }

        // Coordinate track emphasis with the complete control focus stroke.
        let focus_emphasis = if !self.value_editing && cx.has_focus_cue() {
            1.0
        } else {
            0.0
        };

        paint_track_well(cx, snapped_track, bh * 0.5, 2);

        // Thumb (raised — circular, snapped position)
        let t = self.thumb_render_t.get();
        let thumb_travel = (bw - thumb_diameter_snapped).max(0.0);
        let thumb_x = bx + thumb_travel * t;
        let thumb_x_snapped = snap_to_physical(thumb_x, scale);

        // Filled portion of track (from track start to thumb center)
        let thumb_center_x = thumb_x_snapped + thumb_diameter_snapped * 0.5;
        let fill_width = thumb_center_x - bx;
        if fill_width > 0.0 {
            let fill_rect = RectF::new(bx, by, fill_width, bh);
            let fill_color = if self.state.disabled {
                theme.colors.borders.default
            } else {
                let blend = self.hover_t.get().max(focus_emphasis);
                accent.lerp(accent_hover, blend)
            };
            paint_track_fill(cx, fill_rect, bh * 0.5, fill_color, self.state.disabled, 4);
        }
        let thumb_rect = RectF::new(
            thumb_x_snapped,
            thumb_y_snapped,
            thumb_diameter_snapped,
            thumb_diameter_snapped,
        );
        // Decoration grows about the value center; cached mapping keeps the resting diameter.
        let painted_thumb = if self.track_is_active(cx) {
            thumb_rect.expand(thumb_diameter_snapped * 0.125)
        } else {
            thumb_rect
        };

        let thumb_color = if self.state.disabled {
            theme.colors.typography.muted
        } else {
            accent.lerp(accent_hover, self.hover_t.get())
        };

        paint_track_thumb(
            cx,
            painted_thumb,
            thumb_color,
            self.state.active,
            self.state.disabled,
            5,
        );

        self.paint_focus_cue(cx, thumb_rect, rect);
        self.paint_label(cx, rect);
    }

    fn paint_bar_style(&self, cx: &mut PaintContext, rect: RectF) {
        if !self.aligned_body_has_travel() {
            self.paint_label(cx, rect);
            return;
        }
        let theme = cx.theme;
        let geom = &theme.geometry.track_bar;
        let (accent, accent_hover) =
            theme.track_accent_colors(cx.has_capture() && !self.value_editing);
        let bar_height = geom.bar_height;

        let bar_y = rect.y + (rect.height - bar_height) * 0.5;

        // SPEC-HAL-093 v3: per-coordinate snap to physical pixel grid;
        // 1-physical-pixel sunken sculpted border (input control).
        let scale = cx.scale_factor;
        let one_px = cx.one_px();

        let parts = self
            .layout_parts
            .expect("Slider must be laid out before Paint");
        let actual_track_w = parts.body.width;
        let track_x = rect.x + parts.body.x;

        let bx = snap_to_physical(track_x, scale);
        let by = snap_to_physical(bar_y, scale);
        let br = snap_to_physical(track_x + actual_track_w, scale);
        let bb = snap_to_physical(bar_y + bar_height, scale);
        let bw = br - bx;
        let bh = bb - by;
        if !self.aligned_allocation {
            self.cached_paint_width.set(bw);
            self.cached_track_offset_x.set(track_x - rect.x);
        }
        let snapped_bar = RectF::new(bx, by, bw, bh);

        // Background fill
        cx.scene
            .push_rect(snapped_bar, theme.colors.surfaces.content, 1, None);

        // Sunken border: shadow top-left, highlight bottom-right
        let shadow = theme.colors.borders.shadow;
        let highlight = theme.colors.borders.highlight;
        cx.scene
            .push_rect(RectF::new(bx, by, bw, one_px), shadow, 2, None);
        cx.scene.push_rect(
            RectF::new(bx, by + one_px, one_px, bh - 2.0 * one_px),
            shadow,
            2,
            None,
        );
        cx.scene
            .push_rect(RectF::new(bx, bb - one_px, bw, one_px), highlight, 2, None);
        cx.scene.push_rect(
            RectF::new(br - one_px, by + one_px, one_px, bh - 2.0 * one_px),
            highlight,
            2,
            None,
        );

        // Fill from left to value position
        let t = self.thumb_render_t.get();
        let fill_x = t * bw;
        if fill_x > 0.0 {
            let fill_rect = RectF::new(bx, bar_y, fill_x, bar_height);
            // Existing hover/press and navigation emphasis modulate the palette
            // already qualified for this owner's keyboard Capture.
            let focus_emphasis = if self.track_is_active(cx) {
                1.0
            } else if !self.value_editing && cx.has_focus_cue() {
                0.5
            } else {
                0.0
            };
            let fill_color = if self.state.disabled {
                theme.colors.borders.default
            } else {
                let blend = self.hover_t.get().max(focus_emphasis);
                accent.lerp(accent_hover, blend)
            };
            paint_track_fill(cx, fill_rect, 0.0, fill_color, self.state.disabled, 3);
        }

        // Handle: vertical edge tab at value position
        let handle_w = 4.0;
        let handle_overhang = 3.0;
        let handle_x = bx + fill_x;
        let handle_color = if self.state.disabled {
            theme.colors.typography.muted
        } else if self.track_is_active(cx) {
            accent
        } else {
            theme.colors.borders.strong
        };
        let handle_rect = RectF::new(
            handle_x - handle_w * 0.5,
            bar_y - handle_overhang,
            handle_w,
            bar_height + handle_overhang * 2.0,
        );
        let painted_handle = if self.track_is_active(cx) {
            handle_rect.expand(handle_w * 0.25)
        } else {
            handle_rect
        };
        cx.scene.push_rect(painted_handle, handle_color, 4, None);

        self.paint_focus_cue(cx, handle_rect, rect);
        self.paint_label(cx, rect);
    }

    fn paint_radial_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let (accent, accent_hover) =
            theme.track_accent_colors(cx.has_capture() && !self.value_editing);
        let body = self
            .layout_parts
            .expect("Slider must be laid out before Paint")
            .body;
        let size = body.width;
        let track_offset_x = body.x;
        let track_x = rect.x + track_offset_x;

        let track_color = theme.colors.surfaces.content;
        // Keyboard Capture selects its own palette; hover/press and navigation
        // only modulate the current palette through the existing emphasis.
        // INV-FC-NO-NEW-GEOMETRY: same `fill_color` value used by the same
        // `push_radial_gauge` call — no new primitive.
        let focus_emphasis = if self.track_is_active(cx) {
            1.0
        } else if !self.value_editing && cx.has_focus_cue() {
            0.5
        } else {
            0.0
        };
        let fill_color = if self.state.disabled {
            theme.colors.borders.default
        } else {
            let blend = self.hover_t.get().max(focus_emphasis);
            accent.lerp(accent_hover, blend)
        };

        let fill = self.thumb_render_t.get();
        let gauge_rect = RectF::new(track_x, rect.y, size, size);
        let empty_zones = [(0.0, 0.0, [0.0; 4]); 4];
        cx.scene.push_radial_gauge(
            gauge_rect,
            fill,
            if self.track_is_active(cx) { 0.20 } else { 0.15 },
            track_color,
            fill_color,
            fill_color,
            0.0,
            1.0,
            0.0,
            0,
            0,
            0,
            empty_zones,
            0,
            0,
            0.0,
            0.0,
            [0.0; 4],
            2,
            None,
        );

        self.paint_focus_cue(cx, gauge_rect, rect);
        self.cached_track_offset_x.set(track_offset_x);
        self.paint_label(cx, rect);
    }

    /// Compute the widest formatted value text in the domain range.
    /// Used to size the value field so it doesn't resize as the value changes.
    fn max_value_text(&self) -> String {
        if let Some(fmt) = &self.value_formatter {
            let a = fmt(self.domain.min);
            let b = fmt(self.domain.max);
            if a.len() >= b.len() { a } else { b }
        } else {
            let a = format!("{:.1}", self.domain.min);
            let b = format!("{:.1}", self.domain.max);
            if a.len() >= b.len() { a } else { b }
        }
    }

    /// Enter direct-edit mode for the value field.
    fn enter_value_edit(&mut self, cx: &mut EventContext) {
        self.value_editing = true;
        self.value_entry_pending = true;
        self.value_edit_start = self.state.value;
        // The existing Slider owner requests Capture for the editor's caret and keys.
        cx.request_keyboard_capture();
        if cx
            .request_composite_edit(
                CompositeEditOperation::SeedForEntry {
                    text: self.value_text.clone(),
                    select_all: true,
                },
                CompositeHostContinuation::Slider(CompositeSliderContinuation::Entry),
            )
            .is_err()
        {
            self.value_editing = false;
            self.value_entry_pending = false;
        }
        // The engine settles the child seed before the ordinary Layout pass.
        cx.invalidate_layout();
    }

    fn forward_value_event(
        &mut self,
        cx: &mut EventContext,
        event: &Event,
        phase: EventPhase,
    ) -> bool {
        let _ = (event, phase);
        cx.request_composite_edit(
            CompositeEditOperation::ForwardCurrentInput,
            CompositeHostContinuation::Slider(CompositeSliderContinuation::Input),
        )
        .is_ok()
    }

    /// Commit value edit: parse, clamp, update value.
    fn commit_value_edit_text(&mut self, text: &str) {
        self.value_editing = false;
        if let Ok(parsed) = text.trim().parse::<f32>() {
            let clamped = self.domain.constrain(parsed);
            if (clamped - self.state.value).abs() > f32::EPSILON {
                self.state.value = clamped;
                self.update_value_text();
                if let Some(cb) = &mut self.on_value_changed {
                    cb(self.state.value);
                }
                if let Some((key, writer)) = &self.binding {
                    writer.set_f32(*key, self.state.value);
                }
            }
        }
    }

    /// Cancel value edit: revert to value at edit start.
    fn cancel_value_edit(&mut self) {
        self.value_editing = false;
        self.state.value = self.value_edit_start;
        self.update_value_text();
    }

    /// Convert pointer position to domain value for radial style (angular mapping).
    fn angle_to_value(&self, local_x: f32, local_y: f32, size: f32) -> f32 {
        let center_x = self.cached_track_offset_x.get() + size * 0.5;
        let center_y = size * 0.5;
        let dx = local_x - center_x;
        let dy = local_y - center_y;
        let angle = f32::atan2(-dx, dy); // top-center = 0, clockwise
        let norm = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
        self.domain.denormalize(norm.clamp(0.0, 1.0))
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new(0.0, 100.0)
    }
}

impl WidgetDowncast for Slider {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Slider {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.slider;
        self.layout_parts = None;
        self.aligned_allocation = false;
        self.cached_label_rect = self.label.as_ref().map(|label_text| {
            let font_size = cx.theme.typography.font_size;
            let (width, _) = cx.text_engine.measure(label_text, font_size);
            let metrics = cx.text_engine.font_metrics(font_size);
            RectF::new(
                geom.label_gap,
                compute_control_text_baseline_y(
                    0.0,
                    metrics.ascent,
                    metrics.descent,
                    cx.theme.typography.baseline_offset,
                ) - metrics.ascent,
                width,
                metrics.ascent + metrics.descent,
            )
        });
        self.cached_part_width = self
            .cached_label_rect
            .map_or(0.0, |rect| rect.x + rect.width);
        if self.display_style == TrackDisplayStyle::Radial {
            assert!(
                self.label_side == SliderLabelSide::Right || self.label.is_none(),
                "Slider left captions require a horizontal display style"
            );
            let size = cx.theme.geometry.track_bar.bar_height * 2.0;
            self.cached_track_length = size;
            self.cached_thumb_diameter = geom.thumb_diameter;
            let mut total_width = size;
            let mut value_width = 0.0;
            let mut height = size;
            if self.label.is_some() || self.show_value {
                let font_size = if self.show_value {
                    cx.theme
                        .typography
                        .input_font_size
                        .max(cx.theme.typography.font_size)
                } else {
                    cx.theme.typography.font_size
                };
                let metrics = cx.text_engine.font_metrics(font_size);
                let label_height = metrics.ascent + metrics.descent;
                height = height.max(label_height);
            }
            if let Some(label_rect) = self.cached_label_rect {
                total_width += label_rect.x + label_rect.width;
            }
            if self.show_value {
                let max_text = self.max_value_text();
                let font_size = cx.theme.typography.input_font_size;
                let padding_x = cx.theme.geometry.text_field.padding_x;
                self.cached_font_size = font_size;
                self.cached_text_field_padding_x = padding_x;
                let (text_w, _) = cx.text_engine.measure(&max_text, font_size);
                let value_field_width = text_w + padding_x * 2.0;
                value_width = value_field_width;
                self.cached_part_width += geom.label_gap + value_field_width;
                total_width += geom.label_gap + value_field_width;
            }
            let width = total_width.clamp(constraints.min_width, constraints.max_width);
            let height = height.clamp(constraints.min_height, constraints.max_height);
            self.layout_bounds.set(Size::new(width, height));
            let body_x = if self.show_value && self.value_field_side == ValueFieldSide::Left {
                value_width + geom.label_gap
            } else {
                0.0
            };
            let label = self.cached_label_rect.map(|mut label| {
                label.x += body_x + size;
                label.y += height * 0.5;
                label
            });
            let value = self.show_value.then(|| {
                RectF::new(
                    if self.value_field_side == ValueFieldSide::Left {
                        0.0
                    } else {
                        body_x
                            + size
                            + self.cached_label_rect.map_or(0.0, |r| r.x + r.width)
                            + geom.label_gap
                    },
                    0.0,
                    value_width,
                    height,
                )
            });
            self.layout_parts = Some(ResolvedControlParts {
                body: RectF::new(body_x, 0.0, size, size),
                label,
                value,
            });
            if let Some(value) = value {
                self.value_field.layout(
                    cx,
                    Constraints::tight(Size::new(value.width, value.height)),
                    value.x,
                    value.y,
                );
            }
            return LayoutResult::fit(self.layout_bounds.get());
        }

        let track_height = match self.display_style {
            TrackDisplayStyle::Track => geom.thumb_diameter, // Height accommodates thumb
            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => {
                cx.theme.geometry.track_bar.bar_height
            }
            TrackDisplayStyle::Radial => unreachable!(),
        };
        let track_width = geom.track_length;

        // Cache geometry for handle_event (no theme access in EventContext).
        // The final parent allocation is resolved below before child Layout.
        self.cached_track_length = track_width;
        self.cached_thumb_diameter = geom.thumb_diameter;

        // SPEC-HAL-210 INV-D1: Layout accounts for track + label + value label
        let mut total_width = track_width;
        let mut height = track_height.max(cx.theme.geometry.text_field.height);

        if self.label.is_some() || self.show_value {
            let font_size = if self.show_value {
                cx.theme
                    .typography
                    .input_font_size
                    .max(cx.theme.typography.font_size)
            } else {
                cx.theme.typography.font_size
            };
            let metrics = cx.text_engine.font_metrics(font_size);
            let label_height = metrics.ascent + metrics.descent;
            height = height.max(label_height);
        }

        if let Some(label_rect) = self.cached_label_rect {
            total_width += label_rect.x + label_rect.width;
        }

        if self.show_value {
            let max_text = self.max_value_text();
            let font_size = cx.theme.typography.input_font_size;
            let padding_x = cx.theme.geometry.text_field.padding_x;
            self.cached_font_size = font_size;
            self.cached_text_field_padding_x = padding_x;
            let (text_w, _) = cx.text_engine.measure(&max_text, font_size);
            let value_field_width = text_w + padding_x * 2.0;
            self.cached_part_width += geom.label_gap + value_field_width;
            total_width += geom.label_gap + value_field_width;
        }

        let requirements = ControlPartsRequirements {
            leading: ControlSideRequirements {
                label: self
                    .cached_label_rect
                    .filter(|_| self.label_side == SliderLabelSide::Left)
                    .map(|r| ControlPartRequirement {
                        width: r.width,
                        gap: geom.label_gap,
                    }),
                value: (self.show_value && self.value_field_side == ValueFieldSide::Left)
                    .then_some(ControlPartRequirement {
                        width: self.cached_part_width
                            - self.cached_label_rect.map_or(0.0, |r| r.x + r.width)
                            - geom.label_gap,
                        gap: geom.label_gap,
                    }),
            },
            trailing: ControlSideRequirements {
                label: self
                    .cached_label_rect
                    .filter(|_| self.label_side == SliderLabelSide::Right)
                    .map(|r| ControlPartRequirement {
                        width: r.width,
                        gap: geom.label_gap,
                    }),
                value: (self.show_value && self.value_field_side == ValueFieldSide::Right)
                    .then_some(ControlPartRequirement {
                        width: self.cached_part_width
                            - self.cached_label_rect.map_or(0.0, |r| r.x + r.width)
                            - geom.label_gap,
                        gap: geom.label_gap,
                    }),
            },
            body_width: track_width,
            height,
        };
        let size = constraints.constrain(Size::new(total_width, height));
        let columns = match cx.control_parts_request() {
            Some(ControlPartsRequest::Measure) => {
                cx.report_control_parts(requirements);
                crate::core::layout::ControlPartColumns::with_trailing_order(
                    size.width,
                    requirements.leading,
                    requirements.trailing,
                    crate::core::layout::ControlTrailingOrder::LabelThenValue,
                )
            }
            Some(ControlPartsRequest::Arrange(columns)) => {
                self.aligned_allocation = true;
                columns
            }
            None if self.label_side == SliderLabelSide::Left && self.label.is_some() => {
                self.aligned_allocation = true;
                crate::core::layout::ControlPartColumns::new(
                    size.width,
                    requirements.leading,
                    requirements.trailing,
                )
            }
            None => crate::core::layout::ControlPartColumns::with_trailing_order(
                size.width,
                requirements.leading,
                requirements.trailing,
                crate::core::layout::ControlTrailingOrder::LabelThenValue,
            ),
        };
        let mut parts = columns.resolve(requirements, size.height);
        if let (Some(label), Some(measured)) = (parts.label.as_mut(), self.cached_label_rect) {
            label.y = measured.y + size.height * 0.5;
            label.height = measured.height;
        }
        self.cached_track_length = parts.body.width;
        if let Some(value) = parts.value {
            self.value_field.layout(
                cx,
                Constraints::tight(Size::new(value.width, value.height)),
                value.x,
                value.y,
            );
        }
        self.layout_parts = Some(parts);
        self.layout_bounds.set(size);
        LayoutResult::new(
            size,
            Size::new(
                columns.content_width().max(size.width),
                height.max(size.height),
            ),
        )
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Consume hover and focus animations
        {
            let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
            if let Some(handle) = self.hover_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.hover_t.set(value);
                } else {
                    self.hover_anim.set(None);
                }
            }
            if let Some(handle) = self.thumb_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.thumb_render_t.set(value);
                } else {
                    self.thumb_anim.set(None);
                    self.thumb_render_t
                        .set(self.domain.normalize(self.state.value));
                }
            } else {
                self.thumb_render_t
                    .set(self.domain.normalize(self.state.value));
            }
        }

        match self.display_style {
            TrackDisplayStyle::Track => self.paint_track_style(cx, rect),
            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => self.paint_bar_style(cx, rect),
            TrackDisplayStyle::Radial => self.paint_radial_style(cx, rect),
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

        if let Event::Pointer(pe) = event {
            if self.show_value
                && !self.state.active
                && RectF::new(
                    0.0,
                    0.0,
                    self.layout_bounds.get().width,
                    self.layout_bounds.get().height,
                )
                .contains(Point::new(pe.local_pos.0, pe.local_pos.1))
                && self
                    .value_field_rect()
                    .contains(Point::new(pe.local_pos.0, pe.local_pos.1))
            {
                cx.request_cursor(crate::core::event::CursorIcon::Text);
            }
        }

        // If editing the value field, intercept Enter/Escape and forward all other
        // events to the TextField. If TextField handles, return; otherwise fall
        // through to normal slider handling (e.g. click on track cancels edit).
        if self.value_editing {
            if let Event::Key(ke) = event {
                if ke.is_down {
                    match ke.key_code {
                        KeyCode::Enter => {
                            let _ = cx.request_composite_edit(
                                CompositeEditOperation::ObserveAndActivity,
                                CompositeHostContinuation::Slider(
                                    CompositeSliderContinuation::Commit,
                                ),
                            );
                            return true;
                        }
                        KeyCode::Escape => {
                            self.cancel_value_edit();
                            return true;
                        }
                        _ => {}
                    }
                }
            }

            // Gate pointer events: only forward to value_field if the
            // pointer is inside the value field rect. Clicks outside
            // cancel edit and fall through to normal slider handling.
            if let Event::Pointer(pe) = event {
                let (local_x, local_y) = pe.local_pos;
                let vr = self.value_field_rect();
                if (vr.width > 0.0 && vr.contains(Point::new(local_x, local_y)))
                    || (cx.has_pointer_capture()
                        && matches!(pe.kind, PointerEventKind::Move | PointerEventKind::Up))
                {
                    // Escape preserves the draft while releasing Capture.
                    // A new primary press must reacquire the real host authority,
                    // including fused double/triple presses, before child input.
                    if cx.keyboard_mode == KeyboardAuthorityMode::Navigation
                        && pe.button == Some(PointerButton::Primary)
                        && matches!(
                            pe.kind,
                            PointerEventKind::Down
                                | PointerEventKind::DoubleClick
                                | PointerEventKind::TripleClick
                        )
                    {
                        cx.request_keyboard_capture();
                    }
                    // Click inside value field → forward to TextField
                    if self.forward_value_event(cx, event, phase) {
                        return true;
                    }
                } else if pe.kind == PointerEventKind::Down
                    || pe.kind == PointerEventKind::DoubleClick
                {
                    // Click outside value field → cancel edit, fall through
                    // to normal slider handling (track click, drag, etc.)
                    self.cancel_value_edit();
                    // DON'T return — fall through to the match event below
                }
                // Move/Up outside value field during editing: ignore
                // (fall through to normal handler)
            } else {
                // Non-pointer events (Key not handled above, Text, etc.)
                // → forward to value_field
                if self.forward_value_event(cx, event, phase) {
                    return true;
                }
            }
        }

        match event {
            Event::Pointer(pe) => {
                // SPEC-HAL-210 INV-D3: Full-area hit testing
                let (local_x, local_y) = pe.local_pos;
                let bounds = self.layout_bounds.get();
                let inside = local_x >= 0.0
                    && local_x <= bounds.width
                    && local_y >= 0.0
                    && local_y <= bounds.height;

                match pe.kind {
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if inside {
                            if self.show_value {
                                let vr = self.value_field_rect();
                                if vr.width > 0.0 && vr.contains(Point::new(local_x, local_y)) {
                                    self.on_semantic_event(SliderEvent::FocusGain);
                                    self.enter_value_edit(cx);
                                    // Entry keeps select-all; later presses place the caret.
                                    return true;
                                }
                            }

                            if self
                                .label_rect()
                                .is_some_and(|rect| rect.contains(Point::new(local_x, local_y)))
                            {
                                self.on_semantic_event(SliderEvent::FocusGain);
                                return true;
                            }

                            if let Some(parts) = self.aligned_parts() {
                                if !self.aligned_body_has_travel()
                                    || !parts.body.contains(Point::new(local_x, local_y))
                                {
                                    return false;
                                }
                            }

                            // Pointer down on slider proper exits any in-progress edit (revert).
                            if self.value_editing {
                                self.cancel_value_edit();
                            }
                            self.on_semantic_event(SliderEvent::FocusGain);
                            self.state.active = true;
                            // SPEC-HAL-210 INV-B1: Set value on click position
                            let raw = match self.display_style {
                                TrackDisplayStyle::Track => self.x_to_value(local_x),
                                TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => {
                                    self.x_to_value_bar(local_x)
                                }
                                TrackDisplayStyle::Radial => {
                                    self.angle_to_value(local_x, local_y, self.cached_track_length)
                                }
                            };
                            self.on_semantic_event(SliderEvent::SetValue(raw));
                            cx.capture_pointer(PointerButton::Primary);
                            // Keep frame loop alive during drag for smooth thumb movement
                            let keep_alive = Animation {
                                from: 0.0,
                                to: 1.0,
                                duration: 3600.0,
                                easing: Easing::Linear,
                            };
                            if let Some(handle) = cx.start_animation(keep_alive) {
                                self.drag_anim.set(Some(handle));
                            }
                            true
                        } else {
                            false
                        }
                    }
                    // SPEC-HAL-210 INV-B2: Drag is continuous intent
                    PointerEventKind::Move => {
                        if self.state.active {
                            if !self.aligned_body_has_travel() {
                                return false;
                            }
                            self.thumb_anim.set(None);
                            let raw = match self.display_style {
                                TrackDisplayStyle::Track => self.x_to_value(local_x),
                                TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => {
                                    self.x_to_value_bar(local_x)
                                }
                                TrackDisplayStyle::Radial => {
                                    self.angle_to_value(local_x, local_y, self.cached_track_length)
                                }
                            };
                            self.on_semantic_event(SliderEvent::SetValue(raw));
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Up => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if self.state.active {
                            self.state.active = false;
                            self.drag_anim.set(None);
                            self.thumb_anim.set(None);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            // SPEC-HAL-210 INV-B3: Keyboard parity
            Event::Key(ke) if ke.is_down => {
                if !self.value_editing {
                    match ke.key_code {
                        KeyCode::Right => {
                            if cx.keyboard_mode != KeyboardAuthorityMode::Capture {
                                return false;
                            }
                            if self.state.focused {
                                let old_t = self.thumb_render_t.get();
                                self.on_semantic_event(SliderEvent::Adjust(
                                    self.domain.small_delta(),
                                ));
                                let new_t = self.domain.normalize(self.state.value);
                                if (new_t - old_t).abs() > 0.001 {
                                    let anim = Animation {
                                        from: old_t,
                                        to: new_t,
                                        duration: 0.08,
                                        easing: Easing::EaseOut,
                                    };
                                    if let Some(handle) = cx.start_animation(anim) {
                                        self.thumb_anim.set(Some(handle));
                                    }
                                }
                                return true;
                            }
                            return false;
                        }
                        KeyCode::Left => {
                            if cx.keyboard_mode != KeyboardAuthorityMode::Capture {
                                return false;
                            }
                            if self.state.focused {
                                let old_t = self.thumb_render_t.get();
                                self.on_semantic_event(SliderEvent::Adjust(
                                    -self.domain.small_delta(),
                                ));
                                let new_t = self.domain.normalize(self.state.value);
                                if (new_t - old_t).abs() > 0.001 {
                                    let anim = Animation {
                                        from: old_t,
                                        to: new_t,
                                        duration: 0.08,
                                        easing: Easing::EaseOut,
                                    };
                                    if let Some(handle) = cx.start_animation(anim) {
                                        self.thumb_anim.set(Some(handle));
                                    }
                                }
                                return true;
                            }
                            return false;
                        }
                        KeyCode::PageUp => {
                            if self.state.focused {
                                let old_t = self.thumb_render_t.get();
                                self.on_semantic_event(SliderEvent::Adjust(
                                    self.domain.large_delta(),
                                ));
                                let new_t = self.domain.normalize(self.state.value);
                                if (new_t - old_t).abs() > 0.001 {
                                    let anim = Animation {
                                        from: old_t,
                                        to: new_t,
                                        duration: 0.08,
                                        easing: Easing::EaseOut,
                                    };
                                    if let Some(handle) = cx.start_animation(anim) {
                                        self.thumb_anim.set(Some(handle));
                                    }
                                }
                                return true;
                            }
                            return false;
                        }
                        KeyCode::PageDown => {
                            if self.state.focused {
                                let old_t = self.thumb_render_t.get();
                                self.on_semantic_event(SliderEvent::Adjust(
                                    -self.domain.large_delta(),
                                ));
                                let new_t = self.domain.normalize(self.state.value);
                                if (new_t - old_t).abs() > 0.001 {
                                    let anim = Animation {
                                        from: old_t,
                                        to: new_t,
                                        duration: 0.08,
                                        easing: Easing::EaseOut,
                                    };
                                    if let Some(handle) = cx.start_animation(anim) {
                                        self.thumb_anim.set(Some(handle));
                                    }
                                }
                                return true;
                            }
                            return false;
                        }
                        KeyCode::Home => {
                            if self.state.focused {
                                self.on_semantic_event(SliderEvent::SetValue(self.domain.min));
                                return true;
                            }
                            return false;
                        }
                        KeyCode::End => {
                            if self.state.focused {
                                self.on_semantic_event(SliderEvent::SetValue(self.domain.max));
                                return true;
                            }
                            return false;
                        }
                        _ => {}
                    }
                }
                if self.value_editing {
                    self.forward_value_event(cx, event, phase)
                } else {
                    false
                }
            }
            // SPEC-HAL-210 INV-B3 + SPEC-HAL-176: Plain unmodified h/l in
            // Capture mode act as small-delta decrement/increment, mirroring
            // the KeyCode::Left/Right Capture-mode behavior. h/l arrive as
            // Event::Text (not KeyDown) for unmodified letter keys per the
            // platform layer. Widget-owned interpretation in Capture mode.
            Event::Text(te) => {
                if !self.value_editing
                    && self.state.focused
                    && cx.keyboard_mode == KeyboardAuthorityMode::Capture
                    && (te.text == "h" || te.text == "l")
                {
                    let delta = if te.text == "l" {
                        self.domain.small_delta()
                    } else {
                        -self.domain.small_delta()
                    };
                    let old_t = self.thumb_render_t.get();
                    self.on_semantic_event(SliderEvent::Adjust(delta));
                    let new_t = self.domain.normalize(self.state.value);
                    if (new_t - old_t).abs() > 0.001 {
                        let anim = Animation {
                            from: old_t,
                            to: new_t,
                            duration: 0.08,
                            easing: Easing::EaseOut,
                        };
                        if let Some(handle) = cx.start_animation(anim) {
                            self.thumb_anim.set(Some(handle));
                        }
                    }
                    return true;
                }
                if self.value_editing {
                    return self.forward_value_event(cx, event, phase);
                }
                false
            }
            _ => {
                if self.value_editing {
                    self.forward_value_event(cx, event, phase)
                } else {
                    false
                }
            }
        }
    }

    fn focusable(&self) -> bool {
        !self.state.disabled
    }

    fn paints_own_focus_indicator(&self) -> bool {
        true
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.on_semantic_event(SliderEvent::FocusGain);
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        // Focus loss reverts in-progress edit (no implicit commit).
        if self.value_editing {
            self.cancel_value_edit();
        }
        let _ = cx.request_composite_edit(
            CompositeEditOperation::RetireDelegation,
            CompositeHostContinuation::Slider(CompositeSliderContinuation::Retire),
        );
        self.on_semantic_event(SliderEvent::FocusLoss);
    }

    fn accepts_keyboard_activation(&self) -> bool {
        false
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if self.state.disabled {
            return;
        }
        let anim = Animation {
            from: self.hover_t.get(),
            to: 1.0,
            duration: 0.08,
            easing: Easing::EaseOut,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.hover_anim.set(Some(handle));
        } else {
            self.hover_t.set(1.0);
            self.hover_anim.set(None);
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        let anim = Animation {
            from: self.hover_t.get(),
            to: 0.0,
            duration: 0.08,
            easing: Easing::EaseIn,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.hover_anim.set(Some(handle));
        } else {
            self.hover_t.set(0.0);
            self.hover_anim.set(None);
        }
    }

    fn on_mount(&mut self) {
        self.hover_t.set(0.0);
        self.hover_anim.set(None);
    }

    fn touch_target(&self, hit: crate::core::event::TouchHit) -> Option<crate::core::event::TouchTarget> {
        use crate::core::event::TouchTarget;
        if self.state.disabled {
            return None;
        }
        let position = hit.local_position;
        if self.show_value && self.value_field_rect().contains(position) {
            Some(TouchTarget::Editor)
        } else if self.label_rect().is_some_and(|rect| rect.contains(position)) {
            Some(TouchTarget::Control)
        } else {
            (self.aligned_body_has_travel()
                && self.aligned_parts().is_none_or(|parts| parts.body.contains(position)))
                .then_some(TouchTarget::Track)
        }
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, tap: crate::core::event::TouchTap) -> bool {
        use crate::core::event::TouchTarget;
        match tap.target {
            TouchTarget::Editor if !self.value_editing => {
                self.enter_value_edit(cx);
                true
            }
            TouchTarget::Editor => cx.request_composite_edit(
                CompositeEditOperation::PlaceTouchCaret { local_x: tap.local_position.x },
                CompositeHostContinuation::Slider(CompositeSliderContinuation::Input),
            ).is_ok(),
            TouchTarget::Track => {
                self.set_touch_track_value(cx, tap.local_position);
                true
            }
            TouchTarget::Control => true,
            _ => false,
        }
    }

    fn gesture_surface(&self) -> &'static [crate::core::event::GestureKind] {
        &[crate::core::event::GestureKind::Drag]
    }

    fn touch_drag_axis(&self, target: crate::core::event::TouchTarget) -> Option<crate::core::event::GestureAxis> {
        use crate::core::event::GestureAxis;
        (target == crate::core::event::TouchTarget::Track).then_some(
            if self.display_style == TrackDisplayStyle::Radial { GestureAxis::Any } else { GestureAxis::Horizontal }
        )
    }

    fn on_touch_drag_motion(&mut self, cx: &mut EventContext, motion: crate::core::event::TouchDragMotion) {
        self.set_touch_track_value(cx, motion.local_position);
    }

    fn capture_eligible(&self) -> bool {
        !self.state.disabled
    }

    fn on_capture_enter(&mut self, cx: &mut EventContext) {
        if self.value_editing && !self.value_entry_pending {
            self.value_entry_pending = true;
            if cx
                .request_composite_edit(
                    CompositeEditOperation::SeedForEntry {
                        text: self.value_field.text().to_owned(),
                        select_all: false,
                    },
                    CompositeHostContinuation::Slider(CompositeSliderContinuation::Entry),
                )
                .is_err()
            {
                self.value_entry_pending = false;
            }
        }
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.value_entry_pending = false;
        let _ = cx.request_composite_edit(
            CompositeEditOperation::RetireDelegation,
            CompositeHostContinuation::Slider(CompositeSliderContinuation::Retire),
        );
    }

    fn debug_type_name(&self) -> &'static str {
        "Slider"
    }
}
