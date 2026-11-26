// SPEC-HAL-101: TextField Widget
// SPEC-HAL-105: Stage 1 — State Unification (Cleanup)
//
// ============================================================================
// HORIZONTAL FOCUS VISIBILITY INVARIANTS (SPEC-HAL-109 Stage H.1)
// ============================================================================
//
// INV-H.VIS1 — Focus X visibility:
//   After any cursor/selection change, focus target must be fully visible:
//   focus_x ∈ [scroll_x + margin, scroll_x + viewport_width - margin - target_w]
//
// INV-H.VIS2 — Idempotence:
//   Calling ensure repeatedly with unchanged focus target must not change scroll_x.
//
// INV-H.VIS3 — Monotonicity:
//   Focus moves right → scroll_x may only increase or stay.
//   Focus moves left → scroll_x may only decrease or stay.
//
// INV-H.VIS4 — No overscroll:
//   scroll_x ∈ [0, max(0, content_width - viewport_width)]
//
// INV-H.FOCUS1 — Single focus target:
//   selection active → selection.head
//   else → caret (cursor.pos)
//   All logic uses focus_target_content_x() exclusively.
//
// ============================================================================

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::appearance::{FrameworkAppearanceFacts, InputSurfaceAppearance};

// Above input chrome; the separate caret uses the reserved system-overlay band.
const TEXT_FIELD_TEXT_LAYER: crate::core::paint::ZIndex = LAYER_CONTENT + 3;
const TEXT_FIELD_CARET_LAYER: crate::core::paint::ZIndex = crate::core::paint::LAYER_OVERLAY;
use crate::core::editing::{
    ClusterBoundary, Selection, TextCursor, WordBoundary, find_word_at, hit_test_x_y,
};
use crate::core::editing_history::{EditKind, EditSnapshot, UndoHistory};
use crate::core::event::{
    Event, EventPhase, ImeEvent, KeyCode, KeyEvent, KeyboardAuthorityMode, PointerButton,
    PointerEventKind,
};
use crate::core::icon::IconId;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{Color, LAYER_CONTENT, RoundedCorners, TextBackgroundSpan};
use crate::core::paragraph::TextAlign;
use crate::core::text_edit_delegate as delegate;
use crate::core::text_edit_policy::{
    PolicyDecision, TextEditPolicy, TextEditingPolicy, decide_text_edit_policy,
};
use crate::core::text_engine::{
    GlyphRun, TextBinding, TextEngine, compute_control_text_baseline_y,
};
use crate::core::theme::{ControlChromeCategory, ControlChromeState};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::text_document::TextDocument;
use crate::widgets::text_layout_cache::TextLayoutCache;
use std::cell::Cell;

mod edit_projection;

const PASSWORD_MASK_TEXT: &str = "•";

/// Browser paste carries source identity, never a borrowed editor or copied text.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BrowserPasteState {
    revision: u64,
    cursor: usize,
    anchor: usize,
    head: usize,
    document_bytes: usize,
    selected_bytes: usize,
    max_len_bytes: Option<usize>,
    remaining_bytes: Option<usize>,
    normalize_paste_newlines: bool,
}

#[cfg(any(target_arch = "wasm32", test))]
impl BrowserPasteState {
    pub(crate) fn text_policy(&self) -> (Option<usize>, bool) {
        (self.remaining_bytes, self.normalize_paste_newlines)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BrowserPasteRefusal {
    Disabled,
    ReadOnly,
    CompositionActive,
    InvalidState,
    StaleSource,
    ContentRejected,
}

// ============================================================================
// TextFieldConfig
// ============================================================================

/// Configuration options for TextField behavior.
pub struct TextFieldConfig {
    pub placeholder: Option<String>,
    pub is_password: bool,
    pub select_all_on_focus: bool,
    pub commit_on_enter: bool,
    pub max_len_bytes: Option<usize>,
    pub is_read_only: bool,
    pub is_enabled: bool,
}

impl Default for TextFieldConfig {
    fn default() -> Self {
        Self {
            placeholder: None,
            is_password: false,
            select_all_on_focus: false,
            commit_on_enter: false,
            max_len_bytes: None,
            is_read_only: false,
            is_enabled: true,
        }
    }
}

/// Detached committed editor state; contains no widget, IME or input authority.
#[derive(Clone)]
pub struct TextFieldEditHandoff {
    text: String,
    cursor: TextCursor,
    selection: Selection,
}

impl std::fmt::Debug for TextFieldEditHandoff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TextFieldEditHandoff { .. }")
    }
}

impl TextFieldEditHandoff {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor_byte(&self) -> usize {
        self.cursor.pos
    }

    pub fn selection_anchor_byte(&self) -> usize {
        self.selection.anchor
    }

    pub fn selection_head_byte(&self) -> usize {
        self.selection.head
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldHandoffError {
    CompositionActive,
    InvalidBoundary,
    LimitExceeded,
    InputRejected,
    ShapingUnavailable,
    UnsupportedTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldHandoffStatus {
    NotRequested,
    Pending,
    Applied,
    Rejected(TextFieldHandoffError),
}

// ============================================================================
// TextFieldDragMode
// ============================================================================

/// Drag mode for pointer-based selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextFieldDragMode {
    None,
    Character { anchor: usize },
    Word { initial: WordBoundary },
    Line { anchor_line: usize },
}

impl Default for TextFieldDragMode {
    fn default() -> Self {
        Self::None
    }
}

// ============================================================================
// TextFieldState
// ============================================================================

/// Unified editing state for TextField widget.
/// Contains only editing substrate state (document, cursor, selection, cache, scroll, drag, undo).
pub struct TextFieldState {
    pub document: TextDocument,
    pub cursor: TextCursor,
    pub selection: Selection,
    pub layout_cache: TextLayoutCache,
    pub scroll_x: f32, // SPEC-HAL-141: Layout-owned, no Cell (mutated only in layout/event phase)
    pub drag_mode: TextFieldDragMode,
    /// SPEC-HAL-055: Undo/redo history
    pub undo_history: UndoHistory,
    /// SPEC-HAL-055 Stage 1.1: Snapshot before IME composition started (for atomic undo)
    pub ime_pre_composition_snapshot: Option<EditSnapshot>,
}

impl TextFieldState {
    pub fn new() -> Self {
        Self {
            document: TextDocument::new(),
            cursor: TextCursor::new(),
            selection: Selection::new(),
            layout_cache: TextLayoutCache::default(),
            scroll_x: 0.0,
            drag_mode: TextFieldDragMode::None,
            undo_history: UndoHistory::new(),
            ime_pre_composition_snapshot: None,
        }
    }

    /// SPEC-HAL-055 Stage 1.1: Check if IME composition is active.
    pub fn is_composing(&self) -> bool {
        self.ime_pre_composition_snapshot.is_some()
    }

    /// SPEC-HAL-055 Stage 1.1: Clear IME composition state.
    pub fn clear_composition(&mut self) {
        self.ime_pre_composition_snapshot = None;
    }
}

impl Default for TextFieldState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TextField
// ============================================================================

/// Single-line editable text widget.
///
/// SPEC-HAL-105 Stage 1: State unification complete.
/// Uses TextDocument, TextCursor, Selection from editing substrate.
pub struct TextField {
    id: WidgetInternalId,
    state: TextFieldState,
    pending_edit_handoff: Option<TextFieldEditHandoff>,
    edit_handoff_status: TextFieldHandoffStatus,
    edit_handoff_mounted: bool,
    focused: bool,
    caret_animation_demand: Option<bool>,
    smooth_caret_enabled: bool,
    smooth_caret: SmoothCaret,
    hover: bool,
    /// SPEC-HAL-159: Visual-only blink state. Mutated during paint with AllowPaintMutationGuard.
    blink_last_ms: Cell<u64>,
    /// SPEC-HAL-159: Visual-only blink state. Mutated during paint with AllowPaintMutationGuard.
    blink_caret_visible: Cell<bool>,
    config: TextFieldConfig,
    // SPEC-HAL-109 Stage F: Single-line vs multi-line editing policy
    editing_policy: TextEditingPolicy,
    // SPEC-HAL-107: Layout dirty flag - set by editing events, cleared by layout()
    layout_dirty: Cell<bool>,
    // Cached metrics for paint (computed in layout)
    ascent: f32,
    descent: f32,
    // Cached geometry from theme (synced in layout for event handler use)
    padding_x: f32,
    // INV-TEXT-GEOM-PHASE: Last layout width for synchronous rebuild
    last_layout_width: f32,
    // When true, skip painting background/border (hosted inside ComboBox or similar)
    embedded: bool,
    /// Construction-time selected standalone chrome; never an editing policy.
    input_surface_appearance: Option<InputSurfaceAppearance>,
    leading_icon: Option<IconId>,
    leading_icon_size: Option<f32>,
    leading_icon_y_offset: f32,
    /// Layout-published icon plus content gap, consumed by paint and hit testing.
    leading_width: f32,
    /// A real, protected WidgetTree child; never an independently focused editor.
    composite_role: bool,
    composite_input_palette: bool,
    composite_delegation: Option<crate::core::composite_text_edit::CompositeDelegationFacts>,
    /// Called when Enter is pressed during editing.
    on_submit: Option<Box<dyn FnMut(&str)>>,
    /// Called when Escape is pressed during editing.
    on_cancel: Option<Box<dyn FnMut()>>,
    /// When true, TextField enters capture mode on first layout.
    /// Used by PropertyEditor to auto-activate on mount.
    start_captured: bool,
    /// Character filter. When set, only characters passing this
    /// filter are inserted. Others are silently consumed.
    input_filter: Option<Box<dyn Fn(char) -> bool>>,
    /// Called after every text mutation (typing, delete, paste, undo).
    /// Receives the current text content.
    on_change: Option<Box<dyn FnMut(&str)>>,
    /// When true, on_capture_enter selects all text.
    /// Used by filter TextFields where re-entering the field
    /// should select existing text for easy overwrite.
    select_all_on_capture: bool,
    /// After on_submit fires, redirect focus to this widget.
    on_submit_redirect: Option<TreeWidgetId>,
    /// After on_cancel/on_capture_escape fires, redirect focus to this widget.
    on_escape_redirect: Option<TreeWidgetId>,
    /// Text alignment within the viewport. Default: Left.
    /// Center and Right only apply when text is shorter than
    /// the viewport. When text overflows (scrolling active),
    /// alignment is ignored — text is always left-anchored
    /// with scroll offset.
    text_align: TextAlign,
    /// Cached paint rect width for hit-test alignment in handle_event.
    cached_paint_rect_width: Cell<f32>,
    /// Width of one password mask slot at the current font size.
    /// Paint normalizes it to a whole physical-pixel advance so every mask
    /// glyph is realized at the same horizontal subpixel phase.
    password_mask_advance: Cell<f32>,
    /// Scalar text-owner font identity, qualified during active password Layout.
    password_mask_font_id: Option<u64>,
    /// Set true before `on_submit` fires and `release_keyboard_capture` is
    /// called from the Enter handler. Read by `on_capture_leave` to skip
    /// firing `on_cancel` on the commit path (avoiding double-dispatch).
    /// Cleared at the end of `on_capture_leave`. Same shape as ComboBox's
    /// `committed_latch` guard. SPEC-HAL-328 §3.2: capture release is the
    /// framework's domain; this flag only disambiguates the cause for the
    /// consumer-facing callback.
    commit_pending: bool,
}

// HAL-101 §8.4.1: scalar presentation only; editing never reads these values.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SmoothCaretDestination {
    revision: u64,
    cursor: usize,
    x: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SmoothCaretSpace {
    geometry: crate::core::widget::OwnedWidgetPresentationGeometry,
    facts: FrameworkAppearanceFacts,
    delegation: Option<(u64, u64, u64)>,
    metrics: [f32; 10],
    leading_width: f32,
    mask_advance: Option<f32>,
    mask_font_id: Option<u64>,
    typography_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SmoothCaretProjection {
    destination: SmoothCaretDestination,
    space: SmoothCaretSpace,
    x: f32,
}

#[derive(Default)]
struct SmoothCaret {
    handle: Option<TransitionHandle>,
    projection: Option<SmoothCaretProjection>,
    pending: Option<SmoothCaretDestination>,
    snap: bool,
    boundary: Option<CaretBoundary>,
    mask_boundary: Option<PasswordMaskBoundary>,
    range_origin: Option<SmoothCaretProjection>,
    projection_refusal: Option<edit_projection::Refusal>,
}

// Password presentation never carries a document, String, TextBinding or glyph run.
#[derive(Clone, Copy)]
struct PasswordMaskOrigin {
    destination: SmoothCaretDestination,
    count: usize,
    previous_byte: usize,
    advance: f32,
}

#[derive(Clone, Copy)]
struct PasswordMaskBoundary {
    kind: CaretBoundaryKind,
    origin: SmoothCaretDestination,
    destination: SmoothCaretDestination,
    old_count: usize,
    new_count: usize,
    advance: f32,
    pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaretBoundaryKind {
    Reveal,
    Conceal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaretRangeKind {
    WordDelete,
    SelectionDelete,
    Paste,
}

#[derive(Clone)]
struct CaretRange {
    kind: CaretRangeKind,
    old: std::ops::Range<usize>,
    new: std::ops::Range<usize>,
}

struct GeneralCaretEdit {
    old: std::ops::Range<usize>,
    new: std::ops::Range<usize>,
    progress: f32,
    travel: f32,
    old_width: f32,
    new_width: f32,
    previous_width: Option<f32>,
    sample_is_caret: bool,
}

// HAL-101 §8.4.2: owned source and scalar intent only, never shaped output.
struct CaretBoundary {
    kind: CaretBoundaryKind,
    old_source: Option<String>,
    source_revision: u64,
    origin: SmoothCaretDestination,
    destination: SmoothCaretDestination,
    binding: TextBinding,
    metrics: [f32; 4],
    pending: bool,
    range: Option<CaretRange>,
    source_caret_x: f32,
    general: Option<GeneralCaretEdit>,
}

impl TextField {
    pub(crate) fn cell_editor_visual_facts(&self) -> crate::core::appearance::CellEditorVisualFacts {
        crate::core::appearance::CellEditorVisualFacts {
            disabled: !self.config.is_enabled,
            hovered: self.hover,
        }
    }

    /// Select initial policy before mounting. Runtime changes use WidgetTree requests.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.config.is_enabled = enabled;
        self
    }

    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.config.is_read_only = read_only;
        self
    }

    /// Select this standalone field's checked gradient and corner geometry.
    ///
    /// Default chrome is unchanged when absent. Qualified cell and composite
    /// hosts retain their own exterior presentation; this does not skin them.
    /// Focus/Capture cues, content padding and all editing semantics are unchanged.
    pub fn with_input_surface_appearance(mut self, value: InputSurfaceAppearance) -> Self {
        self.input_surface_appearance = Some(value);
        self
    }

    /// Place a registered icon inside this field's existing input shell.
    /// It is content, not another widget or input target (HAL101/HAL012).
    pub fn with_leading_icon(mut self, icon: IconId) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// Request a leading icon size in logical pixels before mounting.
    ///
    /// The icon defaults to the input font size. Layout fits the request
    /// within the field's padded bounds and reserves room for the caret.
    ///
    /// # Panics
    /// Panics when `logical_px` is non-finite or non-positive.
    pub fn with_leading_icon_size(mut self, logical_px: f32) -> Self {
        assert!(
            logical_px.is_finite() && logical_px > 0.0,
            "leading icon size must be finite and positive"
        );
        self.leading_icon_size = Some(logical_px);
        self
    }

    /// Offset only the leading icon's paint in logical pixels. Negative moves up.
    /// The offset defaults to zero and fits within the existing icon's vertical clearance.
    ///
    /// # Panics
    /// Panics when `logical_px` is non-finite.
    pub fn with_leading_icon_y_offset(mut self, logical_px: f32) -> Self {
        assert!(logical_px.is_finite(), "leading icon offset must be finite");
        self.leading_icon_y_offset = logical_px;
        self
    }

    fn leading_width_for(
        &self,
        width: f32,
        height: f32,
        padding_x: f32,
        padding_y: f32,
        font_size: f32,
        caret: f32,
    ) -> f32 {
        if self.leading_icon.is_none() {
            return 0.0;
        }
        let size = self
            .leading_icon_size
            .unwrap_or(font_size)
            .min((height - padding_y * 2.0).max(0.0))
            .min((width - padding_x * 3.0 - caret).max(0.0));
        if size > 0.0 { size + padding_x } else { 0.0 }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled
    }

    pub fn is_read_only(&self) -> bool {
        self.config.is_read_only
    }

    pub(crate) fn apply_availability_policy(&mut self, enabled: bool, read_only: bool) {
        self.snap_smooth_caret();
        if !enabled || (!self.config.is_read_only && read_only) {
            delegate::handle_ime_cancel(
                &mut self.state.ime_pre_composition_snapshot,
                &mut self.state.undo_history,
            );
            self.state.clear_composition();
        }
        self.config.is_enabled = enabled;
        self.config.is_read_only = read_only;
        if !enabled {
            self.state.drag_mode = TextFieldDragMode::None;
            self.blink_caret_visible.set(false);
            self.caret_animation_demand = Some(false);
        }
    }

    /// Enable qualified TextField motion, or select plain presentation with false.
    /// Ordinary fields default to enabled; password motion uses only qualified masks.
    pub fn with_smooth_caret_prototype(mut self, enabled: bool) -> Self {
        self.set_smooth_caret_prototype(enabled);
        self
    }

    pub fn smooth_caret_prototype_enabled(&self) -> bool {
        self.smooth_caret_enabled
    }

    /// Reconciliation and Timeline cancellation occur at the next ordinary Update.
    pub fn set_smooth_caret_prototype(&mut self, enabled: bool) {
        if self.smooth_caret_enabled != enabled {
            self.smooth_caret_enabled = enabled;
            self.snap_smooth_caret();
        }
    }

    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: TextFieldState::new(),
            pending_edit_handoff: None,
            edit_handoff_status: TextFieldHandoffStatus::NotRequested,
            edit_handoff_mounted: false,
            focused: false,
            caret_animation_demand: None,
            smooth_caret_enabled: true,
            smooth_caret: SmoothCaret::default(),
            hover: false,
            blink_last_ms: Cell::new(0),
            blink_caret_visible: Cell::new(false),
            config: TextFieldConfig::default(),
            editing_policy: TextEditingPolicy::single_line(),
            layout_dirty: Cell::new(true),
            ascent: 0.0,
            descent: 0.0,
            padding_x: 6.0, // Default Regular density padding_x, synced from theme in layout()
            last_layout_width: Self::SINGLE_LINE_LAYOUT_WIDTH, // INV-TEXT-GEOM-PHASE
            embedded: false,
            input_surface_appearance: None,
            leading_icon: None,
            leading_icon_size: None,
            leading_icon_y_offset: 0.0,
            leading_width: 0.0,
            composite_role: false,
            composite_input_palette: false,
            composite_delegation: None,
            on_submit: None,
            on_cancel: None,
            start_captured: false,
            input_filter: None,
            on_change: None,
            select_all_on_capture: false,
            on_submit_redirect: None,
            on_escape_redirect: None,
            text_align: TextAlign::Left,
            cached_paint_rect_width: Cell::new(0.0),
            password_mask_advance: Cell::new(0.0),
            password_mask_font_id: None,
            commit_pending: false,
        }
    }

    /// Capture committed text and the full native cursor/ordered selection.
    /// Active IME preedit is never converted into a committed draft.
    pub fn capture_edit_handoff(&self) -> Result<TextFieldEditHandoff, TextFieldHandoffError> {
        if self.state.is_composing() {
            return Err(TextFieldHandoffError::CompositionActive);
        }
        Ok(TextFieldEditHandoff {
            text: self.state.document.text().to_owned(),
            cursor: self.state.cursor,
            selection: self.state.selection.clone(),
        })
    }

    /// Stage only on a fresh, unmounted ordinary field. Success is inert Pending;
    /// the engine validates genuine target shaping at its pre-Layout owner cut.
    /// A rejected staging attempt leaves this field and any prior request intact.
    pub fn try_stage_edit_handoff(
        &mut self,
        handoff: &TextFieldEditHandoff,
    ) -> Result<(), TextFieldHandoffError> {
        if self.edit_handoff_mounted
            || self.edit_handoff_status != TextFieldHandoffStatus::NotRequested
        {
            return Err(TextFieldHandoffError::UnsupportedTarget);
        }
        self.validate_edit_handoff(handoff)?;
        self.pending_edit_handoff = Some(handoff.clone());
        self.edit_handoff_status = TextFieldHandoffStatus::Pending;
        Ok(())
    }

    pub fn edit_handoff_status(&self) -> TextFieldHandoffStatus {
        self.edit_handoff_status
    }

    pub(crate) fn has_pending_edit_handoff(&self) -> bool {
        self.pending_edit_handoff.is_some()
    }

    fn validate_edit_handoff(
        &self,
        handoff: &TextFieldEditHandoff,
    ) -> Result<(), TextFieldHandoffError> {
        use TextFieldHandoffError as Error;
        if self.state.is_composing() {
            return Err(Error::CompositionActive);
        }
        if self.embedded
            || self.composite_role
            || self.config.is_password
            || !self.editing_policy.single_line
            || self.start_captured
            || self.config.select_all_on_focus
            || self.select_all_on_capture
            || self.state.document.version() != 0
            || !self.state.document.is_empty()
            || self.state.cursor != TextCursor::new()
            || self.state.selection != Selection::new()
        {
            return Err(Error::UnsupportedTarget);
        }
        if [
            handoff.cursor.pos,
            handoff.selection.anchor,
            handoff.selection.head,
        ]
        .into_iter()
        .any(|pos| pos > handoff.text.len() || !handoff.text.is_char_boundary(pos))
        {
            return Err(Error::InvalidBoundary);
        }
        if self
            .config
            .max_len_bytes
            .is_some_and(|max| handoff.text.len() > max)
        {
            return Err(Error::LimitExceeded);
        }
        if self
            .input_filter
            .as_ref()
            .is_some_and(|filter| handoff.text.chars().any(|ch| !filter(ch)))
        {
            return Err(Error::InputRejected);
        }
        Ok(())
    }

    fn validate_edit_handoff_shaping(
        handoff: &TextFieldEditHandoff,
        text_engine: &mut dyn TextEngine,
        theme: &crate::core::theme::Theme,
    ) -> Result<(), TextFieldHandoffError> {
        use TextFieldHandoffError as Error;
        let text = handoff.text.as_str();
        let font_size = theme.typography.input_font_size;
        if !font_size.is_finite() || font_size <= 0.0
            || (text_engine.font_face_catalog().is_none() && text_engine.shaper().is_none()) {
            return Err(Error::ShapingUnavailable);
        }
        let mut cache = TextLayoutCache::new(font_size, theme.colors.typography.primary);
        cache.try_rebuild(text, 0, Self::SINGLE_LINE_LAYOUT_WIDTH, text_engine)
            .map_err(|_| Error::ShapingUnavailable)?;

        // A fresh/sentinel-only cache is not proof of a shaped nonempty string.
        // Run ranges may be in visual order, so qualify their source coverage.
        let mut ranges = Vec::new();
        for run in cache.lines().iter().flat_map(|line| &line.runs) {
            if run.range.start >= run.range.end
                || run.range.end > text.len()
                || !text.is_char_boundary(run.range.start)
                || !text.is_char_boundary(run.range.end)
                || run.glyphs.is_empty()
                || run.glyphs.iter().any(|glyph| {
                    run.range
                        .start
                        .checked_add(glyph.cluster as usize)
                        .is_none_or(|pos| pos >= run.range.end || !text.is_char_boundary(pos))
                })
            {
                return Err(Error::ShapingUnavailable);
            }
            ranges.push(run.range.clone());
        }
        ranges.sort_unstable_by_key(|range| range.start);
        let mut end = 0;
        for range in ranges {
            if range.start != end {
                return Err(Error::ShapingUnavailable);
            }
            end = range.end;
        }
        if end != text.len() {
            return Err(Error::ShapingUnavailable);
        }
        let boundaries = cache.boundaries();
        if boundaries.first().is_none()
            || boundaries.last().is_none_or(|b| b.byte_start != text.len())
            || !boundaries.iter().any(|b| b.byte_start == 0)
            || boundaries.iter().any(|b| {
                b.byte_start > b.byte_end
                    || b.byte_end > text.len()
                    || !text.is_char_boundary(b.byte_start)
                    || !text.is_char_boundary(b.byte_end)
                    || !b.x_logical.is_finite()
            })
        {
            return Err(Error::ShapingUnavailable);
        }
        if [
            handoff.cursor.pos,
            handoff.selection.anchor,
            handoff.selection.head,
        ]
        .into_iter()
        .any(|pos| !boundaries.iter().any(|b| b.byte_start == pos))
        {
            return Err(Error::InvalidBoundary);
        }
        Ok(())
    }

    /// HAL383 §9: called only by the mounted tree owner at the existing deferred
    /// cut, with the same real TextEngine/Theme as the upcoming ordinary Layout.
    pub(crate) fn apply_pending_edit_handoff(
        &mut self,
        text_engine: &mut dyn TextEngine,
        theme: &crate::core::theme::Theme,
    ) -> bool {
        let Some(handoff) = self.pending_edit_handoff.as_ref() else {
            return false;
        };
        let validation = self
            .validate_edit_handoff(handoff)
            .and_then(|()| Self::validate_edit_handoff_shaping(handoff, text_engine, theme));
        if let Err(error) = validation {
            self.pending_edit_handoff = None;
            self.edit_handoff_status = TextFieldHandoffStatus::Rejected(error);
            return false;
        }
        let handoff = self.pending_edit_handoff.take().unwrap();
        self.state.document.set_text(handoff.text);
        self.state.cursor = handoff.cursor;
        self.state.selection = handoff.selection;
        self.state.layout_cache.mark_stale();
        self.layout_dirty.set(true);
        self.edit_handoff_status = TextFieldHandoffStatus::Applied;
        true
    }

    /// Set text alignment within the viewport. Default: Left.
    pub fn with_text_align(mut self, align: TextAlign) -> Self {
        self.text_align = align;
        self
    }

    pub fn set_on_submit<F: FnMut(&str) + 'static>(&mut self, f: F) {
        self.on_submit = Some(Box::new(f));
    }

    pub fn set_on_cancel<F: FnMut() + 'static>(&mut self, f: F) {
        self.on_cancel = Some(Box::new(f));
    }

    /// Start in capture mode (caret visible, typing active) on mount.
    pub fn set_start_captured(&mut self, v: bool) {
        self.start_captured = v;
    }

    /// Set a character filter for text input.
    ///
    /// Only characters where `filter(ch)` returns true are inserted.
    /// Others are silently consumed.
    pub fn set_input_filter<F: Fn(char) -> bool + 'static>(&mut self, filter: F) {
        self.input_filter = Some(Box::new(filter));
    }

    /// Set a callback that fires after every text mutation.
    ///
    /// Called after typing, deletion, paste, and undo/redo.
    /// Receives the current text content.
    pub fn set_on_change<F: FnMut(&str) + 'static>(&mut self, f: F) {
        self.on_change = Some(Box::new(f));
    }

    /// When true, entering Capture mode selects all text.
    pub fn set_select_all_on_capture(&mut self, v: bool) {
        self.select_all_on_capture = v;
    }

    /// After Enter commits, redirect focus to this widget.
    pub fn set_on_submit_redirect(&mut self, target: TreeWidgetId) {
        self.on_submit_redirect = Some(target);
    }

    /// After Escape cancels, redirect focus to this widget.
    pub fn set_on_escape_redirect(&mut self, target: TreeWidgetId) {
        self.on_escape_redirect = Some(target);
    }

    fn snap_smooth_caret(&mut self) {
        if !self.smooth_caret_enabled
            && self.smooth_caret.handle.is_none()
            && self.smooth_caret.projection.is_none()
            && self.smooth_caret.pending.is_none()
            && self.smooth_caret.boundary.is_none()
            && self.smooth_caret.mask_boundary.is_none()
        {
            return;
        }
        self.smooth_caret.snap = true;
        self.smooth_caret.pending = None;
    }

    fn smooth_destination(&self) -> Option<SmoothCaretDestination> {
        if self.has_active_selection() || (self.config.is_password && self.embedded) {
            return None;
        }
        self.active_caret_destination()
    }

    fn active_caret_destination(&self) -> Option<SmoothCaretDestination> {
        let doc = &self.state.document;
        let cache = &self.state.layout_cache;
        if self.layout_dirty.get()
            || cache.needs_rebuild(doc.version(), self.last_layout_width)
            || !cache.is_fresh_for_doc_len(doc.len())
            || self.state.is_composing()
        {
            return None;
        }
        let cursor = self.state.cursor.pos;
        let mut matching = cache
            .boundaries()
            .iter()
            .enumerate()
            .filter(|(_, b)| b.byte_start == cursor);
        let (index, boundary) = matching.next()?;
        let x = if self.config.is_password {
            let advance = self.password_mask_advance.get();
            if !advance.is_finite() || advance <= 0.0 {
                return None;
            }
            index as f32 * advance
        } else {
            boundary.x_logical
        };
        if matching.next().is_some() || !x.is_finite() {
            return None;
        }
        Some(SmoothCaretDestination {
            revision: doc.version(),
            cursor,
            x,
        })
    }

    // Called only after qualified navigation changes a fresh canonical x.
    fn record_smooth_destination(&mut self) {
        if self.smooth_caret.boundary.is_some() || self.smooth_caret.mask_boundary.is_some() {
            self.snap_smooth_caret();
            return;
        }
        if self.smooth_caret_enabled && !self.smooth_caret.snap {
            match self.smooth_destination() {
                Some(destination) => self.smooth_caret.pending = Some(destination),
                None => self.snap_smooth_caret(),
            }
        }
    }

    fn smooth_space(
        &self,
        geometry: crate::core::widget::OwnedWidgetPresentationGeometry,
        facts: FrameworkAppearanceFacts,
        theme: &crate::core::theme::Theme,
    ) -> Option<SmoothCaretSpace> {
        if self.has_active_selection() {
            return None;
        }
        self.caret_space(geometry, facts, theme)
    }

    fn text_capture(&self, cx: &PaintContext) -> bool {
        if self.composite_role {
            cx.composite_delegation().is_some()
        } else {
            cx.has_capture()
        }
    }

    fn text_selection_visible(&self, cx: &PaintContext) -> bool {
        if self.composite_role {
            cx.composite_delegation().is_some()
        } else {
            self.focused
        }
    }

    fn event_text_active(&self, cx: &EventContext) -> bool {
        if self.composite_role {
            cx.composite_edit_authority().is_some() && cx.composite_edit_has_capture()
        } else {
            self.focused
        }
    }

    fn motion_eligible(&self, facts: Option<FrameworkAppearanceFacts>) -> bool {
        self.smooth_caret_enabled
            && self.config.is_enabled
            && (!self.config.is_password || !self.embedded)
            && (!self.embedded || (self.composite_role && self.composite_delegation.is_some()))
            && !self.state.is_composing()
            && self
                .composite_delegation
                .map(|delegation| delegation.host_facts)
                .or(facts)
                .is_some_and(|facts| {
                    facts.focused()
                        && facts.keyboard_captured()
                        && facts.window_active()
                        && facts.interaction_domain_active()
                })
    }

    fn caret_space(
        &self,
        geometry: crate::core::widget::OwnedWidgetPresentationGeometry,
        facts: FrameworkAppearanceFacts,
        theme: &crate::core::theme::Theme,
    ) -> Option<SmoothCaretSpace> {
        if !self.motion_eligible(Some(facts)) {
            return None;
        }
        if self.config.is_password && self.password_mask_font_id.is_none() {
            return None;
        }
        let geom = theme.geometry.text_field;
        let typography = &theme.typography;
        let metrics = [
            typography.input_font_size,
            self.ascent,
            self.descent,
            geom.padding_x,
            geom.padding_y,
            typography.baseline_offset,
            typography.caret_thickness,
            typography.line_height,
            self.state.scroll_x,
            self.alignment_offset(),
        ];
        if metrics.iter().any(|v| !v.is_finite())
            || self.state.layout_cache.font_size() != typography.input_font_size
            || self.state.layout_cache.caret_thickness() != typography.caret_thickness
            || self.padding_x != geom.padding_x
            || !self.leading_width.is_finite()
        {
            return None;
        }
        Some(SmoothCaretSpace {
            typography_revision: self.state.layout_cache.typography_revision(),
            geometry,
            facts,
            delegation: self
                .composite_delegation
                .map(|facts| (facts.relation, facts.session, facts.presentation_revision)),
            metrics,
            leading_width: self.leading_width,
            mask_advance: self
                .config
                .is_password
                .then_some(self.password_mask_advance.get()),
            mask_font_id: if self.config.is_password {
                self.password_mask_font_id
            } else {
                None
            },
        })
    }

    fn refresh_range_origin(&mut self, cx: &UpdateContext) {
        self.smooth_caret.range_origin = if self.smooth_caret.handle.is_none()
            && self.smooth_caret.boundary.is_none()
        {
            self.range_destination()
                .zip(cx.own_presentation_geometry())
                .zip(cx.appearance_facts())
                .zip(cx.theme())
                .and_then(|(((destination, geometry), facts), theme)| {
                    self.caret_space(geometry, facts, theme)
                        .map(|space| SmoothCaretProjection {
                            destination,
                            space,
                            x: destination.x,
                        })
                })
        } else {
            None
        };
    }

    fn update_smooth_caret(&mut self, cx: &mut UpdateContext) {
        // Eligibility precedes all motion-specific document scans and geometry.
        if !self.motion_eligible(cx.appearance_facts()) {
            let old = std::mem::take(&mut self.smooth_caret);
            let repaint = old.projection.is_some()
                || old.boundary.is_some()
                || old.mask_boundary.is_some()
                || old.pending.is_some()
                || old.handle.is_some();
            if let Some(handle) = old.handle {
                cx.cancel_animation(handle);
            }
            if repaint {
                cx.invalidate_paint();
            }
            return;
        }
        let destination = self.smooth_destination();
        let space = cx
            .own_presentation_geometry()
            .zip(cx.appearance_facts())
            .zip(cx.theme())
            .and_then(|((geometry, facts), theme)| self.smooth_space(geometry, facts, theme));
        let old = self.smooth_caret.projection;
        if let Some(boundary) = self.smooth_caret.mask_boundary.take() {
            self.update_password_mask_boundary(cx, boundary, destination, space, old);
            return;
        }
        if let Some(boundary) = self.smooth_caret.boundary.take() {
            let old = if boundary.range.is_some() {
                old.or(self.smooth_caret.range_origin)
            } else {
                old
            };
            self.update_caret_boundary(cx, boundary, destination, space, old);
            self.refresh_range_origin(cx);
            return;
        }
        let pending = self.smooth_caret.pending.take();
        let compatible = old
            .zip(space)
            .is_some_and(|(old, space)| old.space == space)
            && old.zip(destination).is_some_and(|(old, destination)| {
                old.destination.revision == destination.revision
                    && (pending == Some(destination) || old.destination == destination)
            });
        let mut x = destination.map(|d| d.x);
        if !self.smooth_caret.snap && compatible {
            let old = old.unwrap();
            let mut sampled = Some(old.x);
            if let Some(handle) = self.smooth_caret.handle {
                match cx.sample_animation::<f32>(handle) {
                    Some(AnimationSample::Active(value)) if value.is_finite() => {
                        sampled = Some(value)
                    }
                    Some(AnimationSample::Completed(value)) if value.is_finite() => {
                        sampled = Some(value);
                        self.smooth_caret.handle = None;
                    }
                    Some(AnimationSample::Cancelled)
                    | Some(AnimationSample::Missing)
                    | Some(AnimationSample::TypeMismatch)
                    | None
                    | Some(AnimationSample::Active(_))
                    | Some(AnimationSample::Completed(_)) => {
                        cx.cancel_animation(handle);
                        self.smooth_caret.handle = None;
                        sampled = None;
                    }
                }
            }
            if let (Some(from), Some(destination)) = (sampled, destination) {
                x = Some(from);
                if destination.x != old.destination.x {
                    if let Some(handle) = self.smooth_caret.handle.take() {
                        cx.cancel_animation(handle);
                    }
                    if from != destination.x {
                        match cx.try_start_animation(Animation::new(
                            from,
                            destination.x,
                            0.040,
                            Easing::EaseOut,
                        )) {
                            Ok(Some(handle)) => self.smooth_caret.handle = Some(handle),
                            Ok(None) | Err(_) => x = Some(destination.x),
                        }
                    } else {
                        x = Some(destination.x);
                    }
                }
            }
        } else if let Some(handle) = self.smooth_caret.handle.take() {
            cx.cancel_animation(handle);
        }
        self.smooth_caret.snap = false;
        self.smooth_caret.projection =
            destination
                .zip(space)
                .zip(x)
                .map(|((destination, space), x)| SmoothCaretProjection {
                    destination,
                    space,
                    x,
                });
        if old.map(|p| p.x) != self.smooth_caret.projection.map(|p| p.x) {
            cx.invalidate_paint();
        }
        self.refresh_range_origin(cx);
    }

    fn painted_caret_x(&self, cx: &PaintContext, rect: RectF, canonical: f32) -> f32 {
        if self.composite_role && cx.composite_delegation() != self.composite_delegation {
            return canonical;
        }
        // No Timeline or motion-state write is permitted at this consuming gate.
        // A failed boundary paint must never fall back to a detached moving caret.
        if self.smooth_caret.boundary.is_some() || self.smooth_caret.mask_boundary.is_some() {
            return canonical;
        }
        let Some(projection) = self.smooth_caret.projection else {
            return canonical;
        };
        if !self.motion_eligible(cx.appearance_facts()) {
            return canonical;
        }
        if self.smooth_caret.snap
            || self.smooth_caret.pending.is_some()
            || self.smooth_destination() != Some(projection.destination)
        {
            return canonical;
        }
        let Some(geometry) = cx.own_presentation_geometry() else {
            return canonical;
        };
        if geometry.rect != rect {
            return canonical;
        }
        let space = cx
            .appearance_facts()
            .and_then(|facts| self.smooth_space(geometry, facts, cx.theme));
        if space == Some(projection.space) {
            projection.x
        } else {
            canonical
        }
    }

    fn password_mask_origin(&self) -> Option<PasswordMaskOrigin> {
        if !self.config.is_password
            || self.embedded
            || !self.smooth_caret_enabled
            || self.smooth_caret.snap
            || !self.config.is_enabled
            || self.config.is_read_only
            || self.state.cursor.pos != self.state.document.len()
        {
            return None;
        }
        let destination = self.smooth_destination()?;
        let cache = &self.state.layout_cache;
        if cache.line_count() != 1 {
            return None;
        }
        let count = self.password_cluster_count();
        let boundaries = cache.boundaries();
        if boundaries.first()?.byte_start != 0
            || boundaries.get(count)?.byte_start != destination.cursor
        {
            return None;
        }
        Some(PasswordMaskOrigin {
            destination,
            count,
            previous_byte: boundaries.get(count.saturating_sub(1))?.byte_start,
            advance: self.password_mask_advance.get(),
        })
    }

    fn record_password_mask_boundary(
        &mut self,
        before: Option<PasswordMaskOrigin>,
        kind: CaretBoundaryKind,
        allowed: bool,
    ) {
        let pair = before.zip(self.password_mask_origin());
        let Some((before, after)) = pair.filter(|(before, after)| {
            allowed
                && self.smooth_caret.boundary.is_none()
                && before.advance == after.advance
                && before.destination.revision < after.destination.revision
                && match kind {
                    CaretBoundaryKind::Reveal => {
                        before.count.checked_add(1) == Some(after.count)
                            && self
                                .state
                                .layout_cache
                                .boundaries()
                                .get(before.count)
                                .is_some_and(|b| b.byte_start == before.destination.cursor)
                    }
                    CaretBoundaryKind::Conceal => {
                        after.count.checked_add(1) == Some(before.count)
                            && after.destination.cursor == before.previous_byte
                    }
                }
        }) else {
            self.snap_smooth_caret();
            return;
        };
        if let Some(boundary) = self.smooth_caret.mask_boundary.as_mut() {
            if boundary.kind != kind
                || boundary.destination != before.destination
                || boundary.advance != after.advance
            {
                self.snap_smooth_caret();
                return;
            }
            if !boundary.pending {
                boundary.origin = before.destination;
            }
            boundary.destination = after.destination;
            boundary.new_count = after.count;
            boundary.pending = true;
        } else {
            if self.smooth_caret.handle.is_some() {
                // A navigation caret can still be inside fully visible masks.
                // It is not a coherent reveal/conceal edge for a new edit.
                self.snap_smooth_caret();
                return;
            }
            self.smooth_caret.mask_boundary = Some(PasswordMaskBoundary {
                kind,
                origin: before.destination,
                destination: after.destination,
                old_count: before.count,
                new_count: after.count,
                advance: after.advance,
                pending: true,
            });
        }
        self.smooth_caret.pending = None;
        self.smooth_caret.range_origin = None;
    }

    fn update_password_mask_boundary(
        &mut self,
        cx: &mut UpdateContext,
        mut boundary: PasswordMaskBoundary,
        destination: Option<SmoothCaretDestination>,
        space: Option<SmoothCaretSpace>,
        old: Option<SmoothCaretProjection>,
    ) {
        let pending = boundary.pending;
        let expected = if pending {
            boundary.origin
        } else {
            boundary.destination
        };
        let compatible = !self.smooth_caret.snap
            && destination == Some(boundary.destination)
            && self.password_mask_origin().is_some_and(|current| {
                current.count == boundary.new_count && current.advance == boundary.advance
            })
            && old
                .zip(space)
                .is_some_and(|(old, space)| old.space == space && old.destination == expected);
        let mut x = destination.map(|d| d.x);
        if compatible {
            let mut sampled = old.map(|p| p.x);
            let mut complete = false;
            if let Some(handle) = self.smooth_caret.handle {
                sampled = match cx.sample_animation::<f32>(handle) {
                    Some(AnimationSample::Active(value)) if value.is_finite() => Some(value),
                    Some(AnimationSample::Completed(value)) if value.is_finite() => {
                        self.smooth_caret.handle = None;
                        complete = true;
                        Some(value)
                    }
                    _ => None,
                };
            }
            if let Some(from) = sampled {
                let target = boundary.destination.x;
                let directed = match boundary.kind {
                    CaretBoundaryKind::Reveal => from <= target,
                    CaretBoundaryKind::Conceal => from >= target,
                };
                if directed && pending && from != target {
                    if let Some(handle) = self.smooth_caret.handle.take() {
                        cx.cancel_animation(handle);
                    }
                    if let Ok(Some(handle)) =
                        cx.try_start_animation(Animation::new(from, target, 0.040, Easing::EaseOut))
                    {
                        self.smooth_caret.handle = Some(handle);
                        boundary.pending = false;
                        self.smooth_caret.mask_boundary = Some(boundary);
                        x = Some(from);
                    }
                } else if directed && !pending && !complete && self.smooth_caret.handle.is_some() {
                    self.smooth_caret.mask_boundary = Some(boundary);
                    x = Some(from);
                }
            }
        }
        if self.smooth_caret.mask_boundary.is_none() {
            if let Some(handle) = self.smooth_caret.handle.take() {
                cx.cancel_animation(handle);
            }
        }
        self.smooth_caret.snap = false;
        self.smooth_caret.pending = None;
        self.smooth_caret.projection =
            destination
                .zip(space)
                .zip(x)
                .map(|((destination, space), x)| SmoothCaretProjection {
                    destination,
                    space,
                    x,
                });
        if pending
            || self.smooth_caret.mask_boundary.is_none()
            || old.map(|p| p.x) != self.smooth_caret.projection.map(|p| p.x)
        {
            cx.invalidate_paint();
        }
    }

    fn paint_password_mask_boundary(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        viewport: RectF,
        text_top_y: f32,
        font_size: f32,
        caret_height: f32,
        caret_color: Color,
    ) -> bool {
        if !self.config.is_password || self.embedded {
            return false;
        }
        let Some(boundary) = self.smooth_caret.mask_boundary else {
            return false;
        };
        let Some(projection) = self.smooth_caret.projection else {
            return false;
        };
        if !self.motion_eligible(cx.appearance_facts())
            || self.smooth_caret.snap
            || boundary.pending
        {
            return false;
        }
        // Match the existing mask painter's physical-pixel advance before its
        // consuming gate. No password source is shaped or stored by this path.
        let advance = Self::password_mask_advance_for_scale(
            cx.text_engine.measure(PASSWORD_MASK_TEXT, font_size).0,
            cx.scale_factor,
        );
        self.password_mask_advance.set(advance);
        let run = cx.text_engine.shape(PASSWORD_MASK_TEXT, font_size);
        let font_id = run.binding.as_ref().map(|binding| binding.face().font_id());
        if font_id.is_none() || font_id != projection.space.mask_font_id {
            return false;
        }
        if !self.password_mask_origin().is_some_and(|current| {
            current.destination == boundary.destination
                && current.count == boundary.new_count
                && current.advance == boundary.advance
        }) || projection.destination != boundary.destination
        {
            return false;
        }
        let Some(geometry) = cx.own_presentation_geometry() else {
            return false;
        };
        let space = cx
            .appearance_facts()
            .and_then(|facts| self.smooth_space(geometry, facts, cx.theme));
        if geometry.rect != rect || space != Some(projection.space) {
            return false;
        }
        let text_x = snap_to_physical(
            viewport.x + self.alignment_offset() - self.state.scroll_x,
            cx.scale_factor,
        );
        let edge = snap_to_physical(text_x + projection.x, cx.scale_factor);
        if !edge.is_finite() || edge < viewport.x || edge > viewport.right() {
            return false;
        }
        cx.scene.begin_clip(
            RectF::new(viewport.x, viewport.y, edge - viewport.x, viewport.height),
            None,
        );
        let count = match boundary.kind {
            CaretBoundaryKind::Reveal => boundary.new_count,
            CaretBoundaryKind::Conceal => boundary.old_count,
        };
        for index in 0..count {
            cx.text_engine.draw(
                &run,
                text_x + index as f32 * advance,
                text_top_y,
                cx.theme.colors.typography.primary,
                TEXT_FIELD_TEXT_LAYER,
                cx.scene,
            );
        }
        cx.scene.end_clip();
        if self.text_capture(cx) && self.blink_caret_visible.get() {
            let empty = cx.text_engine.shape("", font_size);
            cx.text_engine.draw_with_backgrounds(
                &empty,
                edge,
                text_top_y,
                caret_color,
                vec![TextBackgroundSpan {
                    x_offset: 0.0,
                    y_offset: 0.0,
                    width: cx.theme.typography.caret_thickness,
                    height: caret_height,
                    color: caret_color,
                }],
                TEXT_FIELD_CARET_LAYER,
                cx.scene,
            );
        }
        true
    }

    fn caret_boundary_origin(&self) -> Option<SmoothCaretDestination> {
        if !self.smooth_caret_enabled
            || self.smooth_caret.snap
            || !self.config.is_enabled
            || self.config.is_read_only
            || self.config.is_password
            || (self.embedded && !self.composite_role)
            || self.text_align != TextAlign::Left
            || self.state.cursor.pos != self.state.document.len()
        {
            return None;
        }
        let destination = self.smooth_destination()?;
        if self.state.layout_cache.line_count() != 1 {
            return None;
        }
        Some(destination)
    }

    fn boundary_prefix_matches(short: &GlyphRun, long: &GlyphRun) -> bool {
        let join = short.text.len();
        if short.binding.is_none()
            || short.binding != long.binding
            || !long.text.starts_with(&short.text)
            || short.glyphs.len() > long.glyphs.len()
            || short
                .glyphs
                .iter()
                .chain(&long.glyphs)
                .any(|glyph| glyph.id == 0)
            || [short.font_size, short.height, short.ascent, short.descent]
                != [long.font_size, long.height, long.ascent, long.descent]
            || [
                short.width,
                long.width,
                short.font_size,
                short.height,
                short.ascent,
                short.descent,
            ]
            .iter()
            .any(|v| !v.is_finite())
            || short.width < 0.0
            || long.width < short.width
        {
            return false;
        }
        for run in [short, long] {
            let mut end = 0;
            let mut x = 0.0;
            for cluster in &run.clusters {
                if cluster.source.start != end
                    || cluster.source.end <= end
                    || cluster.source.end > run.text.len()
                    || cluster.direction != crate::core::text_shaper::TextDirection::LeftToRight
                    || !cluster.x_start.is_finite()
                    || !cluster.x_end.is_finite()
                    || cluster.x_start != x
                    || cluster.x_end < cluster.x_start
                {
                    return false;
                }
                end = cluster.source.end;
                x = cluster.x_end;
            }
            if end != run.text.len()
                || x != run.width
                || run.glyphs.iter().any(|g| {
                    !g.x.is_finite()
                        || !g.y.is_finite()
                        || !g.advance.is_finite()
                        || g.advance < 0.0
                })
            {
                return false;
            }
        }
        if long
            .clusters
            .iter()
            .any(|c| c.source.start < join && c.source.end > join)
        {
            return false;
        }
        let prefix_clusters = long.clusters.iter().take_while(|c| c.source.end <= join);
        if !short.clusters.iter().eq(prefix_clusters)
            || (long.text.len() > join
                && !long
                    .clusters
                    .iter()
                    .any(|c| c.source.start == join && c.x_start == short.width))
        {
            return false;
        }
        short
            .glyphs
            .iter()
            .zip(&long.glyphs)
            .all(|(a, b)| a.id == b.id && a.x == b.x && a.y == b.y && a.advance == b.advance)
    }

    fn record_caret_boundary(
        &mut self,
        before: &EditSnapshot,
        origin: Option<SmoothCaretDestination>,
        kind: CaretBoundaryKind,
        allowed: bool,
        text_engine: &mut dyn TextEngine,
    ) {
        let qualified = origin.zip(self.caret_boundary_origin()).filter(|_| {
            allowed
                && before.selection_anchor == before.selection_head
                && before.cursor_pos == before.text.len()
        });
        let Some((origin, destination)) = qualified else {
            self.snap_smooth_caret();
            return;
        };
        let old_run = text_engine.shape(&before.text, self.state.layout_cache.font_size());
        let new_run = text_engine.shape(
            self.state.document.text(),
            self.state.layout_cache.font_size(),
        );
        let (short, long) = match kind {
            CaretBoundaryKind::Reveal => (&old_run, &new_run),
            CaretBoundaryKind::Conceal => (&new_run, &old_run),
        };
        if !Self::boundary_prefix_matches(short, long)
            || old_run.width != origin.x
            || new_run.width != destination.x
            || self
                .smooth_caret
                .boundary
                .as_ref()
                .is_some_and(|b| b.general.is_some())
        {
            let previous = self.smooth_caret.boundary.as_ref();
            if previous
                .is_some_and(|b| b.kind != kind || b.range.is_some() || b.destination != origin)
            {
                self.snap_smooth_caret();
                return;
            }
            let source = previous
                .and_then(|b| b.old_source.as_deref())
                .unwrap_or(&before.text);
            let current = self.state.document.text();
            let (removed, inserted) = match kind {
                CaretBoundaryKind::Reveal if current.starts_with(source) => {
                    (source.len()..source.len(), source.len()..current.len())
                }
                CaretBoundaryKind::Conceal if source.starts_with(current) => {
                    (current.len()..source.len(), current.len()..current.len())
                }
                _ => {
                    self.snap_smooth_caret();
                    return;
                }
            };
            let original_run = text_engine.shape(source, self.state.layout_cache.font_size());
            self.record_general_boundary(
                before,
                origin,
                destination,
                kind,
                None,
                removed,
                inserted,
                &original_run,
                &new_run,
                text_engine,
            );
            return;
        }
        let Some(binding) = new_run.binding.as_ref() else {
            self.snap_smooth_caret();
            return;
        };
        if let Some(boundary) = self.smooth_caret.boundary.as_mut() {
            if boundary.kind != kind
                || boundary.destination != origin
                || &boundary.binding != binding
                || (kind == CaretBoundaryKind::Conceal
                    && boundary
                        .old_source
                        .as_ref()
                        .is_some_and(|source| !source.starts_with(self.state.document.text())))
            {
                self.snap_smooth_caret();
                return;
            }
            if !boundary.pending {
                boundary.origin = origin;
            }
            boundary.destination = destination;
            boundary.pending = true;
        } else {
            if self.smooth_caret.handle.is_some()
                || !self
                    .smooth_caret
                    .projection
                    .is_some_and(|p| p.destination == origin && p.x == origin.x)
            {
                self.snap_smooth_caret();
                return;
            }
            self.smooth_caret.boundary = Some(CaretBoundary {
                kind,
                old_source: Some(before.text.clone()),
                source_revision: origin.revision,
                origin,
                destination,
                binding: binding.clone(),
                metrics: [
                    new_run.font_size,
                    new_run.height,
                    new_run.ascent,
                    new_run.descent,
                ],
                pending: true,
                range: None,
                source_caret_x: origin.x,
                general: None,
            });
        }
        self.smooth_caret.pending = None;
    }

    fn range_destination(&self) -> Option<SmoothCaretDestination> {
        if !self.smooth_caret_enabled
            || !self.config.is_enabled
            || self.config.is_read_only
            || self.config.is_password
            || (self.embedded && !self.composite_role)
            || self.text_align != TextAlign::Left
            || self.state.cursor.pos != self.state.selection.head
        {
            return None;
        }
        // Update precedes Layout. A programmatic edit can leave the cache stale;
        // qualify freshness before reading even its line count.
        let destination = self.active_caret_destination()?;
        if self.state.layout_cache.line_count() != 1 {
            return None;
        }
        Some(destination)
    }

    fn word_delete_modifiers(
        modifiers: crate::core::event::Modifiers,
        browser: bool,
        macos: bool,
    ) -> bool {
        if modifiers.shift || modifiers.meta {
            return false;
        }
        if browser {
            modifiers.alt ^ modifiers.ctrl
        } else if macos {
            modifiers.alt && !modifiers.ctrl
        } else {
            modifiers.ctrl && !modifiers.alt
        }
    }

    fn range_event_kind(&self, event: &Event) -> Option<CaretRangeKind> {
        self.range_destination()?;
        if self.smooth_caret.snap {
            return None;
        }
        match event {
            Event::Key(key) if key.is_down => {
                if self.has_active_selection()
                    && matches!(key.key_code, KeyCode::Backspace | KeyCode::Delete)
                {
                    Some(CaretRangeKind::SelectionDelete)
                } else if key.key_code == KeyCode::Backspace
                    && Self::word_delete_modifiers(
                        key.modifiers,
                        cfg!(target_arch = "wasm32"),
                        cfg!(target_os = "macos"),
                    )
                {
                    Some(CaretRangeKind::WordDelete)
                } else if crate::core::text_edit_policy::is_primary_shortcut(key.modifiers) {
                    match key.key_code {
                        // These exact command keys may precede the existing Text shortcut.
                        // This grants no text provenance or edit permission.
                        KeyCode::Character('x' | 'X') if self.has_active_selection() => {
                            Some(CaretRangeKind::SelectionDelete)
                        }
                        KeyCode::Character('v' | 'V') if !self.has_active_selection() => {
                            Some(CaretRangeKind::Paste)
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Event::Text(text)
                if crate::core::text_edit_policy::is_primary_shortcut(text.modifiers) =>
            {
                match text.text.as_str() {
                    "x" | "X" if self.has_active_selection() => {
                        Some(CaretRangeKind::SelectionDelete)
                    }
                    "v" | "V" if !self.has_active_selection() => Some(CaretRangeKind::Paste),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn range_runs_match(
        old: &GlyphRun,
        new: &GlyphRun,
        range: &CaretRange,
        shaper: Option<&crate::core::text_shaper::TextShaper>,
        scale: f32,
        viewport_width: f32,
    ) -> Option<(f32, f32)> {
        let a = range.old.start;
        let b = range.old.end;
        let c = range.new.end;
        if range.new.start != a
            || a > b
            || b > old.text.len()
            || a > c
            || c > new.text.len()
            || !old.text.is_char_boundary(a)
            || !old.text.is_char_boundary(b)
            || !new.text.is_char_boundary(a)
            || !new.text.is_char_boundary(c)
            || old.text[..a] != new.text[..a]
            || old.text[b..] != new.text[c..]
            || old.binding.is_none()
            || old
                .glyphs
                .iter()
                .chain(&new.glyphs)
                .any(|glyph| glyph.id == 0)
            || old.binding != new.binding
            || [old.font_size, old.height, old.ascent, old.descent]
                != [new.font_size, new.height, new.ascent, new.descent]
            || !viewport_width.is_finite()
            || viewport_width <= 0.0
        {
            return None;
        }
        match range.kind {
            CaretRangeKind::Paste if a == b && c > a => {}
            CaretRangeKind::WordDelete | CaretRangeKind::SelectionDelete if c == a && b > a => {
                // Actual accepted deletion; size is not presentation policy.
            }
            _ => return None,
        }
        fn join(run: &GlyphRun, byte: usize) -> Option<f32> {
            let mut end = 0;
            let mut previous_x = 0.0;
            for cluster in &run.clusters {
                if cluster.source.start != end
                    || cluster.source.end <= end
                    || cluster.source.end > run.text.len()
                    || cluster.direction != crate::core::text_shaper::TextDirection::LeftToRight
                    || !cluster.x_start.is_finite()
                    || !cluster.x_end.is_finite()
                    || cluster.x_start < previous_x
                    || cluster.x_end < cluster.x_start
                    || (cluster.source.start < byte && byte < cluster.source.end)
                {
                    return None;
                }
                end = cluster.source.end;
                previous_x = cluster.x_end;
            }
            if end != run.text.len() || !run.width.is_finite() || previous_x > run.width {
                return None;
            }
            if byte == run.text.len() {
                Some(run.width)
            } else {
                let mut found = run.clusters.iter().filter(|c| c.source.start == byte);
                let x = found.next()?.x_start;
                found.next().is_none().then_some(x)
            }
        }
        let old_left = join(old, a)?;
        let old_right = join(old, b)?;
        let new_left = join(new, a)?;
        let new_right = join(new, c)?;
        if old_left != new_left {
            return None;
        }
        let extent = match range.kind {
            CaretRangeKind::Paste => new_right - new_left,
            _ => old_right - old_left,
        };
        if !extent.is_finite() || extent <= 0.0 {
            return None;
        }
        let old_sources = crate::core::text_engine::glyph_source_ranges(old, shaper, scale)?;
        let new_sources = crate::core::text_engine::glyph_source_ranges(new, shaper, scale)?;
        for suffix in [false, true] {
            let mut old_glyphs = old.glyphs.iter().zip(&old_sources).filter(|(_, source)| {
                if suffix {
                    source.start >= b
                } else {
                    source.end <= a
                }
            });
            let mut new_glyphs = new.glyphs.iter().zip(&new_sources).filter(|(_, source)| {
                if suffix {
                    source.start >= c
                } else {
                    source.end <= a
                }
            });
            loop {
                match (old_glyphs.next(), new_glyphs.next()) {
                    (None, None) => break,
                    (Some((og, os)), Some((ng, ns))) => {
                        let (old_byte, new_byte, old_x, new_x) = if suffix {
                            (b, c, old_right, new_right)
                        } else {
                            (0, 0, 0.0, 0.0)
                        };
                        if os.start - old_byte != ns.start - new_byte
                            || os.end - old_byte != ns.end - new_byte
                            || og.id != ng.id
                            || og.x - old_x != ng.x - new_x
                            || og.y != ng.y
                            || og.advance != ng.advance
                        {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
        }
        Some((
            if range.kind == CaretRangeKind::Paste {
                old_left
            } else {
                old_right
            },
            new_right,
        ))
    }

    fn record_range_boundary(
        &mut self,
        before: &EditSnapshot,
        origin: Option<SmoothCaretDestination>,
        kind: CaretRangeKind,
        text_engine: &mut dyn TextEngine,
    ) {
        let qualified = origin
            .zip(self.range_destination())
            .filter(|_| !self.smooth_caret.snap);
        let Some((origin, destination)) = qualified else {
            self.snap_smooth_caret();
            return;
        };
        if self.has_active_selection() {
            self.snap_smooth_caret();
            return;
        }
        let a = match kind {
            CaretRangeKind::WordDelete => destination.cursor,
            CaretRangeKind::SelectionDelete => before.selection_anchor.min(before.selection_head),
            CaretRangeKind::Paste => before.cursor_pos,
        };
        let b = match kind {
            CaretRangeKind::WordDelete => before.cursor_pos,
            CaretRangeKind::SelectionDelete => before.selection_anchor.max(before.selection_head),
            CaretRangeKind::Paste => a,
        };
        let c = destination.cursor;
        let selected = before.selection_anchor != before.selection_head;
        if (kind == CaretRangeKind::SelectionDelete && (!selected || before.cursor_pos != b))
            || (kind != CaretRangeKind::SelectionDelete && selected)
        {
            self.snap_smooth_caret();
            return;
        }
        let direction = if kind == CaretRangeKind::Paste {
            CaretBoundaryKind::Reveal
        } else {
            CaretBoundaryKind::Conceal
        };
        let existing = self.smooth_caret.boundary.as_ref();
        let mut range = CaretRange {
            kind,
            old: a..b,
            new: a..c,
        };
        let (source, static_origin) = if let Some(boundary) = existing {
            let Some(previous) = boundary.range.as_ref() else {
                self.snap_smooth_caret();
                return;
            };
            if previous.kind != kind
                || kind == CaretRangeKind::SelectionDelete
                || boundary.destination != origin
                || boundary.kind != direction
            {
                self.snap_smooth_caret();
                return;
            }
            let Some(source) = boundary.old_source.as_deref() else {
                self.snap_smooth_caret();
                return;
            };
            match kind {
                CaretRangeKind::WordDelete if b == previous.new.end => {
                    range.old.end = previous.old.end;
                }
                CaretRangeKind::Paste if a == previous.new.end => {
                    range.old = previous.old.clone();
                    range.new.start = previous.new.start;
                }
                _ => {
                    self.snap_smooth_caret();
                    return;
                }
            }
            (
                source,
                self.smooth_caret
                    .projection
                    .or(self.smooth_caret.range_origin),
            )
        } else {
            (before.text.as_str(), self.smooth_caret.range_origin)
        };
        let Some(static_origin) = static_origin else {
            self.snap_smooth_caret();
            return;
        };
        if existing.is_none()
            && (self.smooth_caret.handle.is_some()
                || static_origin.destination != origin
                || static_origin.x != origin.x)
        {
            self.snap_smooth_caret();
            return;
        }
        let old_run = text_engine.shape(source, self.state.layout_cache.font_size());
        let new_run = text_engine.shape(
            self.state.document.text(),
            self.state.layout_cache.font_size(),
        );
        let viewport_width = static_origin.space.geometry.rect.width
            - 2.0 * self.padding_x
            - static_origin.space.leading_width;
        let joins = Self::range_runs_match(
            &old_run,
            &new_run,
            &range,
            text_engine.shaper(),
            static_origin.space.geometry.scale_factor,
            viewport_width,
        );
        let Some((old_join, new_join)) = joins else {
            self.record_general_boundary(
                before,
                origin,
                destination,
                direction,
                Some(range.clone()),
                range.old,
                range.new,
                &old_run,
                &new_run,
                text_engine,
            );
            return;
        };
        if existing.is_some_and(|b| b.general.is_some()) {
            self.record_general_boundary(
                before,
                origin,
                destination,
                direction,
                Some(range.clone()),
                range.old,
                range.new,
                &old_run,
                &new_run,
                text_engine,
            );
            return;
        }
        if new_join != destination.x || (existing.is_none() && old_join != origin.x) {
            self.snap_smooth_caret();
            return;
        }
        let binding = new_run.binding.as_ref().unwrap().clone();
        if let Some(boundary) = self.smooth_caret.boundary.as_mut() {
            if !boundary.pending {
                boundary.origin = origin;
            }
            boundary.destination = destination;
            boundary.pending = true;
            boundary.range = Some(range);
        } else {
            self.smooth_caret.boundary = Some(CaretBoundary {
                kind: direction,
                old_source: Some(before.text.clone()),
                source_revision: origin.revision,
                origin,
                destination,
                binding,
                metrics: [
                    new_run.font_size,
                    new_run.height,
                    new_run.ascent,
                    new_run.descent,
                ],
                pending: true,
                range: Some(range),
                source_caret_x: origin.x,
                general: None,
            });
        }
        self.smooth_caret.pending = None;
    }

    fn range_duration(from: f32, target: f32) -> Option<f32> {
        let distance = (target - from).abs();
        if !distance.is_finite() || distance == 0.0 {
            return None;
        }
        // P8 measures remaining logical travel from the current Timeline sample.
        // Saturate before multiplication to preserve the finite upper bound.
        Some((80.0 + 1.25 * distance.min(112.0)).clamp(100.0, 220.0) / 1000.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn record_general_boundary(
        &mut self,
        before: &EditSnapshot,
        origin: SmoothCaretDestination,
        destination: SmoothCaretDestination,
        kind: CaretBoundaryKind,
        range: Option<CaretRange>,
        removed: std::ops::Range<usize>,
        inserted: std::ops::Range<usize>,
        old_run: &GlyphRun,
        new_run: &GlyphRun,
        text_engine: &mut dyn TextEngine,
    ) {
        let Some(presentation) = self
            .smooth_caret
            .projection
            .or(self.smooth_caret.range_origin)
        else {
            self.snap_smooth_caret();
            return;
        };
        if self.smooth_caret.boundary.is_none()
            && (self.smooth_caret.handle.is_some()
                || presentation.destination != origin
                || presentation.x != origin.x)
        {
            self.snap_smooth_caret();
            return;
        }
        let plan = edit_projection::Projection::build(
            old_run,
            new_run,
            removed.clone(),
            inserted.clone(),
            text_engine.shaper(),
            presentation.space.geometry.scale_factor,
        );
        let plan = match plan {
            Ok(plan) => plan,
            Err(reason) => {
                if self.smooth_caret.projection_refusal != Some(reason) {
                    tracing::debug!(
                        ?reason,
                        "HAL101 U1: canonical text; edit projection unavailable"
                    );
                }
                self.snap_smooth_caret();
                self.smooth_caret.projection_refusal = Some(reason);
                return;
            }
        };
        let Some(binding) = new_run.binding.as_ref() else {
            return;
        };
        if let Some(previous) = self
            .smooth_caret
            .boundary
            .as_ref()
            .filter(|_| self.smooth_caret.handle.is_some())
        {
            let before_run = text_engine.shape(&before.text, self.state.layout_cache.font_size());
            let (removed_before, inserted_before) = previous.range.as_ref().map_or_else(
                || match previous.kind {
                    CaretBoundaryKind::Reveal => (
                        old_run.text.len()..old_run.text.len(),
                        old_run.text.len()..before.text.len(),
                    ),
                    CaretBoundaryKind::Conceal => (
                        before.text.len()..old_run.text.len(),
                        before.text.len()..before.text.len(),
                    ),
                },
                |range| (range.old.clone(), range.new.clone()),
            );
            let prior = edit_projection::Projection::build(
                old_run,
                &before_run,
                removed_before,
                inserted_before,
                text_engine.shaper(),
                presentation.space.geometry.scale_factor,
            );
            let ratio = (before_run.width - old_run.width) / (new_run.width - old_run.width);
            let proven = prior.is_ok_and(|prior| plan.retarget_geometry_matches(&prior, ratio))
                && edit_projection::Projection::current_prefix_matches(
                    &before_run,
                    new_run,
                    text_engine.shaper(),
                    presentation.space.geometry.scale_factor,
                );
            if !proven {
                self.snap_smooth_caret();
                self.smooth_caret.projection_refusal =
                    Some(edit_projection::Refusal::RetargetGeometry);
                return;
            }
        }
        let previous = self.smooth_caret.boundary.take();
        let source_caret_x = previous.as_ref().map_or(origin.x, |b| b.source_caret_x);
        let source_revision = previous
            .as_ref()
            .map_or(origin.revision, |b| b.source_revision);
        let initial_origin = previous
            .as_ref()
            .filter(|b| b.pending)
            .map_or(origin, |b| b.origin);
        let progress = previous
            .as_ref()
            .and_then(|b| b.general.as_ref())
            .map_or(0.0, |g| g.progress);
        let previous_width = previous.as_ref().map(|b| {
            b.general.as_ref().map_or(b.destination.x, |g| {
                if b.pending {
                    g.previous_width.unwrap_or(g.new_width)
                } else {
                    g.new_width
                }
            })
        });
        let sample_is_caret = previous.as_ref().is_some_and(|b| {
            b.general
                .as_ref()
                .map_or(true, |g| b.pending && g.sample_is_caret)
        });
        let old_source = previous
            .and_then(|mut b| b.old_source.take())
            .unwrap_or_else(|| before.text.clone());
        self.smooth_caret.boundary = Some(CaretBoundary {
            kind,
            old_source: Some(old_source),
            source_revision,
            origin: initial_origin,
            destination,
            binding: binding.clone(),
            metrics: [
                new_run.font_size,
                new_run.height,
                new_run.ascent,
                new_run.descent,
            ],
            pending: true,
            range,
            source_caret_x,
            general: Some(GeneralCaretEdit {
                old: removed,
                new: inserted,
                progress,
                travel: plan.travel,
                old_width: old_run.width,
                new_width: new_run.width,
                previous_width,
                sample_is_caret,
            }),
        });
        self.smooth_caret.pending = None;
        self.smooth_caret.projection_refusal = None;
    }

    fn update_general_boundary(
        &mut self,
        cx: &mut UpdateContext,
        mut boundary: CaretBoundary,
        destination: Option<SmoothCaretDestination>,
        space: Option<SmoothCaretSpace>,
        old: Option<SmoothCaretProjection>,
    ) {
        let expected = if boundary.pending {
            boundary.origin
        } else {
            boundary.destination
        };
        let compatible = !self.smooth_caret.snap
            && destination == Some(boundary.destination)
            && old
                .zip(space)
                .is_some_and(|(old, space)| old.space == space && old.destination == expected);
        let general = boundary
            .general
            .as_mut()
            .expect("owned general edit descriptor");
        let was_pending = boundary.pending;
        let previous_progress = general.progress;
        let mut progress = general.progress;
        let mut complete = false;
        if compatible {
            if general.sample_is_caret && self.smooth_caret.handle.is_none() {
                progress = old.map_or(f32::NAN, |p| p.x);
            }
            if let Some(handle) = self.smooth_caret.handle {
                match cx.sample_animation::<f32>(handle) {
                    Some(AnimationSample::Active(value)) if value.is_finite() => progress = value,
                    Some(AnimationSample::Completed(value)) if value.is_finite() => {
                        progress = value;
                        complete = !boundary.pending;
                        self.smooth_caret.handle = None;
                    }
                    _ => progress = f32::NAN,
                }
            }
            if boundary.pending {
                if let Some(previous_width) = general.previous_width {
                    let displayed_width = if general.sample_is_caret {
                        progress
                    } else {
                        general.old_width + (previous_width - general.old_width) * progress
                    };
                    if general.new_width != general.old_width {
                        progress = (displayed_width - general.old_width)
                            / (general.new_width - general.old_width);
                    }
                }
                if let Some(handle) = self.smooth_caret.handle.take() {
                    cx.cancel_animation(handle);
                }
                if progress.is_finite() && (0.0..1.0).contains(&progress) {
                    let duration = if boundary.range.is_some() {
                        Self::range_duration(0.0, general.travel * (1.0 - progress))
                    } else {
                        Some(0.040)
                    };
                    if let Some(duration) = duration {
                        if let Ok(Some(handle)) = cx.try_start_animation(Animation::new(
                            progress,
                            1.0,
                            duration,
                            Easing::EaseOut,
                        )) {
                            self.smooth_caret.handle = Some(handle);
                            boundary.pending = false;
                            general.previous_width = None;
                            general.sample_is_caret = false;
                        }
                    }
                }
            }
        }
        let active = compatible
            && !boundary.pending
            && !complete
            && progress.is_finite()
            && (0.0..1.0).contains(&progress)
            && self.smooth_caret.handle.is_some();
        let x = if active {
            boundary.source_caret_x + (boundary.destination.x - boundary.source_caret_x) * progress
        } else {
            // A safety snap may follow a newer edit. Never pair that newer
            // revision with the retired descriptor's old logical endpoint.
            destination.map_or(boundary.destination.x, |current| current.x)
        };
        if active {
            boundary.general.as_mut().unwrap().progress = progress;
            self.smooth_caret.boundary = Some(boundary);
        } else if let Some(handle) = self.smooth_caret.handle.take() {
            cx.cancel_animation(handle);
        }
        self.smooth_caret.snap = false;
        self.smooth_caret.pending = None;
        self.smooth_caret.projection =
            destination
                .zip(space)
                .map(|(destination, space)| SmoothCaretProjection {
                    destination,
                    space,
                    x,
                });
        if was_pending || !active || previous_progress != progress || old.map(|p| p.x) != Some(x) {
            cx.invalidate_paint();
        }
    }

    fn update_caret_boundary(
        &mut self,
        cx: &mut UpdateContext,
        mut boundary: CaretBoundary,
        destination: Option<SmoothCaretDestination>,
        space: Option<SmoothCaretSpace>,
        old: Option<SmoothCaretProjection>,
    ) {
        if boundary.general.is_some() {
            self.update_general_boundary(cx, boundary, destination, space, old);
            return;
        }
        let expected_origin = if boundary.pending {
            boundary.origin
        } else {
            boundary.destination
        };
        let compatible = !self.smooth_caret.snap
            && destination == Some(boundary.destination)
            && if boundary.range.is_some() {
                self.range_destination() == destination && !self.has_active_selection()
            } else {
                self.caret_boundary_origin() == destination
            }
            && old.zip(space).is_some_and(|(old, space)| {
                old.space == space && old.destination == expected_origin
            })
            && boundary.source_revision <= boundary.destination.revision;
        let pending = boundary.pending;
        let mut x = destination.map(|d| d.x);
        let mut sampled = old.map(|p| p.x);
        let mut complete = false;
        if compatible {
            if let Some(handle) = self.smooth_caret.handle {
                sampled = match cx.sample_animation::<f32>(handle) {
                    Some(AnimationSample::Active(value)) if value.is_finite() => Some(value),
                    Some(AnimationSample::Completed(value)) if value.is_finite() => {
                        self.smooth_caret.handle = None;
                        complete = true;
                        Some(value)
                    }
                    _ => None,
                };
            }
            if let Some(from) = sampled {
                let target = boundary.destination.x;
                let directed = match boundary.kind {
                    CaretBoundaryKind::Reveal => from <= target,
                    CaretBoundaryKind::Conceal => from >= target,
                };
                if directed && pending && from != target {
                    if let Some(handle) = self.smooth_caret.handle.take() {
                        cx.cancel_animation(handle);
                    }
                    let duration = if boundary.range.is_some() {
                        Self::range_duration(from, target)
                    } else {
                        Some(0.040)
                    };
                    if let Some(duration) = duration {
                        if let Ok(Some(handle)) = cx.try_start_animation(Animation::new(
                            from,
                            target,
                            duration,
                            Easing::EaseOut,
                        )) {
                            self.smooth_caret.handle = Some(handle);
                            boundary.pending = false;
                            x = Some(from);
                            self.smooth_caret.boundary = Some(boundary);
                        }
                    }
                } else if directed && !pending && !complete && self.smooth_caret.handle.is_some() {
                    x = Some(from);
                    self.smooth_caret.boundary = Some(boundary);
                }
            }
        }
        if self.smooth_caret.boundary.is_none() {
            if let Some(handle) = self.smooth_caret.handle.take() {
                cx.cancel_animation(handle);
            }
        }
        self.smooth_caret.snap = false;
        self.smooth_caret.pending = None;
        self.smooth_caret.projection =
            destination
                .zip(space)
                .zip(x)
                .map(|((destination, space), x)| SmoothCaretProjection {
                    destination,
                    space,
                    x,
                });
        if pending
            || self.smooth_caret.boundary.is_none()
            || old.map(|p| p.x) != self.smooth_caret.projection.map(|p| p.x)
        {
            cx.invalidate_paint();
        }
    }

    fn paint_caret_boundary(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        viewport: RectF,
        text_top_y: f32,
        font_size: f32,
        caret_height: f32,
        caret_color: Color,
    ) -> bool {
        if self.config.is_password {
            return false;
        }
        if self.composite_role && cx.composite_delegation() != self.composite_delegation {
            return false;
        }
        let Some(boundary) = self.smooth_caret.boundary.as_ref() else {
            return false;
        };
        let Some(projection) = self.smooth_caret.projection else {
            return false;
        };
        if !self.motion_eligible(cx.appearance_facts()) {
            return false;
        }
        if self.smooth_caret.snap
            || boundary.pending
            || (if boundary.range.is_some() {
                self.range_destination()
            } else {
                self.caret_boundary_origin()
            }) != Some(boundary.destination)
            || projection.destination != boundary.destination
        {
            return false;
        }
        let Some(geometry) = cx.own_presentation_geometry() else {
            return false;
        };
        let space = cx
            .appearance_facts()
            .and_then(|facts| self.smooth_space(geometry, facts, cx.theme));
        if geometry.rect != rect || space != Some(projection.space) {
            return false;
        }
        let current = cx.text_engine.shape(self.state.document.text(), font_size);
        if !self.state.layout_cache.matches_single_run(&current)
            || current.binding.as_ref() != Some(&boundary.binding)
            || [
                current.font_size,
                current.height,
                current.ascent,
                current.descent,
            ] != boundary.metrics
            || (boundary.general.is_none()
                && boundary.range.is_none()
                && current.width != boundary.destination.x)
        {
            return false;
        }
        let old = boundary
            .old_source
            .as_deref()
            .map(|source| cx.text_engine.shape(source, font_size));
        if let Some(general) = boundary.general.as_ref() {
            let Some(old) = old.as_ref() else {
                return false;
            };
            let Ok(plan) = edit_projection::Projection::build(
                old,
                &current,
                general.old.clone(),
                general.new.clone(),
                cx.text_engine.shaper(),
                cx.scale_factor,
            ) else {
                return false;
            };
            let text_x = viewport.x - self.state.scroll_x;
            let Ok(fragments) = plan.fragments(
                general.progress,
                (viewport.x - text_x)..(viewport.right() - text_x),
            ) else {
                return false;
            };
            for fragment in fragments {
                let left = snap_to_physical(text_x + fragment.clip.start, cx.scale_factor);
                let right = snap_to_physical(text_x + fragment.clip.end, cx.scale_factor);
                if right <= left {
                    continue;
                }
                cx.scene.begin_clip(
                    RectF::new(left, viewport.y, right - left, viewport.height),
                    None,
                );
                let run = match fragment.source {
                    edit_projection::Source::Old => old,
                    edit_projection::Source::Current => &current,
                };
                cx.text_engine.draw(
                    run,
                    text_x + fragment.offset,
                    text_top_y,
                    cx.theme.colors.typography.primary,
                    TEXT_FIELD_TEXT_LAYER,
                    cx.scene,
                );
                cx.scene.end_clip();
            }
            if self.text_capture(cx) && self.blink_caret_visible.get() {
                let edge = snap_to_physical(text_x + projection.x, cx.scale_factor);
                let empty = cx.text_engine.shape("", font_size);
                cx.text_engine.draw_with_backgrounds(
                    &empty,
                    text_x,
                    text_top_y,
                    cx.theme.colors.typography.primary,
                    vec![TextBackgroundSpan {
                        x_offset: edge - text_x,
                        y_offset: 0.0,
                        width: cx.theme.typography.caret_thickness,
                        height: caret_height,
                        color: caret_color,
                    }],
                    TEXT_FIELD_CARET_LAYER,
                    cx.scene,
                );
            }
            return true;
        }
        if let Some(range) = boundary.range.as_ref() {
            let Some(old) = old.as_ref() else {
                return false;
            };
            if Self::range_runs_match(
                old,
                &current,
                range,
                cx.text_engine.shaper(),
                cx.scale_factor,
                viewport.width,
            )
            .map(|(_, new_join)| new_join)
                != Some(boundary.destination.x)
            {
                return false;
            }
        } else if boundary.kind == CaretBoundaryKind::Conceal
            && !old
                .as_ref()
                .is_some_and(|old| Self::boundary_prefix_matches(&current, old))
        {
            return false;
        }
        let run = if boundary.kind == CaretBoundaryKind::Conceal {
            old.as_ref().unwrap()
        } else {
            &current
        };
        let text_width = if boundary.range.is_some() {
            current.width
        } else {
            boundary.destination.x
        };
        let text_x = if text_width <= viewport.width {
            viewport.x
        } else {
            viewport.x - self.state.scroll_x
        };
        let boundary_x = snap_to_physical(text_x + projection.x, cx.scale_factor);
        if !boundary_x.is_finite()
            || boundary_x < viewport.x
            || boundary_x > viewport.x + viewport.width
        {
            return false;
        }
        cx.scene.begin_clip(
            RectF::new(
                viewport.x,
                viewport.y,
                boundary_x - viewport.x,
                viewport.height,
            ),
            None,
        );
        cx.text_engine.draw(
            run,
            text_x,
            text_top_y,
            cx.theme.colors.typography.primary,
            TEXT_FIELD_TEXT_LAYER,
            cx.scene,
        );
        cx.scene.end_clip();
        if boundary.range.is_some() {
            // The complete current suffix joins the same physical edge. This
            // changes only its draw origin; canonical layout is untouched.
            cx.scene.begin_clip(
                RectF::new(
                    boundary_x,
                    viewport.y,
                    viewport.x + viewport.width - boundary_x,
                    viewport.height,
                ),
                None,
            );
            cx.text_engine.draw(
                &current,
                boundary_x - boundary.destination.x,
                text_top_y,
                cx.theme.colors.typography.primary,
                TEXT_FIELD_TEXT_LAYER,
                cx.scene,
            );
            cx.scene.end_clip();
        }
        if self.text_capture(cx) && self.blink_caret_visible.get() {
            let empty = cx.text_engine.shape("", font_size);
            cx.text_engine.draw_with_backgrounds(
                &empty,
                text_x,
                text_top_y,
                cx.theme.colors.typography.primary,
                vec![TextBackgroundSpan {
                    x_offset: boundary_x - text_x,
                    y_offset: 0.0,
                    width: cx.theme.typography.caret_thickness,
                    height: caret_height,
                    color: caret_color,
                }],
                TEXT_FIELD_CARET_LAYER,
                cx.scene,
            );
        }
        true
    }

    /// Fire the on_change callback with the current text.
    fn fire_on_change(&mut self) {
        if let Some(cb) = &mut self.on_change {
            cb(self.state.document.text());
        }
    }

    /// SPEC-HAL-101 Stage-4C: Update caret blink from raw frame time.
    fn update_caret_blink_from_frame_time(
        &self,
        now_ms: u64,
        blink_interval_ms: u64,
        active: bool,
    ) {
        delegate::update_caret_blink(
            active,
            &self.blink_last_ms,
            &self.blink_caret_visible,
            now_ms,
            blink_interval_ms,
        );
    }

    /// Reset caret to visible on user activity.
    /// Called from event handlers (NOT paint phase) - no guard needed.
    pub(crate) fn reset_caret_blink_on_activity(&self, cx: &EventContext) {
        if self.event_text_active(cx) {
            delegate::reset_caret_blink(
                &self.blink_last_ms,
                &self.blink_caret_visible,
                cx.frame_time_ms(),
            );
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.set_text(&text);
        self
    }

    /// Enable or disable SecureEntry password masking.
    pub fn with_password(mut self, is_password: bool) -> Self {
        self.set_password(is_password);
        self
    }

    /// Set the maximum text length in UTF-8 bytes.
    pub fn with_max_len(mut self, max_len_bytes: Option<usize>) -> Self {
        self.set_max_len_bytes(max_len_bytes);
        self
    }

    /// Enable or disable SecureEntry password masking.
    pub fn set_password(&mut self, is_password: bool) {
        self.snap_smooth_caret();
        self.smooth_caret.mask_boundary = None;
        self.password_mask_font_id = None;
        if is_password {
            // SecureEntry must not retain an ordinary reveal/conceal source.
            // Keep only the scalar handle until its lawful Update cancellation.
            self.smooth_caret.boundary = None;
            self.smooth_caret.projection = None;
            self.smooth_caret.range_origin = None;
        }
        if self.config.is_password != is_password {
            self.config.is_password = is_password;
            self.layout_dirty.set(true);
        }
    }

    /// Set the maximum text length in UTF-8 bytes.
    pub fn set_max_len_bytes(&mut self, max_len_bytes: Option<usize>) {
        self.config.max_len_bytes = max_len_bytes;
        self.enforce_current_text_constraints();
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.config.placeholder = Some(placeholder.into());
        self
    }

    /// SPEC-HAL-319: Mutate placeholder in place (used by ConfigureWidget).
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.config.placeholder = Some(placeholder.into());
    }

    /// SPEC-HAL-319: Read placeholder (used by tree snapshot for diff reporting).
    pub fn placeholder(&self) -> Option<&str> {
        self.config.placeholder.as_deref()
    }

    // Embedded hosts supply value colors while retaining ownership of their shell.
    pub(crate) fn paint_with_value_colors(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        caret_color: Color,
        selection_color: Color,
    ) {
        self.update_caret_blink_from_frame_time(
            cx.frame_time_ms(),
            530,
            self.text_selection_visible(cx),
        );

        self.cached_paint_rect_width.set(rect.width);

        let t = cx.theme;
        let geom = t.geometry.text_field;
        let chrome_state = if !self.config.is_enabled {
            ControlChromeState::Disabled
        } else if self.text_capture(cx) {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = t.input_chrome(chrome_state);

        // Paint background + border (skip when embedded in host control like ComboBox)
        if !self.embedded {
            let emphasized = cx.has_focus_cue() || self.text_capture(cx);
            if let Some(appearance) = &self.input_surface_appearance
                && cx.cell_editor_presentation()
                    == crate::core::appearance::CellEditorPresentation::Standalone
            {
                paint_selected_input_surface(
                    cx,
                    rect,
                    appearance,
                    &style,
                    chrome_state,
                    self.hover,
                );
            } else {
                paint_input_surface(cx, rect, &style, emphasized);
            }
        }

        let padding_x = geom.padding_x;
        let padding_y = geom.padding_y;
        let baseline_offset = t.typography.baseline_offset;
        let font_size = t.typography.input_font_size;
        let ascent = self.ascent;
        let descent = self.descent;

        let text_viewport = self.text_viewport(rect, padding_x, padding_y);
        if self.leading_width > 0.0
            && let Some(definition) = self.leading_icon.and_then(|id| cx.icon_registry.get(id))
        {
            let size = self.leading_width - self.padding_x;
            let centered_top = (rect.height - size) * 0.5;
            let clearance = centered_top.max(0.0);
            let offset = self.leading_icon_y_offset.clamp(-clearance, clearance);
            cx.scene.begin_clip(rect, None);
            cx.scene.push_icon(
                definition,
                snap_to_physical(rect.x + self.padding_x, cx.scale_factor),
                snap_to_physical(rect.y + centered_top + offset, cx.scale_factor),
                size,
                t.colors.typography.primary,
                LAYER_CONTENT + 2,
            );
            cx.scene.end_clip();
        }
        // SPEC-HAL-092 Section 8A: Use baseline-anchored text placement
        // INV-TYPO-CTRL-1: Control text uses actual font metrics, not line_height
        // INV-TYPO-CTRL-2: Glyphs must be fully contained within control bounds
        let baseline_y =
            rect.y + compute_control_text_baseline_y(rect.height, ascent, descent, baseline_offset);
        // draw() takes top-of-text-box, so subtract ascent from baseline
        let text_top_y = baseline_y - ascent;

        // Get current caret position (for rendering)
        let caret_idx = self.state.cursor.pos;

        // SPEC-HAL-141: ensure_focus_visible is now called from layout, not paint.
        // Paint only reads scroll_x, never writes it.

        let viewport_width = text_viewport.width;
        let scroll_x = self.state.scroll_x;
        let text_align = self.text_align;
        // Alignment helper: when text fits the viewport, honor text_align;
        // otherwise (overflow), keep the existing left-anchored scroll behavior.
        let align_x_for = |text_width: f32| -> f32 {
            if text_width <= viewport_width {
                match text_align {
                    TextAlign::Left => text_viewport.x,
                    TextAlign::Center => text_viewport.x + (viewport_width - text_width) / 2.0,
                    TextAlign::Right => text_viewport.x + viewport_width - text_width,
                }
            } else {
                text_viewport.x - scroll_x
            }
        };

        // Begin clip
        cx.scene.begin_clip(text_viewport, None);

        let caret_height = ascent + descent;

        // Draw text or placeholder with integrated selection/caret backgrounds
        let text = self.state.document.text();
        if self.paint_password_mask_boundary(
            cx,
            rect,
            text_viewport,
            text_top_y,
            font_size,
            caret_height,
            caret_color,
        ) || self.paint_caret_boundary(
            cx,
            rect,
            text_viewport,
            text_top_y,
            font_size,
            caret_height,
            caret_color,
        ) {
            cx.scene.end_clip();
            return;
        }
        if !text.is_empty() {
            if self.layout_dirty.get() {
                // Cache is stale (mutation between layout and paint, or a
                // freshly-constructed TextField whose ascent/descent are
                // still 0). Recompute metrics + text position from the text
                // engine directly. No boundary reads. Selection/caret skip
                // one frame (layout rebuilds cache next frame).
                let metrics = cx.text_engine.font_metrics(font_size);
                let fresh_ascent = metrics.ascent;
                let fresh_descent = metrics.descent;
                let fresh_baseline_y = rect.y
                    + compute_control_text_baseline_y(
                        rect.height,
                        fresh_ascent,
                        fresh_descent,
                        baseline_offset,
                    );
                let fresh_text_top_y = fresh_baseline_y - fresh_ascent;
                if self.config.is_password {
                    let natural_advance = cx.text_engine.measure(PASSWORD_MASK_TEXT, font_size).0;
                    let mask_advance =
                        Self::password_mask_advance_for_scale(natural_advance, cx.scale_factor);
                    self.password_mask_advance.set(mask_advance);
                    let mask_count = text.chars().count();
                    let fresh_text_width = mask_count as f32 * mask_advance;
                    let fresh_text_x =
                        snap_to_physical(align_x_for(fresh_text_width), cx.scale_factor);
                    let mask_run = cx.text_engine.shape(PASSWORD_MASK_TEXT, font_size);
                    for index in 0..mask_count {
                        cx.text_engine.draw(
                            &mask_run,
                            fresh_text_x + index as f32 * mask_advance,
                            fresh_text_top_y,
                            t.colors.typography.primary,
                            TEXT_FIELD_TEXT_LAYER,
                            cx.scene,
                        );
                    }
                } else {
                    // HAL049 §18: ephemeral preparation keeps the defensive
                    // stale-cache presentation registered without publishing
                    // geometry or canonical editor state from Paint.
                    let mut fresh = TextLayoutCache::new(font_size, t.colors.typography.primary);
                    fresh.rebuild(text, self.state.document.version(), Self::SINGLE_LINE_LAYOUT_WIDTH, cx.text_engine);
                    let text_x = align_x_for(fresh.content_width() - fresh.caret_thickness());
                    let mut line_y = 0.0;
                    for line in fresh.lines() {
                        let mut run_x = text_x;
                        for face_run in &line.runs {
                            let run = fresh.shape_run(text, face_run, cx.text_engine)
                                .expect("UI-TYPOGRAPHY-INPUT: TextField stale paint binding failed");
                            cx.text_engine.draw(&run, run_x, fresh_baseline_y + line_y - run.ascent,
                                t.colors.typography.primary, TEXT_FIELD_TEXT_LAYER, cx.scene);
                            run_x += face_run.width;
                        }
                        line_y += line.height;
                    }
                }
            } else {
                // Normal path — cache is fresh, full rendering with
                // selection highlights and caret.
                if self.config.is_password {
                    let natural_advance = cx.text_engine.measure(PASSWORD_MASK_TEXT, font_size).0;
                    let mask_advance =
                        Self::password_mask_advance_for_scale(natural_advance, cx.scale_factor);
                    self.password_mask_advance.set(mask_advance);
                    let text_width = self.text_width_from_boundaries();
                    let text_x = snap_to_physical(align_x_for(text_width), cx.scale_factor);
                    let mut backgrounds = Vec::new();

                    if self.text_selection_visible(cx)
                        && self.state.selection.anchor != self.state.selection.head
                    {
                        let sel_start = self.state.selection.anchor.min(self.state.selection.head);
                        let sel_end = self.state.selection.anchor.max(self.state.selection.head);
                        let x0_logical = self.x_for_selection_start(sel_start);
                        let x1_logical = self.x_for_selection_end(sel_end);
                        let sel_width = x1_logical - x0_logical;
                        if sel_width > 0.01 {
                            backgrounds.push(TextBackgroundSpan {
                                x_offset: x0_logical,
                                y_offset: 0.0,
                                width: sel_width,
                                height: caret_height,
                                color: selection_color,
                            });
                        }
                    }

                    if !backgrounds.is_empty() {
                        let background_run = cx.text_engine.shape("", font_size);
                        cx.text_engine.draw_with_backgrounds(
                            &background_run,
                            text_x,
                            text_top_y,
                            t.colors.typography.primary,
                            backgrounds,
                            TEXT_FIELD_TEXT_LAYER,
                            cx.scene,
                        );
                    }

                    let mask_run = cx.text_engine.shape(PASSWORD_MASK_TEXT, font_size);
                    for index in 0..self.password_cluster_count() {
                        cx.text_engine.draw(
                            &mask_run,
                            text_x + index as f32 * mask_advance,
                            text_top_y,
                            t.colors.typography.primary,
                            LAYER_CONTENT + 3,
                            cx.scene,
                        );
                    }

                    if self.text_capture(cx) && self.blink_caret_visible.get() {
                        let canonical = self.caret_x_from_boundaries(caret_idx);
                        let font_id = mask_run
                            .binding
                            .as_ref()
                            .map(|binding| binding.face().font_id());
                        let projected =
                            if font_id.is_some() && font_id == self.password_mask_font_id {
                                self.painted_caret_x(cx, rect, canonical)
                            } else {
                                canonical
                            };
                        let caret_x = snap_to_physical(text_x + projected, cx.scale_factor);
                        let caret_run = cx.text_engine.shape("", font_size);
                        cx.text_engine.draw_with_backgrounds(
                            &caret_run,
                            caret_x,
                            text_top_y,
                            caret_color,
                            vec![TextBackgroundSpan {
                                x_offset: 0.0,
                                y_offset: 0.0,
                                width: t.typography.caret_thickness,
                                height: caret_height,
                                color: caret_color,
                            }],
                            TEXT_FIELD_CARET_LAYER,
                            cx.scene,
                        );
                    }
                } else {
                    let text_width = self.text_width_from_boundaries();
                    let text_x = align_x_for(text_width);
                    let mut backgrounds = Vec::new();

                    if self.text_selection_visible(cx)
                        && self.state.selection.anchor != self.state.selection.head
                    {
                        let sel_start = self.state.selection.anchor.min(self.state.selection.head);
                        let sel_end = self.state.selection.anchor.max(self.state.selection.head);
                        let x0_logical = self.x_for_selection_start(sel_start);
                        let x1_logical = self.x_for_selection_end(sel_end);
                        let sel_width = x1_logical - x0_logical;
                        if sel_width > 0.01 {
                            backgrounds.push(TextBackgroundSpan {
                                x_offset: x0_logical,
                                y_offset: 0.0,
                                width: sel_width,
                                height: caret_height,
                                color: selection_color,
                            });
                        }
                    }

                    if self.text_capture(cx) && self.blink_caret_visible.get() {
                        let caret_x_logical =
                            self.painted_caret_x(cx, rect, self.caret_x_from_boundaries(caret_idx));
                        backgrounds.push(TextBackgroundSpan {
                            x_offset: caret_x_logical,
                            y_offset: 0.0,
                            width: t.typography.caret_thickness,
                            height: caret_height,
                            color: caret_color,
                        });
                    }

                    // Attach the existing backgrounds once to the first visual
                    // run, preserving the single-run scene and canonical control
                    // baseline while a fallback face may have another ascent.
                    let mut backgrounds = Some(backgrounds);
                    let cache = &self.state.layout_cache;
                    let mut line_y = 0.0;
                    for line in cache.lines() {
                        let mut run_x = text_x;
                        for face_run in &line.runs {
                            let run = cache.shape_run(text, face_run, cx.text_engine)
                                .expect("UI-TYPOGRAPHY-INPUT: TextField paint binding failed");
                            let run_y = baseline_y + line_y - run.ascent;
                            let mut run_backgrounds = backgrounds.take().unwrap_or_default();
                            for background in &mut run_backgrounds {
                                background.y_offset -= run_y - text_top_y;
                            }
                            cx.text_engine.draw_with_backgrounds(&run, run_x, run_y,
                                t.colors.typography.primary, run_backgrounds, TEXT_FIELD_TEXT_LAYER, cx.scene);
                            run_x += face_run.width;
                        }
                        line_y += line.height;
                    }
                }
            }
        } else if let Some(ref placeholder) = self.config.placeholder {
            if !placeholder.is_empty() {
                let mut color = t.colors.typography.muted;
                color.a = t.typography.placeholder_opacity;
                let mut placeholder_cache = TextLayoutCache::new(font_size, color);
                placeholder_cache.rebuild(placeholder, 0, Self::SINGLE_LINE_LAYOUT_WIDTH, cx.text_engine);
                let placeholder_x = align_x_for(placeholder_cache.content_width() - placeholder_cache.caret_thickness());
                let mut line_y = 0.0;
                for line in placeholder_cache.lines() {
                    let mut run_x = placeholder_x;
                    for face_run in &line.runs {
                        let run = placeholder_cache.shape_run(placeholder, face_run, cx.text_engine)
                            .expect("UI-TYPOGRAPHY-INPUT: TextField placeholder binding failed");
                        cx.text_engine.draw(&run, run_x, baseline_y + line_y - run.ascent,
                            color, TEXT_FIELD_TEXT_LAYER, cx.scene);
                        run_x += face_run.width;
                    }
                    line_y += line.height;
                }
            }

            // Draw caret via text pipeline for cross-pipeline z-order
            if self.text_capture(cx) && self.blink_caret_visible.get() {
                let caret_x = align_x_for(0.0);
                let caret_bg = vec![TextBackgroundSpan {
                    x_offset: self.painted_caret_x(cx, rect, 0.0),
                    y_offset: 0.0,
                    width: t.typography.caret_thickness,
                    height: caret_height,
                    color: caret_color,
                }];
                let empty_run = cx.text_engine.shape("\u{200B}", font_size);
                cx.text_engine.draw_with_backgrounds(
                    &empty_run,
                    caret_x,
                    text_top_y,
                    t.colors.typography.primary,
                    caret_bg,
                    TEXT_FIELD_TEXT_LAYER,
                    cx.scene,
                );
            }
        } else {
            // No text and no placeholder - just draw caret if captured
            if self.text_capture(cx) && self.blink_caret_visible.get() {
                let caret_x = align_x_for(0.0);
                let caret_bg = vec![TextBackgroundSpan {
                    x_offset: self.painted_caret_x(cx, rect, 0.0),
                    y_offset: 0.0,
                    width: t.typography.caret_thickness,
                    height: caret_height,
                    color: caret_color,
                }];
                let empty_run = cx.text_engine.shape("\u{200B}", font_size);
                cx.text_engine.draw_with_backgrounds(
                    &empty_run,
                    caret_x,
                    text_top_y,
                    t.colors.typography.primary,
                    caret_bg,
                    TEXT_FIELD_TEXT_LAYER,
                    cx.scene,
                );
            }
        }

        // End clip
        cx.scene.end_clip();
    }

    /// Mark this TextField as embedded inside a host control (e.g. ComboBox).
    /// When embedded, the TextField skips painting its own background and border,
    /// rendering only text, caret, and selection. The host control owns the outer shell.
    #[cfg(test)]
    pub(crate) fn set_embedded(&mut self, embedded: bool) {
        self.snap_smooth_caret();
        self.embedded = embedded;
    }

    pub(crate) fn from_composite_seed(
        seed: crate::widgets::composite_edit_host::CompositeEditorSeed,
    ) -> Self {
        use crate::widgets::composite_edit_host::CompositeEditorFilter;
        let mut field = Self::new().with_text(seed.text).with_text_align(seed.align);
        field.set_smooth_caret_prototype(seed.smooth_caret_enabled);
        field.composite_input_palette = seed.palette
            == crate::widgets::composite_edit_host::CompositeEditorPalette::InputChrome;
        field.embedded = true;
        field.composite_role = true;
        if let Some(placeholder) = seed.placeholder {
            field.set_placeholder(placeholder);
        }
        match seed.filter {
            CompositeEditorFilter::Any => {}
            CompositeEditorFilter::Integer => {
                field.set_input_filter(|ch| ch.is_ascii_digit() || ch == '-')
            }
            CompositeEditorFilter::Decimal => {
                field.set_input_filter(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '.'))
            }
        }
        field
    }

    pub(crate) fn composite_snapshot(
        &self,
        id: crate::core::composite_text_edit::CompositeEditorId,
        session: u64,
    ) -> crate::core::composite_text_edit::CompositeEditSnapshot {
        crate::core::composite_text_edit::CompositeEditSnapshot {
            id,
            session,
            text: self.text().to_owned(),
            revision: self.state.document.version(),
            cursor: self.state.cursor.pos,
            selection_anchor: self.state.selection.anchor,
            selection_focus: self.state.selection.head,
        }
    }

    fn place_touch_caret(&mut self, cx: &mut EventContext, local_x: f32) {
        if self.layout_dirty.get() {
            self.rebuild_layout_cache_now(cx.text_engine());
        }
        let x = local_x - self.padding_x - self.leading_width - self.alignment_offset()
            + self.state.scroll_x;
        let hit = self.hit_test_text_x(self.state.layout_cache.boundaries(), x);
        self.state.cursor.pos = hit;
        self.state.cursor.preferred_x = None;
        self.state.selection = Selection::at(hit);
        self.state.drag_mode = TextFieldDragMode::None;
        self.reset_caret_blink(cx);
        cx.invalidate_paint();
    }

    pub(crate) fn run_composite_operation(
        &mut self,
        operation: crate::core::composite_text_edit::CompositeEditOperation,
        cx: &mut EventContext,
        event: Option<&Event>,
    ) -> (crate::core::composite_text_edit::CompositeEditOutcome, bool) {
        use crate::core::composite_text_edit::{
            CompositeEditOperation as Op, CompositeEditOutcome as Outcome,
        };
        let forward = matches!(
            operation,
            Op::ForwardCurrentInput | Op::SeedForEntryThenCurrentInput { .. }
        );
        match operation {
            Op::SeedForEntry { text, select_all }
            | Op::SeedForEntryThenCurrentInput { text, select_all }
            | Op::ResetAfterResolverCommit { text, select_all } => {
                self.set_text(&text);
                if select_all {
                    self.select_all();
                }
                self.reset_caret_blink_on_activity(cx);
            }
            Op::ClearAfterTagCommand => self.clear_document(),
            Op::ObserveAndActivity => self.reset_caret_blink_on_activity(cx),
            Op::PlaceTouchCaret { local_x } => {
                let x = cx.composite_local_position().map_or(local_x, |p| p.0);
                self.place_touch_caret(cx, x);
            }
            #[cfg(any(target_arch = "wasm32", test))]
            Op::BrowserText(action) => {
                return (Outcome::Applied, self.handle_browser_text(cx, action));
            }
            Op::RetireDelegation => {
                self.snap_smooth_caret();
                self.composite_delegation = None;
                self.state.drag_mode = TextFieldDragMode::None;
                self.state.clear_composition();
                self.state.undo_history.resume();
                self.blink_caret_visible.set(false);
            }
            Op::ForwardCurrentInput => {}
        }
        if !forward {
            return (Outcome::Applied, true);
        }
        let Some(event) = event else {
            return (Outcome::WrongPhase, false);
        };
        use crate::core::text_edit_policy::{
            PolicyDecision, TextEditPolicy, decide_text_edit_policy,
        };
        match decide_text_edit_policy(
            &TextEditPolicy {
                enabled: self.config.is_enabled,
                read_only: self.config.is_read_only,
            },
            event,
        ) {
            PolicyDecision::DenyConsume => return (Outcome::DeniedConsume, true),
            PolicyDecision::DenyBubble => return (Outcome::DeniedBubble, false),
            PolicyDecision::Allow => {}
        }
        // Enter/Cancel are host operations. A child must never substitute its
        // standalone submit/release behavior for a composite continuation.
        if matches!(event, Event::Key(key) if matches!(key.key_code, KeyCode::Enter | KeyCode::Escape))
        {
            return (Outcome::DeniedBubble, false);
        }
        (
            Outcome::Applied,
            self.handle_event(cx, event, EventPhase::Target),
        )
    }

    pub fn text(&self) -> &str {
        self.state.document.text()
    }

    /// Read-only selection projection within the text owner.
    #[allow(
        dead_code,
        reason = "Retained read-only selection query within the text owner"
    )]
    pub(crate) fn selection(&self) -> &Selection {
        &self.state.selection
    }

    /// Read-only cursor byte position within the text owner.
    #[allow(dead_code, reason = "Retained byte-position query for the text owner")]
    pub(crate) fn cursor_pos(&self) -> usize {
        self.state.cursor.pos
    }

    /// Clear text content without requiring tree context.
    /// Preserves capture/focus state. For use by embedded hosts (e.g. TagInput).
    pub(crate) fn clear_document(&mut self) {
        self.snap_smooth_caret();
        self.state.document.set_text("");
        self.state.cursor.pos = 0;
        self.state.selection.anchor = 0;
        self.state.selection.head = 0;
        self.layout_dirty.set(true);
    }

    /// Select all text in the field.
    pub fn select_all(&mut self) {
        self.snap_smooth_caret();
        let text_len = self.state.document.text().len();
        crate::core::text_edit_delegate::select_all(
            &mut self.state.cursor,
            &mut self.state.selection,
            text_len,
        );
    }

    /// Replace the entire text content programmatically.
    pub fn set_text(&mut self, text: &str) {
        self.snap_smooth_caret();
        let text = self.clamp_text_to_max_len(text);
        self.state.document.set_text(&text);
        self.state.cursor.pos = text.len();
        self.state.selection = Selection::at(text.len());
        self.state.layout_cache.mark_stale();
        self.layout_dirty.set(true);
    }

    fn clamp_text_to_max_len(&self, text: &str) -> String {
        match self.config.max_len_bytes {
            Some(max_len) if text.len() > max_len => {
                let end = utf8_floor_boundary(text, max_len);
                text[..end].to_string()
            }
            _ => text.to_string(),
        }
    }

    fn enforce_current_text_constraints(&mut self) {
        let Some(max_len) = self.config.max_len_bytes else {
            return;
        };
        if self.state.document.len() <= max_len {
            return;
        }

        let constrained = {
            let text = self.state.document.text();
            let end = utf8_floor_boundary(text, max_len);
            text[..end].to_string()
        };
        self.state.document.set_text(&constrained);
        let len = constrained.len();
        self.state.cursor.pos = len;
        self.state.selection = Selection::at(len);
        self.state.layout_cache.mark_stale();
        self.layout_dirty.set(true);
    }

    fn selected_len(&self) -> usize {
        self.state
            .selection
            .anchor
            .abs_diff(self.state.selection.head)
    }

    fn can_insert_text(&self, text: &str) -> bool {
        self.state
            .document
            .len()
            .checked_sub(self.selected_len())
            .and_then(|retained| retained.checked_add(text.len()))
            .is_some_and(|length| {
                length <= isize::MAX as usize
                    && self.config.max_len_bytes.is_none_or(|max| length <= max)
            })
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn browser_text_projection(
        &self,
    ) -> Option<crate::core::event_router::browser_text::BrowserTextProjection> {
        use crate::core::event_router::browser_text::{BrowserTextKind, BrowserTextProjection};
        if !self.config.is_enabled || self.config.is_read_only {
            return None;
        }
        BrowserTextProjection::new(
            self.text(), self.state.document.version(),
            (self.state.selection.anchor, self.state.selection.head),
            if self.config.is_password { BrowserTextKind::Password } else { BrowserTextKind::Line },
            self.state.is_composing(),
        )
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn browser_clipboard_selection(&self, cut: bool) -> Option<&str> {
        let policy = TextEditPolicy {
            enabled: self.config.is_enabled,
            read_only: self.config.is_read_only,
        };
        if self.config.is_password
            || (cut && self.state.is_composing())
            || crate::core::text_edit_policy::decide_text_edit_operation(&policy, || cut)
                != PolicyDecision::Allow
        {
            return None;
        }
        delegate::get_selection_text(self.state.document.text(), &self.state.selection)
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn handle_browser_text(
        &mut self, cx: &mut EventContext,
        action: crate::core::event_router::browser_text::BrowserTextAction,
    ) -> bool {
        use crate::core::event_router::browser_text::BrowserTextAction;
        if !self.config.is_enabled || self.config.is_read_only
            || cx.keyboard_mode != KeyboardAuthorityMode::Capture
        {
            return false;
        }
        match action {
            BrowserTextAction::Cut => {
                if self.browser_clipboard_selection(true).is_none() {
                    return false;
                }
                let range = (!self.smooth_caret.snap && self.range_destination().is_some())
                    .then_some(CaretRangeKind::SelectionDelete);
                self.apply_cut(cx, range);
                true
            }
            BrowserTextAction::Composition(ime) => {
                if let ImeEvent::Commit { text } = &ime
                    && self.input_filter.as_ref().is_some_and(|filter| {
                        !text.chars().all(filter)
                    })
                {
                    return self.handle_event(cx, &Event::Ime(ImeEvent::Cancel), EventPhase::Target);
                }
                let revision = self.state.document.version();
                let handled = self.handle_event(cx, &Event::Ime(ime), EventPhase::Target);
                if self.state.document.version() != revision {
                    self.fire_on_change();
                }
                handled
            }
            BrowserTextAction::Select { anchor, head } => {
                if self.state.is_composing()
                    || !self.text().is_char_boundary(anchor) || !self.text().is_char_boundary(head)
                {
                    return false;
                }
                self.snap_smooth_caret();
                self.state.selection = Selection { anchor, head };
                self.state.cursor.pos = head;
                self.reset_caret_blink(cx);
                cx.invalidate_layout();
                true
            }
            BrowserTextAction::Replace { start, end, text } => {
                if self.state.is_composing() || start > end
                    || !self.text().is_char_boundary(start) || !self.text().is_char_boundary(end)
                    || self.input_filter.as_ref().is_some_and(|filter| !text.chars().all(filter))
                {
                    return false;
                }
                let previous = self.state.selection.clone();
                self.state.selection = Selection { anchor: start, head: end };
                if !self.can_insert_text(&text) {
                    self.state.selection = previous;
                    return false;
                }
                self.state.cursor.pos = end;
                self.handle_event(cx, &Event::Text(crate::core::event::TextInputEvent {
                    text, modifiers: Default::default(),
                }), EventPhase::Target)
            }
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn browser_paste_state(&self) -> Result<BrowserPasteState, BrowserPasteRefusal> {
        if !self.config.is_enabled {
            return Err(BrowserPasteRefusal::Disabled);
        }
        if self.config.is_read_only {
            return Err(BrowserPasteRefusal::ReadOnly);
        }
        if self.state.is_composing() {
            return Err(BrowserPasteRefusal::CompositionActive);
        }
        let document = self.state.document.text();
        if [
            self.state.cursor.pos,
            self.state.selection.anchor,
            self.state.selection.head,
        ]
        .into_iter()
        .any(|offset| !document.is_char_boundary(offset))
        {
            return Err(BrowserPasteRefusal::InvalidState);
        }
        let selected_bytes = self.selected_len();
        let retained = document
            .len()
            .checked_sub(selected_bytes)
            .ok_or(BrowserPasteRefusal::InvalidState)?;
        let remaining_bytes = self
            .config
            .max_len_bytes
            .map(|max| {
                max.checked_sub(retained)
                    .ok_or(BrowserPasteRefusal::InvalidState)
            })
            .transpose()?;
        Ok(BrowserPasteState {
            revision: self.state.document.version(),
            cursor: self.state.cursor.pos,
            anchor: self.state.selection.anchor,
            head: self.state.selection.head,
            document_bytes: document.len(),
            selected_bytes,
            max_len_bytes: self.config.max_len_bytes,
            remaining_bytes,
            normalize_paste_newlines: self.editing_policy.normalize_paste_newlines,
        })
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn apply_browser_paste(
        &mut self,
        cx: &mut EventContext,
        source: BrowserPasteState,
        text: String,
    ) -> Result<(), BrowserPasteRefusal> {
        if cx.keyboard_mode != KeyboardAuthorityMode::Capture
            || self.browser_paste_state()? != source
        {
            return Err(BrowserPasteRefusal::StaleSource);
        }
        // Same P7/P8 admission as the existing Paste command; no synthetic Key/Text.
        let range = (!self.has_active_selection()
            && !self.smooth_caret.snap
            && self.range_destination().is_some())
        .then_some(CaretRangeKind::Paste);
        if self.apply_clipboard_text(cx, text, range) {
            Ok(())
        } else {
            Err(BrowserPasteRefusal::ContentRejected)
        }
    }

    /// Complete the Cut after the clipboard transport has accepted the selection.
    fn apply_cut(&mut self, cx: &mut EventContext, range_event: Option<CaretRangeKind>) {
        let before = self.capture_snapshot();
        let timestamp = cx.frame_time_ms();
        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read.
        if self.layout_dirty.get() {
            self.rebuild_layout_cache_now(cx.text_engine());
        }
        let range_origin = range_event.and_then(|_| self.range_destination());
        let boundaries = self.state.layout_cache.boundaries();
        delegate::perform_cut(
            self.state.document.content_mut(),
            &mut self.state.selection,
            &mut self.state.cursor,
            boundaries,
        );
        self.state.document.mark_modified();
        self.layout_dirty.set(true);
        self.state.layout_cache.mark_stale();
        self.rebuild_layout_cache_now(cx.text_engine());
        if let Some(kind) = range_event {
            self.record_range_boundary(&before, range_origin, kind, cx.text_engine());
        } else {
            self.snap_smooth_caret();
        }
        self.record_edit(before, EditKind::Cut, timestamp);
        cx.invalidate_layout();
        self.reset_caret_blink(cx);
        self.fire_on_change();
    }

    /// Shared native/browser Paste transaction. Transport never owns editing/history.
    fn apply_clipboard_text(
        &mut self,
        cx: &mut EventContext,
        clipboard_text: String,
        range_event: Option<CaretRangeKind>,
    ) -> bool {
        let paste_text = if self.editing_policy.normalize_paste_newlines {
            delegate::normalize_paste_newlines(clipboard_text)
        } else {
            clipboard_text
        };
        if paste_text.is_empty() || !self.can_insert_text(&paste_text) {
            self.snap_smooth_caret();
            return false;
        }
        let before = self.capture_snapshot();
        let timestamp = cx.frame_time_ms();
        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read.
        if self.layout_dirty.get() {
            self.rebuild_layout_cache_now(cx.text_engine());
        }
        let range_origin = range_event.and_then(|_| self.range_destination());
        let boundaries = self.state.layout_cache.boundaries();
        delegate::perform_paste(
            self.state.document.content_mut(),
            &mut self.state.selection,
            &mut self.state.cursor,
            boundaries,
            &paste_text,
        );
        self.state.document.mark_modified();
        self.reset_caret_blink(cx);
        self.layout_dirty.set(true);
        self.state.layout_cache.mark_stale();
        self.rebuild_layout_cache_now(cx.text_engine());
        if let Some(kind) = range_event {
            self.record_range_boundary(&before, range_origin, kind, cx.text_engine());
        } else {
            self.snap_smooth_caret();
        }
        self.record_edit(before, EditKind::Paste, timestamp);
        cx.invalidate_layout();
        self.fire_on_change();
        true
    }

    // =========================================================================
    // Text editing helpers (Stage 1: minimal changes for compilation)
    // =========================================================================

    /// Reset caret blink to visible
    fn reset_caret_blink(&self, cx: &EventContext) {
        delegate::reset_caret_blink(
            &self.blink_last_ms,
            &self.blink_caret_visible,
            cx.frame_time_ms(),
        );
    }

    // =========================================================================
    // SPEC-HAL-055: Undo/Redo helpers
    // =========================================================================

    /// Capture current state as an EditSnapshot for undo/redo.
    fn capture_snapshot(&self) -> EditSnapshot {
        delegate::capture_snapshot(
            self.state.document.text(),
            &self.state.cursor,
            &self.state.selection,
        )
    }

    /// Record an edit in undo history.
    ///
    /// Call this AFTER the mutation is complete, passing the before snapshot.
    fn record_edit(&mut self, before: EditSnapshot, kind: EditKind, timestamp_ms: u64) {
        delegate::record_edit(
            &mut self.state.undo_history,
            self.state.document.text(),
            &self.state.cursor,
            &self.state.selection,
            before,
            kind,
            timestamp_ms,
        );
    }

    /// Perform undo operation.
    ///
    /// Returns true if undo was performed, false if nothing to undo.
    /// SPEC-HAL-055 Stage 1.1: Clears any active IME composition.
    fn perform_undo(&mut self, cx: &mut EventContext) -> bool {
        let performed = delegate::perform_undo(
            &mut self.state.ime_pre_composition_snapshot,
            &mut self.state.undo_history,
            self.state.document.content_mut(),
            &mut self.state.cursor,
            &mut self.state.selection,
            &self.layout_dirty,
        );
        if performed {
            self.snap_smooth_caret();
            self.state.document.mark_modified();
            // INV-TEXT-GEOM-PHASE: Mark cache stale after document mutation
            self.layout_dirty.set(true);
            self.state.layout_cache.mark_stale();
            cx.invalidate_layout();
            self.reset_caret_blink(cx);
            self.fire_on_change();
        }
        performed
    }

    /// Perform redo operation.
    ///
    /// Returns true if redo was performed, false if nothing to redo.
    /// SPEC-HAL-055 Stage 1.1: Clears any active IME composition.
    fn perform_redo(&mut self, cx: &mut EventContext) -> bool {
        let performed = delegate::perform_redo(
            &mut self.state.ime_pre_composition_snapshot,
            &mut self.state.undo_history,
            self.state.document.content_mut(),
            &mut self.state.cursor,
            &mut self.state.selection,
            &self.layout_dirty,
        );
        if performed {
            self.snap_smooth_caret();
            self.state.document.mark_modified();
            // INV-TEXT-GEOM-PHASE: Mark cache stale after document mutation
            self.layout_dirty.set(true);
            self.state.layout_cache.mark_stale();
            cx.invalidate_layout();
            self.reset_caret_blink(cx);
            self.fire_on_change();
        }
        performed
    }

    // =========================================================================
    // Scrolling helpers
    // =========================================================================

    /// SPEC-HAL-107: Single-line TextField uses huge width to prevent wrapping.
    /// Paragraph engine will never wrap at this width, ensuring single visual line.
    /// SPEC-HAL-109 Stage F: Used when editing_policy.single_line is true.
    const SINGLE_LINE_LAYOUT_WIDTH: f32 = 100_000.0;

    /// Get layout width based on editing policy.
    fn layout_width_for_policy(&self, constrained_width: f32) -> f32 {
        if self.editing_policy.single_line {
            Self::SINGLE_LINE_LAYOUT_WIDTH
        } else {
            constrained_width
        }
    }

    fn password_cluster_count(&self) -> usize {
        if self.state.document.is_empty() {
            return 0;
        }
        self.state.layout_cache.boundaries().len().saturating_sub(1)
    }

    fn password_mask_index_for_byte(&self, byte_idx: usize) -> usize {
        let boundaries = self.state.layout_cache.boundaries();
        let cluster_count = boundaries.len().saturating_sub(1);

        for (i, boundary) in boundaries.iter().enumerate() {
            if boundary.byte_start == byte_idx {
                return i.min(cluster_count);
            }
            if boundary.byte_start > byte_idx {
                return i.saturating_sub(1).min(cluster_count);
            }
        }

        cluster_count
    }

    fn password_x_for_byte(&self, byte_idx: usize) -> f32 {
        self.password_mask_index_for_byte(byte_idx) as f32 * self.password_mask_advance.get()
    }

    fn password_hit_test_x(&self, x_logical: f32) -> usize {
        let boundaries = self.state.layout_cache.boundaries();
        let cluster_count = boundaries.len().saturating_sub(1);
        let mask_advance = self.password_mask_advance.get();
        if cluster_count == 0 || mask_advance <= 0.0 {
            return 0;
        }

        let index = ((x_logical / mask_advance) + 0.5)
            .floor()
            .clamp(0.0, cluster_count as f32) as usize;
        boundaries
            .get(index)
            .map(|boundary| boundary.byte_start)
            .unwrap_or_else(|| self.state.document.len())
    }

    fn hit_test_text_x(&self, boundaries: &[ClusterBoundary], x_logical: f32) -> usize {
        if self.config.is_password {
            self.password_hit_test_x(x_logical)
        } else {
            hit_test_x_y(boundaries, x_logical, 0.0)
        }
    }

    fn double_click_selection(
        &self,
        boundaries: &[ClusterBoundary],
        text: &str,
        hit_pos: usize,
    ) -> WordBoundary {
        if self.config.is_password {
            WordBoundary {
                start: 0,
                end: text.len(),
            }
        } else {
            find_word_at(boundaries, text, hit_pos)
        }
    }

    fn password_mask_advance_for_scale(natural_advance: f32, scale: f32) -> f32 {
        if !natural_advance.is_finite()
            || natural_advance <= 0.0
            || !scale.is_finite()
            || scale <= 0.0
        {
            return natural_advance;
        }

        (natural_advance * scale).round().max(1.0) / scale
    }

    /// SPEC-HAL-107: Get caret x_logical from boundaries (no shaping)
    fn caret_x_from_boundaries(&self, caret_idx: usize) -> f32 {
        if self.config.is_password {
            return self.password_x_for_byte(caret_idx);
        }

        let boundaries = self.state.layout_cache.boundaries();
        // Find boundary where byte_start == caret_idx
        for b in boundaries {
            if b.byte_start == caret_idx {
                return b.x_logical;
            }
        }
        // Fallback: if caret_idx not found, use sentinel (last boundary) x_logical
        boundaries.last().map(|b| b.x_logical).unwrap_or(0.0)
    }

    /// Get x_logical for selection start edge (left edge of first selected cluster).
    fn x_for_selection_start(&self, byte_idx: usize) -> f32 {
        if self.config.is_password {
            return self.password_x_for_byte(byte_idx);
        }

        let boundaries = self.state.layout_cache.boundaries();
        for b in boundaries {
            if b.byte_start == byte_idx {
                return b.x_logical;
            }
        }
        // Fallback: sentinel
        boundaries.last().map(|b| b.x_logical).unwrap_or(0.0)
    }

    /// Get x_logical for selection end edge (right edge of last selected cluster).
    /// Returns the NEXT boundary's x_logical to include full glyph width.
    fn x_for_selection_end(&self, byte_idx: usize) -> f32 {
        if self.config.is_password {
            return self.password_x_for_byte(byte_idx);
        }

        let boundaries = self.state.layout_cache.boundaries();
        // Find the boundary at byte_idx, then return the next boundary's x_logical
        for (i, b) in boundaries.iter().enumerate() {
            if b.byte_start == byte_idx {
                // Return next boundary's x_logical if it exists
                if let Some(next) = boundaries.get(i + 1) {
                    return next.x_logical;
                }
                // No next boundary - use this one (shouldn't happen for valid selection)
                return b.x_logical;
            }
        }
        // Fallback: sentinel
        boundaries.last().map(|b| b.x_logical).unwrap_or(0.0)
    }

    /// SPEC-HAL-107: Get total text width from boundaries (sentinel's x_logical)
    fn text_width_from_boundaries(&self) -> f32 {
        if self.config.is_password {
            return self.password_cluster_count() as f32 * self.password_mask_advance.get();
        }

        let boundaries = self.state.layout_cache.boundaries();
        // Sentinel is the last boundary on line 0 - its x_logical is the line width
        boundaries.last().map(|b| b.x_logical).unwrap_or(0.0)
    }

    /// Compute the x offset from text_viewport.x to the aligned text start.
    /// Returns 0.0 for Left, centering offset for Center, right offset for Right.
    /// When text overflows the viewport, returns 0.0 (scroll takes over).
    fn alignment_offset(&self) -> f32 {
        if self.text_align == TextAlign::Left {
            return 0.0;
        }
        let rect_width = self.cached_paint_rect_width.get();
        if rect_width <= 0.0 {
            return 0.0;
        }
        let viewport_width = (rect_width - self.padding_x * 2.0 - self.leading_width).max(0.0);
        let text_width = self.text_width_from_boundaries();
        if text_width > viewport_width {
            return 0.0; // overflow — scroll mode, no alignment offset
        }
        match self.text_align {
            TextAlign::Left => 0.0,
            TextAlign::Center => (viewport_width - text_width) / 2.0,
            TextAlign::Right => viewport_width - text_width,
        }
    }

    // =========================================================================
    // Focus target helpers (SPEC-HAL-109 Stage H.1)
    // =========================================================================

    /// Caret X position in content coordinates (INV-H.FOCUS1).
    fn caret_content_x(&self) -> f32 {
        self.caret_x_from_boundaries(self.state.cursor.pos)
    }

    /// Selection head X position in content coordinates (INV-H.FOCUS1).
    fn selection_head_content_x(&self) -> f32 {
        self.caret_x_from_boundaries(self.state.selection.head)
    }

    /// Determine if selection is active (anchor != head).
    fn has_active_selection(&self) -> bool {
        self.state.selection.anchor != self.state.selection.head
    }

    /// Unified focus target X position in content coordinates.
    /// INV-H.FOCUS1: Single source of horizontal focus tracking.
    fn focus_target_content_x(&self) -> f32 {
        if self.has_active_selection() {
            self.selection_head_content_x()
        } else {
            self.caret_content_x()
        }
    }

    // =========================================================================
    // Horizontal scroll visibility (SPEC-HAL-101 section 7)
    // =========================================================================

    /// Ensure focus target is visible within text viewport.
    /// SPEC-HAL-107: Uses boundary lookup instead of prefix-shaping.
    /// SPEC-HAL-101 section 7: Scroll extent includes the trailing caret footprint.
    /// SPEC-HAL-141: Takes &mut self - called from layout/event phase only, not paint.
    fn ensure_focus_visible(&mut self, text_viewport: RectF, caret_margin: f32) {
        let viewport_width = text_viewport.width;

        let target_w = self.state.layout_cache.caret_thickness();
        let content_width = if self.config.is_password {
            self.text_width_from_boundaries() + target_w
        } else {
            self.state.layout_cache.content_width()
        };
        let max_scroll = (content_width - viewport_width).max(0.0);
        self.state.scroll_x = self.state.scroll_x.clamp(0.0, max_scroll);

        // Tight alignment may allocate less than one caret, including zero.
        // Keep scrolling bounded; full containment is physically impossible here.
        if viewport_width < target_w {
            return;
        }

        let scroll_x_before = self.state.scroll_x;

        // INV-H.VIS4: No scroll when content fits viewport
        if content_width <= viewport_width {
            self.state.scroll_x = 0.0;
            return;
        }

        // INV-H.FOCUS1: Single focus target (selection.head or caret)
        let target_x = self.focus_target_content_x();

        // Visibility bounds with margin
        let effective_margin = if viewport_width < caret_margin * 2.0 + target_w + 1.0 {
            0.0
        } else {
            caret_margin
        };
        let visible_left = scroll_x_before + effective_margin;
        let visible_right = scroll_x_before + viewport_width - effective_margin - target_w;

        // INV-H.VIS2: Target fully visible → return (idempotence)
        if target_x >= visible_left && target_x <= visible_right {
            return;
        }

        // Compute new scroll position
        let new_scroll_x = if target_x < visible_left {
            // Target left of viewport → align left edge
            (target_x - effective_margin).max(0.0)
        } else {
            // Target right of viewport → align right edge
            target_x + target_w + effective_margin - viewport_width
        };

        // INV-H.VIS4: Clamp to valid range (no overscroll)
        let clamped_scroll_x = new_scroll_x.clamp(0.0, max_scroll);

        self.state.scroll_x = clamped_scroll_x;

        // Debug assertions for post-conditions
        #[cfg(debug_assertions)]
        {
            let final_scroll = self.state.scroll_x;
            // INV-H.VIS4: No overscroll
            debug_assert!(
                final_scroll >= 0.0 && final_scroll <= max_scroll + 0.001,
                "INV-H.VIS4 violated: scroll_x {} not in [0, {}]",
                final_scroll,
                max_scroll
            );
            // INV-H.VIS1: Target should be visible.
            // Margin is best-effort: at content boundaries (target_x near 0 or near
            // content_width), scroll can't create margin because it clamps to [0, max_scroll].
            // Only assert the target is actually on-screen.
            let target_viewport_x = target_x - final_scroll;
            debug_assert!(
                target_viewport_x >= -0.5 && target_viewport_x + target_w <= viewport_width + 0.5,
                "INV-H.VIS1 violated: target at {:.1} not visible in viewport width {:.1} \
                 (scroll={:.1}, target_x={:.1})",
                target_viewport_x,
                viewport_width,
                final_scroll,
                target_x
            );
        }
    }

    /// Compute caret X position with scroll offset applied
    /// SPEC-HAL-107: Uses boundary lookup instead of prefix-shaping.
    #[allow(dead_code, reason = "Retained scroll-adjusted caret geometry query")]
    fn caret_x_scrolled(&self, caret_idx: usize, text_viewport: RectF) -> f32 {
        let text_start_x = text_viewport.x;
        let caret_x_raw = text_start_x + self.caret_x_from_boundaries(caret_idx);
        caret_x_raw - self.state.scroll_x
    }

    /// Compute text viewport rect from widget rect and padding
    fn text_viewport(&self, rect: RectF, padding_x: f32, padding_y: f32) -> RectF {
        let width = rect.width - padding_x * 2.0 - self.leading_width;
        RectF::new(
            rect.x + padding_x + self.leading_width,
            rect.y + padding_y,
            if self.leading_icon.is_some() {
                width.max(0.0)
            } else {
                width
            },
            if self.leading_icon.is_some() {
                (rect.height - padding_y * 2.0).max(0.0)
            } else {
                rect.height - padding_y * 2.0
            },
        )
    }

    // =========================================================================
    // INV-TEXT-GEOM-PHASE: Synchronous Cache Rebuild
    // =========================================================================

    /// Synchronously rebuild layout cache.
    ///
    /// INV-TEXT-GEOM-PHASE: Use this when geometry must be read after mutation
    /// in the same handler. Requires text_engine access.
    ///
    /// Pattern:
    /// 1. Perform mutation
    /// 2. mark_stale()
    /// 3. rebuild_layout_cache_now()
    /// 4. Read geometry
    fn rebuild_layout_cache_now(
        &mut self,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) {
        let doc = &self.state.document;
        let doc_version = doc.version();
        let cache = &mut self.state.layout_cache;
        cache.rebuild(doc.text(), doc_version, self.last_layout_width, text_engine);
        self.layout_dirty.set(false);
    }

    /// Public hook for hosts (e.g. ComboBox) that mutate or replace the
    /// embedded TextField during event handling — `set_text()` or rebuilding
    /// the TextField marks the cache stale, but the framework's next layout
    /// pass may not run before paint. Call this from the event handler with
    /// the width paint will use (typically the host's cached content width)
    /// so the next paint reads a fresh cache instead of tripping
    /// `assert_fresh`.
    ///
    /// `font_size` is needed to populate `ascent`/`descent` so paint can
    /// compute the correct text baseline before the next `layout()` pass
    /// runs — without this, a freshly-replaced TextField's metrics are 0
    /// and text renders shifted/clipped.
    pub fn rebuild_layout_cache(
        &mut self,
        layout_width: f32,
        font_size: f32,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) {
        self.last_layout_width = self.layout_width_for_policy(layout_width);
        let metrics = text_engine.font_metrics(font_size);
        self.ascent = metrics.ascent;
        self.descent = metrics.descent;
        let doc = &self.state.document;
        let doc_version = doc.version();
        let cache = &mut self.state.layout_cache;
        cache.set_font_size(font_size);
        cache.rebuild(doc.text(), doc_version, self.last_layout_width, text_engine);
        self.layout_dirty.set(false);
    }
}

fn utf8_floor_boundary(text: &str, max_len: usize) -> usize {
    if max_len >= text.len() {
        return text.len();
    }

    let mut end = 0;
    for (idx, ch) in text.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_len {
            break;
        }
        end = next;
    }
    end
}

#[cfg(test)]
mod config_enforcement_tests {
    use super::*;

    #[test]
    fn max_len_clamps_programmatic_text_at_utf8_boundary() {
        let mut field = TextField::new().with_max_len(Some(5));

        field.set_text("abcdef");
        assert_eq!(field.text(), "abcde");
        assert_eq!(field.cursor_pos(), 5);

        field.set_text("ééé");
        assert_eq!(field.text(), "éé");
        assert_eq!(field.cursor_pos(), 4);
    }

    #[test]
    fn max_len_rejects_whole_insert_that_would_exceed_limit() {
        let mut field = TextField::new().with_max_len(Some(5)).with_text("abcd");

        assert!(field.can_insert_text("e"));
        assert!(!field.can_insert_text("ef"));

        field.state.selection.anchor = 1;
        field.state.selection.head = 4;
        assert!(field.can_insert_text("xyz"));
        assert!(!field.can_insert_text("xyzabc"));
    }

    #[test]
    fn password_geometry_maps_underlying_bytes_to_mask_clusters() {
        let mut field = TextField::new().with_text("aé🙂").with_password(true);
        field.password_mask_advance.set(10.0);
        field.state.layout_cache.set_boundaries_for_test(vec![
            ClusterBoundary {
                byte_start: 0,
                byte_end: 1,
                line_index: 0,
                x_logical: 0.0,
            },
            ClusterBoundary {
                byte_start: 1,
                byte_end: 3,
                line_index: 0,
                x_logical: 7.0,
            },
            ClusterBoundary {
                byte_start: 3,
                byte_end: 7,
                line_index: 0,
                x_logical: 19.0,
            },
            ClusterBoundary {
                byte_start: 7,
                byte_end: 7,
                line_index: 0,
                x_logical: 31.0,
            },
        ]);
        field.state.layout_cache.set_cached_doc_len_for_test(7);
        field.state.layout_cache.set_lines_for_test(1, 20.0);
        field.state.layout_cache.clear_stale_for_test();

        assert_eq!(field.password_cluster_count(), 3);
        assert_eq!(field.caret_x_from_boundaries(0), 0.0);
        assert_eq!(field.caret_x_from_boundaries(1), 10.0);
        assert_eq!(field.caret_x_from_boundaries(3), 20.0);
        assert_eq!(field.caret_x_from_boundaries(7), 30.0);
        assert_eq!(
            field.hit_test_text_x(field.state.layout_cache.boundaries(), 16.0),
            3
        );
    }

    #[test]
    fn password_mask_advance_uses_one_physical_pixel_phase() {
        let advance = TextField::password_mask_advance_for_scale(6.055_664, 2.0);
        assert_eq!(advance, 6.0);

        let origin = 18.5;
        for index in 0..32 {
            let x_phys = (origin + index as f32 * advance) * 2.0;
            assert_eq!(x_phys.fract(), 0.0);
        }
    }

    #[test]
    fn password_double_click_selects_the_whole_masked_value() {
        let field = TextField::new()
            .with_text("alpha-beta-gamma")
            .with_password(true);

        let selection = field.double_click_selection(&[], field.text(), 8);

        assert_eq!(selection.start, 0);
        assert_eq!(selection.end, field.text().len());
    }
}

/// The ordinary TextField surface, also used by tree-owned numeric editor hosts.
/// This paints chrome only; callers retain geometry and Focus/Capture authority.
pub(crate) fn paint_input_surface(
    cx: &mut PaintContext,
    rect: RectF,
    style: &crate::core::theme::InputChrome,
    emphasized: bool,
) {
    let chrome = style.chrome;
    let scale = cx.scale_factor;
    let one_px = cx.one_px();
    let bx = snap_to_physical(rect.x, scale);
    let by = snap_to_physical(rect.y, scale);
    let br = snap_to_physical(rect.right(), scale);
    let bb = snap_to_physical(rect.bottom(), scale);
    let bw = br - bx;
    let bh = bb - by;
    if cx.cell_editor_presentation() == crate::core::appearance::CellEditorPresentation::CellIntegrated {
        cx.scene.push_rect(
            RectF::new(bx, by, bw, bh),
            chrome.fill.unwrap_or(cx.theme.colors.surfaces.content),
            LAYER_CONTENT + 2,
            None,
        );
        return;
    }
    let edge_thick = if emphasized {
        (chrome.border_width * scale).round().max(1.0) / scale
    } else {
        one_px
    };
    let radius = cx.theme.chrome_radius(
        ControlChromeCategory::RecessedEditable,
        cx.theme.geometry.text_field.radius,
        false,
    );
    cx.scene.push_rounded_rect(
        RectF::new(bx, by, bw, bh),
        chrome.border_color,
        RoundedCorners::uniform(radius),
        LAYER_CONTENT,
        None,
    );
    cx.scene.push_rounded_rect(
        RectF::new(
            bx + edge_thick,
            by + edge_thick,
            (bw - edge_thick * 2.0).max(0.0),
            (bh - edge_thick * 2.0).max(0.0),
        ),
        style.inner_edge,
        RoundedCorners::uniform((radius - edge_thick).max(0.0)),
        LAYER_CONTENT + 1,
        cx.scene_key,
    );
    let inset = edge_thick + one_px;
    cx.scene.push_rounded_rect(
        RectF::new(
            bx + inset,
            by + inset,
            (bw - 2.0 * inset).max(0.0),
            (bh - 2.0 * inset).max(0.0),
        ),
        chrome.fill.unwrap_or(cx.theme.colors.surfaces.content),
        RoundedCorners::uniform((radius - inset).max(0.0)),
        LAYER_CONTENT + 2,
        None,
    );
    if emphasized {
        cx.scene.push_border_rounded_rect(
            RectF::new(bx, by, bw, bh),
            chrome.border_color,
            edge_thick,
            RoundedCorners::uniform(radius),
            crate::core::paint::LAYER_FOCUS + 1,
            None,
        );
    }
}

/// Selected standalone chrome only; shared numeric/composite chrome stays intact.
fn paint_selected_input_surface(
    cx: &mut PaintContext,
    rect: RectF,
    appearance: &InputSurfaceAppearance,
    style: &crate::core::theme::InputChrome,
    state: ControlChromeState,
    hovered: bool,
) {
    let scale = cx.scale_factor;
    let bx = snap_to_physical(rect.x, scale);
    let by = snap_to_physical(rect.y, scale);
    let br = snap_to_physical(rect.right(), scale);
    let bb = snap_to_physical(rect.bottom(), scale);
    let (width, height) = (br - bx, bb - by);
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let face = RectF::new(bx, by, width, height);
    let radius = appearance.radius().min(width * 0.5).min(height * 0.5);
    let emphasized = matches!(
        state,
        ControlChromeState::Focused | ControlChromeState::Captured
    );
    let edge = if emphasized {
        (style.chrome.border_width * scale).round().max(1.0) / scale
    } else {
        cx.one_px()
    };
    let border = if state == ControlChromeState::Idle {
        appearance.border()
    } else {
        appearance
            .focus_border(state)
            .unwrap_or(style.chrome.border_color)
    };

    // The real material is first. Subsequent outlines have transparent interiors,
    // preserving both gradient interpolation and its authored alpha.
    let surface = appearance.surface_for_interaction(
        hovered && state == ControlChromeState::Idle,
        emphasized,
        false,
    );
    cx.scene.push_gradient_rect(
        face,
        radius,
        surface.definition(),
        LAYER_CONTENT,
        None,
    );
    if width > edge * 2.0 && height > edge * 2.0 {
        cx.scene.push_border_rounded_rect(
            RectF::new(
                bx + edge,
                by + edge,
                width - edge * 2.0,
                height - edge * 2.0,
            ),
            appearance.inner_edge(),
            cx.one_px(),
            RoundedCorners::uniform((radius - edge).max(0.0)),
            LAYER_CONTENT + 1,
            cx.scene_key,
        );
    }
    cx.scene.push_border_rounded_rect(
        face,
        border,
        edge,
        RoundedCorners::uniform(radius),
        if emphasized {
            crate::core::paint::LAYER_FOCUS + 1
        } else {
            LAYER_CONTENT + 2
        },
        None,
    );
}

impl Default for TextField {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for TextField {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TextField {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Scoped intrinsic measurement must not consume editing/focus initialization.
        if !self.embedded
            && cx.control_parts_request() == Some(crate::core::layout::ControlPartsRequest::Measure)
        {
            let geom = cx.theme.geometry.text_field;
            let metrics = cx
                .text_engine
                .font_metrics(cx.theme.typography.input_font_size);
            let line_height = crate::core::text_engine::compute_control_line_height(
                cx.theme.typography.line_height,
                &metrics,
                cx.theme.typography.baseline_offset,
            );
            let req = crate::core::layout::ControlPartsRequirements {
                leading: Default::default(),
                trailing: Default::default(),
                // A flexible editor reserves a usable minimum, never the edited string.
                body_width: geom.min_hit_size.max(
                    geom.padding_x * 2.0
                        + cx.theme.typography.caret_thickness
                        + self.leading_width_for(
                            f32::INFINITY,
                            geom.padding_y * 2.0 + line_height,
                            geom.padding_x,
                            geom.padding_y,
                            cx.theme.typography.input_font_size,
                            cx.theme.typography.caret_thickness,
                        ),
                ),
                height: geom.padding_y * 2.0 + line_height,
            };
            cx.report_control_parts(req);
            return LayoutResult::fit(Size::new(req.body_width, req.height));
        }
        if self.start_captured {
            self.start_captured = false;
            // SPEC-HAL-328 §3.1: request focus + capture via the proper framework
            // machinery. The post-layout handler fires `on_focus` and
            // `on_capture_enter`, which set `self.focused` and
            // `self.blink_caret_visible` for us — no direct state writes here.
            cx.defer_focus_to_self();
            // Place cursor at end of text so user can append.
            self.state.cursor.pos = self.state.document.text().len();
            self.state.cursor.preferred_x = None;
            self.state.selection.clear(self.state.cursor.pos);
        }
        // INV-GEOM-SYNC-TEXTFIELD: Theme geometry sync (CANONICAL LOCATION)
        // Theme → cached field sync MUST happen here in layout(), before paint/event.
        // Paint/event code MUST NOT read theme directly for geometry.
        // Constructor defaults are inert fallbacks only.
        let geom = cx.theme.geometry.text_field;
        let font_size = cx.theme.typography.input_font_size;
        let metrics = cx.text_engine.font_metrics(font_size);
        let line_height = crate::core::text_engine::compute_control_line_height(
            cx.theme.typography.line_height,
            &metrics,
            cx.theme.typography.baseline_offset,
        );
        let padding_y = geom.padding_y;
        let padding_x = geom.padding_x;

        // Sync cached geometry for event handler use
        self.padding_x = padding_x;

        self.ascent = metrics.ascent;
        self.descent = metrics.descent;
        self.password_mask_advance
            .set(cx.text_engine.measure(PASSWORD_MASK_TEXT, font_size).0);
        self.password_mask_font_id = if self.config.is_password
            && !self.embedded
            && self.smooth_caret_enabled
            && self.config.is_enabled
            && !self.state.is_composing()
            && self.caret_animation_demand == Some(true)
        {
            cx.text_engine
                .shape(PASSWORD_MASK_TEXT, font_size)
                .binding
                .as_ref()
                .map(|binding| binding.face().font_id())
        } else {
            None
        };

        // Text viewport width (inside padding)
        let max_width = constraints.max_width;
        let height =
            (padding_y * 2.0 + line_height).clamp(constraints.min_height, constraints.max_height);
        self.leading_width = self.leading_width_for(
            max_width,
            height,
            padding_x,
            padding_y,
            font_size,
            cx.theme.typography.caret_thickness,
        );
        let text_width = (max_width - padding_x * 2.0 - self.leading_width).max(0.0);

        let doc = &self.state.document;
        let doc_version = doc.version();

        // SPEC-HAL-109 Stage F: Get layout width from policy
        let layout_width = self.layout_width_for_policy(text_width);

        // INV-TEXT-GEOM-PHASE: Track layout width for synchronous rebuild
        self.last_layout_width = layout_width;

        // Configure cache
        let cache = &mut self.state.layout_cache;
        cache.set_font_size(font_size);
        cache.set_color(cx.theme.colors.typography.primary); // use the main text color
        cache.set_caret_thickness(cx.theme.typography.caret_thickness);

        // SPEC-HAL-107: Rebuild if layout_dirty OR cache needs rebuild
        // layout_dirty is set by editing events in handle_event()
        if self.layout_dirty.get() || cache.needs_rebuild(doc_version, layout_width)
            || cache.typography_changed(cx.text_engine)
                .expect("UI-TYPOGRAPHY-INPUT: TextField typography identity failed") {
            cache.rebuild(doc.text(), doc_version, layout_width, cx.text_engine);
            self.layout_dirty.set(false);
        }

        #[cfg(debug_assertions)]
        {
            let b = cache.boundaries();
            let t = doc.text();
            debug_assert!(
                b.len() >= 2 || t.is_empty(),
                "TextField layout cache must provide at least 2 boundaries (clusters + sentinel) for non-empty text"
            );
            if !t.is_empty() {
                debug_assert!(
                    b.last().unwrap().byte_start == t.len(),
                    "TextField cache sentinel must end at text.len()"
                );
            }
        }

        debug_assert!(
            !cache.boundaries().is_empty(),
            "TextLayoutCache: no boundaries for TextField"
        );
        debug_assert!(
            !cache.lines().is_empty(),
            "TextLayoutCache: no lines for TextField"
        );

        let width = constraints.max_width;
        // Embedded editors can receive their first click before their first Paint.
        if width.is_finite() {
            self.cached_paint_rect_width.set(width);
        }

        // SPEC-HAL-141: Compute scroll visibility in layout phase (not paint).
        // This ensures scroll_x is set before paint reads it.
        let text_viewport = RectF::new(
            padding_x + self.leading_width,
            padding_y,
            text_width,
            (height - padding_y * 2.0).max(0.0),
        );
        let caret_margin = cx.theme.typography.caret_margin;
        self.ensure_focus_visible(text_viewport, caret_margin);

        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let style = cx.theme.input_chrome(ControlChromeState::Idle);
        let caret = if self.embedded && !self.composite_input_palette {
            cx.theme.colors.accents.primary
        } else {
            style.caret
        };
        self.paint_with_value_colors(cx, rect, caret, style.selection);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        if matches!(event, Event::Key(_) | Event::Text(_) | Event::Ime(_))
            && cx.keyboard_mode != KeyboardAuthorityMode::Capture
        {
            return false;
        }

        let range_event = if cx.keyboard_mode == KeyboardAuthorityMode::Capture
            && matches!(event, Event::Key(_) | Event::Text(_))
        {
            self.range_event_kind(event)
        } else {
            None
        };
        if self.smooth_caret_enabled {
            let boundary = self.has_active_selection()
                || match event {
                    Event::Ime(_) => true,
                    Event::Pointer(pe) => {
                        pe.kind != PointerEventKind::Move
                            || self.state.drag_mode != TextFieldDragMode::None
                    }
                    Event::Text(te) => te.modifiers.ctrl || te.modifiers.alt || te.modifiers.meta,
                    Event::Key(ke) if ke.is_down => {
                        // INV-101-SC-REPEAT: only the event's verified repeat flag qualifies.
                        (ke.is_repeat
                            && !(matches!(ke.key_code, KeyCode::Space | KeyCode::Backspace)
                                && !ke.modifiers.shift
                                && !ke.modifiers.ctrl
                                && !ke.modifiers.alt
                                && !ke.modifiers.meta
                                && (self.caret_boundary_origin().is_some()
                                    || self.password_mask_origin().is_some()))
                            && matches!(
                                ke.key_code,
                                KeyCode::Space
                                    | KeyCode::Left
                                    | KeyCode::Right
                                    | KeyCode::Home
                                    | KeyCode::End
                                    | KeyCode::Backspace
                                    | KeyCode::Delete
                            ))
                            || ke.modifiers.ctrl
                            || ke.modifiers.alt
                            || ke.modifiers.meta
                            || (ke.modifiers.shift
                                && matches!(
                                    ke.key_code,
                                    KeyCode::Left
                                        | KeyCode::Right
                                        | KeyCode::Home
                                        | KeyCode::End
                                        | KeyCode::Backspace
                                        | KeyCode::Delete
                                ))
                            || matches!(
                                ke.key_code,
                                KeyCode::Enter | KeyCode::Escape
                            )
                    }
                    _ => false,
                };
            if boundary && range_event.is_none() {
                self.snap_smooth_caret();
            }
        }

        let policy = TextEditPolicy {
            enabled: self.config.is_enabled,
            read_only: self.config.is_read_only,
        };
        match decide_text_edit_policy(&policy, event) {
            PolicyDecision::Allow => {}
            PolicyDecision::DenyConsume => return true,
            PolicyDecision::DenyBubble => return false,
        }

        if !self.embedded && self.config.is_enabled && !self.config.is_read_only {
            if let Event::Pointer(pe) = event {
                if cx.widget_global_rect().is_some_and(|rect| {
                    RectF::new(0.0, 0.0, rect.width, rect.height).contains(
                        crate::core::math::Point::new(
                            cx.composite_local_position().unwrap_or(pe.local_pos).0,
                            cx.composite_local_position().unwrap_or(pe.local_pos).1,
                        ),
                    )
                }) {
                    cx.request_cursor(crate::core::event::CursorIcon::Text);
                }
            }
        }

        // Reset caret to visible on any user activity (event phase, not paint phase)
        self.reset_caret_blink_on_activity(cx);

        match event {
            Event::Pointer(pe) => {
                // Use cached padding_x (synced from theme in layout)
                let padding_x = self.padding_x + self.leading_width;

                match pe.kind {
                    PointerEventKind::Down => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        cx.request_keyboard_capture();
                        if !self.composite_role {
                            self.focused = true;
                        }

                        // Request pointer capture for drag
                        cx.capture_pointer(PointerButton::Primary);

                        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                        if self.layout_dirty.get() {
                            self.rebuild_layout_cache_now(cx.text_engine());
                        }

                        // Immediate caret placement via hit-test
                        let boundaries = self.state.layout_cache.boundaries();
                        let scroll_x = self.state.scroll_x;
                        let align_offset = self.alignment_offset();
                        let x_logical = cx.composite_local_position().unwrap_or(pe.local_pos).0
                            - padding_x
                            - align_offset
                            + scroll_x;
                        let hit_pos = self.hit_test_text_x(boundaries, x_logical);

                        // SPEC-HAL-107 §6.7: Selection reset rules
                        self.state.cursor.pos = hit_pos;
                        self.state.cursor.preferred_x = None; // §6.7.3: click clears preferred_x
                        if pe.modifiers.shift {
                            // §6.7.2: Shift-click preserves anchor, moves head
                            self.state.selection.head = hit_pos;
                        } else {
                            // §6.7.1: Plain click collapses selection
                            self.state.selection.anchor = hit_pos;
                            self.state.selection.head = hit_pos;
                        }

                        // Start drag mode - use selection anchor for shift-click consistency
                        self.state.drag_mode = TextFieldDragMode::Character {
                            anchor: self.state.selection.anchor,
                        };

                        self.reset_caret_blink(cx);
                        true
                    }
                    PointerEventKind::Move => {
                        // Drag selection while button is held
                        if let TextFieldDragMode::Character { anchor } = self.state.drag_mode {
                            // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                            if self.layout_dirty.get() {
                                self.rebuild_layout_cache_now(cx.text_engine());
                            }
                            let boundaries = self.state.layout_cache.boundaries();
                            let scroll_x = self.state.scroll_x;
                            let align_offset = self.alignment_offset();
                            let x_logical = cx.composite_local_position().unwrap_or(pe.local_pos).0
                                - padding_x
                                - align_offset
                                + scroll_x;
                            let hit_pos = self.hit_test_text_x(boundaries, x_logical);

                            // Update selection: anchor stays, head follows mouse
                            self.state.selection.anchor = anchor;
                            self.state.selection.head = hit_pos;
                            self.state.cursor.pos = hit_pos;

                            self.reset_caret_blink(cx);
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Up => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        // End drag mode and release capture
                        self.state.drag_mode = TextFieldDragMode::None;
                        cx.release_pointer();
                        true
                    }
                    PointerEventKind::DoubleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                        if self.layout_dirty.get() {
                            self.rebuild_layout_cache_now(cx.text_engine());
                        }
                        // Regular fields select the word at the click position.
                        // Password fields select the whole visually uniform mask.
                        let boundaries = self.state.layout_cache.boundaries();
                        let text = self.state.document.text();
                        let scroll_x = self.state.scroll_x;
                        let align_offset = self.alignment_offset();
                        let x_logical = cx.composite_local_position().unwrap_or(pe.local_pos).0
                            - padding_x
                            - align_offset
                            + scroll_x;
                        let hit_pos = self.hit_test_text_x(boundaries, x_logical);
                        let word = self.double_click_selection(boundaries, text, hit_pos);

                        // Apply the resolved double-click selection.
                        self.state.selection.anchor = word.start;
                        self.state.selection.head = word.end;
                        self.state.cursor.pos = word.end;

                        // Don't start drag on double-click
                        self.state.drag_mode = TextFieldDragMode::None;

                        self.reset_caret_blink(cx);
                        true
                    }
                    PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        // Triple-click: select all (single-line TextField)
                        let text_len = self.state.document.len();
                        self.state.selection.anchor = 0;
                        self.state.selection.head = text_len;
                        self.state.cursor.pos = text_len;
                        self.state.drag_mode = TextFieldDragMode::None;
                        self.reset_caret_blink(cx);
                        true
                    }
                    _ => false,
                }
            }
            // SPEC-HAL-107: Key handling via substrate (no char-based helpers)
            Event::Key(KeyEvent {
                key_code,
                modifiers,
                is_down: true,
                is_repeat,
                ..
            }) if cx.keyboard_mode == KeyboardAuthorityMode::Capture => {
                // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                if self.layout_dirty.get() {
                    self.rebuild_layout_cache_now(cx.text_engine());
                }
                let range_origin = range_event.and_then(|_| self.range_destination());
                let cursor_before = self.state.cursor.pos;
                let boundaries = self.state.layout_cache.boundaries();
                let eligible_navigation = self.smooth_caret_enabled
                    && !*is_repeat
                    && !modifiers.shift
                    && !modifiers.ctrl
                    && !modifiers.alt
                    && !modifiers.meta
                    && matches!(
                        key_code,
                        KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
                    );
                let navigation_before = if eligible_navigation {
                    self.smooth_destination()
                } else {
                    None
                };
                let mut cursor_moved = false;

                match key_code {
                    KeyCode::Enter => {
                        // Mark commit-in-progress BEFORE on_submit so the
                        // ensuing release_keyboard_capture's on_capture_leave
                        // sees the flag and skips firing on_cancel (would be
                        // a double-dispatch — see on_capture_leave below).
                        self.commit_pending = true;
                        if let Some(cb) = &mut self.on_submit {
                            cb(self.state.document.text());
                            cx.invalidate_layout();
                        }
                        if let Some(target) = self.on_submit_redirect {
                            cx.redirect_focus(target);
                        }
                        // SPEC-HAL-328 §11.4: single-line TextField Enter
                        // commits and releases capture.
                        cx.release_keyboard_capture();
                        return true;
                    }
                    KeyCode::Escape => {
                        if let Some(cb) = &mut self.on_cancel {
                            cb();
                            cx.invalidate_layout();
                        }
                        return true;
                    }
                    KeyCode::Left => {
                        let text = self.state.document.text();
                        delegate::handle_left_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            text,
                            modifiers,
                        );
                        cursor_moved = true;
                    }
                    KeyCode::Right => {
                        let text = self.state.document.text();
                        delegate::handle_right_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            text,
                            modifiers,
                        );
                        cursor_moved = true;
                    }
                    KeyCode::Home => {
                        delegate::handle_home_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            modifiers.shift,
                        );
                        cursor_moved = true;
                    }
                    KeyCode::End => {
                        delegate::handle_end_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            modifiers.shift,
                        );
                        cursor_moved = true;
                    }
                    KeyCode::Backspace => {
                        let edit_kind = delegate::edit_kind_for_backspace(&self.state.selection);
                        let boundary_origin = self.caret_boundary_origin();
                        let mask_origin = self.password_mask_origin();
                        let before = self.capture_snapshot();
                        let timestamp = cx.frame_time_ms();
                        if modifiers.alt || modifiers.ctrl {
                            crate::core::editing::apply_delete_word_back(
                                self.state.document.content_mut(),
                                &mut self.state.selection,
                                &mut self.state.cursor,
                                boundaries,
                            );
                        } else {
                            delegate::perform_backspace(
                                self.state.document.content_mut(),
                                &mut self.state.selection,
                                &mut self.state.cursor,
                                boundaries,
                            );
                        }
                        if before.text != self.state.document.text() {
                            self.state.document.mark_modified();
                            self.layout_dirty.set(true);
                            self.state.layout_cache.mark_stale();
                            self.rebuild_layout_cache_now(cx.text_engine());
                            if self.config.is_password {
                                self.record_password_mask_boundary(
                                    mask_origin,
                                    CaretBoundaryKind::Conceal,
                                    !modifiers.shift
                                        && !modifiers.ctrl
                                        && !modifiers.alt
                                        && !modifiers.meta,
                                );
                            } else if let Some(kind) = range_event {
                                self.record_range_boundary(
                                    &before,
                                    range_origin,
                                    kind,
                                    cx.text_engine(),
                                );
                            } else {
                                self.record_caret_boundary(
                                    &before,
                                    boundary_origin,
                                    CaretBoundaryKind::Conceal,
                                    !modifiers.shift
                                        && !modifiers.ctrl
                                        && !modifiers.alt
                                        && !modifiers.meta,
                                    cx.text_engine(),
                                );
                            }
                            self.record_edit(before, edit_kind, timestamp);
                            cx.invalidate_layout();
                            self.fire_on_change();
                        } else if before.selection_anchor != self.state.selection.anchor
                            || before.selection_head != self.state.selection.head
                            || *is_repeat
                        {
                            self.snap_smooth_caret();
                        }
                        cursor_moved = true;
                    }
                    KeyCode::Delete => {
                        let edit_kind = delegate::edit_kind_for_delete(&self.state.selection);
                        let before = self.capture_snapshot();
                        let timestamp = cx.frame_time_ms();
                        if modifiers.alt || modifiers.ctrl {
                            crate::core::editing::apply_delete_word_forward(
                                self.state.document.content_mut(),
                                &mut self.state.selection,
                                &mut self.state.cursor,
                                boundaries,
                            );
                        } else {
                            delegate::perform_delete(
                                self.state.document.content_mut(),
                                &mut self.state.selection,
                                &mut self.state.cursor,
                                boundaries,
                            );
                        }
                        if before.text != self.state.document.text() {
                            self.state.document.mark_modified();
                            self.layout_dirty.set(true);
                            self.state.layout_cache.mark_stale();
                            self.rebuild_layout_cache_now(cx.text_engine());
                            if let Some(kind) = range_event {
                                self.record_range_boundary(
                                    &before,
                                    range_origin,
                                    kind,
                                    cx.text_engine(),
                                );
                            } else {
                                self.snap_smooth_caret();
                            }
                            self.record_edit(before, edit_kind, timestamp);
                            cx.invalidate_layout();
                            self.fire_on_change();
                        } else if before.selection_anchor != self.state.selection.anchor
                            || before.selection_head != self.state.selection.head
                        {
                            self.snap_smooth_caret();
                        }
                        cursor_moved = true;
                    }
                    // Space is treated as text input (prevents activation fallback)
                    KeyCode::Space => {
                        if !self.can_insert_text(" ") {
                            if *is_repeat {
                                self.snap_smooth_caret();
                            }
                            return true;
                        }
                        let edit_kind = delegate::edit_kind_for_insert(&self.state.selection);
                        let boundary_origin = self.caret_boundary_origin();
                        let mask_origin = self.password_mask_origin();
                        let before = self.capture_snapshot();
                        let timestamp = cx.frame_time_ms();
                        delegate::perform_text_insert(
                            self.state.document.content_mut(),
                            &mut self.state.selection,
                            &mut self.state.cursor,
                            boundaries,
                            " ",
                        );
                        if before.text != self.state.document.text() {
                            self.state.document.mark_modified();
                            self.layout_dirty.set(true);
                            self.state.layout_cache.mark_stale();
                            self.rebuild_layout_cache_now(cx.text_engine());
                            if self.config.is_password {
                                self.record_password_mask_boundary(
                                    mask_origin,
                                    CaretBoundaryKind::Reveal,
                                    !modifiers.shift
                                        && !modifiers.ctrl
                                        && !modifiers.alt
                                        && !modifiers.meta,
                                );
                            } else {
                                self.record_caret_boundary(
                                    &before,
                                    boundary_origin,
                                    CaretBoundaryKind::Reveal,
                                    !modifiers.shift
                                        && !modifiers.ctrl
                                        && !modifiers.alt
                                        && !modifiers.meta,
                                    cx.text_engine(),
                                );
                            }
                            self.record_edit(before, edit_kind, timestamp);
                            cx.invalidate_layout();
                            self.fire_on_change();
                        } else if before.selection_anchor != self.state.selection.anchor
                            || before.selection_head != self.state.selection.head
                            || *is_repeat
                        {
                            self.snap_smooth_caret();
                        }
                        return true;
                    }
                    _ => {}
                }

                if cursor_moved {
                    if self.state.cursor.pos != cursor_before {
                        // HAL-101 §7: Layout owns scroll/reveal settlement. A
                        // changed logical caret must reach it before retained Paint.
                        cx.invalidate_layout();
                    }
                    self.reset_caret_blink(cx);
                    if eligible_navigation {
                        match (navigation_before, self.smooth_destination()) {
                            (Some(before), Some(after)) if before.revision == after.revision => {
                                if before.x != after.x {
                                    self.record_smooth_destination();
                                }
                            }
                            _ => self.snap_smooth_caret(),
                        }
                    }
                }
                cursor_moved
            }
            // SPEC-HAL-107: Text input via substrate
            Event::Text(text_event) if cx.keyboard_mode == KeyboardAuthorityMode::Capture => {
                // Clipboard and selection use the qualified target's primary modifier.
                let shortcut_mod =
                    crate::core::text_edit_policy::is_primary_shortcut(text_event.modifiers);

                // Select All: Cmd/Ctrl + A
                if shortcut_mod && (text_event.text == "a" || text_event.text == "A") {
                    delegate::select_all(
                        &mut self.state.cursor,
                        &mut self.state.selection,
                        self.state.document.len(),
                    );
                    self.reset_caret_blink(cx);
                    return true;
                }

                // SPEC-HAL-055: Undo - Cmd/Ctrl + Z (without Shift)
                if shortcut_mod
                    && !text_event.modifiers.shift
                    && (text_event.text == "z" || text_event.text == "Z")
                {
                    self.perform_undo(cx);
                    return true;
                }

                // SPEC-HAL-055: Preserve native redo and admit both browser conventions.
                if crate::core::text_edit_policy::is_redo_shortcut(
                    text_event.modifiers,
                    &text_event.text,
                ) {
                    self.perform_redo(cx);
                    return true;
                }

                // SPEC-HAL-056: Copy - Cmd/Ctrl + C
                if shortcut_mod && (text_event.text == "c" || text_event.text == "C") {
                    if self.config.is_password {
                        cx.clipboard_set_text("");
                    } else if let Some(selected_text) = delegate::get_selection_text(
                        self.state.document.text(),
                        &self.state.selection,
                    ) {
                        cx.clipboard_set_text(selected_text);
                    }
                    return true;
                }

                // SPEC-HAL-056: Cut - Cmd/Ctrl + X
                if shortcut_mod && (text_event.text == "x" || text_event.text == "X") {
                    let selected_text = delegate::get_selection_text(
                        self.state.document.text(),
                        &self.state.selection,
                    )
                    .map(str::to_owned);

                    let transferred = if self.config.is_password {
                        cx.clipboard_set_text("")
                    } else {
                        selected_text
                            .as_deref()
                            .is_some_and(|text| cx.clipboard_set_text(text))
                    };

                    if transferred && selected_text.is_some() {
                        self.apply_cut(cx, range_event);
                    }
                    return true;
                }

                // SPEC-HAL-056: Paste - Cmd/Ctrl + V
                if shortcut_mod && (text_event.text == "v" || text_event.text == "V") {
                    // Browser Key/Text is only the trigger. Its trusted ClipboardEvent
                    // owns the payload and the one Paste/motion/history transaction.
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(clipboard_text) = cx.clipboard_get_text() {
                        self.apply_clipboard_text(cx, clipboard_text, range_event);
                    } else {
                        self.snap_smooth_caret();
                    }
                    return true;
                }

                // Skip text insertion for other modifier combos
                if text_event.modifiers.ctrl || text_event.modifiers.meta {
                    return false;
                }

                // Apply input filter — reject characters that don't pass.
                if let Some(ref filter) = self.input_filter {
                    if !text_event.text.chars().all(|c| filter(c)) {
                        return true; // Consume event but don't insert
                    }
                }

                let edit_kind = delegate::edit_kind_for_insert(&self.state.selection);
                if !self.can_insert_text(&text_event.text) {
                    return true;
                }
                let before = self.capture_snapshot();
                let timestamp = cx.frame_time_ms();
                // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                if self.layout_dirty.get() {
                    self.rebuild_layout_cache_now(cx.text_engine());
                }
                let boundary_origin = self.caret_boundary_origin();
                let mask_origin = self.password_mask_origin();
                let boundaries = self.state.layout_cache.boundaries();
                delegate::perform_text_insert(
                    self.state.document.content_mut(),
                    &mut self.state.selection,
                    &mut self.state.cursor,
                    boundaries,
                    &text_event.text,
                );
                if before.text != self.state.document.text() {
                    self.state.document.mark_modified();
                    self.reset_caret_blink(cx);
                    self.layout_dirty.set(true);
                    self.state.layout_cache.mark_stale();
                    self.rebuild_layout_cache_now(cx.text_engine());
                    if self.config.is_password {
                        self.record_password_mask_boundary(
                            mask_origin,
                            CaretBoundaryKind::Reveal,
                            !text_event.modifiers.ctrl
                                && !text_event.modifiers.alt
                                && !text_event.modifiers.meta,
                        );
                    } else {
                        self.record_caret_boundary(
                            &before,
                            boundary_origin,
                            CaretBoundaryKind::Reveal,
                            !text_event.modifiers.ctrl
                                && !text_event.modifiers.alt
                                && !text_event.modifiers.meta,
                            cx.text_engine(),
                        );
                    }
                    self.record_edit(before, edit_kind, timestamp);
                    self.fire_on_change();
                    cx.invalidate_layout();
                } else {
                    if before.selection_anchor != self.state.selection.anchor
                        || before.selection_head != self.state.selection.head
                    {
                        self.snap_smooth_caret();
                    }
                    self.reset_caret_blink(cx);
                }
                true
            }
            // SPEC-HAL-055 Stage 1.1: IME composition events
            Event::Ime(ime_event) if cx.keyboard_mode == KeyboardAuthorityMode::Capture => {
                match ime_event {
                    ImeEvent::PreEdit { text, cursor: _ } => {
                        delegate::handle_ime_preedit(
                            &mut self.state.ime_pre_composition_snapshot,
                            &mut self.state.undo_history,
                            self.state.document.text(),
                            &self.state.cursor,
                            &self.state.selection,
                        );
                        let _ = text; // Acknowledge pre-edit text
                        self.reset_caret_blink(cx);
                        true
                    }
                    ImeEvent::Commit { text } => {
                        self.state.undo_history.resume();
                        let before = self
                            .state
                            .ime_pre_composition_snapshot
                            .take()
                            .unwrap_or_else(|| {
                                delegate::capture_snapshot(
                                    self.state.document.text(),
                                    &self.state.cursor,
                                    &self.state.selection,
                                )
                            });
                        if !self.can_insert_text(text) {
                            self.reset_caret_blink(cx);
                            return true;
                        }
                        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                        if self.layout_dirty.get() {
                            self.rebuild_layout_cache_now(cx.text_engine());
                        }
                        let boundaries = self.state.layout_cache.boundaries();
                        delegate::perform_ime_commit_insert(
                            self.state.document.content_mut(),
                            &mut self.state.selection,
                            &mut self.state.cursor,
                            boundaries,
                            text,
                        );
                        let timestamp = cx.frame_time_ms();
                        self.record_edit(before, EditKind::ImeCommit, timestamp);
                        self.state.document.mark_modified();
                        self.reset_caret_blink(cx);
                        self.layout_dirty.set(true);
                        self.state.layout_cache.mark_stale();
                        cx.invalidate_layout();
                        true
                    }
                    ImeEvent::Cancel => {
                        delegate::handle_ime_cancel(
                            &mut self.state.ime_pre_composition_snapshot,
                            &mut self.state.undo_history,
                        );
                        self.reset_caret_blink(cx);
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

    fn on_mount(&mut self) {
        self.edit_handoff_mounted = true;
    }

    fn focusable(&self) -> bool {
        self.config.is_enabled && !self.composite_role
    }

    fn paints_own_focus_indicator(&self) -> bool {
        true
    }

    fn hit_testable(&self) -> bool {
        !self.composite_role
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        let delegation = cx.composite_delegation();
        if self.composite_role && self.composite_delegation != delegation {
            self.snap_smooth_caret();
        }
        self.composite_delegation = delegation;
        self.caret_animation_demand = if self.composite_role {
            Some(self.composite_delegation.is_some())
        } else {
            cx.appearance_facts().map(|facts| facts.keyboard_captured())
        };
        self.update_smooth_caret(cx);
    }

    fn needs_animation(&self) -> bool {
        self.config.is_enabled && self.caret_animation_demand.unwrap_or(self.focused)
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.snap_smooth_caret();
        self.focused = true;
        self.blink_caret_visible.set(true);
        self.blink_last_ms.set(cx.frame_time_ms());
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.snap_smooth_caret();
        self.focused = false;
        self.blink_caret_visible.set(false);
        // Clear drag mode on blur
        self.state.drag_mode = TextFieldDragMode::None;
        // SPEC-HAL-055 Stage 1.1: Clear IME composition on blur
        self.state.clear_composition();
        self.state.undo_history.resume();
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        // EventRouter invalidates retained paint after each hover transition.
        self.hover = true;
        self.reset_caret_blink_on_activity(cx);
    }

    fn on_hover_leave(&mut self, _cx: &mut EventContext) {
        self.hover = false;
    }

    fn touch_target(&self, _hit: crate::core::event::TouchHit) -> Option<crate::core::event::TouchTarget> {
        self.capture_eligible().then_some(crate::core::event::TouchTarget::Editor)
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, tap: crate::core::event::TouchTap) -> bool {
        self.place_touch_caret(cx, tap.local_position.x);
        true
    }

    fn capture_eligible(&self) -> bool {
        self.config.is_enabled && !self.composite_role
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        self.snap_smooth_caret();
        if self.config.is_password && !self.embedded && self.smooth_caret_enabled {
            // Resolve the constant mask's font through the existing Layout
            // owner before admitting presentation. Capture alone is paint-only.
            _cx.invalidate_layout();
        }
        if self.select_all_on_capture {
            self.select_all();
        }
        self.focused = true;
        self.reset_caret_blink_on_activity(_cx);
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.snap_smooth_caret();
        // SPEC-HAL-328 §3.2: capture release is the framework's domain.
        // This is the notification — the widget reacts to capture loss
        // without consulting Escape itself (framework already intercepted).
        //
        // Edit-lifecycle signal: if a commit just fired (Enter path), the
        // commit_pending flag is set; skip on_cancel to avoid double
        // dispatch with the on_submit that already ran. All other
        // capture-release paths (framework Escape, focus-elsewhere,
        // programmatic) reach here with commit_pending=false and DO fire
        // on_cancel. Consumers (PE editor_registry — SPEC-HAL-260A §6.3)
        // route this to EditIntent::Cancel which clears edit_session.
        //
        // Same shape as ComboBox::on_capture_leave → close_without_commit
        // → on_cancelled guard against committed_latch.
        if !self.commit_pending {
            if let Some(cb) = &mut self.on_cancel {
                cb();
            }
        }
        self.commit_pending = false;
        // SPEC-HAL-026: invalidate layout so the host's reconciliation
        // pass runs on the next frame. PE Body's edit_session unmount
        // (body.rs step 6a) is the canonical consumer — without this
        // dirty mark, the editor child stays in the tree (and visually
        // mounted in edit mode) until an unrelated event triggers
        // layout. The framework's request_continuous_frames_after_mode_
        // transition (app.rs:2566) schedules paint, not layout.
        //
        // Unconditional: on_capture_leave is only invoked on a real
        // capture release. Standalone TextField hosts pay one no-op
        // layout pass per release, which is acceptable.
        if !cx.is_availability_transition() {
            cx.invalidate_layout();
        }
        // Keep focused — capture release doesn't blur.
    }

    fn debug_type_name(&self) -> &'static str {
        "TextField"
    }
}

// ============================================================================
// Test Harness (SPEC-HAL-109 Stage J: Consolidated)
// ============================================================================

#[cfg(any(test, feature = "test-harness"))]
use crate::widgets::text_widget_test_harness::{TextWidgetTestHarness, TextWidgetTestHarnessRead};

#[cfg(any(test, feature = "test-harness"))]
impl TextWidgetTestHarness for TextField {
    fn focused_mut(&mut self) -> &mut bool {
        &mut self.focused
    }

    fn cursor_mut(&mut self) -> &mut TextCursor {
        &mut self.state.cursor
    }

    fn selection_mut(&mut self) -> &mut Selection {
        &mut self.state.selection
    }

    fn layout_cache_mut(&mut self) -> &mut TextLayoutCache {
        &mut self.state.layout_cache
    }
}

#[cfg(any(test, feature = "test-harness"))]
impl TextWidgetTestHarnessRead for TextField {
    fn cursor_ref(&self) -> &TextCursor {
        &self.state.cursor
    }

    fn selection_ref(&self) -> &Selection {
        &self.state.selection
    }
}

/// TextField-specific test helpers (not shared with TextArea).
#[cfg(any(test, feature = "test-harness"))]
impl TextField {
    /// Replace config for testing policy behavior.
    pub fn set_config_for_test(&mut self, config: TextFieldConfig) {
        self.config = config;
        self.enforce_current_text_constraints();
        self.layout_dirty.set(true);
    }

    /// Get current scroll_x for test assertions.
    pub fn scroll_x_for_test(&self) -> f32 {
        self.state.scroll_x
    }

    /// Set scroll_x directly for testing.
    pub fn set_scroll_x_for_test(&mut self, scroll_x: f32) {
        self.state.scroll_x = scroll_x;
    }

    /// Set cursor position for testing (also clears selection).
    pub fn set_cursor_pos_for_test(&mut self, pos: usize) {
        self.state.cursor.pos = pos;
        self.state.selection.anchor = pos;
        self.state.selection.head = pos;
    }

    /// Create text viewport for testing with given width.
    pub fn text_viewport_for_test(&self, viewport_width: f32) -> RectF {
        // Match layout padding (8.0, 4.0)
        RectF::new(8.0, 4.0, viewport_width, 20.0)
    }

    /// Call ensure_focus_visible with test viewport.
    /// SPEC-HAL-141: Now requires &mut self since ensure_focus_visible mutates scroll_x.
    pub fn ensure_visible_for_test(&mut self, viewport_width: f32) {
        let viewport = self.text_viewport_for_test(viewport_width);
        // Use default caret_margin (4.0) for tests
        self.ensure_focus_visible(viewport, 4.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_field_caret_demand_stops_after_capture_release() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::event::{InteractionDomains, WindowId};
        use crate::core::event_router::EventRouter;
        use crate::core::icons::builtin_registry;
        use crate::core::rect_authority::UpdateAuthorityGuard;
        use crate::core::scene::{SceneNode, TextNode};
        use crate::core::theme::Theme;
        use crate::core::widget_tree::WidgetTree;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::actions::ActionId;
        use crate::widgets::{flex::Flex, label::Label};
        use std::rc::Rc;
        use std::sync::Arc;

        fn field(tree: &WidgetTree, node: NodeId) -> &TextField {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn text_nodes(nodes: &[SceneNode]) -> Vec<&TextNode> {
            let mut result = Vec::new();
            for node in nodes {
                match node {
                    SceneNode::Text(text) => result.push(text),
                    SceneNode::Clip(clip) => result.extend(text_nodes(&clip.children)),
                    SceneNode::RetainedFragment(fragment) => {
                        result.extend(text_nodes(&fragment.children))
                    }
                    _ => {}
                }
            }
            result
        }

        let changes = Rc::new(Cell::new(0));
        let submits = Rc::new(Cell::new(0));
        let cancels = Rc::new(Cell::new(0));
        let mut control = TextField::new().with_text("Stable text");
        let observed = changes.clone();
        control.set_on_change(move |_| observed.set(observed.get() + 1));
        let observed = submits.clone();
        control.set_on_submit(move |_| observed.set(observed.get() + 1));
        let observed = cancels.clone();
        control.set_on_cancel(move || observed.set(observed.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let theme = Theme::default();
        let icons = builtin_registry();
        let mut text = FontManagerTextEngine::new();
        let mut router = EventRouter::new();
        let mut domains = InteractionDomains::new();
        let mut harness = RetainedSceneTestHarness::new();
        let state = |tree: &WidgetTree| {
            let value = field(tree, node);
            (
                value.state.document.text().to_owned(),
                value.state.cursor.pos,
                value.state.selection.anchor,
                value.state.selection.head,
                value.state.scroll_x,
            )
        };
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
        let check_caret = |nodes: &[SceneNode], expected: usize| {
            let output = text_nodes(nodes);
            assert_eq!(output.len(), 1);
            assert_eq!(output[0].text, "Stable text");
            assert_eq!(output[0].backgrounds.len(), expected);
            if expected == 1 {
                let caret = &output[0].backgrounds[0];
                assert_eq!(caret.width, theme.typography.caret_thickness);
                assert!(caret.height > 0.0);
                assert_eq!(
                    caret.color,
                    theme.input_chrome(ControlChromeState::Idle).caret.into()
                );
            }
        };
        macro_rules! frame {
            () => {
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(320.0, 100.0)),
                    &theme,
                    &mut text,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                )
            };
        }
        harness.advance_transition_time(0.0);
        frame!();
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        frame!();
        assert!(field(&tree, node).focused);
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        check_caret(&harness.local_fragment(owner).unwrap(), 0);
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        let initial_state = state(&tree);
        let initial_geometry = geometry(&tree);
        let origin = initial_geometry[1].2;
        let hit = |tree: &WidgetTree| {
            let hit = tree
                .hit_test(origin.x + 5.0, origin.y + origin.height * 0.5)
                .unwrap();
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        };
        let original_hit = hit(&tree);
        assert_eq!(original_hit.0, owner);
        let peer_local = harness.local_fragment(peer_id).unwrap();
        let root_local = harness.local_fragment(root_id).unwrap();

        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            domains.primary_state_mut(),
            &mut text,
            None
        ));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        assert!(frame!().scene_built);
        let captured = harness.local_fragment(owner).unwrap();
        check_caret(&captured, 1);
        assert!(tree.node(node).unwrap().widget.needs_animation());
        assert!(tree.any_widget_needs_animation());
        harness.advance_transition_time(0.531);
        assert!(frame!().scene_built);
        let blinked = harness.local_fragment(owner).unwrap();
        check_caret(&blinked, 0);
        assert!(!Arc::ptr_eq(&captured, &blinked));
        assert!(tree.node(node).unwrap().widget.needs_animation());
        assert!(tree.any_widget_needs_animation());
        assert!(Arc::ptr_eq(
            &peer_local,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_local,
            &harness.local_fragment(root_id).unwrap()
        ));

        router.set_frame_time_ms(531);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::RELEASE_CAPTURE,
            Some(root),
            domains.primary_state_mut(),
            &mut text,
            None
        ));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert!(field(&tree, node).focused);
        assert_eq!(state(&tree), initial_state);
        assert_eq!((changes.get(), submits.get(), cancels.get()), (0, 0, 1));
        frame!();
        assert!(field(&tree, node).focused);
        assert!(
            !tree.node(node).unwrap().widget.needs_animation(),
            "focus retained after Capture release must not keep caret animation demand"
        );
        assert!(!tree.any_widget_needs_animation());
        check_caret(&harness.local_fragment(owner).unwrap(), 0);
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        let released_geometry = geometry(&tree);
        assert_eq!(
            released_geometry.map(|(id, widget, rect, _)| (id, widget, rect)),
            initial_geometry.map(|(id, widget, rect, _)| (id, widget, rect))
        );
        assert_eq!(hit(&tree), original_hit);
        let released = harness.local_fragment(owner).unwrap();
        let peer_released = harness.local_fragment(peer_id).unwrap();
        let root_released = harness.local_fragment(root_id).unwrap();
        harness.advance_transition_time(1.2);
        let idle = frame!();
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(!tree.node(node).unwrap().widget.needs_animation());
        assert!(!tree.any_widget_needs_animation());
        assert!(Arc::ptr_eq(
            &released,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_released,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_released,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert_eq!(geometry(&tree), released_geometry);
        assert!(!router.continuous_frames_requested());

        router.set_frame_time_ms(1200);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            domains.primary_state_mut(),
            &mut text,
            None
        ));
        router.reset_continuous_frames_request();
        assert!(frame!().scene_built);
        check_caret(&harness.local_fragment(owner).unwrap(), 1);
        assert!(tree.node(node).unwrap().widget.needs_animation());
        assert!(tree.any_widget_needs_animation());
        harness.advance_transition_time(1.731);
        frame!();
        check_caret(&harness.local_fragment(owner).unwrap(), 0);
        router.set_frame_time_ms(1731);
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::RELEASE_CAPTURE,
            Some(root),
            domains.primary_state_mut(),
            &mut text,
            None
        ));
        frame!();
        router.reset_continuous_frames_request();
        assert!(!tree.node(node).unwrap().widget.needs_animation());
        assert!(!tree.any_widget_needs_animation());
        assert_eq!((changes.get(), submits.get(), cancels.get()), (0, 0, 2));
        assert_eq!(state(&tree), initial_state);
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert!(field(&tree, node).focused);
        let settled_geometry = geometry(&tree);
        let settled = harness.local_fragment(owner).unwrap();
        let settled_peer = harness.local_fragment(peer_id).unwrap();
        let settled_root = harness.local_fragment(root_id).unwrap();
        harness.advance_transition_time(2.4);
        let idle = frame!();
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &settled,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &settled_peer,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &settled_root,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert_eq!(geometry(&tree), settled_geometry);
        assert_eq!(hit(&tree), original_hit);

        {
            let _update = UpdateAuthorityGuard::enter();
            let mut cx = crate::core::widget::UpdateContext::new(None);
            cx.begin_widget_update_for_owner(owner);
            tree.node_mut(node).unwrap().widget.update(0.0, &mut cx);
            assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        }
        assert!(tree.node(node).unwrap().widget.needs_animation());
        assert!(tree.any_widget_needs_animation());
        let republished = frame!();
        assert!(!republished.scene_built && !republished.needs_next_frame);
        assert!(!tree.node(node).unwrap().widget.needs_animation());
        assert!(!tree.any_widget_needs_animation());
        assert!(!tree.node(node).unwrap().widget.needs_update());
        assert_eq!(state(&tree), initial_state);
        assert_eq!(geometry(&tree), settled_geometry);
        assert!(Arc::ptr_eq(
            &settled,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &settled_peer,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &settled_root,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert_eq!((changes.get(), submits.get(), cancels.get()), (0, 0, 2));
        assert!(!router.continuous_frames_requested());
    }
}

#[cfg(test)]
mod smooth_caret_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{
        FusedInputEvent, InteractionDomains, Modifiers, PhysicalKeyCode, WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::icons::builtin_registry;
    use crate::core::scene::SceneNode;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::runtime::actions::ActionId;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::{combo_box::ComboBox, flex::Flex, label::Label};
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn hal398_routed_tap_qualifies_one_current_editor_and_reconciles_ime_once() {
        use crate::core::event::{ImeEvent, RawInputEvent, TouchPhase};
        use crate::core::event_router::browser_text::{
            BrowserTextAction, BrowserTextSession, BrowserTextTarget,
        };
        use crate::core::input_fusion::InputFusion;
        let mut f = Fixture::unfocused_widget_and_text(
            Box::new(TextField::new().with_text("cafe")), FontManagerTextEngine::new(),
        );
        let mut fusion = InputFusion::new();
        let mut bindings = BindingRegistry::new();
        let rect = f.tree.get_absolute_paint_rect(f.field).unwrap();
        let position = crate::core::math::Point::new(rect.right() - 12.0, rect.center().y);
        for (phase, expected) in [(TouchPhase::Started, false), (TouchPhase::Ended, true)] {
            let state = f.domains.primary_state_mut();
            let mut events = Vec::new();
            fusion.process_raw_with_gesture_sessions(
                RawInputEvent::Touch {
                    window_id: WindowId::PRIMARY, time: Instant::now(), contact_id: 14,
                    phase, pos: position,
                },
                state.focus_manager.interaction_domain_id(), &mut state.gesture_registry,
                &f.tree, None, &mut events,
            );
            for event in events {
                f.router.route_session_input_event(
                    &mut f.tree, event, Some(f.root), &mut f.text, None, state, &mut bindings,
                );
            }
            let activation = f.router.take_touch_keyboard_activation();
            assert_eq!(activation.is_some(), expected);
            if let Some((motion, owner)) = activation {
                assert_eq!(motion.native_contact, 14);
                assert_eq!(owner, f.owner);
            }
        }
        let (target, projection) =
            BrowserTextTarget::qualify(&f.tree, &f.domains, WindowId::PRIMARY, f.root).unwrap();
        let mut session = BrowserTextSession::new(target, projection);
        macro_rules! edit {
            ($action:expr) => {{
                let action = $action;
                session.projection = f.router.route_browser_text(
                    &mut f.tree, &mut f.domains, f.root, &session.target, &session.projection,
                    &action, &mut f.text,
                ).unwrap();
            }};
        }
        // A replacement can arrive via input alone, for example from autocorrect.
        edit!(session.after_input("café", "insertReplacementText", false, None).unwrap());
        assert_eq!(f.field().text(), "café");
        edit!(session.update_composition("に".into()));
        assert_eq!(f.field().text(), "café");
        assert!(session.projection.composing);
        assert!(session.after_input("caféに", "insertCompositionText", true, None).is_none());
        edit!(session.end_composition("に".into()).unwrap());
        assert_eq!(f.field().text(), "caféに");
        assert!(session.after_input("caféにに", "insertText", false, None).is_none());
        session.before_input();
        edit!(session.after_input("caféに🪴", "insertText", false, None).unwrap());
        assert_eq!(f.field().text(), "caféに🪴");
        edit!(session.update_composition("x".into()));
        edit!(BrowserTextAction::Composition(ImeEvent::Cancel));
        assert_eq!(f.field().text(), "caféに🪴");
        assert!(!session.projection.composing);

        let stale = session.projection.select(0, 0).unwrap();
        session.before_input();
        f.field_mut().set_text("newer document");
        let refreshed = f.router.route_browser_text(
            &mut f.tree, &mut f.domains, f.root, &session.target, &session.projection,
            &stale, &mut f.text,
        ).unwrap();
        assert_eq!(refreshed.selection, (14, 14));
        assert_eq!(f.field().text(), "newer document");
        session.projection = refreshed;
        assert!(session.after_input("caféに🪴x", "insertText", false, None).is_none());
        f.router.clear_focus_with_text_engine(
            &mut f.tree, f.domains.primary_state_mut(), Some(&mut f.text),
        );
        assert!(session.target.current_projection(&f.tree, &f.domains, f.root).is_none());
    }

    #[test]
    fn hal398_textarea_and_combo_share_the_canonical_edit_owners() {
        use crate::core::event_router::browser_text::{BrowserTextSession, BrowserTextTarget};
        use crate::widgets::text_area::TextArea;
        for (widget, value, expected) in [
            (Box::new(TextArea::new()) as Box<dyn Widget>, "one\n🪴", "one\n🪴"),
            (Box::new(ComboBox::simple(vec!["one".into(), "two".into()])), "ji", "ji"),
        ] {
            let mut f = Fixture::with_widget(widget);
            let (target, projection) =
                BrowserTextTarget::qualify(&f.tree, &f.domains, WindowId::PRIMARY, f.root).unwrap();
            let mut session = BrowserTextSession::new(target, projection);
            let action = session.after_input(value, "insertText", false, None).unwrap();
            let after = f.router.route_browser_text(
                &mut f.tree, &mut f.domains, f.root, &session.target, &session.projection,
                &action, &mut f.text,
            ).unwrap();
            assert_eq!(after.value, expected);
            let widget = &f.tree.node(f.field).unwrap().widget;
            if let Some(area) = widget.as_any().downcast_ref::<TextArea>() {
                assert_eq!(area.text(), expected);
            } else {
                let combo = widget.as_any().downcast_ref::<ComboBox>().unwrap();
                assert_eq!(combo.input_text(), expected);
                assert!(combo.committed_value_string().is_none());
            }
        }
    }

    struct Fixture {
        tree: WidgetTree,
        root: NodeId,
        field: NodeId,
        peer: TreeWidgetId,
        owner: TreeWidgetId,
        theme: Theme,
        text: FontManagerTextEngine,
        router: EventRouter,
        domains: InteractionDomains,
        harness: RetainedSceneTestHarness,
        time: Instant,
        now: f64,
        width: f32,
    }

    #[test]
    fn text_selection_palette_text_field_paints_selected_characters() {
        use crate::tests::_support::{assert_painted_text_backgrounds, text_selection_palette_cases};

        for (theme, expected) in text_selection_palette_cases() {
            let mut f = Fixture::new(false);
            f.theme = theme;
            f.key(KeyCode::Home, Modifiers::default());
            f.key(
                KeyCode::Right,
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            );
            assert!(f.router.handle_engine_action(
                &mut f.tree,
                ActionId::RELEASE_CAPTURE,
                Some(f.root),
                f.domains.primary_state_mut(),
                &mut f.text,
                None,
            ));
            assert_eq!(
                f.domains.primary_state().focus_manager.focused(),
                Some(f.owner)
            );
            f.tick(0.002);
            let painted = f.harness.local_fragment(f.owner).unwrap();
            assert_painted_text_backgrounds(&painted, "abc", &[expected]);
        }
    }

    impl Fixture {
        fn new(enabled: bool) -> Self {
            Self::with_field(
                TextField::new()
                    .with_text("abc")
                    .with_smooth_caret_prototype(enabled),
            )
        }

        fn with_field(field: TextField) -> Self {
            Self::with_widget(Box::new(field))
        }

        fn with_widget(widget: Box<dyn Widget>) -> Self {
            Self::with_widget_and_text(widget, FontManagerTextEngine::new())
        }

        fn with_widget_and_text(widget: Box<dyn Widget>, text: FontManagerTextEngine) -> Self {
            let mut result = Self::unfocused_widget_and_text(widget, text);
            result.router.request_focus_for_widget(
                &mut result.tree,
                result.owner,
                result.domains.primary_state_mut(),
            );
            assert!(result.router.handle_engine_action(
                &mut result.tree,
                ActionId::ENTER_WIDGET,
                Some(result.root),
                result.domains.primary_state_mut(),
                &mut result.text,
                None,
            ));
            result.tick(0.001);
            result
        }

        fn unfocused_widget_and_text(widget: Box<dyn Widget>, text: FontManagerTextEngine) -> Self {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()), None, 0);
            let field = tree.add_child(root, widget, None, 0);
            let peer_node = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 1);
            let owner = tree.node(field).unwrap().id;
            let peer = tree.node(peer_node).unwrap().id;
            let mut result = Self {
                tree,
                root,
                field,
                peer,
                owner,
                theme: Theme::default(),
                text,
                router: EventRouter::new(),
                domains: InteractionDomains::new(),
                harness: RetainedSceneTestHarness::new(),
                time: Instant::now(),
                now: 0.0,
                width: 360.0,
            };
            result.tick(0.0);
            result
        }

        fn field(&self) -> &TextField {
            self.tree
                .node(self.field)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }

        fn field_mut(&mut self) -> &mut TextField {
            self.tree
                .node_mut(self.field)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut()
                .unwrap()
        }

        fn tick(&mut self, now: f64) {
            self.now = now;
            self.harness.advance_transition_time(now);
            self.harness.frame_with_appearance(
                &mut self.tree,
                self.root,
                Constraints::tight(Size::new(self.width, 90.0)),
                &self.theme,
                &mut self.text,
                &builtin_registry(),
                &self.domains,
                WindowId::PRIMARY,
            );
        }

        fn route(&mut self, event: FusedInputEvent) {
            self.router.set_frame_time_ms((self.now * 1000.0) as u64);
            self.router.route_input_event(
                &mut self.tree,
                event,
                Some(self.root),
                &mut self.text,
                Some(self.harness.animation_context()),
                self.domains.primary_state_mut(),
                &mut BindingRegistry::new(),
            );
            // Distinct admissions must not imitate a Key/Text pair at one transport time.
            self.time += std::time::Duration::from_nanos(1);
        }

        fn insert(&mut self, text: &str) {
            self.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: self.time,
                text: text.into(),
                modifiers: Modifiers::default(),
            });
        }

        fn browser_paste_request(
            &self,
            text: &str,
        ) -> crate::core::event_router::browser_paste::BrowserPaste {
            crate::core::event_router::browser_paste::BrowserPasteTarget::qualify(
                &self.tree,
                &self.domains,
                WindowId::PRIMARY,
                self.root,
            )
            .unwrap()
            .with_text(text.to_owned())
            .unwrap()
        }

        fn browser_paste(
            &mut self,
            request: crate::core::event_router::browser_paste::BrowserPaste,
        ) -> Result<(), crate::core::event_router::browser_paste::PasteRefusal> {
            self.router.route_browser_paste(
                &mut self.tree,
                &mut self.domains,
                self.root,
                request,
                &mut self.text,
                Some(self.harness.animation_context()),
            )
        }

        fn key(&mut self, key_code: KeyCode, modifiers: Modifiers) {
            self.key_with_repeat(key_code, modifiers, false);
        }

        fn key_with_repeat(&mut self, key_code: KeyCode, modifiers: Modifiers, repeat: bool) {
            self.route(FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: self.time,
                key_code,
                physical_key: PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers,
                repeat,
            });
        }

        fn painted_x(&self) -> f32 {
            fn find(nodes: &[SceneNode]) -> Option<f32> {
                for node in nodes {
                    let found = match node {
                        SceneNode::Text(text) => text.backgrounds.last().map(|b| b.x_offset),
                        SceneNode::Clip(clip) => find(&clip.children),
                        SceneNode::RetainedFragment(fragment) => find(&fragment.children),
                        _ => None,
                    };
                    if found.is_some() {
                        return found;
                    }
                }
                None
            }
            find(&self.harness.local_fragment(self.owner).unwrap()).expect("actual painted caret")
        }

        fn canonical_x(&self) -> f32 {
            self.field()
                .caret_x_from_boundaries(self.field().state.cursor.pos)
        }
    }

    fn selected_input_surface(radius: f32) -> InputSurfaceAppearance {
        use crate::core::paint::{ColorStop, GradientDef, GradientKind, SurfaceGradient};
        let surface = SurfaceGradient::new(GradientDef {
            kind: GradientKind::Linear,
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: Color::from_hex(0x303234).with_alpha(0.7),
                },
                ColorStop {
                    offset: 1.0,
                    color: Color::from_hex(0x101214).with_alpha(0.4),
                },
            ],
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
        })
        .unwrap();
        InputSurfaceAppearance::try_new(
            surface,
            Color::from_hex(0x63676b),
            Color::from_hex(0x41464b),
            Color::from_hex(0x33383d),
            Color::from_hex(0x22272c),
            Color::from_hex(0x454a4f),
            radius,
        )
        .unwrap()
    }

    fn input_surface_frame(f: &mut Fixture, now: f64, scale: f32) {
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        f.now = now;
        f.text.set_scale(scale);
        f.harness.advance_transition_time(now);
        let target = CustomRenderTargetSnapshot::new(
            crate::compositor::SurfaceId::test_id(1),
            PhysicalExtent::new((360.0 * scale) as u32, (90.0 * scale) as u32),
            scale,
        )
        .unwrap();
        f.harness.frame_with_optional_custom_render_target(
            &mut f.tree,
            f.root,
            Constraints::tight(Size::new(f.width, 90.0)),
            &f.theme,
            &mut f.text,
            &builtin_registry(),
            Some(target),
            Some((&f.domains, WindowId::PRIMARY)),
        );
    }

    fn input_surface_leaves(nodes: &[SceneNode]) -> Vec<&SceneNode> {
        let mut leaves = Vec::new();
        for node in nodes {
            match node {
                SceneNode::Clip(clip) => leaves.extend(input_surface_leaves(&clip.children)),
                SceneNode::RetainedFragment(fragment) => {
                    leaves.extend(input_surface_leaves(&fragment.children))
                }
                SceneNode::Layer(layer) => leaves.extend(input_surface_leaves(&layer.children)),
                SceneNode::Scroll(scroll) => leaves.extend(input_surface_leaves(&scroll.children)),
                _ => leaves.push(node),
            }
        }
        leaves
    }

    #[test]
    fn input_adornment_textfield_icon_parity() {
        use crate::core::event::KeyboardAuthorityMode;
        use crate::core::icon::validate_icon;

        let icons = builtin_registry();
        for raw in 1..=IconId::BRIEFCASE.0 {
            let id = IconId(raw);
            assert_eq!(icons.get(id).unwrap().id, id);
        }
        for (id, ink, gaps) in [
            (
                IconId::GLOBE,
                &[[2.75, 12.0], [12.0, 2.75], [7.5, 12.0], [12.0, 8.25]][..],
                &[[12.0, 12.0], [5.0, 12.0], [12.0, 6.0]][..],
            ),
            (
                IconId::BRIEFCASE,
                &[[12.0, 4.75], [3.75, 14.0], [12.0, 14.25], [12.0, 19.25]][..],
                &[[12.0, 6.75], [6.0, 11.0], [8.0, 16.0]][..],
            ),
        ] {
            let icon = icons.get(id).unwrap();
            validate_icon(icon).unwrap();
            let scale = 16.0 / icon.viewbox_width;
            let meshes: Vec<_> = icon
                .layers
                .iter()
                .map(|layer| layer.path.transformed(scale, 0.0, 0.0).to_mesh())
                .collect();
            let covers = |x: f32, y: f32| {
                let point = [x * scale, y * scale];
                let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
                    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
                };
                meshes.iter().any(|mesh| {
                    mesh.indices.chunks_exact(3).any(|triangle| {
                        let [a, b, c] =
                            [0, 1, 2].map(|i| mesh.vertices[triangle[i] as usize]);
                        let area = cross(a, b, c);
                        area != 0.0
                            && [cross(a, b, point), cross(b, c, point), cross(c, a, point)]
                                .iter()
                                .all(|side| side * area >= 0.0)
                    })
                })
            };
            for (points, expected) in [(ink, true), (gaps, false)] {
                for &[x, y] in points {
                    assert_eq!(covers(x, y), expected, "{id:?} at ({x}, {y})");
                    assert_eq!(covers(24.0 - x, y), expected, "{id:?} right half");
                    if id == IconId::GLOBE {
                        assert_eq!(covers(x, 24.0 - y), expected, "{id:?} lower half");
                    }
                }
            }
        }

        let surface = selected_input_surface(8.0);
        let make_field = || {
            TextField::new()
                .with_text("Mail")
                .with_smooth_caret_prototype(false)
                .with_input_surface_appearance(surface.clone())
                .with_leading_icon(IconId::MAIL)
        };
        let assert_mail = |f: &Fixture, size, scale| {
            let rect = f.tree.get_absolute_paint_rect(f.field).unwrap();
            let padding = f.theme.geometry.text_field.padding_x;
            assert_eq!(f.field().leading_width, size + padding);
            let local = f.harness.local_fragment(f.owner).unwrap();
            let leaves = input_surface_leaves(&local);
            let paths: Vec<_> = leaves
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Path(path) => Some(path.path.content_hash()),
                    _ => None,
                })
                .collect();
            let mail = icons.get(IconId::MAIL).unwrap();
            let x = snap_to_physical(rect.x + padding, scale);
            let y = snap_to_physical(rect.y + (rect.height - size) * 0.5, scale);
            let expected: Vec<_> = mail
                .layers
                .iter()
                .map(|layer| layer.path.transformed(size / 24.0, x, y).content_hash())
                .collect();
            assert_eq!(paths, expected);
            assert!(leaves.iter().any(|node| matches!(node,
                SceneNode::GradientRect(g) if &g.gradient == surface.surface().definition()
            )));
            rect
        };
        for scale in [1.0, 2.0] {
            let mut default = Fixture::with_field(make_field());
            let mut sized = Fixture::with_field(make_field().with_leading_icon_size(18.0));
            for f in [&mut default, &mut sized] {
                f.theme.typography.input_font_size = 16.0;
                f.theme.typography.line_height = 20.0;
                f.tree.mark_layout_dirty(f.field);
                input_surface_frame(f, 0.010, scale);
                f.key(KeyCode::End, Modifiers::default());
                input_surface_frame(f, 0.020, scale);
                assert_eq!(
                    f.domains.primary_state().focus_manager.focused(),
                    Some(f.owner)
                );
                assert_eq!(
                    f.domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
                assert_eq!(f.field().state.cursor.pos, 4);
                assert_eq!(f.painted_x(), f.canonical_x());
            }
            assert_eq!(
                assert_mail(&default, 16.0, scale),
                assert_mail(&sized, 18.0, scale)
            );
            let text_position = |f: &Fixture| {
                let local = f.harness.local_fragment(f.owner).unwrap();
                input_surface_leaves(&local)
                    .into_iter()
                    .find_map(|node| match node {
                        SceneNode::Text(text) if text.text == "Mail" => {
                            assert_eq!(text.font_size, 16.0);
                            Some(text.position)
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            let before = text_position(&default);
            let after = text_position(&sized);
            assert_eq!(after.x - before.x, 2.0);
            assert_eq!(after.y, before.y);
            assert_eq!(sized.painted_x(), default.painted_x());

            sized.key(
                KeyCode::Home,
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
            );
            assert_eq!(sized.field().state.selection.anchor, 4);
            assert_eq!(sized.field().state.selection.head, 0);
            sized.width =
                sized.field().padding_x * 3.0 + sized.theme.typography.caret_thickness + 12.0;
            input_surface_frame(&mut sized, 0.030, scale);
            assert_mail(&sized, 12.0, scale);
            assert_eq!(sized.field().text(), "Mail");
        }
    }

    #[test]
    fn input_adornment_textfield_layout_hit_and_history() {
        use crate::core::event::{KeyboardAuthorityMode, PointerDevice};
        let command = Modifiers {
            meta: cfg!(target_os = "macos"),
            ctrl: !cfg!(target_os = "macos"),
            ..Default::default()
        };
        for scale in [1.0, 2.0] {
            let mut f = Fixture::with_field(
                TextField::new()
                    .with_text("abc")
                    .with_leading_icon(IconId::GLOBE),
            );
            let mut plain = Fixture::with_field(TextField::new().with_text("abc"));
            input_surface_frame(&mut f, 0.010, scale);
            input_surface_frame(&mut plain, 0.010, scale);
            let rect = f.tree.get_absolute_paint_rect(f.field).unwrap();
            assert_eq!(
                rect,
                plain.tree.get_absolute_paint_rect(plain.field).unwrap()
            );
            assert_eq!(plain.field().leading_width, 0.0);
            let local = f.harness.local_fragment(f.owner).unwrap();
            let original = plain.harness.local_fragment(plain.owner).unwrap();
            let nodes = input_surface_leaves(&local);
            let old = input_surface_leaves(&original);
            let text_position = |nodes: &[&SceneNode]| {
                nodes
                    .iter()
                    .find_map(|n| match n {
                        SceneNode::Text(t) if t.text == "abc" => Some(t.position),
                        _ => None,
                    })
                    .unwrap()
            };
            assert_eq!(
                text_position(&nodes).x - text_position(&old).x,
                f.field().leading_width
            );
            assert_eq!(text_position(&nodes).y, text_position(&old).y);
            assert_eq!(
                nodes
                    .iter()
                    .filter(|n| matches!(n, SceneNode::Path(_)))
                    .count(),
                builtin_registry().get(IconId::GLOBE).unwrap().layers.len()
            );
            assert!(!old.iter().any(|n| matches!(n, SceneNode::Path(_))));
            let peer = f.harness.local_fragment(f.peer).unwrap();
            let plain_peer = plain.harness.local_fragment(plain.peer).unwrap();
            let icon_point = crate::core::math::Point::new(
                rect.x + f.field().padding_x + 1.0,
                rect.y + rect.height * 0.5,
            );
            for target in [&mut f, &mut plain] {
                for event in [
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: target.time,
                        pos: icon_point,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    },
                    FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: target.time,
                        pos: icon_point,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    },
                ] {
                    target.route(event);
                }
                assert_eq!(
                    target.field().state.cursor.pos,
                    0,
                    "icon uses the same field hit target"
                );
                assert_eq!(
                    target.domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
                target.insert("Почта");
                assert_eq!(target.field().text(), "Почтаabc");
                for (modifiers, expected) in [
                    (command, "abc"),
                    (
                        Modifiers {
                            shift: true,
                            ..command
                        },
                        "Почтаabc",
                    ),
                ] {
                    target.route(FusedInputEvent::TextInput {
                        window_id: WindowId::PRIMARY,
                        time: target.time,
                        text: "z".into(),
                        modifiers,
                    });
                    assert_eq!(target.field().text(), expected);
                }
                target.route(FusedInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: target.time,
                    text: "a".into(),
                    modifiers: command,
                });
                assert!(!target.field().state.selection.is_empty());
                let long = "Ж".repeat(60);
                target.insert(&long);
                assert_eq!(target.field().text(), long);
            }
            let long = "Ж".repeat(60);
            input_surface_frame(&mut f, 0.100, scale);
            input_surface_frame(&mut plain, 0.100, scale);
            assert!(f.field().state.scroll_x > 0.0);
            let viewport = f.field().text_viewport(
                rect,
                f.field().padding_x,
                f.theme.geometry.text_field.padding_y,
            );
            assert_eq!(
                viewport.x,
                rect.x + f.field().padding_x + f.field().leading_width
            );
            assert_eq!(viewport.right(), rect.right() - f.field().padding_x);
            f.key(KeyCode::Home, Modifiers::default());
            plain.key(KeyCode::Home, Modifiers::default());
            input_surface_frame(&mut f, 0.200, scale);
            input_surface_frame(&mut plain, 0.200, scale);
            assert_eq!(f.field().state.scroll_x, 0.0);
            assert_eq!(f.field().state.cursor.pos, 0);
            assert_eq!(
                Arc::ptr_eq(&peer, &f.harness.local_fragment(f.peer).unwrap()),
                Arc::ptr_eq(
                    &plain_peer,
                    &plain.harness.local_fragment(plain.peer).unwrap()
                ),
                "ordinary edit/Layout invalidations match the unadorned control"
            );
            input_surface_frame(&mut f, 0.300, scale);
            let settled_peer = f.harness.local_fragment(f.peer).unwrap();
            input_surface_frame(&mut f, 0.310, scale);
            assert!(Arc::ptr_eq(
                &settled_peer,
                &f.harness.local_fragment(f.peer).unwrap()
            ));
            for (time, width) in [(0.400, 32.0), (0.500, 0.0)] {
                f.width = width;
                f.tick(time);
                let actual = f.tree.get_absolute_paint_rect(f.field).unwrap();
                let viewport = f.field().text_viewport(
                    actual,
                    f.field().padding_x,
                    f.theme.geometry.text_field.padding_y,
                );
                assert!(viewport.width.is_finite() && viewport.width >= 0.0);
                assert!(f.field().leading_width.is_finite() && f.field().leading_width >= 0.0);
                if width == 0.0 {
                    assert_eq!(f.field().leading_width, 0.0);
                }
                assert_eq!(f.field().text(), long);
            }
        }
    }

    #[test]
    fn input_surface_appearance_validates_colors_radius_and_immutable_value() {
        use crate::core::appearance::InputSurfaceAppearanceError;
        let a = selected_input_surface(64.0);
        assert_eq!(a.clone(), a);
        assert_eq!(a.radius(), 64.0);
        assert_eq!(selected_input_surface(0.0).radius(), 0.0);
        for invalid in [f32::NAN, f32::INFINITY, -0.001, 64.001] {
            assert_eq!(
                InputSurfaceAppearance::try_new(
                    a.surface().clone(),
                    a.border(),
                    a.inner_edge(),
                    a.affordance(),
                    a.popup_surface(),
                    a.popup_highlight(),
                    invalid
                ),
                Err(InputSurfaceAppearanceError::InvalidRadius)
            );
        }
        for index in 0..5 {
            for invalid in [f32::NAN, f32::INFINITY, -0.001, 1.001] {
                let mut colors = [
                    a.border(),
                    a.inner_edge(),
                    a.affordance(),
                    a.popup_surface(),
                    a.popup_highlight(),
                ];
                colors[index].a = invalid;
                assert_eq!(
                    InputSurfaceAppearance::try_new(
                        a.surface().clone(),
                        colors[0],
                        colors[1],
                        colors[2],
                        colors[3],
                        colors[4],
                        a.radius()
                    ),
                    Err(InputSurfaceAppearanceError::InvalidColor)
                );
            }
        }
        for channel in 0..3 {
            let mut invalid = a.border();
            match channel {
                0 => invalid.r = f32::NAN,
                1 => invalid.g = -0.001,
                _ => invalid.b = 1.001,
            }
            assert_eq!(
                InputSurfaceAppearance::try_new(
                    a.surface().clone(),
                    invalid,
                    a.inner_edge(),
                    a.affordance(),
                    a.popup_surface(),
                    a.popup_highlight(),
                    a.radius()
                ),
                Err(InputSurfaceAppearanceError::InvalidColor)
            );
        }
    }

    #[test]
    fn input_surface_appearance_routed_textfield_hover_and_capture() {
        use crate::core::event::{KeyboardAuthorityMode, PointerButtonsState, PointerDevice};
        use crate::core::math::Point;
        use crate::core::paint::SurfaceGradient;

        let focus_border = Color::from_hex(0x798A9A);
        let capture_border = Color::from_hex(0xC5A389);
        let appearance = selected_input_surface(8.0)
            .try_with_focus_colors(focus_border, capture_border)
            .unwrap();
        let idle = appearance.surface();
        let variant = |color| {
            let mut definition = idle.definition().clone();
            definition.stops[0].color = Color::from_hex(color);
            SurfaceGradient::new(definition).unwrap()
        };
        let hover = variant(0x344454);
        let active = variant(0x4A4234);
        let decorated = appearance
            .clone()
            .with_state_surfaces(hover.clone(), active.clone());

        for (configured, enabled) in [(true, true), (false, true), (true, false)] {
            let style = if configured { &decorated } else { &appearance };
            let field = TextField::new()
                .with_text("abc")
                .with_enabled(enabled)
                .with_input_surface_appearance(style.clone());
            let mut f =
                Fixture::unfocused_widget_and_text(Box::new(field), FontManagerTextEngine::new());
            let rect = f.tree.get_absolute_paint_rect(f.field).unwrap();
            let inside = Point::new(rect.right() - 2.0, rect.y + rect.height * 0.5);
            let outside = Point::new(rect.right() + 2.0, inside.y);
            let hover_surface = if configured && enabled { &hover } else { idle };
            let active_surface = if configured && enabled { &active } else { idle };
            let rest_border = if enabled {
                style.border()
            } else {
                f.theme
                    .input_chrome(ControlChromeState::Disabled)
                    .chrome
                    .border_color
            };
            let assert_chrome = |f: &Fixture, surface: &SurfaceGradient, border: Color| {
                assert_eq!(f.tree.get_absolute_paint_rect(f.field), Some(rect));
                let local = f.harness.local_fragment(f.owner).unwrap();
                let nodes = input_surface_leaves(&local);
                assert!(nodes.iter().any(|node| matches!(node,
                    SceneNode::GradientRect(g) if &g.gradient == surface.definition()
                )));
                assert!(nodes.iter().any(|node| matches!(node,
                    SceneNode::BorderRect(b) if b.border_color == border.into()
                )));
            };
            let move_pointer = |f: &mut Fixture, pos| {
                f.route(FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time: f.time,
                    pos,
                    delta: (1.0, 0.0),
                    device: PointerDevice::Mouse,
                    buttons: PointerButtonsState::default(),
                });
            };

            assert_chrome(&f, idle, rest_border);
            let epoch = f.tree.node(f.field).unwrap().layout_epoch;
            for (now, pos, expected_surface) in [
                (0.010, inside, hover_surface),
                (0.020, outside, idle),
                (0.030, inside, hover_surface),
            ] {
                move_pointer(&mut f, pos);
                assert_eq!(f.field().hover, enabled && pos == inside);
                if enabled {
                    assert!(f.tree.node(f.field).unwrap().dirty_paint);
                }
                f.tick(now);
                assert_chrome(&f, expected_surface, rest_border);
                assert_eq!(f.tree.node(f.field).unwrap().layout_epoch, epoch);
            }

            f.router
                .request_focus_for_widget(&mut f.tree, f.owner, f.domains.primary_state_mut());
            f.tick(0.040);
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            assert_chrome(
                &f,
                active_surface,
                if enabled { focus_border } else { rest_border },
            );
            f.route(FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: f.time,
                pos: inside,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            });
            f.route(FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: f.time,
                pos: inside,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            });
            f.tick(0.050);
            assert_eq!(
                f.domains.primary_state().focus_manager.focused(),
                enabled.then_some(f.owner)
            );
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                if enabled {
                    KeyboardAuthorityMode::Capture
                } else {
                    KeyboardAuthorityMode::Navigation
                }
            );
            let border = if enabled { capture_border } else { rest_border };
            assert_chrome(&f, active_surface, border);
            move_pointer(&mut f, outside);
            f.tick(0.060);
            assert_chrome(&f, active_surface, border);
            f.insert("!");
            assert_eq!(f.field().text(), if enabled { "abc!" } else { "abc" });
        }
    }

    #[test]
    fn input_surface_appearance_real_gradient_keeps_default_content_and_geometry_1x_2x() {
        for scale in [1.0, 2.0] {
            let appearance = selected_input_surface(64.0);
            let mut f = Fixture::with_field(
                TextField::new()
                    .with_text("abc")
                    .with_input_surface_appearance(appearance.clone()),
            );
            let mut plain = Fixture::with_field(TextField::new().with_text("abc"));
            input_surface_frame(&mut f, 0.010, scale);
            input_surface_frame(&mut plain, 0.010, scale);
            let local = f.harness.local_fragment(f.owner).unwrap();
            let default = plain.harness.local_fragment(plain.owner).unwrap();
            let nodes = input_surface_leaves(&local);
            let old = input_surface_leaves(&default);
            let gradients: Vec<_> = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::GradientRect(g) => Some(g),
                    _ => None,
                })
                .collect();
            assert_eq!(gradients.len(), 1);
            assert!(
                !old.iter()
                    .any(|node| matches!(node, SceneNode::GradientRect(_)))
            );
            let g = gradients[0];
            let old_face = old
                .iter()
                .find_map(|node| match node {
                    SceneNode::RoundedRect(r) if r.z_index == LAYER_CONTENT => Some(r),
                    _ => None,
                })
                .unwrap();
            assert_eq!(g.rect, old_face.rect);
            assert_eq!(old_face.radius, 2.0);
            assert_eq!(
                g.radius,
                64.0_f32.min(g.rect.width * 0.5).min(g.rect.height * 0.5)
            );
            assert!(g.radius > 2.0);
            assert_eq!(&g.gradient, appearance.surface().definition());
            let borders: Vec<_> = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(b) => Some(b),
                    _ => None,
                })
                .collect();
            assert_eq!(borders.len(), 2);
            assert!(borders.iter().all(|b| b.fill_color.a == 0.0));
            let inner = borders
                .iter()
                .find(|b| b.border_color == appearance.inner_edge().into())
                .unwrap();
            assert_eq!(inner.border_width, 1.0 / scale);
            assert!(inner.z_index > g.z_index && inner.z_index < TEXT_FIELD_TEXT_LAYER);
            let outer = borders.iter().find(|b| b.rect == g.rect).unwrap();
            assert_eq!(
                outer.border_color,
                f.theme
                    .input_chrome(ControlChromeState::Captured)
                    .chrome
                    .border_color
                    .into()
            );
            assert_eq!(outer.z_index, crate::core::paint::LAYER_FOCUS + 1);
            assert!(
                !nodes
                    .iter()
                    .any(|node| matches!(node, SceneNode::RoundedRect(_)))
            );
            assert!(!nodes.iter().any(|node| matches!(node, SceneNode::Rect(r) if r.z_index < TEXT_FIELD_TEXT_LAYER && r.color.a != 0.0)));
            let value = |nodes: &[&SceneNode]| {
                nodes
                    .iter()
                    .find_map(|node| match node {
                        SceneNode::Text(t) if t.text == "abc" => {
                            Some((t.text.clone(), t.position, t.font_size))
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            assert_eq!(value(&nodes), value(&old));
            assert_eq!(f.field().state.cursor, plain.field().state.cursor);
            assert_eq!(f.field().state.selection, plain.field().state.selection);
            assert_eq!(f.field().last_layout_width, plain.field().last_layout_width);
            assert_eq!(
                (f.field().ascent, f.field().descent, f.field().padding_x),
                (
                    plain.field().ascent,
                    plain.field().descent,
                    plain.field().padding_x
                )
            );
            assert!(plain.field().input_surface_appearance.is_none());
        }
    }

    #[test]
    fn input_surface_appearance_keeps_original_edit_history_and_authoritative_cues() {
        let appearance = selected_input_surface(14.0);
        let mut f = Fixture::with_field(
            TextField::new()
                .with_text("abc")
                .with_input_surface_appearance(appearance.clone()),
        );
        let mut plain = Fixture::with_field(TextField::new().with_text("abc"));
        let command = Modifiers {
            meta: cfg!(target_os = "macos"),
            ctrl: !cfg!(target_os = "macos"),
            ..Default::default()
        };
        for target in [&mut f, &mut plain] {
            target.insert("Привет e\u{301}");
            assert_eq!(target.field().text(), "abcПривет e\u{301}");
            target.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: target.time,
                text: "a".into(),
                modifiers: command,
            });
            assert!(!target.field().state.selection.is_empty());
            target.insert("мир");
            assert_eq!(target.field().text(), "мир");
            target.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: target.time,
                text: "z".into(),
                modifiers: command,
            });
            assert_eq!(target.field().text(), "abcПривет e\u{301}");
            target.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: target.time,
                text: "z".into(),
                modifiers: Modifiers {
                    shift: true,
                    ..command
                },
            });
            assert_eq!(target.field().text(), "мир");
        }
        assert_eq!(f.field().state.cursor, plain.field().state.cursor);
        assert_eq!(f.field().state.selection, plain.field().state.selection);
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            ActionId::RELEASE_CAPTURE,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            None,
        ));
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        input_surface_frame(&mut f, 0.100, 1.0);
        let local = f.harness.local_fragment(f.owner).unwrap();
        let nodes = input_surface_leaves(&local);
        let focus = nodes
            .iter()
            .find_map(|node| match node {
                SceneNode::BorderRect(b) if b.z_index == crate::core::paint::LAYER_FOCUS + 1 => {
                    Some(b)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(
            focus.border_color,
            f.theme
                .input_chrome(ControlChromeState::Focused)
                .chrome
                .border_color
                .into()
        );
        assert_ne!(
            focus.border_color,
            f.theme
                .input_chrome(ControlChromeState::Captured)
                .chrome
                .border_color
                .into()
        );
    }

    #[test]
    fn input_surface_appearance_idle_disabled_and_clean_retained_output() {
        for enabled in [true, false] {
            let appearance = selected_input_surface(14.0);
            let field = TextField::new()
                .with_text("abc")
                .with_enabled(enabled)
                .with_input_surface_appearance(appearance.clone());
            let mut f =
                Fixture::unfocused_widget_and_text(Box::new(field), FontManagerTextEngine::new());
            input_surface_frame(&mut f, 0.010, 1.0);
            let local = f.harness.local_fragment(f.owner).unwrap();
            let peer = f.harness.local_fragment(f.peer).unwrap();
            let nodes = input_surface_leaves(&local);
            let gradient = nodes
                .iter()
                .find_map(|node| match node {
                    SceneNode::GradientRect(g) => Some(g),
                    _ => None,
                })
                .unwrap();
            let outer = nodes
                .iter()
                .find_map(|node| match node {
                    SceneNode::BorderRect(b) if b.rect == gradient.rect => Some(b),
                    _ => None,
                })
                .unwrap();
            let expected = if enabled {
                appearance.border()
            } else {
                f.theme
                    .input_chrome(ControlChromeState::Disabled)
                    .chrome
                    .border_color
            };
            assert_eq!(
                outer.border_color,
                crate::core::scene::Color::from(expected)
            );
            assert!(outer.z_index < TEXT_FIELD_TEXT_LAYER);
            assert_eq!(f.field().text(), "abc");
            assert_eq!(f.field().is_enabled(), enabled);
            assert!(!f.field().needs_animation());
            input_surface_frame(&mut f, 0.020, 1.0);
            assert!(Arc::ptr_eq(
                &local,
                &f.harness.local_fragment(f.owner).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &peer,
                &f.harness.local_fragment(f.peer).unwrap()
            ));
        }
    }

    #[test]
    fn input_surface_appearance_zero_and_narrow_extents_do_not_invent_geometry() {
        for scale in [1.0, 2.0] {
            for width in [0.0, 1.0] {
                let field =
                    TextField::new().with_input_surface_appearance(selected_input_surface(64.0));
                let mut f = Fixture::unfocused_widget_and_text(
                    Box::new(field),
                    FontManagerTextEngine::new(),
                );
                f.width = width;
                input_surface_frame(&mut f, 0.010, scale);
                let local = f.harness.local_fragment(f.owner).unwrap();
                let nodes = input_surface_leaves(&local);
                let gradient = nodes.iter().find_map(|node| match node {
                    SceneNode::GradientRect(g) => Some(g),
                    _ => None,
                });
                if width == 0.0 {
                    assert!(gradient.is_none());
                } else {
                    let g = gradient.unwrap();
                    assert_eq!(g.rect.width, width);
                    assert_eq!(g.radius, width * 0.5);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn u1_email_append_preserves_previous_glyph_pixels() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::math::Point;
        use crate::core::scene::SceneBuilder;
        use crate::core::text_engine::glyph_source_ranges;
        use crate::platform::font_manager::FontAsset;
        use crate::platform::system_font_provider::load_legacy_embedded_inter;
        use crate::renderer::DrawPlanBuilder;
        use crate::renderer::batcher::PrimitiveData;
        use crate::renderer::wgpu_renderer::WgpuRenderer;
        use std::collections::HashMap;

        struct Draw {
            text: String,
            origin: Point,
            clip: RectF,
        }

        struct Sample {
            pixels: Vec<u8>,
            caret: Vec<RectF>,
            draws: Vec<Draw>,
        }

        let mut failures = Vec::new();
        for scale in [1.0, 2.0] {
            let viewport = RectF::new(0.0, 0.0, 360.0, 90.0);
            let width = (viewport.width * scale) as u32;
            let height = (viewport.height * scale) as u32;
            let mut compositor = Compositor::new(viewport);
            let surface = compositor.create_surface(0, viewport, SceneKey::BASE);
            let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
                .expect("Metal renderer for the mounted TextField regression");
            assert_eq!(
                renderer.gpu_resources().adapter.get_info().backend,
                wgpu::Backend::Metal,
            );
            let face = load_legacy_embedded_inter();
            renderer
                .admit_font_assets(&[FontAsset {
                    family_name: face.family_name,
                    weight: face.weight,
                    italic: face.italic,
                    data: face.data,
                }])
                .unwrap();
            renderer.text_engine_mut().set_scale(scale);
            renderer
                .register_headless_render_target(surface, (width, height), scale)
                .unwrap();
            let target = renderer.custom_render_target_snapshot(surface).unwrap();
            let mut frame = |f: &mut Fixture, now: f64| {
                f.now = now;
                f.harness.advance_transition_time(now);
                let painted = f.harness.frame_with_optional_custom_render_target(
                    &mut f.tree,
                    f.root,
                    Constraints::tight(Size::new(viewport.width, viewport.height)),
                    &f.theme,
                    &mut f.text,
                    &builtin_registry(),
                    Some(target),
                    Some((&f.domains, WindowId::PRIMARY)),
                );
                let fragment = f.harness.local_fragment(f.owner).unwrap();
                let caret = input_surface_leaves(&fragment)
                    .into_iter()
                    .filter_map(|node| match node {
                        SceneNode::Text(text) => Some(text),
                        _ => None,
                    })
                    .flat_map(|text| {
                        text.backgrounds.iter().map(move |span| {
                            RectF::new(
                                (text.position.x + span.x_offset) * scale,
                                (text.position.y + span.y_offset) * scale,
                                span.width * scale,
                                span.height * scale,
                            )
                        })
                    })
                    .collect();
                let mut scene = SceneBuilder::new();
                scene.push_retained_fragment(painted.scene_root.into());
                let plan = DrawPlanBuilder::new()
                    .build_multi(
                        &HashMap::from([(surface, scene.build())]),
                        compositor.snapshot_multi(),
                        &mut crate::renderer::mesh_cache::MeshCache::new(),
                        |_| scale,
                        0,
                    )
                    .unwrap();
                let draws = plan.surface_data[&surface]
                    .primitives
                    .iter()
                    .filter_map(|primitive| match &primitive.data {
                        PrimitiveData::Text { position, text, .. }
                            if !text.is_empty() && text != "Unchanged peer" =>
                        {
                            Some(Draw {
                                text: text.clone(),
                                origin: *position,
                                clip: primitive.clip_state.rect,
                            })
                        }
                        _ => None,
                    })
                    .collect();
                let compiled = renderer
                    .compile_multi(&plan)
                    .unwrap()
                    .expect_ready("TextField frame");
                if let crate::renderer::TextRealizationOutcome::Unavailable(reason) =
                    compiled.text_realization(surface).unwrap().outcome()
                {
                    panic!("mounted TextField realization failed: {reason:?}");
                }
                let gpu = renderer.gpu_resources();
                let size = wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                };
                let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("TextField regression output"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: gpu.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let encoded = renderer
                    .encode_compiled_target_for_test(
                        &compiled,
                        surface,
                        &output.create_view(&Default::default()),
                    )
                    .unwrap();
                let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                    * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("TextField regression readback"),
                    size: u64::from(stride * height),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut copy = gpu.device.create_command_encoder(&Default::default());
                copy.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &output,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(stride),
                            rows_per_image: Some(height),
                        },
                    },
                    size,
                );
                renderer.submit_compiled_target_for_test(encoded);
                gpu.queue.submit([copy.finish()]);
                let (ready, result) = std::sync::mpsc::sync_channel(1);
                let slice = readback.slice(..);
                slice.map_async(wgpu::MapMode::Read, move |status| {
                    ready.send(status).unwrap()
                });
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                result.try_recv().unwrap().unwrap();
                let pixels = slice
                    .get_mapped_range()
                    .expect("mapped readback range")
                    .chunks_exact(stride as usize)
                    .flat_map(|row| row[..(width * 4) as usize].iter().copied())
                    .collect();
                Sample {
                    pixels,
                    caret,
                    draws,
                }
            };
            let mut f = Fixture::with_field(TextField::new().with_text("dev@y"));
            f.text.set_scale(scale);
            f.theme.typography.input_font_size = 16.0;
            f.tree.mark_layout_dirty(f.field);
            frame(&mut f, 0.003);
            let mut before = frame(&mut f, 0.006);
            for (start, append, expected) in [
                (0.010, "f", "dev@yf"),
                (0.100, "r", "dev@yfr"),
                (0.190, "o", "dev@yfro"),
            ] {
                let original = f.field().text().to_owned();
                let old = f.text.shape(&original, 16.0);
                f.insert(append);
                assert_eq!(f.field().text(), expected);
                let new = f.text.shape(expected, 16.0);
                if append != "r" {
                    let source_start = original.char_indices().next_back().unwrap().0;
                    let source = source_start..original.len();
                    let [prior, current] = [&old, &new].map(|run| {
                        let ranges = glyph_source_ranges(run, f.text.shaper(), scale).unwrap();
                        let mut glyphs = run
                            .glyphs
                            .iter()
                            .zip(ranges)
                            .filter(|(_, range)| range == &source);
                        let (glyph, _) = glyphs.next().expect("the same source cluster");
                        assert!(glyphs.next().is_none());
                        glyph
                    });
                    assert_eq!(
                        (prior.id, prior.x, prior.y),
                        (current.id, current.x, current.y),
                        "the complete shapes retain the prefix ink",
                    );
                    assert_ne!(prior.advance, current.advance, "terminal kerning witness");
                    println!(
                        "{original}→{expected} {scale}x source {source:?} id {}→{} xy ({}, {})→({}, {}) advance {}→{}",
                        prior.id,
                        current.id,
                        prior.x,
                        prior.y,
                        current.x,
                        current.y,
                        prior.advance,
                        current.advance,
                    );
                }
                let mut samples = Vec::new();
                for dt in [0.0, 0.003555555, 0.010666666, 0.021333332, 0.035555553] {
                    let image = frame(&mut f, start + dt);
                    assert_eq!(f.field().text(), expected);
                    if append != "r" {
                        let [draw] = image.draws.as_slice() else {
                            panic!(
                                "{original}→{expected} {scale}x at {dt}s requires a single source draw"
                            );
                        };
                        let [prior] = before.draws.as_slice() else {
                            panic!("one canonical prefix draw");
                        };
                        assert_eq!(draw.origin, prior.origin, "canonical position");
                        let general = f
                            .field()
                            .smooth_caret
                            .boundary
                            .as_ref()
                            .and_then(|boundary| boundary.general.as_ref())
                            .expect("qualified kerning reveal");
                        if general.progress == 0.0 {
                            assert_eq!(draw.text, original);
                            assert_eq!(draw.clip, prior.clip);
                        } else {
                            assert_eq!(draw.text, expected);
                            let edge = draw.origin.x
                                + (old.width + (new.width - old.width) * general.progress);
                            assert_eq!(draw.clip.x, prior.clip.x);
                            assert_eq!(draw.clip.right(), snap_to_physical(edge, scale));
                            if dt == 0.021333332 {
                                println!(
                                    "{original}→{expected} {scale}x at {dt}s source {:?} origin {:?} clip {:?}",
                                    draw.text, draw.origin, draw.clip,
                                );
                            }
                        }
                    }
                    samples.push((dt, f.field().smooth_caret.handle.is_some(), image));
                }
                let settled = frame(&mut f, (start + f64::from(0.040_f32)).next_up());
                assert_eq!(f.field().text(), expected);
                assert!(f.field().smooth_caret.handle.is_none());
                if append != "r" {
                    let mut animated = false;
                    for (dt, active, image) in samples {
                        let mut discrepancies = Vec::new();
                        let mut revealed = false;
                        let mut pending = false;
                        for (index, ((prior, current), intermediate)) in before
                            .pixels
                            .chunks_exact(4)
                            .zip(settled.pixels.chunks_exact(4))
                            .zip(image.pixels.chunks_exact(4))
                            .enumerate()
                        {
                            let x = (index % width as usize) as f32;
                            let y = (index / width as usize) as f32;
                            if [&before, &settled, &image].iter().any(|sample| {
                                sample.caret.iter().any(|r| {
                                    x + 1.0 > r.x && x < r.right() && y + 1.0 > r.y && y < r.bottom()
                                })
                            }) {
                                continue;
                            }
                            if prior != current {
                                revealed |= intermediate == current;
                                pending |= intermediate == prior;
                            }
                            if prior == current && intermediate != prior {
                                discrepancies.push((x, y));
                            }
                        }
                        animated |= active && revealed && pending;
                        if !discrepancies.is_empty() {
                            failures.push(format!(
                                "{original}→{expected} at {scale}x +{dt}s (active={active}) changed {} stable pixels, first {:?}",
                                discrepancies.len(),
                                discrepancies.first(),
                            ));
                        }
                    }
                    if !animated {
                        failures.push(format!(
                            "{original}→{expected} {scale}x did not reveal glyph ink"
                        ));
                    }
                }
                before = settled;
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn u1_unicode_end_edits_use_real_source_and_original_deadline() {
        for (prefix, append) in [
            ("Browser input", "!"),
            ("", "Привет мир"),
            ("Hello ", "мир"),
            ("e", "\u{301}"),
            ("👩", "\u{200d}💻"),
            ("سلا", "م"),
            ("שלו", "ם"),
            ("Hello ", "שלום"),
            ("שלום ", "Hello"),
        ] {
            let mut f = Fixture::with_field(TextField::new().with_text(prefix));
            let mut plain = Fixture::with_field(
                TextField::new()
                    .with_text(prefix)
                    .with_smooth_caret_prototype(false),
            );
            f.tick(0.002);
            plain.tick(0.002);
            f.insert(append);
            plain.insert(append);
            let expected = format!("{prefix}{append}");
            assert_eq!(f.field().text(), expected, "immediate logical source");
            assert_eq!(f.field().state.selection, plain.field().state.selection);
            f.tick(0.010);
            // The logical edit owns Layout. Sample the peer after that cut;
            // subsequent motion-only frames must reuse its clean fragment.
            let peer = f.harness.local_fragment(f.peer).unwrap();
            let font_size = f.field().state.layout_cache.font_size();
            let run = f.text.shape(&expected, font_size);
            let covered = run.glyphs.iter().all(|g| g.id != 0);
            assert_eq!(f.field().smooth_caret.handle.is_some(), covered);
            if !covered {
                assert_eq!(
                    f.field().smooth_caret.projection_refusal,
                    Some(edit_projection::Refusal::MissingGlyph)
                );
            }
            println!(
                "U1 end source={expected:?} clusters={} missing_glyphs={}",
                run.clusters.len(),
                run.glyphs.iter().filter(|g| g.id == 0).count()
            );
            f.tick(0.0375);
            assert_eq!(f.field().smooth_caret.boundary.is_some(), covered);
            assert!(std::sync::Arc::ptr_eq(
                &peer,
                &f.harness.local_fragment(f.peer).unwrap()
            ));
            f.tick((0.010 + f64::from(0.040_f32)).next_up());
            assert!(
                f.field().smooth_caret.handle.is_none(),
                "40ms terminal sample"
            );
            assert!(f.field().smooth_caret.boundary.is_none());
            plain.tick(0.070);
            f.key(KeyCode::Backspace, Modifiers::default());
            plain.key(KeyCode::Backspace, Modifiers::default());
            assert_eq!(
                f.field().text(),
                plain.field().text(),
                "same canonical grapheme deletion"
            );
            assert_eq!(f.field().state.cursor, plain.field().state.cursor);
            f.tick(0.080);
            assert_eq!(
                f.field().smooth_caret.handle.is_some(),
                covered,
                "Backspace {expected:?}: {:?}",
                f.field().smooth_caret.projection_refusal
            );
            f.tick((0.080 + f64::from(0.040_f32)).next_up());
            assert!(f.field().smooth_caret.handle.is_none());
            assert!(f.field().smooth_caret.boundary.is_none());
        }
    }

    #[test]
    fn u1_unicode_and_long_selection_deletion_uses_full_viewport() {
        for source in [
            "Привет мир".to_owned(),
            "Hello мир".to_owned(),
            "e\u{301} café".to_owned(),
            "👩\u{200d}💻 👨\u{200d}👩\u{200d}👧\u{200d}👦".to_owned(),
            "السلام عليكم".to_owned(),
            "שלום עולם".to_owned(),
            "Hello שלום world".to_owned(),
            "Ж".repeat(80),
            "Long deletion well beyond the old prototype admission boundary ".repeat(3),
        ] {
            let mut f = Fixture::with_field(TextField::new().with_text(&source));
            let font_size = f.field().state.layout_cache.font_size();
            let run = f.text.shape(&source, font_size);
            let covered = run.glyphs.iter().all(|g| g.id != 0);
            let width = run.width;
            f.width = width * 1.25 + f.field().padding_x * 2.0;
            f.tick(0.002);
            f.key(KeyCode::Home, Modifiers::default());
            f.key(
                KeyCode::End,
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
            );
            f.tick(0.003);
            f.tick(0.004);
            assert_eq!(f.field().state.selection.start(), 0);
            assert_eq!(f.field().state.selection.end(), source.len());
            f.key(KeyCode::Backspace, Modifiers::default());
            assert_eq!(f.field().text(), "", "selection mutation is immediate");
            f.tick(0.010);
            if !covered {
                assert!(f.field().smooth_caret.boundary.is_none());
                assert_eq!(
                    f.field().smooth_caret.projection_refusal,
                    Some(edit_projection::Refusal::MissingGlyph)
                );
                continue; // A typed coverage failure is not a script eligibility rule.
            }
            let boundary = f.field().smooth_caret.boundary.as_ref().unwrap_or_else(|| {
                panic!(
                    "{source:?}: {:?}",
                    f.field().smooth_caret.projection_refusal
                )
            });
            let travel = boundary.general.as_ref().map_or(
                (boundary.source_caret_x - boundary.destination.x).abs(),
                |g| g.travel,
            );
            let duration = TextField::range_duration(0.0, travel).unwrap();
            assert!(width > (f.width - 2.0 * f.field().padding_x) * 0.5);
            f.tick(0.010 + duration as f64 * 0.5);
            assert!(f.field().smooth_caret.handle.is_some());
            f.tick((0.010 + f64::from(duration)).next_up());
            assert!(f.field().smooth_caret.handle.is_none());
            assert!(f.field().smooth_caret.boundary.is_none());
        }
    }

    #[test]
    fn u1_projection_uses_complete_shapes_and_real_visual_order() {
        let mut text = FontManagerTextEngine::new();
        for (before, after, old, new) in [
            ("e", "e\u{301}", 1..1, 1..3),
            ("👩", "👩\u{200d}💻", 4..4, 4..11),
            ("سلام", "سلا", 6..8, 6..6),
            ("שלום", "שלו", 6..8, 6..6),
            ("Hello мир", "Hello ", 6..12, 6..6),
            ("Hello שלום world", "Hello  world", 6..14, 6..6),
        ] {
            let old_run = text.shape(before, 13.0);
            let new_run = text.shape(after, 13.0);
            if old_run
                .glyphs
                .iter()
                .chain(&new_run.glyphs)
                .any(|g| g.id == 0)
            {
                assert_eq!(
                    edit_projection::Projection::build(
                        &old_run,
                        &new_run,
                        old,
                        new,
                        text.shaper(),
                        1.0
                    )
                    .unwrap_err(),
                    edit_projection::Refusal::MissingGlyph
                );
                continue;
            }
            let plan = edit_projection::Projection::build(
                &old_run,
                &new_run,
                old,
                new,
                text.shaper(),
                1.0,
            )
            .unwrap();
            for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let width = old_run.width + (new_run.width - old_run.width) * t;
                let fragments = plan.fragments(t, 0.0..width).unwrap();
                let mut edge = 0.0;
                for fragment in &fragments {
                    assert!(
                        (fragment.clip.start - edge).abs() < 0.0001,
                        "no missing/overlapping region"
                    );
                    assert!(fragment.clip.end >= fragment.clip.start);
                    edge = fragment.clip.end;
                }
                assert!((edge - width).abs() < 0.0001);
                if t == 0.0 || t == 1.0 {
                    assert_eq!(
                        fragments
                            .iter()
                            .map(|f| f.clip.end - f.clip.start)
                            .sum::<f32>(),
                        width
                    );
                }
            }
        }
    }

    #[test]
    fn u1_projection_refuses_missing_source_and_invalid_geometry() {
        let mut text = FontManagerTextEngine::new();
        let old = text.shape("Привет", 13.0);
        let new = text.shape("Привет!", 13.0);
        let build = |a: &GlyphRun, b: &GlyphRun, scale| {
            edit_projection::Projection::build(a, b, 12..12, 12..13, text.shaper(), scale)
        };
        assert!(build(&old, &new, 1.0).is_ok());
        let mut bad = new.clone();
        bad.binding = None;
        assert_eq!(
            build(&old, &bad, 1.0).unwrap_err(),
            edit_projection::Refusal::GlyphEvidence
        );
        let mut bad = new.clone();
        bad.glyphs[0].id = bad.glyphs[0].id.wrapping_add(1);
        assert_eq!(
            build(&old, &bad, 1.0).unwrap_err(),
            edit_projection::Refusal::GlyphEvidence
        );
        assert_eq!(
            build(&old, &new, f32::NAN).unwrap_err(),
            edit_projection::Refusal::GlyphEvidence
        );
        assert_eq!(
            edit_projection::Projection::build(&old, &new, 1..1, 1..2, text.shaper(), 1.0)
                .unwrap_err(),
            edit_projection::Refusal::SourceAssociation
        );
    }

    #[test]
    fn u1_word_deletion_history_and_contextual_retarget_remain_immediate() {
        for source in [
            "Привет мир",
            "Hello мир",
            "السلام عليكم",
            "שלום עולם",
            "A 👩\u{200d}💻",
        ] {
            let mut f = Fixture::with_field(TextField::new().with_text(source));
            let mut plain = Fixture::with_field(
                TextField::new()
                    .with_text(source)
                    .with_smooth_caret_prototype(false),
            );
            f.tick(0.002);
            plain.tick(0.002);
            let run = f.text.shape(source, 13.0);
            let covered = run.glyphs.iter().all(|g| g.id != 0);
            let word = Modifiers {
                alt: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                ..Default::default()
            };
            f.key(KeyCode::Backspace, word);
            plain.key(KeyCode::Backspace, word);
            let deleted = plain.field().text().to_owned();
            assert_eq!(f.field().text(), deleted);
            f.tick(0.010);
            assert_eq!(
                f.field().smooth_caret.handle.is_some(),
                covered,
                "{source}: {:?}",
                f.field().smooth_caret.projection_refusal
            );
            f.tick(0.231);
            let command = Modifiers {
                meta: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                ..Default::default()
            };
            f.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: f.time,
                text: "z".into(),
                modifiers: command,
            });
            assert_eq!(f.field().text(), source, "one original Undo transaction");
            f.tick(0.240);
            assert!(f.field().smooth_caret.boundary.is_none());
            f.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: f.time,
                text: if cfg!(target_os = "macos") { "z" } else { "y" }.into(),
                modifiers: Modifiers {
                    shift: cfg!(target_os = "macos"),
                    ..command
                },
            });
            assert_eq!(f.field().text(), deleted, "original Redo transaction");
        }
        let mut f = Fixture::with_field(TextField::new().with_text("e"));
        f.tick(0.002);
        f.insert("\u{301}");
        f.tick(0.010);
        let first = f.field().smooth_caret.handle.unwrap();
        f.tick(0.020);
        f.insert("x");
        assert_eq!(f.field().text(), "e\u{301}x");
        f.tick(0.030);
        assert!(f.field().smooth_caret.handle.is_none());
        assert_ne!(f.field().smooth_caret.handle, Some(first));
        assert_eq!(
            f.field().smooth_caret.projection_refusal,
            Some(edit_projection::Refusal::RetargetGeometry)
        );
        assert_eq!(
            f.painted_x(),
            f.canonical_x(),
            "an incompatible contextual retarget must not rewind the old reveal"
        );
        f.tick(0.090);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn u1_registered_faces_prove_real_rtl_context_and_emoji_clusters() {
        use crate::platform::font_manager::FontManager;
        for (path, cases) in [
            (
                "/System/Library/Fonts/Supplemental/Arial.ttf",
                vec![("ب", "ب"), ("שלו", "ם"), ("Hello ", "שלום")],
            ),
            (
                "/System/Library/Fonts/Apple Color Emoji.ttc",
                vec![("👩", "\u{200d}💻")],
            ),
        ] {
            let mut manager = FontManager::new();
            let id = manager
                .load_font_file(path)
                .expect("explicit installed test face admission");
            for (prefix, append) in cases {
                let text =
                    FontManagerTextEngine::with_face_catalog(manager.face_catalog(), id).unwrap();
                let mut f = Fixture::with_widget_and_text(
                    Box::new(TextField::new().with_text(prefix)),
                    text,
                );
                f.tick(0.002);
                let old = f.text.shape(prefix, 13.0);
                f.insert(append);
                let expected = format!("{prefix}{append}");
                assert_eq!(f.field().text(), expected);
                let new = f.text.shape(&expected, 13.0);
                assert!(
                    old.glyphs.iter().chain(&new.glyphs).all(|g| g.id != 0),
                    "actual covering test face"
                );
                println!(
                    "U1 admitted_face={} source={expected:?} visual_sources={:?}",
                    f.text.identity().family_name,
                    new.clusters
                        .iter()
                        .map(|c| c.source.clone())
                        .collect::<Vec<_>>()
                );
                f.tick(0.010);
                assert!(
                    f.field().smooth_caret.handle.is_some(),
                    "{expected}: {:?}",
                    f.field().smooth_caret.projection_refusal
                );
                f.tick(0.0375);
                assert!(f.field().smooth_caret.boundary.is_some());
                f.tick((0.010 + f64::from(0.040_f32)).next_up());
                assert!(f.field().smooth_caret.boundary.is_none());
                f.key(KeyCode::Backspace, Modifiers::default());
                f.tick(0.080);
                assert!(f.field().smooth_caret.handle.is_some());
                f.tick((0.080 + f64::from(0.040_f32)).next_up());
                assert!(f.field().smooth_caret.handle.is_none());
            }
        }
    }

    // The ordinary engine cut is explicit here; the retained scene harness by
    // itself is not an App loop and cannot settle pending semantic work.
    fn handoff_fixture(handoff: &TextFieldEditHandoff) -> Fixture {
        let mut staged = TextField::new();
        staged.try_stage_edit_handoff(handoff).unwrap();
        assert_eq!(
            staged.edit_handoff_status(),
            TextFieldHandoffStatus::Pending
        );
        assert!(staged.text().is_empty());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let field = tree.add_child(root, Box::new(staged), None, 0);
        let peer_node = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 1);
        let owner = tree.node(field).unwrap().id;
        let peer = tree.node(peer_node).unwrap().id;
        let mut result = Fixture {
            tree,
            root,
            field,
            owner,
            peer,
            theme: Theme::default(),
            text: FontManagerTextEngine::new(),
            router: EventRouter::new(),
            domains: InteractionDomains::new(),
            harness: RetainedSceneTestHarness::new(),
            time: Instant::now(),
            now: 0.0,
            width: 360.0,
        };
        result.theme.typography.input_font_size = 19.0;
        assert_eq!(result.tree.node(field).unwrap().layout_epoch, 0);
        result
            .tree
            .process_text_field_handoffs(&mut result.text, &result.theme);
        assert_eq!(
            result.field().edit_handoff_status(),
            TextFieldHandoffStatus::Applied
        );
        assert_eq!(result.tree.node(field).unwrap().layout_epoch, 0);
        assert_eq!(result.field().state.cursor, handoff.cursor);
        assert_eq!(result.field().state.selection, handoff.selection);
        result.tick(0.0);
        assert!(result.tree.node(field).unwrap().layout_epoch > 0);
        fn paints_text(nodes: &[SceneNode], expected: &str) -> bool {
            nodes.iter().any(|node| match node {
                SceneNode::Text(text) => text.text == expected,
                SceneNode::Clip(clip) => paints_text(&clip.children, expected),
                SceneNode::RetainedFragment(fragment) => paints_text(&fragment.children, expected),
                _ => false,
            })
        }
        if !handoff.text.is_empty() {
            assert!(paints_text(
                &result.harness.local_fragment(owner).unwrap(),
                &handoff.text
            ));
        }
        result.router.request_focus_for_widget(
            &mut result.tree,
            owner,
            result.domains.primary_state_mut(),
        );
        assert!(result.router.handle_engine_action(
            &mut result.tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            result.domains.primary_state_mut(),
            &mut result.text,
            None,
        ));
        result.tick(0.001);
        result
    }

    #[test]
    fn edit_handoff_shaped_roundtrip_and_continued_editing() {
        let original = "Aé e\u{301} Ж🙂Z";
        for reverse in [true, false] {
            let mut source = Fixture::with_field(TextField::new().with_text(original));
            if reverse {
                source.key(KeyCode::End, Modifiers::default());
                source.key(
                    KeyCode::Home,
                    Modifiers {
                        shift: true,
                        ..Default::default()
                    },
                );
                assert_eq!(source.field().state.selection.anchor, original.len());
                assert_eq!(source.field().state.selection.head, 0);
            } else {
                source.key(KeyCode::Home, Modifiers::default());
                source.key(KeyCode::Right, Modifiers::default());
                source.key(KeyCode::Right, Modifiers::default());
                assert_eq!(source.field().state.cursor.pos, "Aé".len());
            }
            let saved = source.field().capture_edit_handoff().unwrap();
            let mut target = handoff_fixture(&saved);
            assert_eq!(target.field().text(), original);
            assert_eq!(target.field().state.cursor, saved.cursor);
            assert_eq!(target.field().state.selection, saved.selection);
            let version = target.field().state.document.version();
            target
                .tree
                .process_text_field_handoffs(&mut target.text, &target.theme);
            target.tick(0.002);
            assert_eq!(target.field().state.document.version(), version);
            assert_eq!(target.field().state.selection, saved.selection);
            let mut expected = original.to_string();
            expected.replace_range(saved.selection.start()..saved.selection.end(), "Q");
            target.insert("Q");
            assert_eq!(target.field().text(), expected);
            target.tick(0.003);
            // TextField's existing Cmd/Ctrl-Z owner receives routed TextInput,
            // not a fabricated logical KeyDown shortcut.
            target.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: target.time,
                text: "z".into(),
                modifiers: Modifiers {
                    ctrl: !cfg!(target_os = "macos"),
                    meta: cfg!(target_os = "macos"),
                    ..Default::default()
                },
            });
            assert_eq!(target.field().text(), original);
            assert_eq!(saved.text(), original);
        }
    }

    #[test]
    fn edit_handoff_full_cursor_and_silent_publication() {
        let mut source = TextField::new().with_text("éclair");
        source.state.cursor = TextCursor {
            pos: 2,
            preferred_x: Some(17.25),
            affinity: crate::core::editing::CursorAffinity::Right,
        };
        source.state.selection = Selection { anchor: 6, head: 2 };
        let saved = source.capture_edit_handoff().unwrap();
        assert_eq!(format!("{saved:?}"), "TextFieldEditHandoff { .. }");
        let calls = std::rc::Rc::new(Cell::new(0));
        let mut target = TextField::new();
        let observed = calls.clone();
        target.set_on_change(move |_| observed.set(observed.get() + 1));
        let observed = calls.clone();
        target.set_on_submit(move |_| observed.set(observed.get() + 1));
        target.try_stage_edit_handoff(&saved).unwrap();
        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(target), None, 0);
        tree.process_text_field_handoffs(&mut FontManagerTextEngine::new(), &Theme::default());
        let target = tree
            .node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<TextField>()
            .unwrap();
        assert_eq!(
            target.edit_handoff_status(),
            TextFieldHandoffStatus::Applied
        );
        assert_eq!(target.state.cursor, saved.cursor);
        assert_eq!(target.state.selection, saved.selection);
        assert!(!target.focused && !target.state.is_composing());
        assert_eq!(calls.get(), 0);
        let painted = handoff_fixture(&saved);
        assert_eq!(painted.field().state.cursor, saved.cursor);
        assert_eq!(painted.field().state.selection, saved.selection);
    }

    #[test]
    fn edit_handoff_staging_refuses_without_mutation() {
        use TextFieldHandoffError as Error;
        let saved = TextField::new()
            .with_text("éx")
            .capture_edit_handoff()
            .unwrap();
        let mut targets = vec![
            (TextField::new().with_max_len(Some(2)), Error::LimitExceeded),
            (
                TextField::new().with_password(true),
                Error::UnsupportedTarget,
            ),
            (
                TextField::new().with_text("existing"),
                Error::UnsupportedTarget,
            ),
        ];
        let mut filtered = TextField::new();
        filtered.set_input_filter(|ch| ch.is_ascii());
        targets.push((filtered, Error::InputRejected));
        let mut automatic = TextField::new();
        automatic.set_start_captured(true);
        targets.push((automatic, Error::UnsupportedTarget));
        let mut selection_override = TextField::new();
        selection_override.set_select_all_on_capture(true);
        targets.push((selection_override, Error::UnsupportedTarget));
        let mut focus_override = TextField::new();
        focus_override.config.select_all_on_focus = true;
        targets.push((focus_override, Error::UnsupportedTarget));
        let mut embedded = TextField::new();
        embedded.embedded = true;
        targets.push((embedded, Error::UnsupportedTarget));
        let mut composite = TextField::new();
        composite.composite_role = true;
        targets.push((composite, Error::UnsupportedTarget));
        let mut multiline = TextField::new();
        multiline.editing_policy.single_line = false;
        targets.push((multiline, Error::UnsupportedTarget));
        let mut composing = TextField::new();
        composing.state.ime_pre_composition_snapshot = Some(EditSnapshot::capture(
            "",
            &TextCursor::new(),
            &Selection::new(),
        ));
        assert!(matches!(
            composing.capture_edit_handoff(),
            Err(Error::CompositionActive)
        ));
        targets.push((composing, Error::CompositionActive));
        let mut mounted = TextField::new();
        mounted.on_mount();
        targets.push((mounted, Error::UnsupportedTarget));
        for (mut target, error) in targets {
            let before = (
                target.text().to_owned(),
                target.state.cursor,
                target.state.selection.clone(),
            );
            assert_eq!(target.try_stage_edit_handoff(&saved), Err(error));
            assert_eq!(
                (
                    target.text().to_owned(),
                    target.state.cursor,
                    target.state.selection.clone()
                ),
                before
            );
            assert_eq!(
                target.edit_handoff_status(),
                TextFieldHandoffStatus::NotRequested
            );
            assert!(!target.has_pending_edit_handoff());
        }
        for offset in [1, saved.text.len() + 1] {
            let mut bad = saved.clone();
            bad.cursor.pos = offset;
            let mut target = TextField::new();
            assert_eq!(
                target.try_stage_edit_handoff(&bad),
                Err(Error::InvalidBoundary)
            );
            assert!(target.text().is_empty());
        }
        let mut staged = TextField::new();
        staged.try_stage_edit_handoff(&saved).unwrap();
        assert_eq!(
            staged.try_stage_edit_handoff(&saved),
            Err(Error::UnsupportedTarget)
        );
        assert_eq!(
            staged.edit_handoff_status(),
            TextFieldHandoffStatus::Pending
        );
        assert_eq!(
            staged.pending_edit_handoff.as_ref().unwrap().text(),
            saved.text()
        );
    }

    #[test]
    fn edit_handoff_real_shaper_rejects_scalar_inside_cluster() {
        let mut saved = TextField::new()
            .with_text("e\u{301}")
            .capture_edit_handoff()
            .unwrap();
        saved.cursor.pos = 1; // UTF-8 scalar boundary inside the actual shaped cluster.
        saved.selection = Selection::at(1);
        let mut staged = TextField::new();
        staged.try_stage_edit_handoff(&saved).unwrap();
        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(staged), None, 0);
        let mut text = FontManagerTextEngine::new();
        tree.process_text_field_handoffs(&mut text, &Theme::default());
        let target = tree
            .node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<TextField>()
            .unwrap();
        assert_eq!(
            target.edit_handoff_status(),
            TextFieldHandoffStatus::Rejected(TextFieldHandoffError::InvalidBoundary)
        );
        assert_eq!(target.state.document.version(), 0);
        assert_eq!(target.state.cursor, TextCursor::new());
        assert!(target.text().is_empty());
        tree.process_text_field_handoffs(&mut text, &Theme::default());
        assert_eq!(saved.text(), "e\u{301}");
    }

    #[test]
    fn edit_handoff_revalidates_target_and_consumes_once() {
        let saved = TextField::new()
            .with_text("saved")
            .capture_edit_handoff()
            .unwrap();
        for input_changed in [true, false] {
            let mut staged = TextField::new();
            staged.try_stage_edit_handoff(&saved).unwrap();
            if input_changed {
                staged.set_text("new active text");
            } else {
                staged.set_input_filter(|_| false);
            }
            let before = staged.text().to_owned();
            let mut tree = WidgetTree::new();
            let node = tree.create_root(Box::new(staged), None, 0);
            tree.process_text_field_handoffs(&mut FontManagerTextEngine::new(), &Theme::default());
            let target = tree
                .node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextField>()
                .unwrap();
            assert_eq!(target.text(), before);
            assert_eq!(
                target.edit_handoff_status(),
                TextFieldHandoffStatus::Rejected(if input_changed {
                    TextFieldHandoffError::UnsupportedTarget
                } else {
                    TextFieldHandoffError::InputRejected
                })
            );
            assert!(!target.has_pending_edit_handoff());
        }
    }

    #[test]
    fn edit_handoff_unavailable_or_incomplete_shaping_refuses() {
        struct NoShaper(FontManagerTextEngine);
        impl TextEngine for NoShaper {
            fn measure(&mut self, text: &str, size: f32) -> (f32, f32) {
                self.0.measure(text, size)
            }
            fn metrics(&mut self, text: &str, size: f32) -> crate::core::text_engine::TextMetrics {
                self.0.metrics(text, size)
            }
            fn shape(&mut self, text: &str, size: f32) -> GlyphRun {
                self.0.shape(text, size)
            }
            fn draw(
                &mut self,
                run: &GlyphRun,
                x: f32,
                y: f32,
                color: Color,
                z: crate::core::paint::ZIndex,
                sink: &mut dyn crate::core::paint::PaintSink,
            ) {
                self.0.draw(run, x, y, color, z, sink);
            }
            fn draw_with_backgrounds(
                &mut self,
                run: &GlyphRun,
                x: f32,
                y: f32,
                color: Color,
                spans: Vec<TextBackgroundSpan>,
                z: crate::core::paint::ZIndex,
                sink: &mut dyn crate::core::paint::PaintSink,
            ) {
                self.0
                    .draw_with_backgrounds(run, x, y, color, spans, z, sink);
            }
        }
        // No injected cluster maps: the negative engine omits shaper access;
        // the second case uses the real paragraph engine's glyphless newline.
        for missing_shaper in [true, false] {
            let saved = TextField::new()
                .with_text(if missing_shaper { "draft" } else { "\n" })
                .capture_edit_handoff()
                .unwrap();
            let mut staged = TextField::new();
            staged.try_stage_edit_handoff(&saved).unwrap();
            let mut tree = WidgetTree::new();
            let node = tree.create_root(Box::new(staged), None, 0);
            let mut real = FontManagerTextEngine::new();
            if missing_shaper {
                tree.process_text_field_handoffs(&mut NoShaper(real), &Theme::default());
            } else {
                tree.process_text_field_handoffs(&mut real, &Theme::default());
            }
            let target = tree
                .node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextField>()
                .unwrap();
            assert_eq!(
                target.edit_handoff_status(),
                TextFieldHandoffStatus::Rejected(TextFieldHandoffError::ShapingUnavailable)
            );
            assert_eq!(target.state.document.version(), 0);
            assert!(target.text().is_empty());
        }
        let empty = TextField::new().capture_edit_handoff().unwrap();
        assert_eq!(
            handoff_fixture(&empty).field().edit_handoff_status(),
            TextFieldHandoffStatus::Applied
        );
    }

    #[test]
    fn composite_combo_box_default_typing_keeps_textfield_motion() {
        fn combo() -> Fixture {
            let mut f = Fixture::with_widget(Box::new(ComboBox::simple(vec![
                "prod-eu-west-gpu".into(),
                "prod-us-east-gpu".into(),
                "staging-hil".into(),
                "bench-lab".into(),
                "local-sim".into(),
            ])));
            let host = f.field;
            f.field = f
                .tree
                .children(host)
                .into_iter()
                .find(|child| {
                    f.tree
                        .node(*child)
                        .unwrap()
                        .widget
                        .as_any()
                        .is::<TextField>()
                })
                .expect("real tree-owned ComboBox TextField child");
            assert!(f.field().smooth_caret_enabled);
            assert!(f.field().composite_delegation.is_some());
            f
        }

        fn type_character(f: &mut Fixture, character: char) {
            let time = f.time;
            f.route(FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time,
                key_code: KeyCode::Character(character),
                physical_key: PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            });
            f.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time,
                text: character.to_string(),
                modifiers: Modifiers::default(),
            });
        }

        let mut f = combo();
        type_character(&mut f, 'z');
        f.tick(0.010);
        f.tick(0.080);
        let origin = f.canonical_x();
        type_character(&mut f, 'z');
        assert_eq!(f.field().text(), "zz");
        f.tick(0.090);
        assert!(
            f.field().smooth_caret.handle.is_some(),
            "qualified ComboBox insertion"
        );
        assert!(f.field().smooth_caret.boundary.is_some());
        f.tick(0.120);
        let x = f.field().smooth_caret.projection.unwrap().x;
        assert!(x > origin && x < f.canonical_x());

        fn painted_boundary(
            nodes: &[SceneNode],
            clip: Option<RectF>,
            edge: &mut Option<f32>,
            caret: &mut Option<f32>,
        ) {
            for node in nodes {
                match node {
                    SceneNode::Clip(n) => painted_boundary(&n.children, Some(n.rect), edge, caret),
                    SceneNode::RetainedFragment(n) => {
                        painted_boundary(&n.children, clip, edge, caret)
                    }
                    SceneNode::Text(n) => {
                        if n.text == "zz" {
                            let clip = clip.expect("qualified whole-run reveal clip");
                            *edge = Some(clip.x + clip.width);
                        }
                        if n.z_index == TEXT_FIELD_CARET_LAYER {
                            *caret = n.backgrounds.last().map(|b| n.position.x + b.x_offset);
                        }
                    }
                    _ => {}
                }
            }
        }
        let child = f.tree.node(f.field).unwrap().id;
        let (mut edge, mut caret) = (None, None);
        painted_boundary(
            &f.harness.local_fragment(child).unwrap(),
            None,
            &mut edge,
            &mut caret,
        );
        assert_eq!(
            edge, caret,
            "actual child paint shares the moving reveal/caret edge"
        );
        assert!(edge.is_some());
    }

    #[test]
    fn caret_boundary_p6_smoke() {
        fn presentation(f: &Fixture) -> (String, f32, f32) {
            fn collect(
                nodes: &[SceneNode],
                clip: Option<RectF>,
                text: &mut Option<(String, f32)>,
                caret: &mut Option<f32>,
            ) {
                for node in nodes {
                    match node {
                        SceneNode::Clip(n) => collect(&n.children, Some(n.rect), text, caret),
                        SceneNode::RetainedFragment(n) => collect(&n.children, clip, text, caret),
                        SceneNode::Text(n) => {
                            if !n.text.is_empty() {
                                let clip = clip.expect("actual nested whole-run clip");
                                *text = Some((n.text.clone(), clip.x + clip.width));
                                assert_eq!(n.z_index, TEXT_FIELD_TEXT_LAYER);
                            }
                            if let Some(background) = n.backgrounds.last() {
                                *caret = Some(n.position.x + background.x_offset);
                                assert_eq!(n.z_index, TEXT_FIELD_CARET_LAYER);
                            }
                        }
                        _ => {}
                    }
                }
            }
            let (mut text, mut caret) = (None, None);
            collect(
                &f.harness.local_fragment(f.owner).unwrap(),
                None,
                &mut text,
                &mut caret,
            );
            let (source, edge) = text.expect("actual whole string");
            (source, edge, caret.expect("separate text-native caret"))
        }

        let mut f = Fixture::new(true);
        let origin = f.canonical_x();
        f.insert("d");
        assert_eq!(f.field().text(), "abcd", "logical insertion is immediate");
        f.tick(0.010);
        let first = f.field().smooth_caret.handle.expect("40 ms Timeline leg");
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, origin);
        let (source, edge, caret) = presentation(&f);
        assert_eq!(source, "abcd");
        assert_eq!(edge, caret, "one physical clip/caret boundary");
        f.tick(0.030);
        let projection = f.field().smooth_caret.projection.unwrap();
        assert!(projection.x > origin && projection.x < f.canonical_x());
        assert_eq!(f.field().smooth_caret.handle, Some(first));
        let (_, edge, caret) = presentation(&f);
        assert_eq!(edge, caret);
        // Convert the stored f32 duration; one f64 ULP avoids subtracting
        // the start back to an elapsed value just below the 40 ms deadline.
        let deadline = (0.010 + f64::from(0.040_f32)).next_up();
        f.tick(deadline - 0.001);
        assert_eq!(f.field().smooth_caret.handle, Some(first));
        assert!(f.field().smooth_caret.boundary.is_some());
        f.tick(deadline);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
        assert_eq!(
            f.field().smooth_caret.projection.unwrap().x,
            f.canonical_x()
        );

        f.key(KeyCode::Backspace, Modifiers::default());
        assert_eq!(f.field().text(), "abc", "logical deletion is immediate");
        f.tick(0.070);
        assert_eq!(
            presentation(&f).0,
            "abcd",
            "only owned old visual source persists"
        );
        f.tick(0.080);
        let before_repeat = f.field().smooth_caret.projection.unwrap().x;
        f.key_with_repeat(KeyCode::Backspace, Modifiers::default(), true);
        assert_eq!(f.field().text(), "ab");
        f.tick(0.080);
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, before_repeat);
        assert_eq!(
            f.field()
                .smooth_caret
                .boundary
                .as_ref()
                .unwrap()
                .old_source
                .as_deref(),
            Some("abcd")
        );
        let (source, edge, caret) = presentation(&f);
        assert_eq!(source, "abcd");
        assert_eq!(edge, caret);
        let deadline = (0.080 + f64::from(0.040_f32)).next_up();
        let retargeted = f.field().smooth_caret.handle;
        assert!(retargeted.is_some());
        f.tick(deadline - 0.001);
        assert_eq!(f.field().smooth_caret.handle, retargeted);
        assert!(f.field().smooth_caret.boundary.is_some());
        f.tick(deadline);
        assert!(f.field().smooth_caret.boundary.is_none());
        assert!(f.field().smooth_caret.handle.is_none());
        assert_eq!(
            f.field().smooth_caret.projection.unwrap().x,
            f.canonical_x()
        );
        assert_eq!(f.painted_x(), f.canonical_x());

        f.insert("é");
        f.tick(0.140);
        assert_eq!(f.field().text(), "abé");
        assert!(
            f.field().smooth_caret.boundary.is_some(),
            "HAL101 §13: supported Unicode is not an ambiguous prototype source"
        );
        f.tick((0.140 + f64::from(0.040_f32)).next_up());
        assert!(f.field().smooth_caret.boundary.is_none());
        let mut disabled = Fixture::new(false);
        disabled.insert("d");
        disabled.tick(0.010);
        assert!(disabled.field().smooth_caret.boundary.is_none());
        assert!(disabled.field().smooth_caret.handle.is_none());
    }

    #[test]
    fn text_field_default_motion_smoke() {
        caret_boundary_range_smoke(); // includes the P6 40 ms deadline proof
        smooth_caret_selection_ime_configuration_and_capture_boundaries_cancel();
        assert!(TextField::default().smooth_caret_prototype_enabled());

        // This is normal construction, with no per-instance true opt-in.
        let mut f = Fixture::with_field(TextField::new().with_text("abc"));
        assert!(f.field().smooth_caret_prototype_enabled());
        f.insert("d");
        assert_eq!(f.field().text(), "abcd");
        f.tick(0.010);
        assert!(f.field().smooth_caret.handle.is_some());
        f.tick(0.070);
        assert!(f.field().smooth_caret.handle.is_none());

        let word = if cfg!(target_os = "macos") {
            Modifiers {
                alt: true,
                ..Modifiers::default()
            }
        } else {
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            }
        };
        f.key(KeyCode::Backspace, word);
        assert_eq!(
            f.field().text(),
            "",
            "default word editing remains immediate"
        );
        f.tick(0.080);
        assert!(
            f.field()
                .smooth_caret
                .boundary
                .as_ref()
                .unwrap()
                .range
                .is_some()
        );
        f.field_mut().set_smooth_caret_prototype(false);
        f.tick(0.090);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
        assert!(f.field().smooth_caret.projection.is_none());
        assert!(f.field().smooth_caret.range_origin.is_none());
        f.insert("x");
        f.tick(0.100);
        assert!(!f.field().smooth_caret_prototype_enabled());
        assert!(f.field().smooth_caret.handle.is_none());
        assert_eq!(f.painted_x(), f.canonical_x());

        f.field_mut().set_smooth_caret_prototype(true);
        f.tick(0.110); // re-enable establishes a static origin
        assert!(f.field().smooth_caret.handle.is_none());
        let facts = f.field().smooth_caret.projection.unwrap().space.facts;
        let mut password = TextField::new().with_password(true);
        assert!(password.motion_eligible(Some(facts)));
        password.embedded = true;
        assert!(!password.motion_eligible(Some(facts)));
        assert!(
            !TextField::new()
                .with_enabled(false)
                .motion_eligible(Some(facts))
        );
        let mut embedded = TextField::new();
        embedded.set_embedded(true);
        assert!(!embedded.motion_eligible(Some(facts)));

        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.120);
        assert!(f.field().smooth_caret.handle.is_some());
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            ActionId::RELEASE_CAPTURE,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            None,
        ));
        f.tick(0.130);
        assert!(
            f.field().focused,
            "Capture release deliberately retains focus"
        );
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
        assert!(f.field().smooth_caret.projection.is_none());
        assert!(f.field().smooth_caret.range_origin.is_none());
        assert!(!f.field().needs_animation());

        // A legal property change leaves stale cache before Layout. Non-Capture
        // Update must skip all motion geometry/scans and keep its state empty.
        f.field_mut().set_text("normal field");
        f.tree.mark_layout_dirty(f.field);
        f.tick(0.140);
        f.tick(0.150);
        assert!(f.field().smooth_caret.projection.is_none());
        assert!(f.field().smooth_caret.range_origin.is_none());
        assert!(!f.field().needs_animation());
    }

    #[test]
    fn browser_word_delete_target_policy_is_exclusive_and_preserves_native() {
        for bits in 0_u8..16 {
            let modifiers = Modifiers {
                alt: bits & 1 != 0,
                ctrl: bits & 2 != 0,
                meta: bits & 4 != 0,
                shift: bits & 8 != 0,
            };
            assert_eq!(
                TextField::word_delete_modifiers(modifiers, true, false),
                bits == 1 || bits == 2,
                "browser modifier bits={bits}"
            );
            assert_eq!(
                TextField::word_delete_modifiers(modifiers, false, true),
                bits == 1,
                "native macOS modifier bits={bits}"
            );
            assert_eq!(
                TextField::word_delete_modifiers(modifiers, false, false),
                bits == 2,
                "native non-macOS modifier bits={bits}"
            );
        }
    }

    #[test]
    fn browser_copy_uses_canonical_unicode_and_requalifies_proxy_source() {
        use crate::core::event_router::browser_text::{BrowserTextSession, BrowserTextTarget};

        let original = "λ e\u{301} 👩🏽‍🚀 ".repeat(18);
        let mut f = Fixture::with_field(TextField::new().with_text(&original));
        f.key(KeyCode::Home, Modifiers::default());
        let extend = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        f.key(KeyCode::End, extend);
        let (target, projection) = BrowserTextTarget::qualify(
            &f.tree, &f.domains, WindowId::PRIMARY, f.root,
        )
        .unwrap();
        assert_ne!(projection.value, original, "the proxy only holds a selection marker");
        let session = BrowserTextSession::new(target, projection);
        let copy = |f: &Fixture, proxy| {
            BrowserTextTarget::copy_selection(&f.tree, &f.domains, WindowId::PRIMARY, f.root, proxy)
                .map(str::to_owned)
        };
        assert_eq!(copy(&f, None).as_deref(), Some(original.as_str()));
        assert_eq!(copy(&f, Some(&session)).as_deref(), Some(original.as_str()));
        assert!(!f.field().state.undo_history.can_undo());

        f.key(KeyCode::Left, extend);
        assert!(copy(&f, None).is_some());
        assert!(copy(&f, Some(&session)).is_none());
        f.field_mut().set_password(true);
        assert!(copy(&f, None).is_none());
        f.field_mut().set_password(false);
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            ActionId::RELEASE_CAPTURE,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            None,
        ));
        assert!(copy(&f, None).is_none());
        assert_eq!(f.field().text(), original);

        let mut read_only =
            Fixture::with_field(TextField::new().with_text("read-only λ").with_read_only(true));
        read_only.key(KeyCode::Home, Modifiers::default());
        read_only.key(KeyCode::End, extend);
        assert_eq!(copy(&read_only, None).as_deref(), Some("read-only λ"));
        assert!(!read_only.field().state.undo_history.can_undo());
    }

    #[test]
    fn browser_copy_cut_shortcut_cannot_delete_without_clipboard_transfer() {
        use std::cell::Cell;
        use std::rc::Rc;

        let changes = Rc::new(Cell::new(0));
        let observed = changes.clone();
        let mut field = TextField::new().with_text("original λ");
        field.set_on_change(move |_| observed.set(observed.get() + 1));
        let mut f = Fixture::with_field(field);
        f.router.use_browser_clipboard_transport_for_test();
        f.key(KeyCode::Home, Modifiers::default());
        f.key(KeyCode::End, Modifiers { shift: true, ..Modifiers::default() });
        let before = f.field().state.selection.clone();
        assert_ne!(before.anchor, before.head);
        for command in ["c", "x"] {
            f.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: f.time,
                text: command.into(),
                modifiers: Modifiers {
                    meta: cfg!(target_os = "macos"),
                    ctrl: !cfg!(target_os = "macos"),
                    ..Modifiers::default()
                },
            });
            assert_eq!(f.field().text(), "original λ");
            let selection = &f.field().state.selection;
            assert_eq!((selection.anchor, selection.head), (before.anchor, before.head));
            assert!(!f.field().state.undo_history.can_undo());
            assert_eq!(changes.get(), 0);
        }
    }

    #[test]
    fn browser_cut_transfers_the_exact_selection_and_records_one_undoable_edit() {
        use std::cell::Cell;
        use std::rc::Rc;

        let selected = "λ e\u{301} 👩🏽‍🚀 ".repeat(18);
        let original = format!("a{selected}z");
        let changes = Rc::new(Cell::new(0));
        let observed = changes.clone();
        let mut field = TextField::new().with_text(&original);
        field.set_on_change(move |_| observed.set(observed.get() + 1));
        let mut f = Fixture::with_field(field);
        let extend = Modifiers { shift: true, ..Modifiers::default() };
        f.key(KeyCode::Home, Modifiers::default());
        f.key(KeyCode::Right, Modifiers::default());
        f.key(KeyCode::End, extend);
        f.key(KeyCode::Left, extend);
        let before = f.field().capture_snapshot();
        assert_eq!((before.selection_anchor, before.selection_head), (1, original.len() - 1));

        let cut = |f: &mut Fixture, accept| {
            f.router.route_browser_cut(
                &mut f.tree, &mut f.domains, WindowId::PRIMARY, f.root, None, &mut f.text,
                |text| {
                    assert_eq!(text, selected);
                    accept
                },
            )
        };
        assert!(!cut(&mut f, false));
        assert_eq!(f.field().text(), original);
        assert_eq!(f.field().state.cursor.pos, before.cursor_pos);
        assert_eq!(f.field().state.selection.head, before.selection_head);
        assert!(!f.field().state.undo_history.can_undo());
        assert_eq!(changes.get(), 0);

        assert!(cut(&mut f, true));
        assert_eq!(f.field().text(), "az");
        assert_eq!(f.field().state.cursor.pos, 1);
        assert!(!f.field().has_active_selection());
        assert_eq!(changes.get(), 1);
        assert!(!cut(&mut f, true), "a collapsed selection cannot transfer or delete");
        assert_eq!(changes.get(), 1);
        f.tick(0.250);
        assert_eq!(f.field().state.scroll_x, 0.0);

        let command = Modifiers {
            meta: cfg!(target_os = "macos"),
            ctrl: !cfg!(target_os = "macos"),
            ..Modifiers::default()
        };
        for redo in [false, true] {
            f.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: f.time,
                text: "z".into(),
                modifiers: Modifiers { shift: redo, ..command },
            });
            if redo {
                assert_eq!(f.field().text(), "az");
                assert_eq!(f.field().state.cursor.pos, 1);
                assert!(!f.field().has_active_selection());
                assert_eq!(changes.get(), 3);
            } else {
                assert_eq!(f.field().text(), original);
                let state = f.field().capture_snapshot();
                assert_eq!(state.cursor_pos, before.cursor_pos);
                assert_eq!(state.selection_anchor, before.selection_anchor);
                assert_eq!(state.selection_head, before.selection_head);
                assert!(!f.field().state.undo_history.can_undo());
                assert_eq!(changes.get(), 2);
            }
        }
    }

    #[test]
    fn browser_cut_refuses_stale_proxy_and_protected_editor_state_before_transfer() {
        use crate::core::event_router::browser_text::{
            BrowserTextAction, BrowserTextSession, BrowserTextTarget,
        };

        let mut f = Fixture::with_field(TextField::new().with_text("email λ"));
        f.key(KeyCode::Home, Modifiers::default());
        let extend = Modifiers { shift: true, ..Modifiers::default() };
        f.key(KeyCode::End, extend);
        let (target, before) =
            BrowserTextTarget::qualify(&f.tree, &f.domains, WindowId::PRIMARY, f.root).unwrap();
        let stale = BrowserTextSession::new(target, before);
        let refuse = |f: &mut Fixture, proxy| {
            assert!(!f.router.route_browser_cut(
                &mut f.tree, &mut f.domains, WindowId::PRIMARY, f.root, proxy, &mut f.text,
                |_| panic!("refused Cut must never stage clipboard data"),
            ));
            assert_eq!(f.field().text(), "email λ");
            assert!(!f.field().state.undo_history.can_undo());
        };

        f.key(KeyCode::Left, extend);
        refuse(&mut f, Some(&stale));
        f.field_mut().set_password(true);
        refuse(&mut f, None);
        f.field_mut().set_password(false);
        f.field_mut().apply_availability_policy(true, true);
        refuse(&mut f, None);
        f.field_mut().apply_availability_policy(true, false);

        let (target, before) =
            BrowserTextTarget::qualify(&f.tree, &f.domains, WindowId::PRIMARY, f.root).unwrap();
        let active = f.router.route_browser_text(
            &mut f.tree, &mut f.domains, f.root, &target, &before,
            &BrowserTextAction::Composition(ImeEvent::PreEdit {
                text: "preedit".into(),
                cursor: None,
            }),
            &mut f.text,
        ).unwrap();
        assert!(active.composing);
        refuse(&mut f, None);
        let canceled = f.router.route_browser_text(
            &mut f.tree, &mut f.domains, f.root, &target, &active,
            &BrowserTextAction::Composition(ImeEvent::Cancel),
            &mut f.text,
        ).unwrap();
        assert!(!canceled.composing);
        refuse(&mut f, Some(&stale));
        f.domains.set_active_window(None);
        refuse(&mut f, None);
    }

    #[test]
    fn browser_paste_routed_selection_is_one_immediate_history_transaction() {
        use std::cell::Cell;
        use std::rc::Rc;
        let changes = Rc::new(Cell::new(0));
        let observed = changes.clone();
        let mut field = TextField::new().with_text("abc");
        field.set_on_change(move |_| observed.set(observed.get() + 1));
        let mut f = Fixture::with_field(field);
        f.router.use_browser_clipboard_transport_for_test();
        f.key(KeyCode::Home, Modifiers::default());
        f.key(
            KeyCode::End,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(
            (
                f.field().state.selection.anchor,
                f.field().state.selection.head
            ),
            (0, 3)
        );
        let request = f.browser_paste_request("safe probe 42");
        let command = if cfg!(target_os = "macos") {
            Modifiers {
                meta: true,
                ..Modifiers::default()
            }
        } else {
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            }
        };
        // The original Key is not replaced/replayed by the typed Paste intake.
        f.key(KeyCode::Character('v'), command);
        f.route(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: f.time,
            text: "v".into(),
            modifiers: command,
        });
        assert_eq!(f.field().text(), "abc");
        assert_eq!(changes.get(), 0);
        f.browser_paste(request).unwrap();
        assert_eq!(f.field().text(), "safe probe 42");
        assert_eq!(changes.get(), 1);
        assert_eq!(f.field().state.cursor.pos, 13);
        assert!(!f.field().has_active_selection());
        f.tick(0.250);
        let peer = f.harness.local_fragment(f.peer).unwrap();
        f.tick(0.500);
        assert!(Arc::ptr_eq(
            &peer,
            &f.harness.local_fragment(f.peer).unwrap()
        ));
        f.route(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: f.time,
            text: "z".into(),
            modifiers: command,
        });
        assert_eq!(
            f.field().text(),
            "abc",
            "one Undo restores the whole original interval"
        );
        assert!(
            !f.field().state.undo_history.can_undo(),
            "no duplicate Paste transaction"
        );
        f.route(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: f.time,
            text: "z".into(),
            modifiers: Modifiers {
                shift: true,
                ..command
            },
        });
        assert_eq!(f.field().text(), "safe probe 42");
    }

    #[test]
    fn browser_paste_refuses_changed_source_selection_capture_mount_window_and_root() {
        for change in 0..7 {
            let mut f = Fixture::new(false);
            f.key(KeyCode::End, Modifiers::default());
            let request = f.browser_paste_request("safe probe");
            match change {
                0 => f.field_mut().set_text("changed"),
                1 => f.key(KeyCode::Left, Modifiers::default()),
                2 => {
                    assert!(f.router.handle_engine_action(
                        &mut f.tree,
                        ActionId::RELEASE_CAPTURE,
                        Some(f.root),
                        f.domains.primary_state_mut(),
                        &mut f.text,
                        None,
                    ));
                }
                3 => {
                    let other = f
                        .tree
                        .add_child(f.root, Box::new(TextField::new()), None, 3);
                    let owner = f.tree.node(other).unwrap().id;
                    f.router.request_focus_for_widget(
                        &mut f.tree,
                        owner,
                        f.domains.primary_state_mut(),
                    );
                }
                4 => f.tree.replace_widget_only(
                    f.field,
                    Box::new(TextField::new().with_text("replacement")),
                ),
                5 => f.domains.set_active_window(None),
                6 => {
                    f.root = f.tree.add_child(f.root, Box::new(Flex::column()), None, 4);
                }
                _ => unreachable!(),
            }
            let before = f.field().text().to_owned();
            assert!(
                !request.is_current(&f.tree, &f.domains, f.root),
                "change={change}"
            );
            assert!(f.browser_paste(request).is_err(), "change={change}");
            assert_eq!(f.field().text(), before);
            assert!(!f.field().state.undo_history.can_undo());
        }
        let mut f = Fixture::new(false);
        let request = f.browser_paste_request("safe probe");
        f.tree.remove_node(f.field);
        assert!(
            f.browser_paste(request).is_err(),
            "retired owner cannot redirect"
        );
    }

    #[test]
    fn browser_paste_refuses_read_only_disabled_ime_and_length_without_history() {
        use crate::core::event_router::browser_paste::PasteRefusal;
        for change in 0..5 {
            let mut f = Fixture::new(false);
            f.key(KeyCode::End, Modifiers::default());
            let request = f.browser_paste_request("safe probe");
            let expected = match change {
                0 => {
                    f.field_mut().apply_availability_policy(true, true);
                    BrowserPasteRefusal::ReadOnly
                }
                1 => {
                    f.field_mut().apply_availability_policy(false, false);
                    BrowserPasteRefusal::Disabled
                }
                2 => {
                    // The same existing IME handler establishes composition/history suspension.
                    let tree = &mut f.tree as *mut WidgetTree;
                    let mut cx = EventContext::new(
                        f.owner,
                        tree,
                        1,
                        std::ptr::null_mut(),
                        Some(&mut f.text),
                        None,
                        false,
                        false,
                        KeyboardAuthorityMode::Capture,
                    );
                    let field = f
                        .tree
                        .node_mut(f.field)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<TextField>()
                        .unwrap();
                    assert!(field.handle_event(
                        &mut cx,
                        &Event::Ime(ImeEvent::PreEdit {
                            text: "probe".into(),
                            cursor: None,
                        }),
                        EventPhase::Target
                    ));
                    BrowserPasteRefusal::CompositionActive
                }
                3 => {
                    f.field_mut().set_max_len_bytes(Some(3));
                    BrowserPasteRefusal::StaleSource
                }
                4 => {
                    f.field_mut().editing_policy.normalize_paste_newlines = false;
                    BrowserPasteRefusal::StaleSource
                }
                _ => unreachable!(),
            };
            assert_eq!(
                f.browser_paste(request),
                Err(PasteRefusal::Editor(expected))
            );
            assert_eq!(f.field().text(), "abc");
            assert!(!f.field().state.undo_history.can_undo());
        }
        let mut f = Fixture::new(false);
        let request = f.browser_paste_request("safe probe");
        f.tree.request_widget_read_only(f.field, true).unwrap();
        assert_eq!(
            f.browser_paste(request),
            Err(PasteRefusal::AvailabilityPending)
        );
        assert_eq!(f.field().text(), "abc");
        assert!(!f.field().state.undo_history.can_undo());
    }

    #[test]
    fn browser_paste_preserves_unicode_normalization_and_password_presentation() {
        let mut f = Fixture::with_field(TextField::new().with_password(true).with_text("abc"));
        f.key(KeyCode::Home, Modifiers::default());
        f.key(
            KeyCode::End,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        let request = f.browser_paste_request("probe\r\nстрока");
        f.browser_paste(request).unwrap();
        assert_eq!(f.field().text(), "probe строка");
        f.tick(0.250);
        fn assert_masked(nodes: &[SceneNode]) {
            for node in nodes {
                match node {
                    SceneNode::Text(text) => {
                        assert!(!text.text.contains("probe") && !text.text.contains("строка"))
                    }
                    SceneNode::Clip(clip) => assert_masked(&clip.children),
                    SceneNode::Scroll(scroll) => assert_masked(&scroll.children),
                    SceneNode::RetainedFragment(fragment) => assert_masked(&fragment.children),
                    _ => {}
                }
            }
        }
        assert_masked(&f.harness.local_fragment(f.owner).unwrap());
    }

    #[test]
    fn browser_paste_payload_is_bounded_and_unknown_capture_is_refused() {
        use crate::core::event_router::browser_paste::{BrowserPasteTarget, PasteRefusal};
        let mut f = Fixture::new(false);
        let target =
            || BrowserPasteTarget::qualify(&f.tree, &f.domains, WindowId::PRIMARY, f.root).unwrap();
        assert!(matches!(
            target().with_text(String::new()),
            Err(PasteRefusal::Empty)
        ));
        assert!(
            target().with_text("x".repeat(256 * 1024 + 1)).is_ok(),
            "the former transport quota is not an editor limit"
        );
        assert!(BrowserPasteTarget::qualify(&f.tree, &f.domains, WindowId(91), f.root).is_err());
        assert_eq!(
            f.domains.primary_state_mut().focus_manager.clear_focus(),
            Some(f.owner)
        );
        assert!(
            BrowserPasteTarget::qualify(&f.tree, &f.domains, WindowId::PRIMARY, f.root).is_err()
        );
    }

    #[test]
    fn browser_paste_actual_editor_limit_counts_utf8_after_replacement_and_normalization() {
        use crate::core::event_router::browser_paste::{BrowserPasteTarget, PasteRefusal};
        let mut f = Fixture::with_field(TextField::new().with_text("abc").with_max_len(Some(4)));
        f.key(KeyCode::End, Modifiers::default());
        let target =
            BrowserPasteTarget::qualify(&f.tree, &f.domains, WindowId::PRIMARY, f.root).unwrap();
        assert!(
            matches!(target.with_text("é".into()), Err(PasteRefusal::TooLarge)),
            "one remaining byte cannot hold two UTF-8 bytes"
        );
        let request = f.browser_paste_request("Z");
        f.browser_paste(request).unwrap();
        assert_eq!(f.field().text(), "abcZ");

        f.key(KeyCode::Home, Modifiers::default());
        f.key(
            KeyCode::End,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        let request = f.browser_paste_request("éé");
        f.browser_paste(request).unwrap();
        assert_eq!(
            f.field().text(),
            "éé",
            "replacement allowance excludes selected bytes"
        );

        let mut f = Fixture::with_field(TextField::new().with_text("abc").with_max_len(Some(3)));
        f.key(KeyCode::Home, Modifiers::default());
        f.key(
            KeyCode::End,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        let request = f.browser_paste_request("a\r\nb");
        f.browser_paste(request).unwrap();
        assert_eq!(
            f.field().text(),
            "a b",
            "CRLF is one normalized byte, not two"
        );
        assert!(!f.field().can_insert_text("x"));
    }

    #[test]
    fn browser_paste_invalid_private_selection_refuses_without_arithmetic_overflow() {
        let mut f = Fixture::new(false);
        f.field_mut().state.selection.head = usize::MAX;
        assert!(!f.field().can_insert_text("x"));
        assert_eq!(
            f.field().browser_paste_state(),
            Err(BrowserPasteRefusal::InvalidState)
        );
        let mut f = Fixture::with_field(TextField::new().with_text("é"));
        f.field_mut().state.selection.head = 1;
        assert_eq!(
            f.field().browser_paste_state(),
            Err(BrowserPasteRefusal::InvalidState)
        );
    }

    #[test]
    fn browser_word_delete_routed_edit_history_and_native_motion_stay_immediate() {
        for alt in [true, false] {
            let mut f = Fixture::with_field(TextField::new().with_text("abc def ghi"));
            f.tick(0.002);
            assert_eq!(f.domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);
            let before_version = f.field().state.document.version();
            let modifiers = Modifiers { alt, ctrl: !alt, ..Modifiers::default() };
            f.key(KeyCode::Backspace, modifiers);
            assert_eq!(f.field().text(), "abc def ");
            assert_eq!(f.field().state.cursor.pos, 8);
            assert_eq!(f.field().state.selection.anchor, 8);
            assert_eq!(f.field().state.selection.head, 8);
            assert_eq!(f.field().state.document.version(), before_version + 1);
            f.tick(0.010);
            let native_eligible = if cfg!(target_os = "macos") { alt } else { !alt };
            assert_eq!(f.field().smooth_caret.handle.is_some(), native_eligible);
            assert_eq!(
                f.field().smooth_caret.boundary.as_ref().is_some_and(|b| b.range.is_some()),
                native_eligible
            );
            f.tick(0.231);
            assert!(f.field().smooth_caret.handle.is_none());
            let command = Modifiers {
                meta: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                ..Modifiers::default()
            };
            f.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: f.time,
                text: "z".into(),
                modifiers: command,
            });
            assert_eq!(f.field().text(), "abc def ghi", "Undo uses original immediate history");
            f.tick(0.240);
            assert!(f.field().smooth_caret.handle.is_none());
            f.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: f.time,
                text: "z".into(),
                modifiers: Modifiers { shift: true, ..command },
            });
            assert_eq!(f.field().text(), "abc def ", "Redo uses original immediate history");
            f.tick(0.250);
            assert!(f.field().smooth_caret.handle.is_none());
        }
    }

    #[test]
    fn browser_word_delete_rejects_mixed_command_shift_and_unknown_sources() {
        for modifiers in [
            Modifiers { alt: true, ctrl: true, ..Modifiers::default() },
            Modifiers { meta: true, ..Modifiers::default() },
            Modifiers { alt: true, shift: true, ..Modifiers::default() },
            Modifiers { ctrl: true, shift: true, ..Modifiers::default() },
        ] {
            let mut f = Fixture::with_field(TextField::new().with_text("abc def ghi"));
            f.tick(0.002);
            f.key(KeyCode::Backspace, modifiers);
            assert_eq!(
                f.field().text(),
                if modifiers.alt || modifiers.ctrl { "abc def " } else { "abc def gh" },
                "presentation eligibility must not change the existing semantic edit"
            );
            f.tick(0.010);
            assert!(f.field().smooth_caret.handle.is_none());
            assert!(f.field().smooth_caret.boundary.is_none());
        }
        let mut f = Fixture::with_field(TextField::new().with_text("abc def ghi"));
        f.tick(0.002);
        let version = f.field().state.document.version();
        f.key(KeyCode::Character('q'), Modifiers { ctrl: true, ..Modifiers::default() });
        f.key(KeyCode::Delete, Modifiers::default());
        f.tick(0.010);
        assert_eq!(f.field().text(), "abc def ghi");
        assert_eq!(f.field().state.document.version(), version);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
    }

    #[test]
    fn browser_word_delete_requires_original_keyboard_capture() {
        let mut f = Fixture::with_field(TextField::new().with_text("abc def ghi"));
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            ActionId::RELEASE_CAPTURE,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            None,
        ));
        f.tick(0.010);
        f.key(KeyCode::Backspace, Modifiers { alt: true, ..Modifiers::default() });
        f.tick(0.020);
        assert_eq!(f.field().text(), "abc def ghi");
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
    }

    #[test]
    fn caret_boundary_range_smoke() {
        // Reuse the already native-evaluated P6 owner smoke in this one vital pass.
        caret_boundary_p6_smoke();
        assert_eq!(TextField::range_duration(0.0, 16.0), Some(0.100));
        assert_eq!(TextField::range_duration(0.0, 64.0), Some(0.160));
        assert_eq!(TextField::range_duration(0.0, 112.0), Some(0.220));

        fn word_modifier() -> Modifiers {
            if cfg!(target_os = "macos") {
                Modifiers {
                    alt: true,
                    ..Modifiers::default()
                }
            } else {
                Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                }
            }
        }
        fn command(
            f: &mut Fixture,
            key: &str,
            clipboard: &mut crate::platform::clipboard::Clipboard,
        ) {
            let modifiers = if cfg!(target_os = "macos") {
                Modifiers {
                    meta: true,
                    ..Modifiers::default()
                }
            } else {
                Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                }
            };
            let tree = &mut f.tree as *mut WidgetTree;
            let mut cx = EventContext::new(
                f.owner,
                tree,
                (f.now * 1000.0) as u64,
                clipboard,
                Some(&mut f.text),
                None,
                false,
                false,
                KeyboardAuthorityMode::Capture,
            );
            let field = f
                .tree
                .node_mut(f.field)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<TextField>()
                .unwrap();
            assert!(field.handle_event(
                &mut cx,
                &Event::Text(crate::core::event::TextInputEvent {
                    text: key.into(),
                    modifiers,
                }),
                EventPhase::Target
            ));
        }
        fn select_forward(f: &mut Fixture) {
            f.key(KeyCode::Home, Modifiers::default());
            for _ in 0..4 {
                f.key(KeyCode::Right, Modifiers::default());
            }
            for _ in 0..4 {
                f.key(
                    KeyCode::Right,
                    Modifiers {
                        shift: true,
                        ..Modifiers::default()
                    },
                );
            }
        }
        fn assert_join(f: &Fixture, left: &str, right: &str) {
            fn collect(
                nodes: &[SceneNode],
                clip: Option<RectF>,
                text: &mut Vec<(String, f32, RectF)>,
                caret: &mut Option<f32>,
            ) {
                for node in nodes {
                    match node {
                        SceneNode::Clip(n) => collect(&n.children, Some(n.rect), text, caret),
                        SceneNode::RetainedFragment(n) => collect(&n.children, clip, text, caret),
                        SceneNode::Text(n) => {
                            if !n.text.is_empty() {
                                text.push((
                                    n.text.clone(),
                                    n.position.x,
                                    clip.expect("range clip"),
                                ));
                                assert_eq!(n.z_index, TEXT_FIELD_TEXT_LAYER);
                            }
                            if let Some(bg) = n.backgrounds.last() {
                                *caret = Some(n.position.x + bg.x_offset);
                                assert_eq!(n.z_index, TEXT_FIELD_CARET_LAYER);
                            }
                        }
                        _ => {}
                    }
                }
            }
            let (mut runs, mut caret) = (Vec::new(), None);
            collect(
                &f.harness.local_fragment(f.owner).unwrap(),
                None,
                &mut runs,
                &mut caret,
            );
            assert_eq!(runs.len(), 2, "two complementary complete-source draws");
            assert_eq!(runs[0].0, left);
            assert_eq!(runs[1].0, right);
            let edge = caret.expect("one text-native caret");
            assert_eq!(runs[0].2.x + runs[0].2.width, edge);
            assert_eq!(runs[1].2.x, edge, "non-overlapping shared clip edge");
            assert!(
                (runs[1].1 + f.canonical_x() - edge).abs() < 0.001,
                "surviving suffix joins the same physical caret edge"
            );
        }

        let mut f = Fixture::new(true);
        f.field_mut().set_text("abc def ghi");
        f.tree.mark_layout_dirty(f.field);
        f.tick(0.010);
        f.tick(0.011); // fresh static origin after the preceding canonical Layout
        let origin = f.canonical_x();
        f.key(KeyCode::Backspace, word_modifier());
        assert_eq!(f.field().text(), "abc def ", "word edit is immediate");
        f.tick(0.020);
        let first = f.field().smooth_caret.handle.expect("bounded range leg");
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, origin);
        assert_join(&f, "abc def ghi", "abc def ");
        f.tick(0.040);
        assert_eq!(f.field().smooth_caret.handle, Some(first));
        let sampled = f.field().smooth_caret.projection.unwrap().x;
        assert!(sampled < origin && sampled > f.canonical_x());
        f.key_with_repeat(KeyCode::Backspace, word_modifier(), true);
        assert_eq!(
            f.field().text(),
            "abc ",
            "repeat edits without logical backlog"
        );
        f.tick(0.040);
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, sampled);
        assert_ne!(f.field().smooth_caret.handle, Some(first));
        let target = f.canonical_x();
        let duration = TextField::range_duration(sampled, target).unwrap();
        assert_eq!(
            f.field()
                .smooth_caret
                .boundary
                .as_ref()
                .unwrap()
                .old_source
                .as_deref(),
            Some("abc def ghi")
        );
        f.tick(0.090);
        let progress = 0.050 / f64::from(duration);
        let eased = (1.0 - (1.0 - progress).powi(2)) as f32;
        let expected = sampled + (target - sampled) * eased;
        assert!((f.field().smooth_caret.projection.unwrap().x - expected).abs() < 0.001);
        f.tick(0.261); // selected leg cannot outlive the 220 ms upper bound
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());

        f.field_mut().set_text("abc def xyz");
        f.tree.mark_layout_dirty(f.field);
        f.tick(0.300);
        f.tick(0.301);
        select_forward(&mut f);
        f.tick(0.310); // selection is statically reconciled before deletion
        let origin = f.canonical_x();
        f.key(KeyCode::Delete, Modifiers::default());
        assert_eq!(f.field().text(), "abc xyz");
        assert_eq!(
            f.field().state.selection.anchor,
            f.field().state.selection.head
        );
        f.tick(0.320);
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, origin);
        assert_join(&f, "abc def xyz", "abc xyz");
        f.tick(0.370);
        assert_join(&f, "abc def xyz", "abc xyz");
        f.tick(0.541);
        assert!(f.field().smooth_caret.boundary.is_none());

        let mut clipboard = crate::platform::clipboard::Clipboard::new().expect("clipboard facade");
        clipboard.use_memory_transport_for_test("untouched");
        f.field_mut().set_text("abc def xyz");
        f.tree.mark_layout_dirty(f.field);
        f.tick(0.600);
        f.tick(0.601);
        select_forward(&mut f);
        f.tick(0.610);
        command(&mut f, "x", &mut clipboard);
        assert_eq!(
            clipboard.get_text().as_deref(),
            Some("def "),
            "Cut clipboard is immediate"
        );
        assert_eq!(f.field().text(), "abc xyz");
        f.tick(0.620);
        assert_join(&f, "abc def xyz", "abc xyz");
        f.tick(0.841);

        command(&mut f, "v", &mut clipboard);
        assert_eq!(
            f.field().text(),
            "abc def xyz",
            "Paste is immediately canonical"
        );
        assert_eq!(f.field().state.cursor.pos, 8);
        f.tick(0.850);
        assert_join(&f, "abc def xyz", "abc def xyz");
        f.tick(0.900);
        assert_join(&f, "abc def xyz", "abc def xyz");
        f.tick(1.071);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());

        // A backward selection has no lawful right-end origin: preserve the edit, snap.
        f.key(
            KeyCode::Left,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        f.tick(1.080);
        f.key(KeyCode::Backspace, Modifiers::default());
        f.tick(1.090);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
    }

    #[test]
    fn smooth_caret_navigation_retargets_current_value_without_logical_backlog() {
        let mut f = Fixture::new(true);
        let from = f.painted_x();
        f.key(KeyCode::Left, Modifiers::default());
        assert_eq!(f.field().text(), "abc");
        assert_eq!(f.field().state.cursor.pos, 2);
        f.tick(0.010);
        let peer = f.harness.local_fragment(f.peer).unwrap();
        let first = f.field().smooth_caret.handle.expect("real Timeline leg");
        let target = f.canonical_x();
        assert_eq!(f.painted_x(), from, "same-frame exact from");
        f.tick(0.030);
        let middle = from + (target - from) * 0.75;
        assert!((f.painted_x() - middle).abs() < 0.001);
        assert!(Arc::ptr_eq(
            &peer,
            &f.harness.local_fragment(f.peer).unwrap()
        ));
        f.key(KeyCode::Left, Modifiers::default());
        f.key(KeyCode::Left, Modifiers::default());
        assert_eq!(f.field().text(), "abc");
        assert_eq!(f.field().state.cursor.pos, 0);
        f.tick(0.030);
        let second = f.field().smooth_caret.handle.unwrap();
        assert_ne!(first, second);
        assert!(
            (f.painted_x() - middle).abs() < 0.001,
            "retarget from current sample"
        );
        let peer = f.harness.local_fragment(f.peer).unwrap();
        f.tick(0.069);
        assert!(f.field().smooth_caret.handle.is_some());
        assert_ne!(f.painted_x(), f.canonical_x());
        // Timeline's public duration is f32, promoted to its f64 frame clock.
        f.tick((0.030 + f64::from(0.040_f32)).next_up());
        assert_eq!(f.painted_x(), f.canonical_x());
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.pending.is_none());
        assert!(Arc::ptr_eq(
            &peer,
            &f.harness.local_fragment(f.peer).unwrap()
        ));
    }

    #[test]
    fn smooth_caret_disabled_and_rejected_input_preserve_existing_behavior() {
        let mut static_field = Fixture::new(false);
        static_field.key(KeyCode::Home, Modifiers::default());
        static_field.tick(0.010);
        assert_eq!(static_field.painted_x(), static_field.canonical_x());
        assert!(static_field.field().smooth_caret.handle.is_none());
        assert!(static_field.field().smooth_caret.projection.is_none());

        for (navigation, no_op_navigation, no_op_delete) in [
            (KeyCode::Home, KeyCode::Left, KeyCode::Backspace),
            (KeyCode::End, KeyCode::Right, KeyCode::Delete),
        ] {
            let mut f = Fixture::new(true);
            if navigation == KeyCode::End {
                f.key(KeyCode::Home, Modifiers::default());
                f.tick(0.005);
            }
            f.tick(0.075);
            let changes = std::rc::Rc::new(std::cell::Cell::new(0));
            let changes_for_callback = changes.clone();
            f.field_mut()
                .set_on_change(move |_| changes_for_callback.set(changes_for_callback.get() + 1));
            f.key(navigation, Modifiers::default());
            f.tick(0.080);
            let handle = f.field().smooth_caret.handle.unwrap();
            let version = f.field().state.document.version();
            f.field_mut().set_input_filter(|c| c != 'x');
            f.field_mut().config.max_len_bytes = Some(3);
            f.insert("x"); // Rejected by the filter.
            f.insert(""); // Accepted empty insertion is still a semantic no-op.
            f.key(KeyCode::Space, Modifiers::default()); // Rejected by max length.
            f.key(no_op_navigation, Modifiers::default());
            f.key(no_op_delete, Modifiers::default());
            assert_eq!(f.field().state.document.version(), version);
            assert_eq!(changes.get(), 0);
            f.tick(0.090);
            assert_eq!(f.field().smooth_caret.handle, Some(handle));
            assert_eq!(f.field().text(), "abc");
            f.tick(0.119);
            assert_eq!(f.field().smooth_caret.handle, Some(handle));
            assert_ne!(f.painted_x(), f.canonical_x());
            f.tick((0.080 + f64::from(0.040_f32)).next_up());
            assert_eq!(f.painted_x(), f.canonical_x());
            assert!(f.field().smooth_caret.handle.is_none());
        }
    }

    #[test]
    fn smooth_caret_verified_key_repeats_snap_with_immediate_editing() {
        for (key, expected_text, expected_pos) in [
            (KeyCode::Space, "a bc", 2),
            (KeyCode::Left, "abc", 0),
            (KeyCode::Right, "abc", 2),
            (KeyCode::Home, "abc", 0),
            (KeyCode::End, "abc", 3),
            (KeyCode::Backspace, "bc", 0),
            (KeyCode::Delete, "ac", 1),
        ] {
            for enabled in [false, true] {
                let mut f = Fixture::new(enabled);
                f.key(KeyCode::Home, Modifiers::default());
                f.tick(0.005);
                f.tick(0.075);
                f.key(KeyCode::Right, Modifiers::default());
                f.tick(0.080);
                f.tick(0.090);
                let old = f.field().smooth_caret.handle;
                assert_eq!(old.is_some(), enabled);

                f.key_with_repeat(key, Modifiers::default(), true);
                assert_eq!(f.field().text(), expected_text);
                assert_eq!(f.field().state.cursor.pos, expected_pos);
                assert_eq!(f.field().smooth_caret.snap, enabled);
                assert_eq!(f.field().smooth_caret.handle, old, "cancel only in Update");
                assert!(f.field().smooth_caret.pending.is_none());

                f.tick(0.100); // Before the old leg's 0.145 completion deadline.
                assert_eq!(f.painted_x(), f.canonical_x());
                assert!(f.field().smooth_caret.handle.is_none());
                assert!(f.field().smooth_caret.pending.is_none());
                if let Some(old) = old {
                    assert!(matches!(
                        f.harness.animation_context().sample::<f32>(old),
                        AnimationSample::Cancelled
                    ));
                } else {
                    assert!(f.field().smooth_caret.projection.is_none());
                }
                f.tick(0.110);
                assert_eq!(f.painted_x(), f.canonical_x());
                assert!(f.field().smooth_caret.handle.is_none());
            }
        }
    }

    #[test]
    fn smooth_caret_equal_and_rejected_repeats_cancel_the_old_leg() {
        for key in [KeyCode::Right, KeyCode::End, KeyCode::Space] {
            let mut f = Fixture::new(true);
            f.key(KeyCode::Home, Modifiers::default());
            f.tick(0.005);
            f.tick(0.075);
            f.key(KeyCode::End, Modifiers::default());
            f.tick(0.080);
            let old = f.field().smooth_caret.handle.unwrap();
            f.field_mut().config.max_len_bytes = Some(3);
            f.key_with_repeat(key, Modifiers::default(), true);
            assert_eq!(f.field().text(), "abc");
            assert_eq!(f.field().state.cursor.pos, 3);
            assert_eq!(f.field().smooth_caret.handle, Some(old));
            f.tick(0.090);
            assert_eq!(f.painted_x(), f.canonical_x());
            assert!(f.field().smooth_caret.handle.is_none());
            assert!(matches!(
                f.harness.animation_context().sample::<f32>(old),
                AnimationSample::Cancelled
            ));
        }

        for key in [KeyCode::Backspace, KeyCode::Delete, KeyCode::Space] {
            let mut f = Fixture::new(true);
            f.field_mut().apply_availability_policy(true, true);
            f.tick(0.010);
            f.key(KeyCode::Left, Modifiers::default());
            f.tick(0.020);
            let old = f.field().smooth_caret.handle.unwrap();
            f.key_with_repeat(key, Modifiers::default(), true);
            assert_eq!(f.field().text(), "abc");
            assert_eq!(f.field().state.cursor.pos, 2);
            assert_eq!(f.field().smooth_caret.handle, Some(old));
            f.tick(0.030);
            assert_eq!(f.painted_x(), f.canonical_x());
            assert!(f.field().smooth_caret.handle.is_none());
            assert!(matches!(
                f.harness.animation_context().sample::<f32>(old),
                AnimationSample::Cancelled
            ));
        }
    }

    #[test]
    fn smooth_caret_content_edit_batch_stays_static_until_a_new_navigation() {
        let mut f = Fixture::new(true);
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.010);
        let old = f.field().smooth_caret.handle.unwrap();
        f.insert("e");
        f.key(KeyCode::Home, Modifiers::default());
        assert_eq!(f.field().text(), "abec");
        assert_eq!(f.field().state.cursor.pos, 0);
        f.tick(0.020);
        assert_eq!(f.painted_x(), f.canonical_x());
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(matches!(
            f.harness.animation_context().sample::<f32>(old),
            AnimationSample::Cancelled
        ));

        f.key(KeyCode::Right, Modifiers::default());
        f.tick(0.030);
        assert!(f.field().smooth_caret.handle.is_some());
        f.key(KeyCode::Delete, Modifiers::default());
        f.key(KeyCode::End, Modifiers::default());
        assert_eq!(f.field().text(), "aec");
        assert_eq!(f.field().state.cursor.pos, 3);
        f.tick(0.040);
        assert_eq!(f.painted_x(), f.canonical_x());
        assert!(f.field().smooth_caret.handle.is_none());

        fn presentation(f: &Fixture) -> (String, f32, f32, f32) {
            fn collect(
                nodes: &[SceneNode],
                clip: Option<RectF>,
                text: &mut Option<(String, f32, f32)>,
                caret: &mut Option<f32>,
            ) {
                for node in nodes {
                    match node {
                        SceneNode::Clip(n) => collect(&n.children, Some(n.rect), text, caret),
                        SceneNode::RetainedFragment(n) => collect(&n.children, clip, text, caret),
                        SceneNode::Text(n) => {
                            if !n.text.is_empty() {
                                let clip = clip.expect("actual whole-run viewport/clip");
                                assert!(text.is_none(), "one complete current source draw");
                                *text = Some((n.text.clone(), n.position.x, clip.right()));
                            }
                            if let Some(background) = n.backgrounds.last() {
                                *caret = Some(n.position.x + background.x_offset);
                            }
                        }
                        _ => {}
                    }
                }
            }
            let (mut text, mut caret) = (None, None);
            collect(
                &f.harness.local_fragment(f.owner).unwrap(),
                None,
                &mut text,
                &mut caret,
            );
            let (source, origin, edge) = text.expect("actual complete source");
            (
                source,
                origin,
                edge,
                caret.expect("actual text-native caret"),
            )
        }

        // Qualified end edits are the P6 exception to the P4 repeat snap.
        let repeat_from = f.painted_x();
        let version = f.field().state.document.version();
        f.key_with_repeat(KeyCode::Space, Modifiers::default(), true);
        assert_eq!(f.field().text(), "aec ", "logical insertion is immediate");
        assert_eq!(f.field().state.cursor.pos, 4);
        assert_eq!(f.field().state.document.version(), version + 1);
        f.tick(0.050);
        let reveal = f.field().smooth_caret.handle.expect("one P6 Timeline leg");
        assert_eq!(
            f.field().smooth_caret.boundary.as_ref().unwrap().kind,
            CaretBoundaryKind::Reveal
        );
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, repeat_from);
        let (source, origin, edge, caret) = presentation(&f);
        assert_eq!(source, "aec ");
        assert_eq!(edge, caret, "shared physical reveal/caret edge");
        let scale = f
            .field()
            .smooth_caret
            .projection
            .unwrap()
            .space
            .geometry
            .scale_factor;
        assert_eq!(edge, snap_to_physical(origin + repeat_from, scale));
        f.route(FusedInputEvent::KeyUp {
            window_id: WindowId::PRIMARY,
            time: f.time,
            key_code: KeyCode::Space,
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
        });
        assert_eq!(
            f.field().smooth_caret.handle,
            Some(reveal),
            "KeyUp creates no new leg"
        );
        let mut previous = repeat_from;
        for now in [0.060, 0.070] {
            f.tick(now);
            assert_eq!(f.field().smooth_caret.handle, Some(reveal));
            let projection = f.field().smooth_caret.projection.unwrap();
            assert!(projection.x > previous && projection.x < f.canonical_x());
            assert!(matches!(
                f.harness.animation_context().sample::<f32>(reveal),
                AnimationSample::Active(value) if value == projection.x
            ));
            let (source, origin, edge, caret) = presentation(&f);
            assert_eq!(source, "aec ");
            assert_eq!(edge, caret);
            assert_eq!(edge, snap_to_physical(origin + projection.x, scale));
            previous = projection.x;
        }
        let deadline = (0.050 + f64::from(0.040_f32)).next_up();
        f.tick(deadline - 0.001);
        assert_eq!(f.field().smooth_caret.handle, Some(reveal));
        f.tick(deadline);
        assert_eq!(f.painted_x(), f.canonical_x());
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.boundary.is_none());
        assert_eq!(presentation(&f).0, "aec ");
        assert_eq!(f.field().state.document.version(), version + 1);

        let from = f.painted_x();
        f.key(KeyCode::Left, Modifiers::default());
        assert_eq!(f.field().state.cursor.pos, 3);
        f.tick(0.120);
        let next = f.field().smooth_caret.handle.unwrap();
        assert_ne!(old, next);
        assert_ne!(reveal, next);
        assert_eq!(f.painted_x(), from);
        f.tick((0.120 + f64::from(0.040_f32)).next_up());
        assert_eq!(f.painted_x(), f.canonical_x());
        assert!(f.field().smooth_caret.handle.is_none());
    }

    #[test]
    fn smooth_caret_geometry_change_snaps_first_paint_and_retires_next_update() {
        let mut f = Fixture::new(true);
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.010);
        assert!(f.field().smooth_caret.handle.is_some());
        f.width = 280.0; // Canonical Layout discovers this after ordinary Update.
        f.tick(0.030);
        assert_eq!(f.painted_x(), f.canonical_x());
        f.tick(0.035);
        assert!(f.field().smooth_caret.handle.is_none());
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.050);
        assert!(
            f.field().smooth_caret.handle.is_some(),
            "new qualified origin can animate"
        );
    }

    #[test]
    fn smooth_caret_selection_ime_configuration_and_capture_boundaries_cancel() {
        let mut f = Fixture::new(true);
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.010);
        f.key(
            KeyCode::Left,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        f.insert("z"); // Selection boundary dominates later accepted input in the batch.
        f.tick(0.020);
        assert!(f.field().smooth_caret.handle.is_none());
        assert_eq!(f.painted_x(), f.canonical_x());

        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.030);
        assert!(f.field().smooth_caret.handle.is_some());
        // IME is delivered through its existing target EventContext, with real Capture held.
        let tree_ptr = &mut f.tree as *mut WidgetTree;
        let mut cx = EventContext::new(
            f.owner,
            tree_ptr,
            30,
            std::ptr::null_mut(),
            Some(&mut f.text),
            Some(f.harness.animation_context()),
            false,
            false,
            KeyboardAuthorityMode::Capture,
        );
        assert!(f.tree.node_mut(f.field).unwrap().widget.handle_event(
            &mut cx,
            &Event::Ime(ImeEvent::PreEdit {
                text: "候".into(),
                cursor: None
            }),
            EventPhase::Target,
        ));
        drop(cx);
        f.tick(0.040);
        assert!(f.field().smooth_caret.handle.is_none());
        f.field_mut().state.clear_composition();
        f.tick(0.050);
        f.key(KeyCode::Right, Modifiers::default());
        f.tick(0.060);
        assert!(f.field().smooth_caret.handle.is_some());
        f.field_mut().set_smooth_caret_prototype(false);
        f.tick(0.070);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.projection.is_none());

        f.field_mut().set_smooth_caret_prototype(true);
        f.tick(0.080);
        assert!(
            f.field().smooth_caret.handle.is_none(),
            "re-enable never starts motion"
        );
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.090);
        assert!(f.field().smooth_caret.handle.is_some());
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            ActionId::RELEASE_CAPTURE,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            None,
        ));
        f.tick(0.100);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(!f.tree.node(f.field).unwrap().widget.needs_animation());
    }
    #[test]
    fn smooth_caret_read_only_denied_commands_still_snap_without_editing() {
        let mut f = Fixture::new(true);
        f.field_mut().apply_availability_policy(true, true);
        f.tick(0.010);
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.020);
        assert!(f.field().smooth_caret.handle.is_some());
        f.route(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: f.time,
            text: "v".into(),
            modifiers: Modifiers {
                ctrl: true,
                meta: true,
                ..Modifiers::default()
            },
        });
        f.tick(0.030);
        assert!(f.field().smooth_caret.handle.is_none());
        assert_eq!(f.field().text(), "abc");
    }

    #[test]
    fn smooth_caret_unicode_and_empty_text_keep_immediate_cluster_editing() {
        let mut f = Fixture::new(true);
        let short = f.text.shape("abcé", 14.0);
        let long = f.text.shape("abcéa", 14.0);
        assert_ne!(short.glyphs[3].advance, long.glyphs[3].advance);
        assert!(
            edit_projection::Projection::current_prefix_matches(
                &short, &long, f.text.shaper(), 1.0,
            ),
            "same bound prefix ink despite kerning"
        );
        for invalid in [
            {
                let mut r = long.clone();
                r.glyphs[3].advance += 1.0;
                r
            },
            {
                let mut r = long.clone();
                r.glyphs[3].x += 1.0;
                r
            },
            {
                let mut r = long.clone();
                r.glyphs[3].id += 1;
                r
            },
            {
                let mut r = long.clone();
                r.binding = None;
                r
            },
        ] {
            assert!(
                !edit_projection::Projection::current_prefix_matches(
                    &short, &invalid, f.text.shaper(), 1.0,
                ),
                "both complete shapes stay verified"
            );
        }
        for (text, expected, cursor, now) in [
            ("é", "abcé", 5, 0.010),
            ("a", "abcéa", 6, 0.020),
            ("a", "abcéaa", 7, 0.030),
        ] {
            f.insert(text);
            assert_eq!(f.field().text(), expected);
            assert_eq!(f.field().state.cursor.pos, cursor);
            f.tick(now);
            assert_eq!(f.field().smooth_caret.projection_refusal, None);
            assert!(
                f.field().smooth_caret.handle.is_some(),
                "HAL101 §13: complete supported Unicode end edits use P6"
            );
        }
        f.key(KeyCode::Backspace, Modifiers::default());
        assert_eq!(f.field().text(), "abcéa");
        assert_eq!(f.field().state.cursor.pos, 6);
        f.tick(0.040);
        assert_eq!(f.painted_x(), f.canonical_x());
        assert!(f.field().smooth_caret.handle.is_none());
        f.key_with_repeat(KeyCode::Backspace, Modifiers::default(), true);
        assert_eq!(f.field().text(), "abcé");
        assert_eq!(f.field().state.cursor.pos, 5);
        f.tick(0.050);
        assert!(f.field().smooth_caret.handle.is_some());
        let previous = f.field().smooth_caret.boundary.as_ref().unwrap();
        let source = previous.old_source.as_deref().unwrap().to_owned();
        let prior_handle = f.field().smooth_caret.handle.unwrap();
        assert_eq!(source, "abcéa");
        let origin = f.text.shape(&source, 14.0);
        let before_source = f.field().text().to_owned();
        let before = f.text.shape(&before_source, 14.0);
        f.key_with_repeat(KeyCode::Backspace, Modifiers::default(), true);
        assert_eq!(f.field().text(), "abc");
        assert_eq!(f.field().state.cursor.pos, 3);
        let current = f.text.shape("abc", 14.0);
        let prior = edit_projection::Projection::build(
            &origin,
            &before,
            before.text.len()..source.len(),
            before.text.len()..before.text.len(),
            f.text.shaper(),
            1.0,
        )
        .unwrap();
        let next = edit_projection::Projection::build(
            &origin,
            &current,
            current.text.len()..source.len(),
            current.text.len()..current.text.len(),
            f.text.shaper(),
            1.0,
        )
        .unwrap();
        let ratio = (before.width - origin.width) / (current.width - origin.width);
        assert!(edit_projection::Projection::current_prefix_matches(
            &before,
            &current,
            f.text.shaper(),
            1.0,
        ));
        assert!(
            !next.retarget_geometry_matches(&prior, ratio),
            "the prior old/current contextual split has no single-phase continuation"
        );
        f.tick(0.060);
        assert_eq!(
            f.field().smooth_caret.projection_refusal,
            Some(edit_projection::Refusal::RetargetGeometry)
        );
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(matches!(
            f.harness.animation_context().sample::<f32>(prior_handle),
            AnimationSample::Cancelled
        ));
        assert_eq!(f.painted_x(), f.canonical_x(), "qualified canonical snap");
        for _ in 0..3 {
            f.key(KeyCode::Backspace, Modifiers::default());
        }
        assert_eq!(f.field().text(), "");
        assert_eq!(f.field().state.cursor.pos, 0);
        f.tick(0.070);
        assert!(f.field().smooth_caret.handle.is_some());
        f.tick((0.070 + f64::from(0.040_f32)).next_up());
        assert_eq!(f.painted_x(), 0.0);
        assert!(f.field().smooth_caret.handle.is_none());
    }

    #[test]
    fn password_mode_discards_plaintext_projection_and_uses_masked_text_caret() {
        fn check(nodes: &[SceneNode], masks: &mut usize, caret: &mut Option<f32>) {
            for node in nodes {
                match node {
                    SceneNode::Text(text) => {
                        assert!(
                            text.text.is_empty() || text.text == PASSWORD_MASK_TEXT,
                            "SecureEntry scene must contain only masks and empty caret runs"
                        );
                        if text.text == PASSWORD_MASK_TEXT {
                            *masks += 1;
                        }
                        if text.z_index == TEXT_FIELD_CARET_LAYER {
                            assert!(text.text.is_empty());
                            assert_eq!(text.backgrounds.len(), 1);
                            *caret = Some(text.position.x + text.backgrounds[0].x_offset);
                        }
                    }
                    SceneNode::Clip(clip) => check(&clip.children, masks, caret),
                    SceneNode::RetainedFragment(fragment) => {
                        check(&fragment.children, masks, caret)
                    }
                    SceneNode::Layer(layer) => check(&layer.children, masks, caret),
                    _ => {}
                }
            }
        }
        let mut f = Fixture::new(true);
        f.key(KeyCode::Backspace, Modifiers::default());
        f.tick(0.010);
        assert_eq!(
            f.field()
                .smooth_caret
                .boundary
                .as_ref()
                .unwrap()
                .old_source
                .as_deref(),
            Some("abc")
        );
        let handle = f.field().smooth_caret.handle;
        assert!(handle.is_some());
        f.field_mut().set_password(true);
        assert!(
            f.field().smooth_caret.boundary.is_none(),
            "discard owned visual text before Update"
        );
        assert!(f.field().smooth_caret.projection.is_none());
        assert!(f.field().smooth_caret.range_origin.is_none());
        assert_eq!(
            f.field().smooth_caret.handle,
            handle,
            "Update retains cancellation ownership"
        );
        f.tree.mark_layout_dirty(f.field);
        f.tick(0.020);
        assert_eq!(f.field().text(), "ab");
        assert_eq!(f.field().state.cursor.pos, 2);
        assert!(f.field().smooth_caret.handle.is_none());
        let mut masks = 0;
        let mut caret = None;
        check(
            &f.harness.local_fragment(f.owner).unwrap(),
            &mut masks,
            &mut caret,
        );
        assert_eq!(masks, 2);
        let rect = f.tree.get_absolute_paint_rect(f.field).unwrap();
        let expected = snap_to_physical(
            rect.x
                + f.theme.geometry.text_field.padding_x
                + f.field()
                    .caret_x_from_boundaries(f.field().state.cursor.pos)
                - f.field().state.scroll_x,
            1.0,
        );
        assert_eq!(caret, Some(expected));
        f.insert("é🙂");
        f.tick(0.030);
        assert_eq!(f.field().text(), "abé🙂");
        assert!(f.field().smooth_caret.boundary.is_none());
        assert!(f.field().smooth_caret.handle.is_none());
        check(
            &f.harness.local_fragment(f.owner).unwrap(),
            &mut 0,
            &mut None,
        );
    }

    #[test]
    fn password_mask_motion_keeps_scalar_source_and_shared_paint_boundary() {
        fn check(f: &Fixture, moving: bool) {
            fn walk(
                nodes: &[SceneNode],
                clip: Option<RectF>,
                edge: &mut Option<f32>,
                caret: &mut Option<f32>,
            ) {
                for node in nodes {
                    match node {
                        SceneNode::Text(text) => {
                            assert!(
                                text.text.is_empty() || text.text == PASSWORD_MASK_TEXT,
                                "password scene contains only the constant mask and empty caret run"
                            );
                            if text.text == PASSWORD_MASK_TEXT {
                                let clip = clip.unwrap();
                                *edge = Some(clip.right());
                            }
                            if text.z_index == TEXT_FIELD_CARET_LAYER {
                                *caret = text
                                    .backgrounds
                                    .last()
                                    .map(|b| text.position.x + b.x_offset);
                            }
                        }
                        SceneNode::Clip(n) => walk(&n.children, Some(n.rect), edge, caret),
                        SceneNode::RetainedFragment(n) => walk(&n.children, clip, edge, caret),
                        SceneNode::Layer(n) => walk(&n.children, clip, edge, caret),
                        _ => {}
                    }
                }
            }
            assert!(
                f.field().smooth_caret.boundary.is_none(),
                "no ordinary old-source channel"
            );
            assert!(f.field().smooth_caret.range_origin.is_none());
            let (mut edge, mut caret) = (None, None);
            walk(
                &f.harness.local_fragment(f.owner).unwrap(),
                None,
                &mut edge,
                &mut caret,
            );
            assert!(caret.is_some());
            if moving {
                assert_eq!(edge, caret, "mask ink and caret share the actual clip edge");
            }
        }
        let mut f = Fixture::with_field(TextField::new().with_text("qxv").with_password(true));
        // Capture's first Layout qualifies the font; the next ordinary Update
        // establishes its static projection before any eligible operation.
        f.tick(0.002);
        assert!(f.field().password_mask_font_id.is_some());
        assert!(f.field().smooth_caret.projection.is_some());
        let origin = f.canonical_x();
        let time = f.time;
        f.route(FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time,
            key_code: KeyCode::Character('a'),
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        });
        f.route(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time,
            text: "a".into(),
            modifiers: Modifiers::default(),
        });
        assert_eq!(f.field().text(), "qxva");
        f.tick(0.010);
        let boundary = f.field().smooth_caret.mask_boundary.unwrap();
        assert_eq!((boundary.old_count, boundary.new_count), (3, 4));
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, origin);
        check(&f, true);
        f.tick(0.040);
        let x = f.field().smooth_caret.projection.unwrap().x;
        assert!(x > origin && x < f.canonical_x());
        check(&f, true);
        let deadline = (0.010 + f64::from(0.040_f32)).next_up();
        let reveal = f.field().smooth_caret.handle;
        assert!(reveal.is_some());
        f.tick(deadline - 0.001);
        assert_eq!(f.field().smooth_caret.handle, reveal);
        assert!(f.field().smooth_caret.mask_boundary.is_some());
        check(&f, true);
        f.tick(deadline);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.mask_boundary.is_none());
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, f.canonical_x());
        check(&f, false);

        f.key(KeyCode::Backspace, Modifiers::default());
        assert_eq!(f.field().text(), "qxv");
        f.tick(0.080);
        let first = f.field().smooth_caret.handle.unwrap();
        f.tick(0.100);
        check(&f, true);
        f.key_with_repeat(KeyCode::Backspace, Modifiers::default(), true);
        f.key_with_repeat(KeyCode::Backspace, Modifiers::default(), true);
        assert_eq!(f.field().text(), "q");
        f.tick(0.110);
        let boundary = f.field().smooth_caret.mask_boundary.unwrap();
        assert_eq!((boundary.old_count, boundary.new_count), (4, 1));
        assert_ne!(f.field().smooth_caret.handle.unwrap(), first);
        check(&f, true);
        let deadline = (0.110 + f64::from(0.040_f32)).next_up();
        let conceal = f.field().smooth_caret.handle;
        f.tick(deadline - 0.001);
        assert_eq!(f.field().smooth_caret.handle, conceal);
        assert!(f.field().smooth_caret.mask_boundary.is_some());
        check(&f, true);
        f.tick(deadline);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.mask_boundary.is_none());
        assert_eq!(f.field().smooth_caret.projection.unwrap().x, f.canonical_x());

        f.key(KeyCode::Home, Modifiers::default());
        assert_eq!(f.field().state.cursor.pos, 0);
        f.tick(0.200);
        assert!(f.field().smooth_caret.handle.is_some());
        f.tick(0.2325);
        let x = f.field().smooth_caret.projection.unwrap().x;
        assert!(x > 0.0 && x < origin / 3.0);
        check(&f, false);
        f.tick(0.266);
        assert!(f.field().smooth_caret.handle.is_none());

        f.key(KeyCode::End, Modifiers::default());
        f.tick(0.280);
        assert!(f.field().smooth_caret.handle.is_some());
        f.insert("b");
        assert_eq!(f.field().text(), "qb");
        f.tick(0.281);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.mask_boundary.is_none());
        check(&f, false);
        f.key(KeyCode::Home, Modifiers::default());
        f.tick(0.282);
        assert!(f.field().smooth_caret.handle.is_some());
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            ActionId::RELEASE_CAPTURE,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            None
        ));
        f.tick(0.290);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.mask_boundary.is_none());
        assert!(f.field().smooth_caret.projection.is_none());

        let mut plain = Fixture::with_field(
            TextField::new()
                .with_text("qxv")
                .with_password(true)
                .with_smooth_caret_prototype(false),
        );
        plain.insert("a");
        plain.tick(0.010);
        assert!(plain.field().smooth_caret.handle.is_none());
        assert!(plain.field().smooth_caret.mask_boundary.is_none());
        check(&plain, false);

        let mut ambiguous =
            Fixture::with_field(TextField::new().with_text("qxv").with_password(true));
        ambiguous.insert("ab");
        ambiguous.tick(0.010);
        assert_eq!(ambiguous.field().text(), "qxvab");
        assert!(ambiguous.field().smooth_caret.handle.is_none());
        assert!(ambiguous.field().smooth_caret.mask_boundary.is_none());
        check(&ambiguous, false);

        let mut last = Fixture::with_field(TextField::new().with_text("q").with_password(true));
        last.tick(0.002);
        last.key(KeyCode::Backspace, Modifiers::default());
        assert!(last.field().text().is_empty());
        last.tick(0.010);
        let boundary = last.field().smooth_caret.mask_boundary.unwrap();
        assert_eq!((boundary.old_count, boundary.new_count), (1, 0));
        check(&last, true);
        last.tick(0.071);
        assert!(last.field().smooth_caret.handle.is_none());
        last.insert("a");
        last.tick(0.080);
        let boundary = last.field().smooth_caret.mask_boundary.unwrap();
        assert_eq!((boundary.old_count, boundary.new_count), (0, 1));
        check(&last, true);
    }

    #[test]
    fn smooth_caret_scroll_and_password_snap_without_changing_logical_values() {
        let mut f = Fixture::new(true);
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.010);
        let before = (f.field().text().to_owned(), f.field().state.cursor.pos);
        f.width = 25.0;
        f.tick(0.020);
        assert!(f.field().state.scroll_x > 0.0);
        assert_eq!(
            (f.field().text().to_owned(), f.field().state.cursor.pos),
            before
        );
        f.tick(0.030);
        assert!(f.field().smooth_caret.handle.is_none());

        let mut f = Fixture::new(true);
        f.key(KeyCode::Left, Modifiers::default());
        f.tick(0.010);
        f.field_mut().set_password(true);
        f.tree.mark_layout_dirty(f.field);
        f.tick(0.020);
        assert!(f.field().smooth_caret.handle.is_none());
        assert!(f.field().smooth_caret.mask_boundary.is_none());
        assert_eq!(f.field().text(), "abc");
        assert_eq!(f.field().state.cursor.pos, 2);
    }
}
