// SPEC-HAL-212: SpinBox Control
// SPEC-HAL-208: Integrated label support — label is owned, not a child widget.
// Discrete numeric editor with explicit increment/decrement affordances.
// Binding choice: i64 value, emitted via ChangeWriter::set_f32.
//
// INV-A1: Value is external. Widget emits change intents only.
// INV-A2: No hidden state (no cached deltas, no accumulation).
// INV-A3: No async ownership (no debounce, no delay, no animation).
// INV-B1: Explicit numeric domain (i64, optional min/max, explicit step).
// INV-B2: Step is declarative (no acceleration, no variable step, no modifier-key magic).
// INV-B3: Bounds policy is CLAMP — documented here and in constrain().
// INV-C1: Explicit change emission (only on increment/decrement activation).
// INV-C2: Exactly-once per action.
// INV-C3: No auto-commit guessing.
// INV-D1: Three logical regions (decrement, value display, increment).
// INV-D2: Label is owned, non-interactive, clicking label does NOT change value.
// INV-D3: Hit regions are exact. No full-row activation.
// INV-E1: Arrow keys explicit (Up = increment, Down = decrement, no acceleration).
// INV-E2: Modifier keys inert.
// INV-F1: Disabled is absolute (no emission, no value change, visual only).
// INV-F2: Focus does not change value.
// INV-G1: Application-layer binding via ChangeWriter::set_f32.
// INV-G2: No self-synchronization.
// INV-H1: Deterministic behavior.
// INV-H2: No per-frame allocation.
// INV-H3: Invariant supremacy.

use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::composite_text_edit::{
    CompositeEditOperation, CompositeEditReply, CompositeEditorId,
};
use crate::core::event::{
    Event, EventPhase, KeyCode, KeyboardAuthorityMode, PointerButton, PointerEventKind,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{LAYER_CONTENT, ZIndex};
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

const SPIN_BOX_TEXT_LAYER: ZIndex = LAYER_CONTENT + 5;

// ============================================================================
// State & Events (SPEC-HAL-212)
// ============================================================================

/// SpinBox state vector.
#[derive(Clone, Debug, PartialEq)]
pub struct SpinBoxState {
    pub value: i64,
    pub disabled: bool,
    pub focused: bool,
}

/// Semantic events for the SpinBox — closed set.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SpinBoxEvent {
    /// Increment value by step.
    Increment,
    /// Decrement value by step.
    Decrement,
    FocusGain,
    FocusLoss,
}

/// Domain specification for the SpinBox.
#[derive(Copy, Clone, Debug)]
pub struct SpinBoxDomain {
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub step: i64,
}

impl SpinBoxDomain {
    /// INV-B3: Bounds policy is CLAMP — explicit, documented.
    /// If min/max are defined, the value is clamped to [min, max].
    /// If neither is defined, the value passes through unconstrained.
    fn constrain(&self, value: i64) -> i64 {
        let mut v = value;
        if let Some(min) = self.min {
            if v < min {
                v = min;
            }
        }
        if let Some(max) = self.max {
            if v > max {
                v = max;
            }
        }
        v
    }
}

/// Pure reducer for SpinBox state.
///
/// Returns `Some(new_value)` when value actually changes.
/// Returns `None` otherwise.
pub fn reduce_spin_box(
    state: &mut SpinBoxState,
    event: SpinBoxEvent,
    domain: &SpinBoxDomain,
) -> Option<i64> {
    // INV-F1: Disabled is absolute — no emission, no value change.
    if state.disabled {
        return None;
    }

    match event {
        // INV-C2: Exactly-once per action.
        SpinBoxEvent::Increment => {
            let new_value = domain.constrain(state.value.saturating_add(domain.step));
            if new_value != state.value {
                state.value = new_value;
                Some(new_value)
            } else {
                None
            }
        }
        SpinBoxEvent::Decrement => {
            let new_value = domain.constrain(state.value.saturating_sub(domain.step));
            if new_value != state.value {
                state.value = new_value;
                Some(new_value)
            } else {
                None
            }
        }
        // INV-F2: Focus does not change value.
        SpinBoxEvent::FocusGain => {
            state.focused = true;
            None
        }
        SpinBoxEvent::FocusLoss => {
            state.focused = false;
            None
        }
    }
}

// ============================================================================
// Hit region (INV-D1: Three logical regions)
// ============================================================================

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum HitRegion {
    Decrement,
    Value,
    Increment,
}

// ============================================================================
// Widget (SPEC-HAL-212 + SPEC-HAL-208)
// ============================================================================

/// SpinBox widget — discrete numeric editor.
///
/// Provides explicit increment/decrement affordances with a declared step.
/// Value is i64. Optional min/max bounds with CLAMP policy (INV-B3).
///
/// SPEC-HAL-212 INV-A1: Value is external (application-owned).
/// SPEC-HAL-212 INV-G1: Emits via ChangeWriter::set_f32.
pub struct SpinBox {
    id: WidgetInternalId,
    state: SpinBoxState,
    domain: SpinBoxDomain,
    /// SPEC-HAL-208: Owned label text. Visual only (INV-D2).
    label: Option<String>,
    /// Pre-formatted value text. Updated on value change (INV-H2: no per-frame allocation).
    value_text: String,
    /// Callback invoked on value change.
    on_value_changed: Option<Box<dyn FnMut(i64)>>,
    /// SPEC-HAL-207: Binding key + writer for change store emission.
    binding: Option<(BindingKey, ChangeWriter)>,
    /// Layout-computed bounds for hit-testing.
    layout_bounds: Size,
    /// Cached button width from layout (no theme access in EventContext).
    cached_button_width: f32,
    /// Cached total control width (buttons + value, excluding label).
    cached_control_width: f32,
    /// Cached value region width from layout (content-aware).
    /// Paint MUST use this, not geom.value_width — SPEC-PM-009.
    cached_value_width: f32,
    /// Cached font size from layout (EventContext has no theme).
    cached_font_size: f32,
    /// Cached TextField padding_x from layout (EventContext has no theme).
    cached_text_field_padding_x: f32,
    /// Currently pressed region (for press/release semantics). INV-D3.
    pressed_region: Option<HitRegion>,
    hover_t: Cell<f32>,
    cell_hovered: bool,
    hover_anim: Cell<Option<TransitionHandle>>,
    focus_t: Cell<f32>,
    focus_anim: Cell<Option<TransitionHandle>>,
    /// Tree-owned direct editor relation and source-qualified draft projection.
    editing_field: CompositeEditorState,
    /// Whether we're in direct-edit mode.
    editing: bool,
    /// Value when editing started (for revert on Escape).
    edit_start_value: i64,
    /// SPEC-HAL-260A §6.3: fired when an edit cycle ends with a successful
    /// commit (direct-edit Enter, or `force_commit` while in editing mode).
    /// Per-step Increment/Decrement do NOT fire this — they use the
    /// existing `on_value_changed` callback (INV-C1).
    on_commit: Option<Box<dyn Fn(i64)>>,
    /// SPEC-HAL-260A §6.3: fired when the user cancels the direct-edit
    /// cycle via Escape. Not fired from internal `cancel_edit` invocations
    /// triggered by blur, capture-leave, or +/- button presses during
    /// editing — those are state cleanup, not user-explicit cancel intent.
    on_cancel: Option<Box<dyn Fn()>>,
}

pub(crate) enum CompositeSpinContinuation {
    Entry,
    Input,
    Commit { release: bool },
    Retire,
}

impl SpinBox {
    pub(crate) fn cell_editor_visual_facts(
        &self,
    ) -> crate::core::appearance::CellEditorVisualFacts {
        crate::core::appearance::CellEditorVisualFacts {
            disabled: self.state.disabled,
            hovered: self.cell_hovered,
        }
    }

    /// Select shared text-entry presentation before mounting this control.
    pub fn with_smooth_caret_prototype(mut self, enabled: bool) -> Self {
        self.editing_field
            .set_initial_motion(enabled)
            .expect("composite presentation is a construction property");
        self
    }
    pub(crate) fn composite_seed(&self) -> CompositeEditorSeed {
        let mut seed = self.editing_field.construction_seed();
        seed.text = self.value_text.clone();
        seed.palette = crate::widgets::composite_edit_host::CompositeEditorPalette::InputChrome;
        seed
    }
    pub(crate) fn composite_id(&self) -> Option<&CompositeEditorId> {
        self.editing_field.id.as_ref()
    }

    pub(crate) fn composite_bind(&mut self, id: CompositeEditorId, child: NodeId) -> bool {
        self.editing_field.bind(id, child)
    }

    pub(crate) fn composite_active(&self) -> bool {
        self.editing && !self.state.disabled
    }
    pub(crate) fn composite_visible(&self) -> bool {
        self.editing
    }
    pub(crate) fn composite_policy(&self) -> crate::core::text_edit_policy::TextEditPolicy {
        crate::core::text_edit_policy::TextEditPolicy {
            enabled: !self.state.disabled,
            read_only: false,
        }
    }
    pub(crate) fn composite_observe(&mut self, reply: &CompositeEditReply) -> bool {
        self.editing_field.absorb(reply)
    }

    fn forward_edit(&self, cx: &mut EventContext) -> bool {
        cx.request_composite_edit(
            CompositeEditOperation::ForwardCurrentInput,
            CompositeHostContinuation::Spin(CompositeSpinContinuation::Input),
        )
        .is_ok()
    }

    pub(crate) fn resume_composite(
        &mut self,
        cx: &mut EventContext,
        continuation: CompositeSpinContinuation,
        reply: CompositeEditReply,
    ) -> CompositeHostStep {
        if matches!(continuation, CompositeSpinContinuation::Retire) {
            return CompositeHostStep::Done { handled: true };
        }
        if !self.editing_field.absorb(&reply) {
            if matches!(continuation, CompositeSpinContinuation::Entry) {
                self.editing = false;
            }
            return CompositeHostStep::Done {
                handled: reply.handled,
            };
        }
        match continuation {
            CompositeSpinContinuation::Commit { release } => {
                let text = self.editing_field.text().to_owned();
                self.commit_edit_text(&text);
                if release {
                    cx.release_keyboard_capture();
                }
                cx.invalidate_layout();
                CompositeHostStep::Done { handled: true }
            }
            CompositeSpinContinuation::Entry => CompositeHostStep::Done { handled: true },
            CompositeSpinContinuation::Input => CompositeHostStep::Done {
                handled: reply.handled,
            },
            CompositeSpinContinuation::Retire => unreachable!(),
        }
    }

    /// Create a new SpinBox with step=1, no bounds.
    pub fn new() -> Self {
        let mut s = Self {
            id: WidgetInternalId::new(),
            state: SpinBoxState {
                value: 0,
                disabled: false,
                focused: false,
            },
            domain: SpinBoxDomain {
                min: None,
                max: None,
                step: 1,
            },
            label: None,
            value_text: String::new(),
            on_value_changed: None,
            binding: None,
            layout_bounds: Size::zero(),
            cached_button_width: 0.0,
            cached_control_width: 0.0,
            cached_value_width: 0.0,
            // INV-CONSTRUCTOR-FALLBACK: synced from theme in layout()
            cached_font_size: 13.0,
            cached_text_field_padding_x: 0.0,
            pressed_region: None,
            hover_t: Cell::new(0.0),
            cell_hovered: false,
            hover_anim: Cell::new(None),
            focus_t: Cell::new(0.0),
            focus_anim: Cell::new(None),
            editing_field: CompositeEditorState::new(
                TextAlign::Center,
                CompositeEditorFilter::Integer,
            ),
            editing: false,
            edit_start_value: 0,
            on_commit: None,
            on_cancel: None,
        };
        s.update_value_text();
        s
    }

    /// Set initial value (builder pattern).
    pub fn with_value(mut self, value: i64) -> Self {
        self.state.value = self.domain.constrain(value);
        self.update_value_text();
        self
    }

    /// Set step value (builder pattern). INV-B2: Step is declarative.
    pub fn with_step(mut self, step: i64) -> Self {
        self.domain.step = step;
        self
    }

    /// Set minimum bound (builder pattern).
    pub fn with_min(mut self, min: i64) -> Self {
        self.domain.min = Some(min);
        self.state.value = self.domain.constrain(self.state.value);
        self.update_value_text();
        self
    }

    /// Set maximum bound (builder pattern).
    pub fn with_max(mut self, max: i64) -> Self {
        self.domain.max = Some(max);
        self.state.value = self.domain.constrain(self.state.value);
        self.update_value_text();
        self
    }

    /// SPEC-HAL-208: Set label (builder pattern). Visual only (INV-D2).
    pub fn with_label(mut self, text: impl Into<String>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Set the value externally. Clamped to domain bounds (INV-B3).
    pub fn set_value(&mut self, value: i64) {
        self.state.value = self.domain.constrain(value);
        self.update_value_text();
    }

    /// Set disabled state. Visual only — affects colors, not logic (INV-F1).
    pub fn set_disabled(&mut self, disabled: bool) {
        self.state.disabled = disabled;
    }

    /// Get current value.
    pub fn value(&self) -> i64 {
        self.state.value
    }

    /// Set the value changed callback (builder pattern).
    pub fn on_value_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(i64) + 'static,
    {
        self.on_value_changed = Some(Box::new(callback));
        self
    }

    /// SPEC-HAL-207 INV-G1: Configure binding for change store emission.
    pub fn set_binding(&mut self, key: &str, writer: ChangeWriter) {
        let binding_key = writer.intern(key);
        self.binding = Some((binding_key, writer));
    }

    /// SPEC-HAL-260A §6.3: set the callback fired on successful commit of
    /// an edit cycle (direct-edit Enter, or `force_commit` while in
    /// editing mode). Argument: the final clamped value.
    pub fn set_on_commit<F: Fn(i64) + 'static>(&mut self, f: F) {
        self.on_commit = Some(Box::new(f));
    }

    /// SPEC-HAL-260A §6.3: set the callback fired when the user cancels
    /// direct-edit via Escape.
    pub fn set_on_cancel<F: Fn() + 'static>(&mut self, f: F) {
        self.on_cancel = Some(Box::new(f));
    }

    /// SPEC-HAL-260A §6.3 (blur_disposition = Commit): force a commit of
    /// the current state regardless of editing mode. If currently in
    /// direct-edit mode, runs the same logic as Enter (parse the field
    /// text, clamp, update state, fire `on_value_changed` and `on_commit`).
    /// Returns the final value. Consumed by PropertyEditor during cell
    /// teardown so PE blur / cell-to-cell move commits typed-but-not-Enter
    /// values per policy.

    /// Get immutable reference to state.
    pub fn state(&self) -> &SpinBoxState {
        &self.state
    }

    /// Process a semantic event through the reducer.
    fn on_semantic_event(&mut self, event: SpinBoxEvent) -> Option<i64> {
        let result = reduce_spin_box(&mut self.state, event, &self.domain);

        if let Some(new_value) = result {
            self.update_value_text();
            if let Some(callback) = &mut self.on_value_changed {
                callback(new_value);
            }
            // INV-G1: Emit via ChangeWriter::set_f32 (only available typed channel for numeric).
            if let Some((key, writer)) = &self.binding {
                writer.set_f32(*key, new_value as f32);
            }
        }

        result
    }

    /// Update the cached value text from current state. INV-H2: reuses buffer.
    fn update_value_text(&mut self) {
        self.value_text.clear();
        use std::fmt::Write;
        let _ = write!(self.value_text, "{}", self.state.value);
    }

    /// Enter direct-edit mode: TextField becomes active with current value selected.
    ///
    /// The pending Capture-entry notification must not seed the same edit twice.
    fn enter_edit_mode(&mut self, cx: &mut EventContext) {
        if self.editing {
            return;
        }
        self.editing = true;
        self.edit_start_value = self.state.value;
        cx.request_keyboard_capture();
        if cx
            .request_composite_edit(
                CompositeEditOperation::SeedForEntry {
                    text: self.value_text.clone(),
                    select_all: true,
                },
                CompositeHostContinuation::Spin(CompositeSpinContinuation::Entry),
            )
            .is_err()
        {
            self.editing = false;
        }
        cx.invalidate_layout();
    }

    /// Commit edit: parse text, clamp, update value, fire callbacks.
    fn commit_edit_text(&mut self, text: &str) {
        self.editing = false;

        if let Ok(parsed) = text.trim().parse::<i64>() {
            let clamped = self.domain.constrain(parsed);
            if clamped != self.state.value {
                self.state.value = clamped;
                self.update_value_text();
                if let Some(cb) = &mut self.on_value_changed {
                    cb(self.state.value);
                }
                if let Some((key, writer)) = &self.binding {
                    writer.set_f32(*key, self.state.value as f32);
                }
            }
            // SPEC-HAL-260A §6.3: fire on_commit on parse success regardless
            // of whether the value changed. User's Enter is explicit commit
            // intent; PropertyEditor wants the signal even for no-op commits
            // (typed-the-same-value-and-pressed-Enter).
            if let Some(cb) = self.on_commit.as_ref() {
                cb(self.state.value);
            }
        }
    }

    /// Cancel edit: revert to value at edit start.
    fn cancel_edit(&mut self) {
        self.editing = false;
        self.state.value = self.edit_start_value;
        self.update_value_text();
    }

    /// Determine which hit region a local X coordinate falls in.
    /// Returns None for label area (INV-D2: label is non-interactive).
    fn hit_region(&self, local_x: f32) -> Option<HitRegion> {
        if local_x < 0.0 {
            return None;
        }
        if local_x < self.cached_button_width {
            Some(HitRegion::Decrement)
        } else if local_x >= self.cached_control_width - self.cached_button_width
            && local_x < self.cached_control_width
        {
            Some(HitRegion::Increment)
        } else if local_x < self.cached_control_width {
            Some(HitRegion::Value)
        } else {
            None
        }
    }
}

impl Default for SpinBox {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for SpinBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SpinBox {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.spin_box;
        let integrated = cx.cell_editor_presentation()
            == crate::core::appearance::CellEditorPresentation::CellIntegrated;
        let control_budget = if integrated && constraints.max_width.is_finite() {
            let label = self.label.as_ref().map_or(0.0, |text| {
                cx.text_engine
                    .measure(text, cx.theme.typography.font_size)
                    .0
                    + geom.label_gap
            });
            Some((constraints.max_width - label).max(0.0))
        } else {
            None
        };
        let button_width = control_budget.map_or(geom.button_width, |width| {
            geom.button_width.min(width * 0.5)
        });
        let height_font_size = if self.label.is_some() {
            cx.theme
                .typography
                .input_font_size
                .max(cx.theme.typography.font_size)
        } else {
            cx.theme.typography.input_font_size
        };
        let metrics = cx.text_engine.font_metrics(height_font_size);
        let line_height = crate::core::text_engine::compute_control_line_height(
            cx.theme.typography.line_height,
            &metrics,
            cx.theme.typography.baseline_offset,
        );
        let height = geom
            .height
            .max(line_height + 2.0 * cx.theme.geometry.text_field.padding_y);

        // Content-aware value area: fits the widest formatted value in the
        // [min..max] range so a SpinBox in a tight Flex row (grow=0) doesn't
        // clip its own digits. Falls back to the theme value_width and a
        // "999" floor so an unbounded SpinBox stays usable.
        let font_size = cx.theme.typography.input_font_size;
        let padding_x = cx.theme.geometry.text_field.padding_x;
        let widest_value_text = match (self.domain.min, self.domain.max) {
            (Some(lo), Some(hi)) => {
                let lo_text = format!("{}", lo);
                let hi_text = format!("{}", hi);
                if hi_text.len() >= lo_text.len() {
                    hi_text
                } else {
                    lo_text
                }
            }
            (Some(v), None) | (None, Some(v)) => format!("{}", v),
            (None, None) => self.value_text.clone(),
        };
        let widest_text_w = if widest_value_text.is_empty() {
            0.0
        } else {
            cx.text_engine.measure(&widest_value_text, font_size).0
        };
        let floor_w = cx.text_engine.measure("999", font_size).0;
        let natural_value_width = (widest_text_w + padding_x * 2.0)
            .max(floor_w + padding_x * 2.0)
            .max(geom.value_width);
        let value_width = control_budget.map_or(natural_value_width, |width| {
            (width - 2.0 * button_width).max(0.0)
        });

        let control_width = button_width + value_width + button_width;

        // Cache geometry for handle_event (no theme access in EventContext).
        self.cached_button_width = button_width;
        self.cached_control_width = control_width;
        self.cached_value_width = value_width;
        self.cached_font_size = font_size;
        self.cached_text_field_padding_x = padding_x;

        let mut total_width = control_width;

        // Account for label
        if let Some(label_text) = &self.label {
            let font_size = cx.theme.typography.font_size;
            let (text_width, _) = cx.text_engine.measure(label_text, font_size);
            total_width += geom.label_gap + text_width;
        }

        let width = total_width.min(constraints.max_width);
        let height = height.clamp(constraints.min_height, constraints.max_height);
        self.layout_bounds = Size::new(width, height);

        let tf_constraints = Constraints::tight(Size::new(value_width, height));
        self.editing_field
            .layout(cx, tf_constraints, button_width, 0.0);

        LayoutResult::fit(self.layout_bounds)
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
            if let Some(handle) = self.focus_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.focus_t.set(value);
                } else {
                    self.focus_anim.set(None);
                }
            }
        }

        let theme = cx.theme;
        let geom = &theme.geometry.spin_box;

        // SPEC-PM-009: all paint geometry read from the layout cache.
        // geom.* is only the theme input; layout owns the final values.
        let button_width = self.cached_button_width;
        let value_width = self.cached_value_width;
        let control_width = self.cached_control_width;

        // SPEC-HAL-093 v3: per-coordinate snap to physical pixel grid;
        // 1-physical-pixel zone borders.
        let scale = cx.scale_factor;
        let one_px = cx.one_px();
        let bx = snap_to_physical(rect.x, scale);
        let by = snap_to_physical(rect.y, scale);
        let br = snap_to_physical(rect.x + control_width, scale);
        let bb = snap_to_physical(rect.y + rect.height, scale);
        let bh = bb - by;

        let disabled = self.state.disabled;
        let field_state = if disabled {
            ControlChromeState::Disabled
        } else if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let field_style = theme.input_chrome(field_state);
        let field_chrome = field_style.chrome;
        let button_chrome = theme.control_chrome(
            ControlChromeCategory::RaisedClickable,
            if disabled {
                ControlChromeState::Disabled
            } else {
                ControlChromeState::Idle
            },
            ControlValidationState::Neutral,
        );

        // Zone geometry (already have button_width, value_width from geom)
        let dec_x = bx;
        let val_x = snap_to_physical(rect.x + button_width, scale);
        let inc_x = snap_to_physical(rect.x + button_width + value_width, scale);
        let dec_w = val_x - dec_x;
        let inc_w = br - inc_x;

        // Keep button frames outside the canonical numeric input surface.
        let border_color = theme
            .control_chrome(
                ControlChromeCategory::RecessedEditable,
                field_state,
                ControlValidationState::Neutral,
            )
            .border_color;
        let cell_integrated = cx.cell_editor_presentation()
            == crate::core::appearance::CellEditorPresentation::CellIntegrated;
        if cell_integrated {
            cx.scene.begin_clip(rect, None);
            cx.scene.push_rect(
                rect,
                field_chrome.fill.unwrap_or(theme.colors.surfaces.content),
                2,
                None,
            );
        }
        if !cell_integrated {
            // The value TextField paints the two internal seams. Button chrome
            // owns only the exterior frame; closed button borders would paint
            // a second vertical edge beside each field edge.
            for (x, width) in [(dec_x, dec_w), (inc_x, inc_w)] {
                cx.scene
                    .push_rect(RectF::new(x, by, width, one_px), border_color, 3, None);
                cx.scene.push_rect(
                    RectF::new(x, bb - one_px, width, one_px),
                    border_color,
                    3,
                    None,
                );
            }
            for x in [bx, br - one_px] {
                cx.scene.push_rect(
                    RectF::new(x, by + one_px, one_px, bh - 2.0 * one_px),
                    border_color,
                    3,
                    None,
                );
            }
        }

        // --- Zone backgrounds ---
        let dec_bg = button_chrome
            .fill
            .unwrap_or(theme.colors.controls.background);
        let inc_bg = button_chrome
            .fill
            .unwrap_or(theme.colors.controls.background);
        cx.scene.push_rect(
            RectF::new(bx + one_px, by + one_px, dec_w - one_px, bh - 2.0 * one_px),
            dec_bg,
            2,
            None,
        );
        cx.scene.push_rect(
            RectF::new(inc_x, by + one_px, inc_w - one_px, bh - 2.0 * one_px),
            inc_bg,
            2,
            None,
        );

        crate::widgets::text_field::paint_input_surface(
            cx,
            RectF::new(rect.x + button_width, rect.y, value_width, rect.height),
            &field_style,
            matches!(
                field_state,
                ControlChromeState::Focused | ControlChromeState::Captured
            ),
        );

        // SPEC-HAL-328: Focus ring is painted on the embedded value field only
        // (see cx.paint_focus_ring(field_rect, ...) below), not on the full
        // widget — the +/- buttons are not part of the focus target.

        // Pressed button highlight
        if let Some(region) = self.pressed_region {
            let pressed_bg = theme
                .control_chrome(
                    ControlChromeCategory::RaisedClickable,
                    ControlChromeState::Pressed,
                    ControlValidationState::Neutral,
                )
                .fill
                .unwrap_or(theme.colors.controls.background_pressed);
            match region {
                HitRegion::Decrement => {
                    let r = RectF::new(dec_x, by, dec_w, bh);
                    cx.scene.push_rect(r, pressed_bg, 4, None);
                }
                HitRegion::Increment => {
                    let r = RectF::new(inc_x, by, inc_w, bh);
                    cx.scene.push_rect(r, pressed_bg, 4, None);
                }
                HitRegion::Value => {}
            }
        }

        // Symbol colors
        let sym_color = if self.state.disabled {
            theme.colors.typography.muted
        } else {
            theme.colors.typography.primary
        };
        let sym_width = button_width * 0.4;
        let sym_thickness = 2.0_f32;

        // Minus symbol in decrement button (centered)
        let dec_center_x = rect.x + button_width * 0.5;
        let dec_center_y = rect.y + rect.height * 0.5;
        cx.scene.push_rect(
            RectF::new(
                dec_center_x - sym_width * 0.5,
                dec_center_y - sym_thickness * 0.5,
                sym_width,
                sym_thickness,
            ),
            sym_color,
            6,
            None,
        );

        // Plus symbol in increment button (centered)
        let inc_center_x = rect.x + button_width + value_width + button_width * 0.5;
        let inc_center_y = rect.y + rect.height * 0.5;
        // Horizontal bar
        cx.scene.push_rect(
            RectF::new(
                inc_center_x - sym_width * 0.5,
                inc_center_y - sym_thickness * 0.5,
                sym_width,
                sym_thickness,
            ),
            sym_color,
            6,
            None,
        );
        // Vertical bar
        cx.scene.push_rect(
            RectF::new(
                inc_center_x - sym_thickness * 0.5,
                inc_center_y - sym_width * 0.5,
                sym_thickness,
                sym_width,
            ),
            sym_color,
            6,
            None,
        );

        // Value text (centered in value region) — or embedded TextField when editing.
        let font_size = theme.typography.input_font_size;
        let metrics = cx.text_engine.font_metrics(font_size);
        let baseline_y = rect.y
            + compute_control_text_baseline_y(
                rect.height,
                metrics.ascent,
                metrics.descent,
                theme.typography.baseline_offset,
            );
        let value_region_x = rect.x + button_width;
        // Focus ring on the value field area only, not full widget
        let field_rect = RectF::new(value_region_x, rect.y, value_width, rect.height);
        let field_radius = theme.chrome_radius(
            ControlChromeCategory::RecessedEditable,
            theme.geometry.text_field.radius,
            false,
        );

        if !self.editing {
            let run = cx.text_engine.shape(&self.value_text, font_size);
            let (text_w, _) = cx.text_engine.measure(&self.value_text, font_size);
            let text_y = baseline_y - metrics.ascent;
            let text_x = value_region_x + (value_width - text_w) * 0.5;
            let value_color = if self.state.disabled {
                theme.colors.typography.muted
            } else {
                theme.colors.typography.primary
            };
            cx.text_engine.draw(
                &run,
                text_x,
                text_y,
                value_color,
                SPIN_BOX_TEXT_LAYER,
                cx.scene,
            );
        }

        // SPEC-HAL-208: Paint label text after control
        if let Some(label_text) = &self.label {
            let font_size = theme.typography.font_size;
            let metrics = cx.text_engine.font_metrics(font_size);
            let baseline_y = rect.y
                + compute_control_text_baseline_y(
                    rect.height,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                );
            let label_x = rect.x + control_width + geom.label_gap;
            let run = cx.text_engine.shape(label_text, font_size);
            let label_y = baseline_y - metrics.ascent;
            let label_color = if self.state.disabled {
                theme.colors.typography.muted
            } else {
                theme.colors.typography.primary
            };
            cx.text_engine.draw(
                &run,
                label_x,
                label_y,
                label_color,
                SPIN_BOX_TEXT_LAYER,
                cx.scene,
            );
        }

        // Focus ring on the value field area only, not full widget
        if !cell_integrated
            && !matches!(
                field_state,
                ControlChromeState::Focused | ControlChromeState::Captured
            )
        {
            let focus_t = self.focus_t.get();
            if focus_t > 0.001 {
                cx.scene.push_border_rounded_rect(
                    field_rect,
                    field_chrome.focus_color.with_alpha(focus_t),
                    field_chrome.focus_width,
                    crate::core::paint::RoundedCorners::uniform(field_radius),
                    crate::core::paint::LAYER_FOCUS + 1,
                    None,
                );
            }
        }
        if cell_integrated {
            cx.scene.end_clip();
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

        // INV-F1: Disabled is absolute.
        if self.state.disabled {
            return false;
        }

        if let Event::Pointer(pe) = event {
            if self.pressed_region.is_none()
                && pe.local_pos.1 >= 0.0
                && pe.local_pos.1 < self.layout_bounds.height
                && pe.local_pos.0 < self.layout_bounds.width
                && self.hit_region(pe.local_pos.0) == Some(HitRegion::Value)
            {
                cx.request_cursor(crate::core::event::CursorIcon::Text);
            }
        }

        // If editing, intercept Enter (commit) / Escape (cancel) before any other dispatch.
        if self.editing {
            if let Event::Key(ke) = event {
                if ke.is_down {
                    match ke.key_code {
                        KeyCode::Enter => {
                            let _ = cx.request_composite_edit(
                                CompositeEditOperation::ObserveAndActivity,
                                CompositeHostContinuation::Spin(
                                    CompositeSpinContinuation::Commit { release: true },
                                ),
                            );
                            return true;
                        }
                        KeyCode::Escape => {
                            self.cancel_edit();
                            // SPEC-HAL-260A §6.3: Escape during direct-edit
                            // is explicit user cancel intent. Other
                            // cancel_edit invocations (blur, capture-leave,
                            // +/- click during edit) are internal state
                            // cleanup and do NOT fire this callback.
                            if let Some(cb) = self.on_cancel.as_ref() {
                                cb();
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }
        }

        match event {
            Event::Pointer(pe) => {
                let (local_x, local_y) = pe.local_pos;
                let inside_y = local_y >= 0.0 && local_y <= self.layout_bounds.height;
                let region = if inside_y {
                    self.hit_region(local_x)
                } else {
                    None
                };

                match pe.kind {
                    // Double-click on value area enters edit mode.
                    PointerEventKind::DoubleClick
                        if region == Some(HitRegion::Value)
                            && pe.button == Some(PointerButton::Primary) =>
                    {
                        self.on_semantic_event(SpinBoxEvent::FocusGain);
                        self.enter_edit_mode(cx);
                        true
                    }
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        // INV-D3: Only button regions start press.
                        // INV-D2: Label clicks do not change value.
                        match region {
                            Some(HitRegion::Decrement) | Some(HitRegion::Increment) => {
                                // +/- buttons exit edit mode if active (no implicit commit).
                                if self.editing {
                                    self.cancel_edit();
                                }
                                self.on_semantic_event(SpinBoxEvent::FocusGain);
                                self.pressed_region = region;
                                cx.capture_pointer(PointerButton::Primary);
                                // Activate immediately on press (value is trivially reversible)
                                match region.unwrap() {
                                    HitRegion::Decrement => {
                                        self.on_semantic_event(SpinBoxEvent::Decrement);
                                    }
                                    HitRegion::Increment => {
                                        self.on_semantic_event(SpinBoxEvent::Increment);
                                    }
                                    HitRegion::Value => {}
                                }
                                true
                            }
                            Some(HitRegion::Value) => {
                                // Click on value region: focus only, no value change (INV-F2).
                                self.on_semantic_event(SpinBoxEvent::FocusGain);
                                if self.editing {
                                    let _ = self.forward_edit(cx);
                                }
                                true
                            }
                            None => false,
                        }
                    }
                    PointerEventKind::Up => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if self.pressed_region.take().is_some() {
                            true
                        } else if self.editing {
                            self.forward_edit(cx)
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Move => {
                        // During capture, track whether pointer is still in pressed region
                        // for visual feedback only. No value change on move.
                        if self.pressed_region.is_some() {
                            true
                        } else if self.editing {
                            self.forward_edit(cx)
                        } else {
                            false
                        }
                    }
                    _ => {
                        if self.editing {
                            self.forward_edit(cx)
                        } else {
                            false
                        }
                    }
                }
            }
            // INV-E1: Arrow keys explicit. Up = increment, Down = decrement.
            // INV-E2: Modifier keys inert (no shift/ctrl handling).
            Event::Key(ke) if ke.is_down => {
                // Enter / F2 on focused (not editing) → enter edit mode.
                if !self.editing
                    && self.state.focused
                    && (ke.key_code == KeyCode::Enter || ke.key_code == KeyCode::F2)
                {
                    self.enter_edit_mode(cx);
                    return true;
                }
                match ke.key_code {
                    KeyCode::Up | KeyCode::Down => {
                        if cx.keyboard_mode != KeyboardAuthorityMode::Capture {
                            return false; // Let framework handle navigation
                        }
                        // ... existing value change logic ...
                        match ke.key_code {
                            KeyCode::Up => {
                                if self.state.focused && !self.editing {
                                    self.on_semantic_event(SpinBoxEvent::Increment);
                                    return true;
                                }
                            }
                            KeyCode::Down => {
                                if self.state.focused && !self.editing {
                                    self.on_semantic_event(SpinBoxEvent::Decrement);
                                    return true;
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => {}
                }
                if self.editing {
                    self.forward_edit(cx)
                } else {
                    false
                }
            }
            _ => {
                if self.editing {
                    self.forward_edit(cx)
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

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.on_semantic_event(SpinBoxEvent::FocusGain);
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
        self.pressed_region = None;
        // Tab away during editing → value reverts (no implicit commit).
        if self.editing {
            self.cancel_edit();
        }
        let _ = cx.request_composite_edit(
            CompositeEditOperation::RetireDelegation,
            CompositeHostContinuation::Spin(CompositeSpinContinuation::Retire),
        );
        self.on_semantic_event(SpinBoxEvent::FocusLoss);
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

    fn accepts_keyboard_activation(&self) -> bool {
        false
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if self.state.disabled {
            return;
        }
        self.cell_hovered = true;
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
        self.cell_hovered = false;
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
        self.focus_t.set(0.0);
        self.focus_anim.set(None);
    }

    fn touch_target(
        &self,
        hit: crate::core::event::TouchHit,
    ) -> Option<crate::core::event::TouchTarget> {
        use crate::core::event::TouchTarget;
        if self.state.disabled || !(0.0..=self.layout_bounds.height).contains(&hit.local_position.y)
        {
            return None;
        }
        self.hit_region(hit.local_position.x)
            .map(|region| match region {
                HitRegion::Increment => TouchTarget::Increment,
                HitRegion::Decrement => TouchTarget::Decrement,
                HitRegion::Value => TouchTarget::Editor,
            })
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, tap: crate::core::event::TouchTap) -> bool {
        use crate::core::event::TouchTarget;
        match tap.target {
            TouchTarget::Editor => cx
                .request_composite_edit(
                    CompositeEditOperation::PlaceTouchCaret {
                        local_x: tap.local_position.x,
                    },
                    CompositeHostContinuation::Spin(CompositeSpinContinuation::Input),
                )
                .is_ok(),
            TouchTarget::Increment | TouchTarget::Decrement => {
                if self.editing {
                    self.cancel_edit();
                }
                self.on_semantic_event(if tap.target == TouchTarget::Increment {
                    SpinBoxEvent::Increment
                } else {
                    SpinBoxEvent::Decrement
                });
                cx.invalidate_paint();
                true
            }
            _ => false,
        }
    }

    fn capture_eligible(&self) -> bool {
        !self.state.disabled
    }

    fn on_capture_enter(&mut self, cx: &mut EventContext) {
        // SPEC-HAL-328 Category C: I/Enter enters Capture → start editing.
        self.enter_edit_mode(cx);
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        if self.editing {
            self.cancel_edit();
            cx.invalidate_layout();
        }
        let _ = cx.request_composite_edit(
            CompositeEditOperation::RetireDelegation,
            CompositeHostContinuation::Spin(CompositeSpinContinuation::Retire),
        );
    }

    fn debug_type_name(&self) -> &'static str {
        "SpinBox"
    }
}
