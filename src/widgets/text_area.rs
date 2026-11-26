// SPEC-HAL-108: TextArea Widget — Multi-line plain text editor
//
// Reuses the editing substrate (editing.rs, editing_history.rs) and paragraph layout engine.
// Key differences from TextField:
// - Multi-line text with soft-wrap
// - Vertical scrolling via ScrollContainer
// - Per-line paint iteration
// - Multi-line selection painting
// - Up/Down arrow key navigation
// - Enter inserts newline
//
// ============================================================================
// VIEWPORT & CARET VISIBILITY INVARIANTS (SPEC-HAL-109 Stage G)
// ============================================================================
//
// INV-G.VIS1 — Caret Y visibility:
//   After any cursor movement or edit, caret must be fully visible:
//   caret_y ∈ [scroll_y, scroll_y + viewport_height - caret_height]
//
// INV-G.VIS2 — Scroll idempotence:
//   Repeated ensure_focus_visible() calls with unchanged focus target
//   must not change scroll_y (stable equilibrium).
//
// INV-G.VIS3 — Scroll monotonicity:
//   Moving caret down may only increase scroll_y (or keep unchanged).
//   Moving caret up may only decrease scroll_y (or keep unchanged).
//
// INV-G.VIS4 — No overscroll:
//   scroll_y ∈ [0, max(0, content_height - viewport_height)]
//
// ============================================================================
// SELECTION VISIBILITY INVARIANTS (SPEC-HAL-109 Stage G.2)
// ============================================================================
//
// INV-G.SEL1 — Active edge visibility:
//   After any selection-changing action, selection.head must be visible.
//   (The "active edge" is always the head, never the anchor.)
//
// INV-G.SEL2 — Anchor neutrality:
//   selection.anchor does NOT affect scrolling. Only head matters.
//
// INV-G.SEL3 — Stability:
//   Repeated ensure with unchanged selection.head must not change scroll.
//
// INV-G.SEL4 — No oscillation:
//   Dragging selection across viewport boundary must not cause scroll jitter.
//
// ============================================================================
// UNIFIED FOCUS TARGET (SPEC-HAL-109 Stage G.3)
// ============================================================================
//
// INV-G.FOCUS1 — Single focus target:
//   At any time, exactly one vertical focus target exists.
//   It is selection.head if selection active, otherwise caret (cursor.pos).
//   All visibility logic consults focus_target_content_y() exclusively.
//
// ============================================================================

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::appearance::FrameworkAppearanceFacts;
use crate::core::editing::{
    Selection,
    TextCursor,
    WordBoundary,
    // Ctrl variants (paragraph/block movement)
    ctrl_move_down,
    ctrl_move_down_select,
    ctrl_move_up,
    ctrl_move_up_select,
    find_word_at,
    hit_test,
    // Vertical navigation (TextArea-specific)
    move_down,
    move_down_select,
    move_up,
    move_up_select,
};
use crate::core::editing_history::{EditKind, EditSnapshot, UndoHistory};
use crate::core::event::{
    Event, EventPhase, ImeEvent, KeyCode, KeyEvent, KeyboardAuthorityMode, PointerButton,
    PointerEventKind,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{LAYER_CONTENT, LAYER_SCROLLBAR, RoundedCorners, TextBackgroundSpan};
use crate::core::scroll_container::ScrollContainer;
use crate::core::scroll_state::{AnchorId, AnchorResolver, ScrollAnchor};
use crate::core::text_edit_delegate as delegate;
use crate::core::text_edit_policy::{
    PolicyDecision, TextEditPolicy, TextEditingPolicy, decide_text_edit_policy,
};
use crate::core::theme::{ControlChromeCategory, ControlChromeState};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::scrollbar::{ScrollbarH, ScrollbarV};
use crate::widgets::text_document::TextDocument;
use crate::widgets::text_layout_cache::TextLayoutCache;
use std::cell::{Cell, RefCell};

// ============================================================================
// TextAreaConfig
// ============================================================================

/// Configuration options for TextArea behavior.
pub struct TextAreaConfig {
    pub placeholder: Option<String>,
    pub is_read_only: bool,
    pub is_enabled: bool,
    /// Minimum height (initial height before content grows)
    pub min_height: f32,
    /// Maximum height before scrolling (None = unlimited growth)
    pub max_height: Option<f32>,
    /// Fixed height (overrides min/max, always scroll when content exceeds)
    pub fixed_height: Option<f32>,
}

impl Default for TextAreaConfig {
    fn default() -> Self {
        Self {
            placeholder: None,
            is_read_only: false,
            is_enabled: true,
            min_height: 100.0, // Reasonable default initial height
            max_height: None,  // No limit by default (auto-grow)
            fixed_height: None,
        }
    }
}

// ============================================================================
// TextAreaDragMode
// ============================================================================

/// Drag mode for pointer-based selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAreaDragMode {
    None,
    Character { anchor: usize },
    Word { initial: WordBoundary },
}

impl Default for TextAreaDragMode {
    fn default() -> Self {
        Self::None
    }
}

// ============================================================================
// TextAreaState
// ============================================================================

/// Unified editing state for TextArea widget.
pub struct TextAreaState {
    pub document: TextDocument,
    pub cursor: TextCursor,
    pub selection: Selection,
    pub layout_cache: TextLayoutCache,
    pub scroll: RefCell<ScrollContainer>,
    pub drag_mode: TextAreaDragMode,
    pub undo_history: UndoHistory,
    pub ime_pre_composition_snapshot: Option<EditSnapshot>,
}

impl TextAreaState {
    pub fn new() -> Self {
        Self {
            document: TextDocument::new(),
            cursor: TextCursor::new(),
            selection: Selection::new(),
            layout_cache: TextLayoutCache::default(),
            scroll: RefCell::new(ScrollContainer::new()),
            drag_mode: TextAreaDragMode::None,
            undo_history: UndoHistory::new(),
            ime_pre_composition_snapshot: None,
        }
    }

    pub fn is_composing(&self) -> bool {
        self.ime_pre_composition_snapshot.is_some()
    }

    pub fn clear_composition(&mut self) {
        self.ime_pre_composition_snapshot = None;
    }
}

impl Default for TextAreaState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TextArea
// ============================================================================

/// Multi-line editable text widget.
///
/// Uses the same editing substrate as TextField but supports:
/// - Multiple lines with soft-wrap
/// - Vertical scrolling
/// - Up/Down arrow navigation
/// - Enter key for newlines
pub struct TextArea {
    id: WidgetInternalId,
    state: TextAreaState,
    focused: bool,
    caret_animation_demand: Option<bool>,
    hover: bool,
    /// SPEC-HAL-159: Visual-only blink state. Mutated during paint with AllowPaintMutationGuard.
    blink_last_ms: Cell<u64>,
    /// SPEC-HAL-159: Visual-only blink state. Mutated during paint with AllowPaintMutationGuard.
    blink_caret_visible: Cell<bool>,
    /// Accumulated time for dt-based blink update in update() phase.
    /// Reset on focus acquisition and user activity.
    blink_accumulated_secs: Cell<f32>,
    config: TextAreaConfig,
    // SPEC-HAL-109 Stage F: Single-line vs multi-line editing policy
    editing_policy: TextEditingPolicy,
    layout_dirty: Cell<bool>,
    // K.INV1: Deferred focus visibility update (set after mutation, cleared after layout)
    needs_focus_visibility_update: Cell<bool>,
    // K3.INV1: Auto-height vertical origin (shifts when content grows above first visible line)
    // SPEC-HAL-141: Visual-only offset, NOT authoritative geometry.
    // Written: layout/event phases. Read: paint.
    visual_content_origin_y: Cell<f32>,
    // Cached metrics for paint (computed in layout)
    ascent: f32,
    descent: f32,
    // SPEC-HAL-141: Widget rect set in layout (no Cell - mutated only in layout phase)
    layout_rect: RectF,
    // Vertical scrollbar state for rendering and drag
    scrollbar_v: ScrollbarV,
    // Horizontal scrollbar state for rendering
    scrollbar_h: ScrollbarH,
    // Whether scrollbar thumb is being dragged (vertical)
    scrollbar_v_dragging: bool,
    // Whether scrollbar thumb is being dragged (horizontal)
    scrollbar_h_dragging: bool,
    // Vertical height stabilization: once laid out with finite height, preserve it.
    // This prevents Flex second-pass (max_height=INFINITY) from triggering auto-grow.
    bounded_height: Cell<Option<f32>>,
    // Cached geometry from theme (synced in layout for use in event handler and text_viewport)
    padding: f32,
    caret_margin: f32,
    /// Scroll margin: minimum distance (in pixels) from caret to viewport edge.
    /// Ensures context lines remain visible when editing near top/bottom.
    scroll_margin: f32,
    gutter_gap: f32,
    caret_thickness: f32,
    selection_y_offset: f32,
    empty_line_selection_width: f32,
    // Auto-scroll velocity during drag-selection (pixels per frame, 0 = inactive)
    // Vertical: Positive = scroll down, negative = scroll up
    auto_scroll_velocity_y: Cell<f32>,
    // Horizontal: Positive = scroll right, negative = scroll left
    auto_scroll_velocity_x: Cell<f32>,
    // Last frame's dt for anticipating scroll delta in selection hit-test
    // Stored in update(), used in PointerMove to eliminate selection-scroll phase skew
    last_frame_dt: Cell<f32>,
    // INV-TEXT-GEOM-PHASE: Last layout width for synchronous rebuild
    last_layout_width: f32,
    smooth_caret_enabled: bool,
    smooth_caret: AreaSmoothCaret,
    motion_font: Option<crate::core::text_engine::TextGenerationLease>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AreaCaretDestination {
    revision: u64,
    cursor: usize,
    line: usize,
    start: usize,
    end: usize,
    font: u64,
    x: f32,
    y: f32,
    height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AreaCaretSpace {
    geometry: crate::core::widget::OwnedWidgetPresentationGeometry,
    facts: FrameworkAppearanceFacts,
    viewport: RectF,
    auto_height: bool,
    line: (usize, usize, usize, u64),
    metrics: [f32; 17],
    typography_revision: u64,
}

struct AreaCaretProjection {
    destination: AreaCaretDestination,
    space: AreaCaretSpace,
    font: crate::core::text_engine::TextGenerationLease,
    x: f32,
}

#[derive(Default)]
struct AreaSmoothCaret {
    handle: Option<TransitionHandle>,
    projection: Option<AreaCaretProjection>,
    pending: Option<AreaCaretDestination>,
    snap: bool,
}

impl TextArea {
    /// SPEC-HAL-109 Stage F: Single-line mode uses huge width to prevent wrapping.
    const SINGLE_LINE_LAYOUT_WIDTH: f32 = 100_000.0;

    /// Auto-scroll speed (pixels per frame at distance threshold)
    const AUTO_SCROLL_BASE_SPEED: f32 = 8.0;
    /// Distance threshold where auto-scroll reaches max speed (pixels outside viewport)
    const AUTO_SCROLL_DISTANCE_THRESHOLD: f32 = 100.0;
    /// Maximum auto-scroll speed (pixels per frame)
    const AUTO_SCROLL_MAX_SPEED: f32 = 24.0;

    /// Get layout width based on editing policy.
    fn layout_width_for_policy(&self, constrained_width: f32) -> f32 {
        if self.editing_policy.single_line {
            Self::SINGLE_LINE_LAYOUT_WIDTH
        } else {
            constrained_width
        }
    }

    fn finite_nonnegative(value: f32, fallback: f32) -> f32 {
        if value.is_finite() && value >= 0.0 {
            value
        } else if fallback.is_finite() && fallback >= 0.0 {
            fallback
        } else {
            0.0
        }
    }

    fn finite_positive(value: f32, fallback: f32) -> f32 {
        if value.is_finite() && value > 0.0 {
            value
        } else if fallback.is_finite() && fallback > 0.0 {
            fallback
        } else {
            1.0
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

    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled
    }

    pub fn is_read_only(&self) -> bool {
        self.config.is_read_only
    }

    pub(crate) fn apply_availability_policy(&mut self, enabled: bool, read_only: bool) {
        if enabled != self.config.is_enabled || read_only != self.config.is_read_only {
            self.snap_smooth_caret();
        }
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
            self.state.drag_mode = TextAreaDragMode::None;
            self.blink_caret_visible.set(false);
            self.caret_animation_demand = Some(false);
            self.auto_scroll_velocity_x.set(0.0);
            self.auto_scroll_velocity_y.set(0.0);
            self.scrollbar_v_dragging = false;
            self.scrollbar_h_dragging = false;
            let mut scroll = self.state.scroll.borrow_mut();
            let visible = scroll.visual_offset();
            scroll.end_drag();
            scroll.set_force_animate(false);
            scroll.jump_to_offset(visible);
        }
    }

    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: TextAreaState::new(),
            focused: false,
            caret_animation_demand: None,
            hover: false,
            blink_last_ms: Cell::new(0),
            blink_caret_visible: Cell::new(false),
            blink_accumulated_secs: Cell::new(0.0),
            config: TextAreaConfig::default(),
            editing_policy: TextEditingPolicy::multi_line(),
            layout_dirty: Cell::new(true),
            needs_focus_visibility_update: Cell::new(false),
            // K3.INV1: Initialize auto-height origin at 0
            visual_content_origin_y: Cell::new(0.0),
            ascent: 0.0,
            descent: 0.0,
            layout_rect: RectF::zero(),
            scrollbar_v: ScrollbarV::new(),
            scrollbar_h: ScrollbarH::new(),
            scrollbar_v_dragging: false,
            scrollbar_h_dragging: false,
            bounded_height: Cell::new(None),
            // Defaults synced from theme in layout()
            padding: 8.0,
            caret_margin: 4.0,
            scroll_margin: 0.0,
            gutter_gap: 2.0,
            caret_thickness: 2.0,
            selection_y_offset: 1.5,
            empty_line_selection_width: 4.0,
            // Auto-scroll inactive by default
            auto_scroll_velocity_y: Cell::new(0.0),
            auto_scroll_velocity_x: Cell::new(0.0),
            // Default dt estimate (~60fps) until first frame
            last_frame_dt: Cell::new(1.0 / 60.0),
            // INV-TEXT-GEOM-PHASE: Default to infinite width until first layout
            last_layout_width: f32::INFINITY,
            smooth_caret_enabled: false,
            smooth_caret: AreaSmoothCaret::default(),
            motion_font: None,
        }
    }

    // =========================================================================
    // Builder Methods
    // =========================================================================

    /// Set minimum height (initial height before content grows)
    pub fn with_min_height(mut self, height: f32) -> Self {
        self.config.min_height = height;
        self
    }

    /// Set maximum height (content scrolls after this)
    pub fn with_max_height(mut self, height: f32) -> Self {
        self.config.max_height = Some(height);
        self
    }

    /// Set fixed height (overrides min/max, always uses this height)
    pub fn with_fixed_height(mut self, height: f32) -> Self {
        self.config.fixed_height = Some(height);
        self
    }

    /// Reset caret to visible on user activity.
    /// Called from event handlers (NOT paint phase) - no guard needed.
    fn reset_caret_blink_on_activity(&self, cx: &EventContext) {
        if self.focused {
            delegate::reset_caret_blink(
                &self.blink_last_ms,
                &self.blink_caret_visible,
                cx.frame_time_ms(),
            );
            self.blink_accumulated_secs.set(0.0); // Reset dt-based timer
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.state.document.set_text(text);
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.config.placeholder = Some(placeholder.into());
        self
    }

    pub fn text(&self) -> &str {
        self.state.document.text()
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
            BrowserTextKind::Multiline, self.state.ime_pre_composition_snapshot.is_some(),
        )
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
            BrowserTextAction::Cut => false,
            BrowserTextAction::Composition(ime) => {
                self.handle_event(cx, &Event::Ime(ime), EventPhase::Target)
            }
            BrowserTextAction::Select { anchor, head } => {
                if self.state.ime_pre_composition_snapshot.is_some()
                    || !self.text().is_char_boundary(anchor) || !self.text().is_char_boundary(head)
                {
                    return false;
                }
                self.snap_smooth_caret();
                self.state.selection = Selection { anchor, head };
                self.state.cursor.pos = head;
                self.needs_focus_visibility_update.set(true);
                self.reset_caret_blink(cx);
                cx.invalidate_layout();
                true
            }
            BrowserTextAction::Replace { start, end, text } => {
                if self.state.ime_pre_composition_snapshot.is_some() || start > end
                    || !self.text().is_char_boundary(start) || !self.text().is_char_boundary(end)
                {
                    return false;
                }
                self.state.selection = Selection { anchor: start, head: end };
                self.state.cursor.pos = end;
                self.handle_event(cx, &Event::Text(crate::core::event::TextInputEvent {
                    text, modifiers: Default::default(),
                }), EventPhase::Target)
            }
        }
    }

    /// Set the text content.
    ///
    /// SPEC-HAL-158B INV-DIRTY-TEXT-1: This setter MUST propagate dirty to the widget tree.
    /// Callers must provide tree context to ensure layout invalidation.
    pub fn set_text(&mut self, text: impl Into<String>, tree: &mut WidgetTree, node_id: NodeId) {
        self.snap_smooth_caret();
        self.state.document.set_text(text);
        let len = self.state.document.len();
        if self.state.cursor.pos > len {
            self.state.cursor.pos = len;
        }
        // K3.INV1: Reset auto-height origin when text is replaced wholesale
        self.visual_content_origin_y.set(0.0);
        // SPEC-HAL-158B: Propagate dirty to tree
        tree.mark_layout_dirty(node_id);
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    fn reset_caret_blink(&self, cx: &EventContext) {
        delegate::reset_caret_blink(
            &self.blink_last_ms,
            &self.blink_caret_visible,
            cx.frame_time_ms(),
        );
        self.blink_accumulated_secs.set(0.0); // Reset dt-based timer
    }

    fn capture_snapshot(&self) -> EditSnapshot {
        delegate::capture_snapshot(
            self.state.document.text(),
            &self.state.cursor,
            &self.state.selection,
        )
    }

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
            self.state.document.mark_modified();
            // INV-TEXT-GEOM-PHASE: Mark cache stale after document mutation
            self.layout_dirty.set(true);
            self.state.layout_cache.mark_stale();
            self.rebuild_layout_cache_now(cx.text_engine());
            cx.invalidate_layout();
            self.reset_caret_blink(cx);
        }
        performed
    }

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
            self.state.document.mark_modified();
            // INV-TEXT-GEOM-PHASE: Mark cache stale after document mutation
            self.layout_dirty.set(true);
            self.state.layout_cache.mark_stale();
            self.rebuild_layout_cache_now(cx.text_engine());
            cx.invalidate_layout();
            self.reset_caret_blink(cx);
        }
        performed
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
        // INV-TEXT-GEOM-PHASE: Geometry may not be queried before first layout.
        // If this fires, something is reading geometry before layout() ever ran.
        debug_assert!(
            self.last_layout_width.is_finite(),
            "rebuild_layout_cache_now called before first layout (last_layout_width not set)"
        );
        let doc = &self.state.document;
        let doc_version = doc.version();
        let cache = &mut self.state.layout_cache;
        cache.rebuild(doc.text(), doc_version, self.last_layout_width, text_engine);
        self.motion_font = self.smooth_caret_enabled
            .then(|| text_engine.presentation_text_generation()).flatten();
        self.layout_dirty.set(false);
        self.request_focus_visibility_update();
    }

    /// Get text viewport rect from widget rect (includes padding, excludes scrollbar gutters)
    ///
    /// This is the content area where text is rendered and clipped.
    /// Scrollbar gutters are reserved when scrollbars are visible.
    ///
    /// Uses `layout_rect` (local coordinates, 0,0 origin) for event handling.
    fn text_viewport(&self) -> RectF {
        self.text_viewport_from_rect(self.layout_rect)
    }

    /// Compute text viewport from an arbitrary rect.
    ///
    /// SPEC-HAL-141: Paint uses this with the absolute paint rect parameter.
    /// Event handlers use `text_viewport()` which passes layout_rect (local coords).
    fn text_viewport_from_rect(&self, rect: RectF) -> RectF {
        let scroll = self.state.scroll.borrow();
        let content_w = scroll.content_size.width;
        let content_h = scroll.content_size.height;
        drop(scroll);

        // Use cached geometry (synced from theme in layout)
        let padding = self.padding;
        let gutter_gap = self.gutter_gap;

        // Base viewport (widget rect minus padding)
        let base_w = (rect.width - padding * 2.0).max(0.0);
        let base_h = (rect.height - padding * 2.0).max(0.0);

        // Reserve gutter for scrollbars when visible
        let v_scrollbar_visible = content_h > base_h;
        let h_scrollbar_visible = content_w > base_w;

        let scrollbar_thickness = self.scrollbar_v.config.thickness;

        let width = if v_scrollbar_visible {
            (base_w - scrollbar_thickness - gutter_gap).max(0.0)
        } else {
            base_w
        };
        let height = if h_scrollbar_visible {
            (base_h - scrollbar_thickness - gutter_gap).max(0.0)
        } else {
            base_h
        };

        RectF::new(rect.x + padding, rect.y + padding, width, height)
    }

    /// Find the line index and x_logical for a given byte position
    ///
    /// K.INV1: Cache must be fresh. K.INV3: No fallback-to-end.
    fn caret_position(&self, byte_pos: usize) -> (usize, f32) {
        // K.INV1: Assert cache freshness in debug builds
        #[cfg(debug_assertions)]
        {
            let doc_len = self.state.document.len();
            let cached_len = self.state.layout_cache.cached_doc_len();
            debug_assert!(
                self.state.layout_cache.is_fresh_for_doc_len(doc_len),
                "K.INV1 violated: caret_position called with stale cache! \
                 doc_len={}, cached_doc_len={}, byte_pos={}",
                doc_len,
                cached_len,
                byte_pos
            );
        }

        let boundaries = self.state.layout_cache.boundaries();

        // K.INV3: No fallback-to-end. Must find exact boundary.
        for b in boundaries {
            if b.byte_start == byte_pos {
                return (b.line_index, b.x_logical);
            }
        }

        // K.INV3: Panic in debug if boundary not found
        #[cfg(debug_assertions)]
        {
            panic!(
                "K.INV3 violated: byte_pos={} not found in boundaries! \
                 valid_positions={:?}, doc_len={}",
                byte_pos,
                boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>(),
                self.state.document.len()
            );
        }

        // In release, return last boundary (but this should never happen if code is correct)
        #[cfg(not(debug_assertions))]
        {
            boundaries
                .last()
                .map(|b| (b.line_index, b.x_logical))
                .unwrap_or((0, 0.0))
        }
    }

    /// Compute caret Y position in content coordinates (S_content).
    /// This is the single source of truth for caret vertical position.
    /// Used by: ensure_focus_visible, paint, debug assertions.
    fn caret_content_y(&self) -> f32 {
        let (caret_line, _) = self.caret_position(self.state.cursor.pos);
        self.state.layout_cache.line_y(caret_line)
    }

    /// Caret height (ascent + descent).
    fn caret_height(&self) -> f32 {
        self.ascent + self.descent
    }

    /// Compute selection head Y position in content coordinates (S_content).
    /// INV-G.SEL2: Only head matters for scrolling, anchor is neutral.
    fn selection_head_content_y(&self) -> f32 {
        let (head_line, _) = self.caret_position(self.state.selection.head);
        self.state.layout_cache.line_y(head_line)
    }

    /// Determine if selection is active (anchor != head).
    fn has_active_selection(&self) -> bool {
        self.state.selection.anchor != self.state.selection.head
    }

    /// Unified focus target Y position in content coordinates (S_content).
    /// INV-G.FOCUS1: Single source of vertical focus tracking.
    fn focus_target_content_y(&self) -> f32 {
        if self.has_active_selection() {
            self.selection_head_content_y()
        } else {
            self.caret_content_y()
        }
    }

    /// S1.INV5: Unified focus target X position in content coordinates.
    /// S1.INV6: Uses caret_x_for_byte for correct EOL positioning.
    fn focus_target_content_x(&self) -> f32 {
        // Use selection.head position for X (same as caret when collapsed)
        let byte_pos = if self.has_active_selection() {
            self.state.selection.head
        } else {
            self.state.cursor.pos
        };
        // S1.INV6: Use layout cache's authoritative caret_x_for_byte
        self.state.layout_cache.caret_x_for_byte(byte_pos)
    }

    /// K.INV1: Request deferred focus visibility update.
    /// Called after mutations to schedule ensure_focus_visible() for after layout rebuild.
    fn request_focus_visibility_update(&self) {
        self.needs_focus_visibility_update.set(true);
    }

    /// Defer visibility until Layout has published current text and scroll geometry.
    fn update_focus_visibility(&self) {
        if self.layout_dirty.get() || self.needs_focus_visibility_update.get() {
            // Layout publishes scroll dimensions before consuming this request.
            self.request_focus_visibility_update();
        } else {
            self.ensure_focus_visible();
        }
    }

    /// K.INV0: Check if in auto-height mode (no scrolling).
    /// True when content fits within viewport.
    fn is_auto_height_mode(&self) -> bool {
        let scroll = self.state.scroll.borrow();
        let content_h = scroll.content_size.height;
        let viewport_h = scroll.viewport_size.height;
        content_h <= viewport_h
    }

    /// Ensure focus target is visible within viewport (vertical + horizontal scrolling)
    /// SPEC-HAL-109 Stage C: Scroll-to-caret discipline
    /// SPEC-HAL-109 Stage G: Viewport & caret visibility invariants
    /// SPEC-HAL-109 Stage G.2: Selection visibility invariants
    /// SPEC-HAL-109 Stage G.3: Unified focus target (INV-G.FOCUS1)
    /// SPEC-HAL-109 Stage K.3: Auto-height vertical origin (K3.INV1)
    /// S1.INV5: Horizontal scroll support
    /// S2.FIX: Use fresh layout widths, not stale scroll container values
    fn ensure_focus_visible(&self) {
        let scroll = self.state.scroll.borrow();
        let viewport_height = scroll.viewport_size.height;
        let content_height = scroll.content_size.height;
        let scroll_vp_w = scroll.viewport_size.width;
        let scroll_ct_w = scroll.content_size.width;
        let (scroll_x_before, scroll_y_before) = scroll.target_offset();
        drop(scroll);

        // S2.FIX: Use fresh layout values for horizontal scroll, not stale scroll container.
        // Fall back to scroll container values if layout values are not yet initialized
        // (happens in tests and before first paint).
        let layout_vp_w = self.text_viewport().width;
        let layout_ct_w = self.state.layout_cache.content_width();
        let viewport_width = if layout_vp_w > 0.0 {
            layout_vp_w
        } else {
            scroll_vp_w
        };
        let content_width = if layout_ct_w > 0.0 {
            layout_ct_w
        } else {
            scroll_ct_w
        };

        // ================================================================
        // Vertical scroll handling
        // ================================================================

        // K3.INV1: Auto-height mode - adjust visual_content_origin_y instead of scroll_y
        let mut final_scroll_y = scroll_y_before;
        if content_height <= viewport_height {
            let target_y = self.focus_target_content_y();
            let target_h = self.caret_height();
            let current_origin = self.visual_content_origin_y.get();

            // Check if focus target is above current origin
            if target_y < current_origin {
                self.visual_content_origin_y.set(target_y);
            }
            // Check if focus target is below visible area
            else if target_y + target_h > current_origin + viewport_height {
                let new_origin = target_y + target_h - viewport_height;
                self.visual_content_origin_y.set(new_origin.max(0.0));
            }
        } else {
            // K3.INV1: Reset visual_content_origin_y when transitioning to scroll mode
            if self.visual_content_origin_y.get() != 0.0 {
                self.visual_content_origin_y.set(0.0);
            }

            // INV-G.FOCUS1: Single focus target (selection.head or caret)
            let target_y = self.focus_target_content_y();
            let target_h = self.caret_height();

            // Visibility check in S_viewport
            let target_viewport_top = target_y - scroll_y_before;
            let target_viewport_bottom = target_viewport_top + target_h;

            // Compute new scroll position if not visible
            // Scroll margin: trigger scroll when caret enters margin zone.
            // Only apply margin if viewport is tall enough (at least 4x caret height).
            let margin = if viewport_height > self.scroll_margin * 2.0 + target_h {
                self.scroll_margin
            } else {
                0.0
            };
            if !(target_viewport_top >= margin
                && target_viewport_bottom <= viewport_height - margin)
            {
                let new_scroll_y = if target_viewport_top < margin {
                    // Caret above margin zone — scroll up to place caret at margin
                    (target_y - margin).max(0.0)
                } else {
                    // Caret below margin zone — scroll down to place caret at margin from bottom
                    target_y + target_h + margin - viewport_height
                };
                let max_scroll_y = (content_height - viewport_height).max(0.0);
                final_scroll_y = new_scroll_y.clamp(0.0, max_scroll_y);
            }

            // Unconditional clamp: scroll_y_before may exceed max_scroll_y
            // after a resize even when the caret is still visible.
            let max_scroll_y = (content_height - viewport_height).max(0.0);
            final_scroll_y = final_scroll_y.clamp(0.0, max_scroll_y);

            // Debug assertions for Stage G invariants
            #[cfg(debug_assertions)]
            {
                let target_viewport_top_after = target_y - final_scroll_y;
                let target_viewport_bottom_after = target_viewport_top_after + target_h;
                let max_scroll_y = (content_height - viewport_height).max(0.0);
                debug_assert!(
                    target_viewport_top_after >= -0.01
                        && target_viewport_bottom_after <= viewport_height + 0.01,
                    "INV-G.VIS1/SEL1 violated: focus target not visible (top={}, bottom={}, vh={})",
                    target_viewport_top_after,
                    target_viewport_bottom_after,
                    viewport_height
                );
                debug_assert!(
                    final_scroll_y >= -0.01 && final_scroll_y <= max_scroll_y + 0.01,
                    "INV-G.VIS4 violated: scroll_y={} outside [0, {}]",
                    final_scroll_y,
                    max_scroll_y
                );
            }
        }

        // ================================================================
        // S1.INV5: Horizontal scroll handling
        // ================================================================

        // Use cached caret_margin (synced from theme in layout)
        let caret_margin = self.caret_margin;

        let mut final_scroll_x = scroll_x_before;
        if content_width > viewport_width {
            let target_x = self.focus_target_content_x();
            let target_w = self.caret_thickness;

            // Visibility bounds with margin
            let visible_left = scroll_x_before + caret_margin;
            let visible_right = scroll_x_before + viewport_width - caret_margin - target_w;

            // Compute new scroll position if not visible
            if !(target_x >= visible_left && target_x <= visible_right) {
                let new_scroll_x = if target_x < visible_left {
                    (target_x - caret_margin).max(0.0)
                } else {
                    target_x + target_w + caret_margin - viewport_width
                };
                let max_scroll_x = (content_width - viewport_width + caret_margin * 2.0).max(0.0);
                final_scroll_x = new_scroll_x.clamp(0.0, max_scroll_x);
            }
        }

        // Apply scroll changes if any
        // S2.FIX: Use jump_to_offset for immediate visual update during caret-follow.
        // This ensures visual_offset == target_offset immediately after typing,
        // preventing the caret from appearing outside the viewport for one frame.
        let scroll_delta_y = final_scroll_y - scroll_y_before;
        let scroll_changed =
            (final_scroll_x - scroll_x_before).abs() > 0.01 || scroll_delta_y.abs() > 0.01;

        // HAL_TEXTAREA_SCROLL_DIAG: Diagnostic for caret-follow scroll issues
        {
            static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if *ENABLED.get_or_init(|| std::env::var_os("HAL_TEXTAREA_SCROLL_DIAG").is_some()) {
                // Compute last_line_bottom_y from layout cache
                let cache = &self.state.layout_cache;
                let last_line_bottom_y = cache.content_height();
                let max_scroll_y = (content_height - viewport_height).max(0.0);
                let target_y = self.focus_target_content_y();
                let target_h = self.caret_height();
                eprintln!(
                    "[SCROLL_DIAG] viewport_y=[{:.1}, {:.1}] caret_y=[{:.1}, {:.1}] scroll_y={:.1} \
                     content_height={:.1} max_scroll_y={:.1} last_line_bottom={:.1} delta_y={:.1} applied={}",
                    scroll_y_before,
                    scroll_y_before + viewport_height,
                    target_y,
                    target_y + target_h,
                    scroll_y_before,
                    content_height,
                    max_scroll_y,
                    last_line_bottom_y,
                    scroll_delta_y,
                    scroll_changed,
                );
            }
        }

        if scroll_changed {
            self.state
                .scroll
                .borrow_mut()
                .jump_to_offset((final_scroll_x, final_scroll_y));
        }
    }

    /// Get x_logical for selection start edge
    ///
    /// K.INV1: Cache must be fresh. K.INV3: No fallback-to-end.
    #[allow(
        dead_code,
        reason = "Retained text geometry query used by the native inspection fixture"
    )]
    fn x_for_byte(&self, byte_idx: usize) -> f32 {
        // K.INV1: Assert cache freshness in debug builds
        #[cfg(debug_assertions)]
        {
            let doc_len = self.state.document.len();
            debug_assert!(
                self.state.layout_cache.is_fresh_for_doc_len(doc_len),
                "K.INV1 violated: x_for_byte called with stale cache! \
                 doc_len={}, cached_doc_len={}, byte_idx={}",
                doc_len,
                self.state.layout_cache.cached_doc_len(),
                byte_idx
            );
        }

        let boundaries = self.state.layout_cache.boundaries();
        for b in boundaries {
            if b.byte_start == byte_idx {
                return b.x_logical;
            }
        }

        // K.INV3: Panic in debug if boundary not found
        #[cfg(debug_assertions)]
        {
            panic!(
                "K.INV3 violated: byte_idx={} not found in x_for_byte! \
                 valid_positions={:?}",
                byte_idx,
                boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>()
            );
        }

        #[cfg(not(debug_assertions))]
        {
            boundaries.last().map(|b| b.x_logical).unwrap_or(0.0)
        }
    }

    /// Get line_index for a byte position
    ///
    /// K.INV1: Cache must be fresh. K.INV3: No fallback-to-end.
    #[allow(
        dead_code,
        reason = "Retained text geometry query used by the native inspection fixture"
    )]
    fn line_for_byte(&self, byte_idx: usize) -> usize {
        // K.INV1: Assert cache freshness in debug builds
        #[cfg(debug_assertions)]
        {
            let doc_len = self.state.document.len();
            debug_assert!(
                self.state.layout_cache.is_fresh_for_doc_len(doc_len),
                "K.INV1 violated: line_for_byte called with stale cache! \
                 doc_len={}, cached_doc_len={}, byte_idx={}",
                doc_len,
                self.state.layout_cache.cached_doc_len(),
                byte_idx
            );
        }

        let boundaries = self.state.layout_cache.boundaries();
        for b in boundaries {
            if b.byte_start == byte_idx {
                return b.line_index;
            }
        }

        // K.INV3: Panic in debug if boundary not found
        #[cfg(debug_assertions)]
        {
            panic!(
                "K.INV3 violated: byte_idx={} not found in line_for_byte! \
                 valid_positions={:?}",
                byte_idx,
                boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>()
            );
        }

        #[cfg(not(debug_assertions))]
        {
            boundaries.last().map(|b| b.line_index).unwrap_or(0)
        }
    }

    /// Get the x_logical of the end of a line (last cluster + advance)
    #[allow(
        dead_code,
        reason = "Retained text geometry query used by the native inspection fixture"
    )]
    fn line_end_x(&self, line_index: usize) -> f32 {
        let lines = self.state.layout_cache.lines();
        lines.get(line_index).map(|l| l.width).unwrap_or(0.0)
    }

    // =========================================================================
    // Scrollbar Drag Handling (mirroring ScrollArea pattern)
    // =========================================================================

    /// Handle scrollbar click (PointerDown) - check for thumb hit
    fn handle_scrollbar_down(
        &mut self,
        cx: &mut EventContext,
        pe: &crate::core::event::PointerEvent,
    ) -> bool {
        // Drag exclusivity: if horizontal drag is active, don't start vertical
        if self.scrollbar_h_dragging {
            return false;
        }

        let scroll = self.state.scroll.borrow();
        let content_h = scroll.content_size.height;
        let content_w = scroll.content_size.width;
        let viewport_h = scroll.viewport_size.height;
        let viewport_w = scroll.viewport_size.width;

        if !self.scrollbar_v.is_visible(content_h, viewport_h) {
            return false;
        }

        let scroll_y = scroll.target_offset().1;
        let max_y = scroll.max_scroll().1;
        drop(scroll);

        let rect = self.layout_rect;
        let (local_x, local_y) = pe.local_pos;
        let (_, global_y) = pe.global_pos;

        // SPEC-HAL-110: Use unified layout for all scrollbar geometry
        let h_visible = self.scrollbar_h.is_visible(content_w, viewport_w);
        let h_thickness = self.scrollbar_h.config.thickness;

        // Track limit accounts for corner exclusion when both scrollbars visible
        let track_limit = if h_visible {
            rect.height - h_thickness
        } else {
            rect.height
        };
        let local_rect = RectF::new(0.0, 0.0, rect.width, rect.height);
        let layout =
            self.scrollbar_v
                .layout(local_rect, track_limit, content_h, viewport_h, scroll_y);

        // Check if click is in scrollbar area (using unified layout)
        let point = crate::core::math::Point::new(local_x, local_y);
        let thumb_hit = layout.thumb.contains(point);
        let hit_area_contains = layout.hit_rect.contains(point);

        // Compute track geometry in global coords for drag session
        let track_top = rect.y + layout.track.y;
        let track_bottom = track_top + layout.track.height;
        let thumb_len = layout.thumb.height;

        if thumb_hit {
            // Start drag on thumb
            self.scrollbar_v.begin_drag(
                global_y,
                scroll_y,
                track_top,
                track_bottom,
                thumb_len,
                max_y,
            );
            self.state.scroll.borrow_mut().begin_drag();
            self.scrollbar_v_dragging = true;
            if let Some(button) = pe.button {
                cx.capture_pointer(button);
            }
            return true;
        } else if hit_area_contains {
            // Click in track area (not on thumb) - jump to position AND start drag
            let click_ratio = (local_y - layout.track.y) / layout.track.height;
            let new_scroll = click_ratio * max_y;
            self.state
                .scroll
                .borrow_mut()
                .set_target_offset((0.0, new_scroll));

            // Begin drag session with the NEW scroll position
            self.scrollbar_v.begin_drag(
                global_y,
                new_scroll,
                track_top,
                track_bottom,
                thumb_len,
                max_y,
            );
            self.state.scroll.borrow_mut().begin_drag();
            self.scrollbar_v_dragging = true;
            if let Some(button) = pe.button {
                cx.capture_pointer(button);
            }
            return true;
        }

        false
    }

    /// Handle scrollbar drag (PointerMove while dragging)
    fn handle_scrollbar_drag(&mut self, pe: &crate::core::event::PointerEvent) -> bool {
        if !self.scrollbar_v_dragging {
            return false;
        }

        let (_, global_y) = pe.global_pos;
        let raw_scroll = self.scrollbar_v.update_drag(global_y);
        self.state.scroll.borrow_mut().push_drag_raw_y(raw_scroll);
        true
    }

    /// End vertical scrollbar drag
    fn end_scrollbar_v_drag(&mut self) {
        if self.scrollbar_v_dragging {
            self.scrollbar_v.end_drag();
            self.state.scroll.borrow_mut().end_drag();
            self.scrollbar_v_dragging = false;
        }
    }

    // =========================================================================
    // Horizontal Scrollbar Drag Handling
    // =========================================================================

    /// Handle horizontal scrollbar click (PointerDown) - check for thumb hit
    fn handle_scrollbar_h_down(
        &mut self,
        cx: &mut EventContext,
        pe: &crate::core::event::PointerEvent,
    ) -> bool {
        // Drag exclusivity: if vertical drag is active, don't start horizontal
        if self.scrollbar_v_dragging {
            return false;
        }

        let scroll = self.state.scroll.borrow();
        let content_w = scroll.content_size.width;
        let content_h = scroll.content_size.height;
        let viewport_w = scroll.viewport_size.width;
        let viewport_h = scroll.viewport_size.height;

        if !self.scrollbar_h.is_visible(content_w, viewport_w) {
            return false;
        }

        let scroll_x = scroll.target_offset().0;
        let scroll_y = scroll.target_offset().1;
        let max_x = scroll.max_scroll().0;
        drop(scroll);

        let rect = self.layout_rect;
        let (local_x, local_y) = pe.local_pos;
        let (global_x, _) = pe.global_pos;

        // SPEC-HAL-110: Use unified layout for all scrollbar geometry
        let v_visible = self.scrollbar_v.is_visible(content_h, viewport_h);
        let v_thickness = self.scrollbar_v.config.thickness;

        // Track limit accounts for corner exclusion when both scrollbars visible
        let track_limit = if v_visible {
            rect.width - v_thickness
        } else {
            rect.width
        };
        let local_rect = RectF::new(0.0, 0.0, rect.width, rect.height);
        let layout =
            self.scrollbar_h
                .layout(local_rect, track_limit, content_w, viewport_w, scroll_x);

        // Check if click is in scrollbar area (using unified layout)
        let point = crate::core::math::Point::new(local_x, local_y);
        let thumb_hit = layout.thumb.contains(point);
        let hit_area_contains = layout.hit_rect.contains(point);

        // Compute track geometry in global coords for drag session
        let track_left = rect.x + layout.track.x;
        let track_right = track_left + layout.track.width;
        let thumb_len = layout.thumb.width;

        if thumb_hit {
            // Start drag on thumb
            self.scrollbar_h.begin_drag(
                global_x,
                scroll_x,
                track_left,
                track_right,
                thumb_len,
                max_x,
            );
            self.state.scroll.borrow_mut().begin_drag();
            self.scrollbar_h_dragging = true;
            if let Some(button) = pe.button {
                cx.capture_pointer(button);
            }
            return true;
        } else if hit_area_contains {
            // Click in track area (not on thumb) - jump to position AND start drag
            let click_ratio = (local_x - layout.track.x) / layout.track.width;
            let new_scroll = click_ratio * max_x;
            self.state
                .scroll
                .borrow_mut()
                .set_target_offset((new_scroll, scroll_y));

            // Begin drag session with the NEW scroll position
            self.scrollbar_h.begin_drag(
                global_x,
                new_scroll,
                track_left,
                track_right,
                thumb_len,
                max_x,
            );
            self.state.scroll.borrow_mut().begin_drag();
            self.scrollbar_h_dragging = true;
            if let Some(button) = pe.button {
                cx.capture_pointer(button);
            }
            return true;
        }

        false
    }

    /// Handle horizontal scrollbar drag (PointerMove while dragging)
    fn handle_scrollbar_h_drag(&mut self, pe: &crate::core::event::PointerEvent) -> bool {
        if !self.scrollbar_h_dragging {
            return false;
        }

        let (global_x, _) = pe.global_pos;
        let raw_scroll = self.scrollbar_h.update_drag(global_x);
        self.state.scroll.borrow_mut().push_drag_raw_x(raw_scroll);
        true
    }

    /// End horizontal scrollbar drag
    fn end_scrollbar_h_drag(&mut self) {
        if self.scrollbar_h_dragging {
            self.scrollbar_h.end_drag();
            self.state.scroll.borrow_mut().end_drag();
            self.scrollbar_h_dragging = false;
        }
    }

    /// Update scrollbar hover state for both vertical and horizontal scrollbars
    fn update_scrollbar_hover(&mut self, pe: &crate::core::event::PointerEvent) {
        let scroll = self.state.scroll.borrow();
        let content_h = scroll.content_size.height;
        let content_w = scroll.content_size.width;
        let viewport_h = scroll.viewport_size.height;
        let viewport_w = scroll.viewport_size.width;
        let scroll_y = scroll.target_offset().1;
        let scroll_x = scroll.target_offset().0;
        drop(scroll);

        let rect = self.layout_rect;
        let (local_x, local_y) = pe.local_pos;
        let point = crate::core::math::Point::new(local_x, local_y);

        let v_visible = self.scrollbar_v.is_visible(content_h, viewport_h);
        let h_visible = self.scrollbar_h.is_visible(content_w, viewport_w);
        let v_thickness = self.scrollbar_v.config.thickness;
        let h_thickness = self.scrollbar_h.config.thickness;

        use crate::widgets::scrollbar::ScrollbarState;

        let local_rect = RectF::new(0.0, 0.0, rect.width, rect.height);

        // SPEC-HAL-110: Update vertical scrollbar hover using unified layout
        if v_visible && !self.scrollbar_v_dragging {
            let track_limit = if h_visible {
                rect.height - h_thickness
            } else {
                rect.height
            };
            let layout =
                self.scrollbar_v
                    .layout(local_rect, track_limit, content_h, viewport_h, scroll_y);
            let hovered = layout.thumb.contains(point);
            self.scrollbar_v.state = if hovered {
                ScrollbarState::Hovered
            } else {
                ScrollbarState::Normal
            };
        } else if !self.scrollbar_v_dragging {
            self.scrollbar_v.state = ScrollbarState::Normal;
        }

        // SPEC-HAL-110: Update horizontal scrollbar hover using unified layout
        if h_visible && !self.scrollbar_h_dragging {
            let track_limit = if v_visible {
                rect.width - v_thickness
            } else {
                rect.width
            };
            let layout =
                self.scrollbar_h
                    .layout(local_rect, track_limit, content_w, viewport_w, scroll_x);
            let hovered = layout.thumb.contains(point);
            self.scrollbar_h.state = if hovered {
                ScrollbarState::Hovered
            } else {
                ScrollbarState::Normal
            };
        } else if !self.scrollbar_h_dragging {
            self.scrollbar_h.state = ScrollbarState::Normal;
        }
    }

    /// Paint scrollbars outside the text clip region.
    /// Called from paint() after end_clip() to ensure scrollbars aren't clipped.
    fn paint_scrollbars(&self, cx: &mut PaintContext, rect: RectF) {
        let thickness = self.scrollbar_v.config.thickness;
        let corner_radius = self.scrollbar_v.config.corner_radius;

        let scroll = self.state.scroll.borrow();
        let content_w = scroll.content_size.width;
        let content_h = scroll.content_size.height;
        let viewport_w = scroll.viewport_size.width;
        let viewport_h = scroll.viewport_size.height;
        let (scroll_x, scroll_y) = scroll.target_offset();
        drop(scroll);

        let v_visible = self.scrollbar_v.is_visible(content_h, viewport_h);
        let h_visible = self.scrollbar_h.is_visible(content_w, viewport_w);
        let local_rect = RectF::new(0.0, 0.0, rect.width, rect.height);

        // Begin a new clip scope at widget rect level for scrollbars
        // This ensures scrollbars are clipped to widget bounds, not text_viewport
        cx.scene.begin_clip(rect, None);

        // Vertical scrollbar
        if v_visible {
            let track_limit = if h_visible {
                rect.height - thickness
            } else {
                rect.height
            };
            let layout =
                self.scrollbar_v
                    .layout(local_rect, track_limit, content_h, viewport_h, scroll_y);

            let abs_thumb = RectF::new(
                rect.x + layout.thumb.x,
                rect.y + layout.thumb.y,
                layout.thumb.width,
                layout.thumb.height,
            );
            let thumb_color = self.scrollbar_v.thumb_color(&cx.theme.colors);
            // INV-PAINT-LAYERS-FOCUS: Scrollbar uses LAYER_SCROLLBAR (above focus indicators)
            cx.scene.push_rounded_rect(
                abs_thumb,
                thumb_color,
                RoundedCorners::uniform(corner_radius),
                LAYER_SCROLLBAR,
                cx.scene_key,
            );
        }

        // Horizontal scrollbar
        if h_visible {
            let track_limit = if v_visible {
                rect.width - thickness
            } else {
                rect.width
            };
            let layout =
                self.scrollbar_h
                    .layout(local_rect, track_limit, content_w, viewport_w, scroll_x);

            let abs_thumb = RectF::new(
                rect.x + layout.thumb.x,
                rect.y + layout.thumb.y,
                layout.thumb.width,
                layout.thumb.height,
            );
            let thumb_color = self.scrollbar_h.thumb_color(&cx.theme.colors);
            // INV-PAINT-LAYERS-FOCUS: Scrollbar uses LAYER_SCROLLBAR (above focus indicators)
            cx.scene.push_rounded_rect(
                abs_thumb,
                thumb_color,
                RoundedCorners::uniform(corner_radius),
                LAYER_SCROLLBAR,
                cx.scene_key,
            );
        }

        // End the widget-rect clip scope
        cx.scene.end_clip();
    }
}

impl TextArea {
    /// Opt into HAL-109 §17 same-line navigation presentation. Default: off.
    pub fn with_smooth_caret_prototype(mut self, enabled: bool) -> Self {
        self.smooth_caret_enabled = enabled;
        self
    }

    pub fn smooth_caret_prototype_enabled(&self) -> bool {
        self.smooth_caret_enabled
    }

    fn snap_smooth_caret(&mut self) {
        if self.smooth_caret_enabled || self.smooth_caret.handle.is_some() {
            self.smooth_caret.snap = true;
            self.smooth_caret.pending = None;
        }
    }

    fn motion_eligible(&self, facts: Option<FrameworkAppearanceFacts>) -> bool {
        self.smooth_caret_enabled
            && self.config.is_enabled
            && !self.state.is_composing()
            && !self.has_active_selection()
            && facts.is_some_and(|f| f.focused() && f.keyboard_captured()
                && f.window_active() && f.interaction_domain_active())
    }

    fn motion_idle(&self) -> bool {
        self.config.is_enabled && !self.layout_dirty.get() && !self.state.is_composing()
            && !self.has_active_selection() && !self.needs_focus_visibility_update.get()
            && !self.scrollbar_v_dragging && !self.scrollbar_h_dragging
            && self.state.drag_mode == TextAreaDragMode::None
            && self.auto_scroll_velocity_x.get() == 0.0 && self.auto_scroll_velocity_y.get() == 0.0
            && {
                let scroll = self.state.scroll.borrow();
                scroll.target_offset() == scroll.visual_offset() && !scroll.needs_animation()
            }
    }

    fn motion_geometry_eligible(
        &self, geometry: crate::core::widget::OwnedWidgetPresentationGeometry,
        theme: &crate::core::theme::Theme,
    ) -> bool {
        self.motion_idle()
            && geometry.rect.width == self.layout_rect.width
            && geometry.rect.height == self.layout_rect.height
            && geometry.scale_factor.is_finite() && geometry.scale_factor > 0.0
            && geometry.cumulative_scroll.0.is_finite() && geometry.cumulative_scroll.1.is_finite()
            && [geometry.rect, geometry.viewport, geometry.root_viewport, geometry.clip, self.text_viewport()]
                .iter().all(|r| [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
                    && r.width > 0.0 && r.height > 0.0)
            && self.state.layout_cache.font_size() == theme.typography.input_font_size
            && self.padding == theme.padding.m && self.gutter_gap == theme.spacing.xxs
            && self.caret_thickness == theme.typography.caret_thickness
    }

    // INV-109-MOTION-SCOPE: exact current cache evidence, never prefix shaping.
    fn smooth_destination(&self) -> Option<AreaCaretDestination> {
        let doc = &self.state.document;
        let cache = &self.state.layout_cache;
        if self.layout_dirty.get() || self.has_active_selection() || self.state.is_composing()
            || cache.needs_rebuild(doc.version(), self.last_layout_width)
            || !cache.is_fresh_for_doc_len(doc.len())
        {
            return None;
        }
        let cursor = self.state.cursor.pos;
        let boundaries = cache.boundaries();
        let index = boundaries.binary_search_by_key(&cursor, |b| b.byte_start).ok()?;
        let boundary = &boundaries[index];
        if index.checked_sub(1).is_some_and(|i| boundaries[i].byte_start == cursor)
            || boundaries.get(index + 1).is_some_and(|b| b.byte_start == cursor)
        {
            return None;
        }
        let line = cache.lines().get(boundary.line_index)?;
        let first = line.runs.first()?;
        let start = first.range.start;
        let end = line.runs.last()?.range.end;
        if cursor < start || cursor > end
            || !doc.text().get(start..end)?.bytes().all(|b| (b' '..=b'~').contains(&b))
        {
            return None;
        }
        let mut next = start;
        for run in &line.runs {
            if run.range.start != next || run.range.end < next || run.font_id != first.font_id
                || run.glyphs.len() != run.range.len()
                || run.glyphs.iter().enumerate().any(|(i, g)| g.cluster as usize != i
                    || !g.advance.is_finite() || g.advance < 0.0)
            {
                return None;
            }
            next = run.range.end;
        }
        let y = cache.line_y(boundary.line_index);
        let height = self.caret_height();
        if ![boundary.x_logical, y, height].iter().all(|v| v.is_finite()) || height <= 0.0 {
            return None;
        }
        Some(AreaCaretDestination {
            revision: doc.version(), cursor, line: boundary.line_index,
            start, end, font: first.font_id, x: boundary.x_logical, y, height,
        })
    }

    fn smooth_space(
        &self,
        geometry: crate::core::widget::OwnedWidgetPresentationGeometry,
        facts: FrameworkAppearanceFacts,
        theme: &crate::core::theme::Theme,
        destination: AreaCaretDestination,
    ) -> Option<AreaCaretSpace> {
        if !self.motion_eligible(Some(facts))
            || self.scrollbar_v_dragging || self.scrollbar_h_dragging
            || self.state.drag_mode != TextAreaDragMode::None
            || self.auto_scroll_velocity_x.get() != 0.0 || self.auto_scroll_velocity_y.get() != 0.0
            || self.needs_focus_visibility_update.get()
            || geometry.rect.width != self.layout_rect.width
            || geometry.rect.height != self.layout_rect.height
        {
            return None;
        }
        let scroll = self.state.scroll.borrow();
        let offset = scroll.visual_offset();
        if offset != scroll.target_offset() || scroll.needs_animation() {
            return None;
        }
        let viewport = self.text_viewport();
        let cache = &self.state.layout_cache;
        if [geometry.rect, geometry.viewport, geometry.root_viewport, geometry.clip, viewport]
            .iter().any(|r| ![r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
                || r.width <= 0.0 || r.height <= 0.0)
            || !geometry.cumulative_scroll.0.is_finite()
            || !geometry.cumulative_scroll.1.is_finite()
        { return None; }
        let metrics = [self.ascent, self.descent, self.padding, self.gutter_gap,
            self.caret_thickness, self.last_layout_width, self.visual_content_origin_y.get(),
            offset.0, offset.1, cache.font_size(), cache.line_height(),
            cache.content_width(), cache.content_height(), destination.y, destination.height,
            self.scrollbar_v.config.thickness, theme.typography.line_height];
        if metrics.iter().any(|v| !v.is_finite())
            || cache.font_size() != theme.typography.input_font_size
            || self.padding != theme.padding.m || self.gutter_gap != theme.spacing.xxs
            || self.caret_thickness != theme.typography.caret_thickness
            || geometry.scale_factor <= 0.0 || !geometry.scale_factor.is_finite()
            || viewport.width <= 0.0 || viewport.height <= 0.0
        {
            return None;
        }
        Some(AreaCaretSpace {
            typography_revision: cache.typography_revision(),
            geometry, facts, viewport, auto_height: self.is_auto_height_mode(),
            line: (destination.line, destination.start, destination.end, destination.font), metrics,
        })
    }

    fn caret_inside_space(space: AreaCaretSpace, x: f32) -> bool {
        let viewport = space.viewport;
        let origin = space.geometry.rect;
        let left = origin.x + viewport.x;
        let top = origin.y + viewport.y;
        let raw_scroll_y = if space.auto_height {
            space.metrics[6]
        } else { space.metrics[8] };
        let scale = space.geometry.scale_factor;
        let scroll_x = (space.metrics[7] * scale).round() / scale;
        let scroll_y = (raw_scroll_y * scale).round() / scale;
        let caret_x = left + x - scroll_x;
        let caret_y = top + space.metrics[13] - scroll_y;
        let clip = space.geometry.clip;
        let root = space.geometry.root_viewport;
        caret_x.is_finite() && caret_y.is_finite()
            && caret_x >= left.max(clip.left()).max(root.left())
            && caret_x + space.metrics[4] <= (left + viewport.width).min(clip.right()).min(root.right())
            && caret_y >= top.max(clip.top()).max(root.top())
            && caret_y + space.metrics[14] <= (top + viewport.height).min(clip.bottom()).min(root.bottom())
    }

    fn update_smooth_caret(&mut self, cx: &mut UpdateContext) {
        // INV-109-MOTION-LIFETIME: excluded paths never scan motion source.
        if !self.motion_eligible(cx.appearance_facts()) {
            let old = std::mem::take(&mut self.smooth_caret);
            if let Some(handle) = old.handle { cx.cancel_animation(handle); }
            if old.projection.is_some() { cx.invalidate_paint(); }
            return;
        }
        let geometry = cx.own_presentation_geometry().zip(cx.theme())
            .filter(|(g, t)| self.motion_geometry_eligible(*g, t)).map(|(g, _)| g);
        let destination = geometry.and_then(|_| self.smooth_destination());
        let space = destination.zip(geometry)
            .zip(cx.appearance_facts()).zip(cx.theme())
            .and_then(|(((d, g), f), t)| self.smooth_space(g, f, t, d))
            .filter(|s| destination.is_some_and(|d| Self::caret_inside_space(*s, d.x)));
        let old = self.smooth_caret.projection.take();
        let pending = self.smooth_caret.pending.take();
        let compatible = old.as_ref().zip(space).is_some_and(|(p, s)| p.space == s)
            && old.as_ref().zip(destination).is_some_and(|(p, d)|
                p.destination.revision == d.revision && (pending == Some(d) || p.destination == d))
            && old.as_ref().zip(self.motion_font.as_ref()).is_some_and(|(p, f)| p.font.same_generation(f));
        let mut x = destination.map(|d| d.x);
        if !self.smooth_caret.snap && compatible {
            let previous = old.as_ref().unwrap();
            let mut from = Some(previous.x);
            if let Some(handle) = self.smooth_caret.handle {
                from = match cx.sample_animation::<f32>(handle) {
                    Some(AnimationSample::Active(v)) if v.is_finite() => Some(v),
                    Some(AnimationSample::Completed(v)) if v.is_finite() => {
                        self.smooth_caret.handle = None;
                        Some(v)
                    }
                    _ => { cx.cancel_animation(handle); self.smooth_caret.handle = None; None }
                };
            }
            if let (Some(from), Some(d)) = (from, destination) {
                x = Some(from);
                if d.x != previous.destination.x {
                    if let Some(handle) = self.smooth_caret.handle.take() { cx.cancel_animation(handle); }
                    if from != d.x {
                        match cx.try_start_animation(Animation::new(from, d.x, 0.040, Easing::EaseOut)) {
                            Ok(Some(handle)) => self.smooth_caret.handle = Some(handle),
                            _ => x = Some(d.x),
                        }
                    }
                }
            }
        } else if let Some(handle) = self.smooth_caret.handle.take() {
            cx.cancel_animation(handle);
        }
        self.smooth_caret.snap = false;
        self.smooth_caret.projection = destination.zip(space).zip(x)
            .zip(self.motion_font.as_ref()).map(|(((destination, space), x), font)|
                AreaCaretProjection { destination, space, x, font: font.clone() });
        if old.as_ref().map(|p| p.x) != self.smooth_caret.projection.as_ref().map(|p| p.x) {
            cx.invalidate_paint();
        }
    }

    fn painted_caret_x(&self, cx: &PaintContext, rect: RectF, canonical: f32) -> f32 {
        if !self.motion_eligible(cx.appearance_facts()) || self.smooth_caret.snap
            || self.smooth_caret.pending.is_some()
        { return canonical; }
        let Some(projection) = self.smooth_caret.projection.as_ref() else { return canonical; };
        let Some(geometry) = cx.own_presentation_geometry()
            .filter(|g| g.rect == rect && self.motion_geometry_eligible(*g, cx.theme))
        else { return canonical; };
        let destination = self.smooth_destination();
        if destination != Some(projection.destination) { return canonical; }
        let space = Some(geometry)
            .zip(cx.appearance_facts()).zip(destination)
            .and_then(|((g, f), d)| self.smooth_space(g, f, cx.theme, d));
        if space != Some(projection.space)
            || !Self::caret_inside_space(projection.space, projection.destination.x)
            || !Self::caret_inside_space(projection.space, projection.x)
            || !cx.text_engine.presentation_text_generation()
                .is_some_and(|font| projection.font.same_generation(&font))
        { return canonical; }
        projection.x
    }

}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for TextArea {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TextArea {
    /// Per-frame update for time-based state changes.
    ///
    /// Handles auto-scroll during drag-selection and scroll visual animation.
    /// This is the single authority for time-based scroll mutation.
    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        self.caret_animation_demand = cx.appearance_facts().map(|facts| facts.keyboard_captured());
        if !self.config.is_enabled {
            self.update_smooth_caret(cx);
            return;
        }
        let scroll_before = self.state.scroll.borrow().visual_offset();
        // Auto-scroll: apply stored velocity if drag-selection is active outside viewport
        let velocity_x = self.auto_scroll_velocity_x.get();
        let velocity_y = self.auto_scroll_velocity_y.get();
        if (velocity_x.abs() > 0.1 || velocity_y.abs() > 0.1)
            && matches!(self.state.drag_mode, TextAreaDragMode::Character { .. })
        {
            // Scale velocity by dt (velocity is in pixels/frame at 60fps baseline)
            // Normalize to ~60fps: dt * 60 gives frame-equivalent multiplier
            let scroll_delta_x = velocity_x * dt * 60.0;
            let scroll_delta_y = velocity_y * dt * 60.0;
            self.state
                .scroll
                .borrow_mut()
                .scroll_by(scroll_delta_x, scroll_delta_y);
        }

        // Animate scroll visual toward target with real dt
        self.state.scroll.borrow_mut().update_visual(dt);
        if self.state.scroll.borrow().visual_offset() != scroll_before {
            cx.invalidate_paint();
        }

        // Store dt for next frame's selection hit-test anticipation
        self.last_frame_dt.set(dt);

        // Caret blink: dt-based timing (530ms = 0.530 seconds interval)
        // This ensures blink is independent of paint frequency.
        if self.focused {
            const BLINK_INTERVAL_SECS: f32 = 0.530;
            let accumulated = self.blink_accumulated_secs.get() + dt;
            if accumulated >= BLINK_INTERVAL_SECS {
                self.blink_caret_visible
                    .set(!self.blink_caret_visible.get());
                // Subtract interval to preserve timing accuracy
                self.blink_accumulated_secs
                    .set(accumulated - BLINK_INTERVAL_SECS);
            } else {
                self.blink_accumulated_secs.set(accumulated);
            }
        }
        self.update_smooth_caret(cx);
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // =======================================================================
        // INV-SCROLLBAR-GEOM-1: Scrollbar config sync (CANONICAL LOCATION)
        // =======================================================================
        // Theme → Config sync MUST happen here, before any paint/event consumer.
        // Layout always runs before paint/event each frame (engine guarantee).
        // All consumers (paint_overlay, handle_scrollbar_down, update_scrollbar_hover,
        // ScrollbarV/H::layout helper) read from config, never theme directly.
        // =======================================================================
        self.scrollbar_v.config.thickness =
            Self::finite_nonnegative(cx.theme.scrollbar_geom.thickness, 0.0);
        self.scrollbar_v.config.corner_radius =
            Self::finite_nonnegative(cx.theme.scrollbar_geom.corner_radius, 0.0);
        self.scrollbar_v.config.min_thumb_size =
            Self::finite_nonnegative(cx.theme.scrollbar_geom.min_thumb_size, 0.0);
        self.scrollbar_v.config.hit_expansion =
            Self::finite_nonnegative(cx.theme.scrollbar_geom.hit_expansion, 0.0);
        self.scrollbar_h.config.thickness = self.scrollbar_v.config.thickness;
        self.scrollbar_h.config.corner_radius = self.scrollbar_v.config.corner_radius;
        self.scrollbar_h.config.min_thumb_size = self.scrollbar_v.config.min_thumb_size;
        self.scrollbar_h.config.hit_expansion = self.scrollbar_v.config.hit_expansion;

        // INV-GEOM-SYNC-TEXTAREA: Theme geometry sync (CANONICAL LOCATION)
        // Theme → cached field sync for event handler and text_viewport use.
        // Paint/event code MUST NOT read theme directly for geometry.
        self.padding = Self::finite_nonnegative(cx.theme.padding.m, 0.0);
        self.caret_margin = Self::finite_nonnegative(cx.theme.typography.caret_margin, 0.0);
        self.gutter_gap = Self::finite_nonnegative(cx.theme.spacing.xxs, 0.0);
        self.caret_thickness = Self::finite_positive(cx.theme.typography.caret_thickness, 1.0);
        self.selection_y_offset =
            Self::finite_nonnegative(cx.theme.typography.selection_y_offset, 0.0);
        self.empty_line_selection_width =
            Self::finite_nonnegative(cx.theme.typography.empty_line_selection_width, 0.0);

        let font_size = Self::finite_positive(cx.theme.typography.input_font_size, 14.0);
        let metrics = cx.text_engine.font_metrics(font_size);
        self.ascent = Self::finite_nonnegative(metrics.ascent, font_size * 0.8);
        self.descent = Self::finite_nonnegative(metrics.descent, font_size * 0.2);
        // Scroll margin: 2 lines of context above/below caret
        self.scroll_margin = (self.ascent + self.descent) * 2.0;

        // Text viewport width (inside padding). Unbounded/zero-width measurement
        // passes are not authoritative for wrapping; keep the last real finite
        // content width for synchronous post-edit cache rebuilds.
        let max_width = Self::finite_nonnegative(constraints.max_width, 0.0);
        let measured_text_width = (max_width - self.padding * 2.0).max(0.0);
        let cache_width = self.state.layout_cache.cached_width();
        let fallback_text_width =
            if self.last_layout_width.is_finite() && self.last_layout_width > 0.5 {
                self.last_layout_width
            } else if cache_width.is_finite() && cache_width > 0.5 {
                cache_width
            } else {
                Self::SINGLE_LINE_LAYOUT_WIDTH
            };
        let has_authoritative_width =
            constraints.max_width.is_finite() && measured_text_width > 0.5;
        let text_width = if has_authoritative_width {
            measured_text_width
        } else {
            fallback_text_width
        };

        let doc = &self.state.document;
        let doc_version = doc.version();

        // SPEC-HAL-109 Stage F: Get layout width from policy
        let layout_width = self.layout_width_for_policy(text_width);

        // INV-TEXT-GEOM-PHASE: Track only real finite content widths for
        // synchronous rebuild. Transient Flex measurement widths must not poison
        // wrapping after edits.
        if has_authoritative_width {
            self.last_layout_width = layout_width;
        }

        // Configure cache
        let cache = &mut self.state.layout_cache;
        cache.set_font_size(font_size);
        cache.set_color(cx.theme.colors.typography.primary);
        cache.set_caret_thickness(self.caret_thickness);

        // =======================================================================
        // INV-TEXTAREA-TIGHT-WIDTH: Only rebuild on tight-width layout passes.
        // =======================================================================
        // Flex performs multiple layout passes with different constraints:
        // - First pass: loose constraints (min=0, max=available) for measurement
        // - Second pass: tight constraints (min≈max) for final sizing
        //
        // Rebuild on document changes or when an authoritative finite width
        // changes. Unbounded/zero-width measurement passes reuse the last real
        // wrapping width.
        // =======================================================================
        let needs_rebuild =
            self.layout_dirty.get() || cache.needs_rebuild(doc_version, layout_width)
                || cache.typography_changed(cx.text_engine)
                    .expect("UI-TYPOGRAPHY-INPUT: TextArea typography identity failed");
        // INV-TEXT-GEOM-PHASE: Only check lines() if we don't already know we need rebuild.
        // Calling lines() on a stale cache (after mark_stale) triggers the assertion.
        let cache_uninitialized = !needs_rebuild && cache.lines().is_empty();

        if needs_rebuild || cache_uninitialized {
            cache.rebuild(doc.text(), doc_version, layout_width, cx.text_engine);
            self.motion_font = self.smooth_caret_enabled
                .then(|| cx.text_engine.presentation_text_generation()).flatten();
            self.layout_dirty.set(false);

            // DEBUG: Post-rebuild validation
            #[cfg(debug_assertions)]
            {
                let boundaries = cache.boundaries();
                let cursor_pos = self.state.cursor.pos;
                let cursor_valid = boundaries.iter().any(|b| b.byte_start == cursor_pos);

                debug_assert!(
                    cursor_valid,
                    "[TextArea Layout] cursor.pos={} not valid after rebuild! \
                     doc_len={}, boundaries={:?}",
                    cursor_pos,
                    doc.len(),
                    boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>()
                );
            }
        }

        // Compute content height (text content plus padding)
        let cache_content_height = Self::finite_nonnegative(cache.content_height(), 0.0);
        let content_height = cache_content_height + self.padding * 2.0;

        // Widget size: width from constraints, height from policy
        let width = max_width;

        // HEIGHT POLICY:
        // 1. fixed_height: always use this height (scroll when content exceeds)
        // 2. Otherwise: clamp(content_height, min_height, max_height)
        //
        // VERTICAL HEIGHT STABILIZATION INVARIANT:
        // Once we compute a bounded height, preserve it across Flex second pass
        // (which gives max_height=INFINITY).
        //
        // VIEWPORT-CONTENT-SEPARATION INVARIANT:
        // Viewport height must come from layout constraints, never from content height.
        // max_h must respect BOTH config.max_height AND constraints.max_height.
        // Without this, viewport == content → no scroll possible.
        let policy_height = if let Some(fixed) = self.config.fixed_height {
            // Fixed height mode: always use this
            Self::finite_nonnegative(fixed, self.config.min_height)
        } else {
            // Auto-grow mode: grow with content, clamped by min/max
            let configured_min = Self::finite_nonnegative(self.config.min_height, 0.0);
            let config_max = self
                .config
                .max_height
                .filter(|h| h.is_finite() && *h >= 0.0)
                .unwrap_or(f32::INFINITY);
            // CRITICAL: Also respect parent's layout constraint
            let constraint_max =
                if constraints.max_height.is_finite() && constraints.max_height >= 0.0 {
                    constraints.max_height
                } else {
                    f32::INFINITY
                };
            let max_h = config_max.min(constraint_max);
            let min_h = configured_min.min(max_h);
            content_height.clamp(min_h, max_h)
        };

        // FLEX-PASS-IMMUNITY: Cache bounded height to survive Flex second pass with INFINITY.
        // BUG FIX: Previous code only set bounded_height on first layout (when None), causing
        // widget height to be frozen during resize. Now we update cache whenever constraints
        // are bounded, and only use cache when constraints are infinite (Flex second pass).
        let raw_height = if !constraints.max_height.is_finite() {
            // Flex second pass: use cached height to avoid using INFINITY
            self.bounded_height
                .get()
                .filter(|h| h.is_finite() && *h >= 0.0)
                .unwrap_or(policy_height)
        } else {
            // Bounded constraints (real resize or first pass): use fresh policy_height
            policy_height
        };
        let final_size = constraints.constrain(Size::new(width, raw_height));
        let width = final_size.width;
        let height = final_size.height;
        if constraints.max_height.is_finite() {
            self.bounded_height.set(Some(height));
        }

        // Update scroll container sizes
        // S1.INV5: Use actual content width for horizontal scroll support
        // Capture cache values before releasing borrow
        let cache_content_width = Self::finite_nonnegative(cache.content_width(), 0.0);
        let cache_content_height = Self::finite_nonnegative(cache.content_height(), 0.0);
        let _cache_doc_len = cache.cached_doc_len();

        // INV-VIEWPORT-FRESH: Compute viewport using FRESH cache values for scrollbar visibility.
        // BUG FIX: text_viewport() reads stale scroll.content_size from previous layout,
        // causing incorrect viewport when content size changes (e.g., scrollbar visibility change).
        // The fix: compute scrollbar visibility using FRESH cache_content_* values.

        // Base viewport from current layout (widget minus padding)
        let base_vp_width = text_width;
        let base_vp_height = (height - self.padding * 2.0).max(0.0);

        // Scrollbar visibility using FRESH cache values (not stale scroll.content_size)
        let v_scrollbar_visible = cache_content_height > base_vp_height;
        let h_scrollbar_visible = cache_content_width > base_vp_width;

        let scrollbar_thickness = Self::finite_nonnegative(self.scrollbar_v.config.thickness, 0.0);
        let stable_vp_width = if v_scrollbar_visible {
            (base_vp_width - scrollbar_thickness - self.gutter_gap).max(0.0)
        } else {
            base_vp_width
        };
        let stable_vp_height = if h_scrollbar_visible {
            (base_vp_height - scrollbar_thickness - self.gutter_gap).max(0.0)
        } else {
            base_vp_height
        };

        {
            let mut scroll = self.state.scroll.borrow_mut();
            scroll.set_content_size(Size::new(cache_content_width, cache_content_height));
            scroll.set_viewport_size(Size::new(stable_vp_width, stable_vp_height));
            // NOTE: clamp_target() moved after ensure_focus_visible() to avoid
            // clamping before caret-follow scroll adjustment is applied.
        }

        // K.INV1: Deferred focus visibility update (after layout rebuild)
        // Now that cache is fresh, safe to compute focus target position.
        if self.needs_focus_visibility_update.get() {
            self.needs_focus_visibility_update.set(false);
            self.ensure_focus_visible();
        }

        // Clamp scroll target after ensure_focus_visible has had a chance to adjust it
        self.state.scroll.borrow_mut().clamp_target();

        // SPEC-HAL-141: Store layout rect for event handling and text_viewport().
        // This replaces the previous paint-phase widget_rect.set() call.
        self.layout_rect = RectF::new(0.0, 0.0, width, height);

        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // SPEC-HAL-141: layout_rect is now set in layout(), not here.
        // Paint only reads layout_rect, never writes it.
        // The paint `rect` parameter is the canonical rect for this paint call.
        // Blink state is now updated in update() via dt-based timing, not here.

        // HAL_TEXTAREA_PAINT_DIAG: Diagnostic counters for primitive emission
        static PAINT_DIAG_ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let diag_enabled = *PAINT_DIAG_ENABLED
            .get_or_init(|| std::env::var_os("HAL_TEXTAREA_PAINT_DIAG").is_some());
        let mut emit_rect_count: u32 = 0;
        let mut emit_text_count: u32 = 0;

        let t = cx.theme;
        let geom = t.geometry.text_field; // Reuse text_field geometry for now

        let chrome_state = if !self.config.is_enabled {
            ControlChromeState::Disabled
        } else if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = t.text_area_chrome(chrome_state);
        let radius = t
            .chrome_radius(ControlChromeCategory::RecessedEditable, geom.radius, false)
            .max(0.0)
            .min(rect.width.max(0.0) * 0.5)
            .min(rect.height.max(0.0) * 0.5);
        let border_w = style
            .chrome
            .border_width
            .max(0.0)
            .min(rect.width.max(0.0) * 0.5)
            .min(rect.height.max(0.0) * 0.5);
        let inset = (border_w + cx.one_px())
            .min(rect.width.max(0.0) * 0.5)
            .min(rect.height.max(0.0) * 0.5);
        // Keep every chrome band above section-container fills.
        cx.scene.push_rounded_rect(
            rect,
            style.chrome.border_color,
            RoundedCorners::uniform(radius),
            LAYER_CONTENT,
            None,
        );
        cx.scene.push_rounded_rect(
            RectF::new(
                rect.x + border_w,
                rect.y + border_w,
                (rect.width - border_w * 2.0).max(0.0),
                (rect.height - border_w * 2.0).max(0.0),
            ),
            style.inner_edge,
            RoundedCorners::uniform((radius - border_w).max(0.0)),
            LAYER_CONTENT + 1,
            None,
        );
        cx.scene.push_rounded_rect(
            RectF::new(
                rect.x + inset,
                rect.y + inset,
                (rect.width - inset * 2.0).max(0.0),
                (rect.height - inset * 2.0).max(0.0),
            ),
            style.chrome.fill.unwrap_or(t.colors.surfaces.content),
            RoundedCorners::uniform((radius - inset).max(0.0)),
            LAYER_CONTENT + 2,
            cx.scene_key,
        );
        emit_rect_count += 3;

        let font_size = t.typography.input_font_size;
        // SPEC-HAL-141 FIX: Compute text viewport from paint rect (absolute coords),
        // not from layout_rect (local coords). Event handlers use layout_rect (local),
        // but paint must use the absolute rect parameter.
        let text_viewport = self.text_viewport_from_rect(rect);
        let (raw_scroll_x, raw_scroll_y) = self.state.scroll.borrow().visual_offset();

        // K3.INV1: Unified vertical offset
        // Auto-height mode uses visual_content_origin_y (shifts when content grows above first visible line)
        // Scroll mode uses scroll_y from ScrollContainer
        let scroll_y = if self.is_auto_height_mode() {
            self.visual_content_origin_y.get()
        } else {
            raw_scroll_y
        };

        // S1.INV5: Horizontal scroll offset
        let scroll_x = raw_scroll_x;

        // Pixel-snap scroll offset to prevent per-line settling twitch.
        // Without this, each line's y position has a different fractional part,
        // causing independent floor() rounding in the renderer — lines snap
        // to physical pixels at different frames during smooth scroll.
        let scale_factor = cx.scene.scale_factor();
        let scroll_y = (scroll_y * scale_factor).round() / scale_factor;
        let scroll_x = (scroll_x * scale_factor).round() / scale_factor;

        // Begin clip
        cx.scene.begin_clip(text_viewport, None);

        // Determine selection range (if any)
        let has_selection =
            self.focused && self.state.selection.anchor != self.state.selection.head;
        let sel_start = self.state.selection.anchor.min(self.state.selection.head);
        let sel_end = self.state.selection.anchor.max(self.state.selection.head);

        let sel_color = style.selection;

        // SPEC-EXIT-A1: Text-native selection and caret rendering
        // Selection/caret are rendered as part of the text pass (backgrounds before glyphs)
        // This guarantees correct layering without z-index or submission order dependencies.
        let text = self.state.document.text();
        // Defensive: if Part 1's post-mutation rebuilds missed any site
        // (or a future mutation path forgets to rebuild), skip the
        // lines() read this frame to avoid the assert_fresh panic.
        // Text content not rendered for this frame; layout reruns next
        // frame and the normal path resumes — one-frame gap is
        // imperceptible.
        if self.layout_dirty.get() {
            cx.scene.end_clip();
            return;
        }
        let lines = self.state.layout_cache.lines();

        if !text.is_empty() {
            let mut y_offset = 0.0_f32;
            for (_line_idx, line) in lines.iter().enumerate() {
                let line_top = y_offset - scroll_y;

                // Draw each run in this line with text-native selection backgrounds
                let mut run_x = 0.0_f32;
                for run in &line.runs {
                    let run_text = text.get(run.range.clone())
                        .expect("UI-TYPOGRAPHY-INPUT: TextArea source range failed");
                    let run_start = run.range.start;
                    let run_end = run.range.end;

                    // Build background spans for this run (selection highlight)
                    let mut backgrounds = Vec::new();

                    // Check if this run overlaps with selection
                    if has_selection && run_end > sel_start && run_start < sel_end {
                        // Calculate selection overlap within this run
                        let overlap_start = sel_start.max(run_start);
                        let overlap_end = sel_end.min(run_end);

                        // SPEC-HAL-141: Calculate X positions using layout cache boundaries,
                        // not paint-phase text_engine.measure() calls.
                        // Boundaries have absolute x_logical from line start;
                        // subtract run_x to get positions relative to run.
                        let sel_x_start = if overlap_start == run_start {
                            0.0 // Relative to run start
                        } else {
                            // Use cached boundary x_logical instead of measure()
                            self.state.layout_cache.caret_x_for_byte(overlap_start) - run_x
                        };

                        let sel_x_end = if overlap_end == run_end {
                            run.width
                        } else {
                            // Use cached boundary x_logical instead of measure()
                            self.state.layout_cache.caret_x_for_byte(overlap_end) - run_x
                        };

                        let sel_width = sel_x_end - sel_x_start;
                        if sel_width > 0.01 {
                            // SPEC-EXIT-A1: Selection as text background span
                            backgrounds.push(TextBackgroundSpan {
                                x_offset: sel_x_start,
                                y_offset: self.selection_y_offset,
                                width: sel_width,
                                height: line.height,
                                color: sel_color,
                            });
                        }
                    }

                    // SPEC-EXIT-A1: Draw text with integrated selection backgrounds
                    if !run_text.is_empty() {
                        let glyph_run = self.state.layout_cache.shape_run(text, run, cx.text_engine)
                            .expect("UI-TYPOGRAPHY-INPUT: TextArea paint binding failed");
                        let ascent_offset = line.baseline - glyph_run.ascent;
                        for background in &mut backgrounds {
                            background.y_offset -= ascent_offset;
                        }
                        cx.text_engine.draw_with_backgrounds(
                            &glyph_run,
                            text_viewport.x + run_x - scroll_x,
                            text_viewport.y + line_top + ascent_offset,
                            // SPEC-HAL-002 §4.7: resolve glyph color from the live theme,
                            // not the cached run color (stale across a theme switch — the
                            // layout cache is not rebuilt on a theme-only repaint).
                            t.colors.typography.primary,
                            backgrounds,
                            LAYER_CONTENT + 4,
                            cx.scene,
                        );
                        emit_text_count += 1;
                    }
                    run_x += run.width;
                }

                // S1.INV2: Empty line selection marker.
                // If this line is within the selection but had no visible runs,
                // emit a small rect so blank lines show selection highlight.
                if has_selection && run_x < 0.01 {
                    // Compute line byte range from runs
                    let line_byte_start = line.runs.first().map(|r| r.range.start).unwrap_or(0);
                    let line_byte_end = line.runs.last().map(|r| r.range.end).unwrap_or(0);

                    // Check if this empty line overlaps with selection
                    // For empty lines, byte_start == byte_end, so check >= sel_start && < sel_end
                    let in_selection = if line_byte_start == line_byte_end {
                        line_byte_start >= sel_start && line_byte_start < sel_end
                    } else {
                        line_byte_end > sel_start && line_byte_start < sel_end
                    };

                    if in_selection {
                        let marker_rect = RectF::new(
                            text_viewport.x - scroll_x,
                            text_viewport.y + line_top + self.selection_y_offset,
                            self.empty_line_selection_width,
                            line.height,
                        );
                        cx.scene
                            .push_rect(marker_rect, sel_color, LAYER_CONTENT + 3, cx.scene_key);
                        emit_rect_count += 1;
                    }
                }

                y_offset += line.height;
            }
        } else if let Some(ref placeholder) = self.config.placeholder {
            if !placeholder.is_empty() {
                let mut color = t.colors.typography.muted;
                color.a = t.typography.placeholder_opacity;
                let mut placeholder_cache = TextLayoutCache::new(font_size, color);
                placeholder_cache.rebuild(placeholder, 0, Self::SINGLE_LINE_LAYOUT_WIDTH, cx.text_engine);
                let mut line_y = 0.0;
                for line in placeholder_cache.lines() {
                    let mut run_x = text_viewport.x - scroll_x;
                    for face_run in &line.runs {
                        let run = placeholder_cache.shape_run(placeholder, face_run, cx.text_engine)
                            .expect("UI-TYPOGRAPHY-INPUT: TextArea placeholder binding failed");
                        cx.text_engine.draw(&run, run_x, text_viewport.y + line_y + line.baseline - run.ascent,
                            color, LAYER_CONTENT + 4, cx.scene);
                        run_x += face_run.width;
                        emit_text_count += 1;
                    }
                    line_y += line.height;
                }
            }
        }

        // SPEC-EXIT-A1: Caret as text-native vertical bar
        // Rendered after all text nodes so it appears above glyphs
        let caret_emitted = cx.has_capture() && self.blink_caret_visible.get();
        if caret_emitted {
            let (_, canonical_x) = self.caret_position(self.state.cursor.pos);
            let caret_x = self.painted_caret_x(cx, rect, canonical_x);
            let caret_y = self.caret_content_y();
            let caret_top = caret_y - scroll_y;
            let caret_h = self.caret_height();

            // Caret rendered as a simple rect
            let caret_rect = RectF::new(
                text_viewport.x + caret_x - scroll_x,
                text_viewport.y + caret_top,
                t.typography.caret_thickness,
                caret_h,
            );
            // Caret must be above glyphs - use LAYER_OVERLAY to render after text
            cx.scene.push_rect(
                caret_rect,
                style.caret,
                crate::core::paint::LAYER_OVERLAY,
                cx.scene_key,
            );
            emit_rect_count += 1; // Caret
        }

        // End clip
        cx.scene.end_clip();

        // Count scrollbar rects that will be emitted by paint_scrollbars
        // (They emit rounded rects for visible thumbs)
        let scroll = self.state.scroll.borrow();
        let content_w = scroll.content_size.width;
        let content_h = scroll.content_size.height;
        let viewport_w = scroll.viewport_size.width;
        let viewport_h = scroll.viewport_size.height;
        drop(scroll);
        let scrollbar_v_visible = self.scrollbar_v.is_visible(content_h, viewport_h);
        let scrollbar_h_visible = self.scrollbar_h.is_visible(content_w, viewport_w);
        let emit_scrollbar_rect_count =
            (if scrollbar_v_visible { 1 } else { 0 }) + (if scrollbar_h_visible { 1 } else { 0 });

        // HAL_TEXTAREA_PAINT_DIAG: Log primitive emission counts
        if diag_enabled {
            let scroll = self.state.scroll.borrow();
            let (scroll_x_val, scroll_y_val) = scroll.visual_offset();
            drop(scroll);
            eprintln!(
                "[TEXTAREA_PAINT] focused={} blink_visible={} has_sel={} scroll=({:.1},{:.1}) rect={} text={} scrollbar_rect={} caret_emitted={}",
                self.focused,
                self.blink_caret_visible.get(),
                has_selection,
                scroll_x_val,
                scroll_y_val,
                emit_rect_count,
                emit_text_count,
                emit_scrollbar_rect_count,
                caret_emitted,
            );
        }

        // SPEC-HAL-XXX: Paint scrollbars AFTER text clip ends
        // Scrollbars are overlays - they must NOT be clipped by text_viewport.
        // Painting them here (after end_clip but still in paint()) ensures
        // they're outside the text clip but in the same scene context.
        self.paint_scrollbars(cx, rect);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let observe = phase == EventPhase::Target && self.smooth_caret_enabled;
        let revision = self.state.document.version();
        let scroll_before = self.state.scroll.borrow().target_offset();
        let navigation = observe && cx.keyboard_mode == KeyboardAuthorityMode::Capture
            && matches!(event, Event::Key(k) if k.is_down && !k.is_repeat
                && !k.modifiers.shift && !k.modifiers.ctrl && !k.modifiers.alt && !k.modifiers.meta
                && matches!(k.key_code, KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End));
        let snap = observe && match event {
            Event::Ime(_) => true,
            Event::Key(k) if k.is_down => k.is_repeat
                || (matches!(k.key_code, KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
                    | KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown) && !navigation),
            Event::Pointer(p) => matches!(p.kind, PointerEventKind::DoubleClick | PointerEventKind::TripleClick)
                || (matches!(p.kind, PointerEventKind::Down) && p.button == Some(PointerButton::Primary))
                || self.state.drag_mode != TextAreaDragMode::None
                || self.scrollbar_v_dragging || self.scrollbar_h_dragging,
            _ => false,
        };
        if snap { self.snap_smooth_caret(); }
        // No source scan before the original centralized editing-policy decision.
        let before = if navigation && !self.smooth_caret.snap {
            self.smooth_caret.pending.or_else(|| self.smooth_caret.projection.as_ref().map(|p| p.destination))
                .filter(|d| d.revision == revision && d.cursor == self.state.cursor.pos)
        } else { None };
        let handled = (|| {
        if phase != EventPhase::Target {
            return false;
        }

        if matches!(event, Event::Key(_) | Event::Text(_) | Event::Ime(_))
            && cx.keyboard_mode != KeyboardAuthorityMode::Capture
        {
            return false;
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

        // Request continuous frames while focused to ensure caret blink updates
        // This must be called every frame (flag is per-frame and reset by event router)
        if self.focused {
            cx.request_continuous_frames();
        }

        // Reset caret to visible on any user activity (event phase, not paint phase)
        self.reset_caret_blink_on_activity(cx);

        match event {
            Event::Pointer(pe) => {
                // Handle scrollbar drag continuation first (we have pointer capture)
                if self.scrollbar_v_dragging {
                    match pe.kind {
                        PointerEventKind::Move => {
                            return self.handle_scrollbar_drag(pe);
                        }
                        PointerEventKind::Up => {
                            self.end_scrollbar_v_drag();
                            cx.release_pointer();
                            return true;
                        }
                        _ => {}
                    }
                }

                // Handle horizontal scrollbar drag continuation
                if self.scrollbar_h_dragging {
                    match pe.kind {
                        PointerEventKind::Move => {
                            return self.handle_scrollbar_h_drag(pe);
                        }
                        PointerEventKind::Up => {
                            self.end_scrollbar_h_drag();
                            cx.release_pointer();
                            return true;
                        }
                        _ => {}
                    }
                }

                if self.config.is_enabled
                    && !self.config.is_read_only
                    && self
                        .text_viewport()
                        .contains(Point::new(pe.local_pos.0, pe.local_pos.1))
                {
                    cx.request_cursor(crate::core::event::CursorIcon::Text);
                }

                let _text_viewport = self.text_viewport();
                // Hit-test uses target_offset for pointer → content mapping.
                // Target (logical) position is used because clicking should select
                // content at the position it will end up, not where it's currently
                // animating from.
                let (scroll_x, scroll_y) = self.state.scroll.borrow().target_offset();
                // Use cached padding (synced from theme in layout)
                let padding = self.padding;

                match pe.kind {
                    PointerEventKind::Down => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }

                        // Check scrollbar hits first before text selection
                        // Check horizontal first (bottom bar), then vertical (right bar)
                        if self.handle_scrollbar_h_down(cx, pe) {
                            return true;
                        }
                        if self.handle_scrollbar_down(cx, pe) {
                            return true;
                        }

                        cx.request_keyboard_capture();
                        self.focused = true;
                        cx.capture_pointer(PointerButton::Primary);

                        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                        if self.layout_dirty.get() {
                            self.rebuild_layout_cache_now(cx.text_engine());
                        }

                        // Hit test with proper multi-line support
                        let boundaries = self.state.layout_cache.boundaries();
                        let lines = self.state.layout_cache.lines();
                        let x_logical = pe.local_pos.0 - padding + scroll_x;
                        let y_logical = pe.local_pos.1 - padding + scroll_y;
                        let hit_pos = hit_test(boundaries, lines, x_logical, y_logical);

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
                        self.state.drag_mode = TextAreaDragMode::Character {
                            anchor: self.state.selection.anchor,
                        };

                        self.reset_caret_blink(cx);
                        true
                    }
                    PointerEventKind::Move => {
                        // Update scrollbar hover state
                        self.update_scrollbar_hover(pe);

                        if let TextAreaDragMode::Character { anchor } = self.state.drag_mode {
                            let text_vp = self.text_viewport();

                            // Compute vertical auto-scroll velocity based on pointer position
                            // Positive velocity = scroll down (pointer below viewport)
                            // Negative velocity = scroll up (pointer above viewport)
                            let pointer_y = pe.local_pos.1;
                            let viewport_top = text_vp.y;
                            let viewport_bottom = text_vp.y + text_vp.height;

                            let velocity_y = if pointer_y < viewport_top {
                                // Pointer above viewport - scroll up
                                let distance = viewport_top - pointer_y;
                                let factor =
                                    (distance / Self::AUTO_SCROLL_DISTANCE_THRESHOLD).min(1.0);
                                let speed = Self::AUTO_SCROLL_BASE_SPEED
                                    + factor
                                        * (Self::AUTO_SCROLL_MAX_SPEED
                                            - Self::AUTO_SCROLL_BASE_SPEED);
                                -speed
                            } else if pointer_y > viewport_bottom {
                                // Pointer below viewport - scroll down
                                let distance = pointer_y - viewport_bottom;
                                let factor =
                                    (distance / Self::AUTO_SCROLL_DISTANCE_THRESHOLD).min(1.0);
                                let speed = Self::AUTO_SCROLL_BASE_SPEED
                                    + factor
                                        * (Self::AUTO_SCROLL_MAX_SPEED
                                            - Self::AUTO_SCROLL_BASE_SPEED);
                                speed
                            } else {
                                // Pointer inside viewport vertically - no vertical auto-scroll
                                0.0
                            };

                            // Compute horizontal auto-scroll velocity based on pointer position
                            // Positive velocity = scroll right (pointer right of viewport)
                            // Negative velocity = scroll left (pointer left of viewport)
                            let pointer_x = pe.local_pos.0;
                            let viewport_left = text_vp.x;
                            let viewport_right = text_vp.x + text_vp.width;

                            let velocity_x = if pointer_x < viewport_left {
                                // Pointer left of viewport - scroll left
                                let distance = viewport_left - pointer_x;
                                let factor =
                                    (distance / Self::AUTO_SCROLL_DISTANCE_THRESHOLD).min(1.0);
                                let speed = Self::AUTO_SCROLL_BASE_SPEED
                                    + factor
                                        * (Self::AUTO_SCROLL_MAX_SPEED
                                            - Self::AUTO_SCROLL_BASE_SPEED);
                                -speed
                            } else if pointer_x > viewport_right {
                                // Pointer right of viewport - scroll right
                                let distance = pointer_x - viewport_right;
                                let factor =
                                    (distance / Self::AUTO_SCROLL_DISTANCE_THRESHOLD).min(1.0);
                                let speed = Self::AUTO_SCROLL_BASE_SPEED
                                    + factor
                                        * (Self::AUTO_SCROLL_MAX_SPEED
                                            - Self::AUTO_SCROLL_BASE_SPEED);
                                speed
                            } else {
                                // Pointer inside viewport horizontally - no horizontal auto-scroll
                                0.0
                            };

                            // Store velocities for update() to apply each frame
                            self.auto_scroll_velocity_y.set(velocity_y);
                            self.auto_scroll_velocity_x.set(velocity_x);

                            // Request continuous frames to drive auto-scroll in update()
                            if velocity_y.abs() > 0.1 || velocity_x.abs() > 0.1 {
                                cx.request_continuous_frames();
                            }

                            // Anticipate scroll delta that will be applied in update() this frame.
                            // This eliminates selection-scroll phase skew: selection observes the
                            // same effective scroll position that will be used for rendering.
                            let pending_scroll_delta_y =
                                velocity_y * self.last_frame_dt.get() * 60.0;
                            let pending_scroll_delta_x =
                                velocity_x * self.last_frame_dt.get() * 60.0;
                            let effective_scroll_y = scroll_y + pending_scroll_delta_y;
                            let effective_scroll_x = scroll_x + pending_scroll_delta_x;

                            // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                            if self.layout_dirty.get() {
                                self.rebuild_layout_cache_now(cx.text_engine());
                            }

                            let boundaries = self.state.layout_cache.boundaries();
                            let lines = self.state.layout_cache.lines();
                            let x_logical = pe.local_pos.0 - padding + effective_scroll_x;
                            let y_logical = pe.local_pos.1 - padding + effective_scroll_y;
                            let hit_pos = hit_test(boundaries, lines, x_logical, y_logical);

                            self.state.selection.anchor = anchor;
                            self.state.selection.head = hit_pos;
                            self.state.cursor.pos = hit_pos;

                            self.reset_caret_blink(cx);
                            true
                        } else {
                            // Clear auto-scroll when not dragging
                            self.auto_scroll_velocity_y.set(0.0);
                            self.auto_scroll_velocity_x.set(0.0);
                            false
                        }
                    }
                    PointerEventKind::Up => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        self.state.drag_mode = TextAreaDragMode::None;
                        // Stop auto-scroll immediately on button release
                        self.auto_scroll_velocity_y.set(0.0);
                        self.auto_scroll_velocity_x.set(0.0);
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
                        let boundaries = self.state.layout_cache.boundaries();
                        let lines = self.state.layout_cache.lines();
                        let text = self.state.document.text();
                        let x_logical = pe.local_pos.0 - padding + scroll_x;
                        let y_logical = pe.local_pos.1 - padding + scroll_y;
                        let hit_pos = hit_test(boundaries, lines, x_logical, y_logical);
                        let word = find_word_at(boundaries, text, hit_pos);

                        self.state.selection.anchor = word.start;
                        self.state.selection.head = word.end;
                        self.state.cursor.pos = word.end;
                        self.state.drag_mode = TextAreaDragMode::None;

                        self.reset_caret_blink(cx);
                        true
                    }
                    PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                        if self.layout_dirty.get() {
                            self.rebuild_layout_cache_now(cx.text_engine());
                        }
                        // Triple-click: select entire line
                        let boundaries = self.state.layout_cache.boundaries();
                        let lines = self.state.layout_cache.lines();
                        let x_logical = pe.local_pos.0 - padding + scroll_x;
                        let y_logical = pe.local_pos.1 - padding + scroll_y;
                        let hit_pos = hit_test(boundaries, lines, x_logical, y_logical);

                        // Use line selection from substrate
                        let (sel, cursor) =
                            crate::core::editing::triple_click_selection(boundaries, hit_pos);
                        self.state.selection = sel;
                        self.state.cursor = cursor;
                        self.state.drag_mode = TextAreaDragMode::None;

                        self.reset_caret_blink(cx);
                        true
                    }
                    PointerEventKind::Scroll { delta_x, delta_y } => {
                        // SPEC-HAL-109 Stage F: Policy-controlled scroll
                        // Delta values are already in logical pixels (no multiplier needed)
                        //
                        // Scroll direction mapping:
                        // - Shift+wheel OR delta_x: horizontal scroll
                        // - Otherwise: vertical scroll
                        //
                        // Only consume (return true) if scrollbar is visible and can scroll.
                        let scroll = self.state.scroll.borrow();
                        let content_w = scroll.content_size.width;
                        let content_h = scroll.content_size.height;
                        let viewport_w = scroll.viewport_size.width;
                        let viewport_h = scroll.viewport_size.height;
                        drop(scroll);

                        let h_visible = self.scrollbar_h.is_visible(content_w, viewport_w);
                        let v_visible = self.scrollbar_v.is_visible(content_h, viewport_h);

                        let shift_held = pe.modifiers.shift;

                        if shift_held || delta_x.abs() > delta_y.abs() {
                            // Horizontal scroll: only consume if h_scrollbar is visible
                            if h_visible {
                                let dx = if delta_x.abs() > 0.1 {
                                    delta_x
                                } else {
                                    delta_y
                                };
                                self.state.scroll.borrow_mut().scroll_by(-dx, 0.0);
                                true
                            } else {
                                false // Let parent handle scroll
                            }
                        } else if self.editing_policy.allow_vertical_scroll && v_visible {
                            // Vertical scroll: only consume if v_scrollbar is visible
                            self.state.scroll.borrow_mut().scroll_by(0.0, -delta_y);
                            true
                        } else {
                            false
                        }
                    }
                    // Stage 45B: DragStart not used by TextArea
                    PointerEventKind::DragStart => false,
                }
            }
            // Stage A+B: Pure delegation to canonical navigation functions
            // NO custom logic - all navigation math lives in editing.rs
            // Modifier priority: Shift+Ctrl > Ctrl > Shift > None
            Event::Key(KeyEvent {
                key_code,
                is_down: true,
                modifiers,
                ..
            }) if cx.keyboard_mode == KeyboardAuthorityMode::Capture => {
                // PERF: Early exit for key codes not handled by this match.
                // During key repeat, the OS sends both KeyDown and Text events.
                // Printable character KeyDown events fall through to `_ => {}`
                // and never use boundaries — but without this guard, each one
                // triggers a full O(n) cache rebuild that is immediately wasted.
                let handled_key = matches!(
                    key_code,
                    KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Home
                        | KeyCode::End
                        | KeyCode::PageUp
                        | KeyCode::PageDown
                        | KeyCode::Enter
                        | KeyCode::Backspace
                        | KeyCode::Delete
                        | KeyCode::Space
                );
                if !handled_key {
                    return false;
                }

                // PERF: Only rebuild cache for keys that use boundaries.
                // Space, Enter, Ctrl+Home/End, Shift+Ctrl+Home/End operate on
                // cursor.pos directly and never read cluster boundaries.
                let needs_boundaries = matches!(
                    key_code,
                    KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Home
                        | KeyCode::End
                        | KeyCode::PageUp
                        | KeyCode::PageDown
                        | KeyCode::Backspace
                        | KeyCode::Delete
                );
                if needs_boundaries && self.layout_dirty.get() {
                    self.rebuild_layout_cache_now(cx.text_engine());
                }
                let boundaries = if needs_boundaries {
                    self.state.layout_cache.boundaries()
                } else {
                    &[]
                };

                let mut cursor_moved = false;
                let shift = modifiers.shift;
                let ctrl = modifiers.ctrl;

                match (key_code, shift, ctrl) {
                    // ============================================================
                    // Shift+Ctrl: Block movement with selection extension
                    // SPEC-HAL-109 Stage F: Policy-controlled vertical navigation
                    // ============================================================
                    (KeyCode::Up, true, true) if self.editing_policy.allow_vertical_navigation => {
                        ctrl_move_up_select(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Down, true, true)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        ctrl_move_down_select(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Home, true, true) => {
                        // Shift+Ctrl+Home: extend selection to document start
                        if self.state.selection.anchor == self.state.selection.head {
                            self.state.selection.anchor = self.state.cursor.pos;
                        }
                        self.state.cursor.pos = 0;
                        self.state.cursor.preferred_x = None;
                        self.state.selection.head = 0;
                        cursor_moved = true;
                    }
                    (KeyCode::End, true, true) => {
                        // Shift+Ctrl+End: extend selection to document end
                        if self.state.selection.anchor == self.state.selection.head {
                            self.state.selection.anchor = self.state.cursor.pos;
                        }
                        let doc_end = self.state.document.len();
                        self.state.cursor.pos = doc_end;
                        self.state.cursor.preferred_x = None;
                        self.state.selection.head = doc_end;
                        cursor_moved = true;
                    }

                    // ============================================================
                    // Ctrl: Block movement (paragraph boundaries)
                    // SPEC-HAL-109 Stage F: Policy-controlled vertical navigation
                    // ============================================================
                    (KeyCode::Up, false, true) if self.editing_policy.allow_vertical_navigation => {
                        ctrl_move_up(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Down, false, true)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        ctrl_move_down(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Home, false, true) => {
                        // Ctrl+Home: move to document start
                        self.state.cursor.pos = 0;
                        self.state.cursor.preferred_x = None;
                        self.state.selection.anchor = 0;
                        self.state.selection.head = 0;
                        cursor_moved = true;
                    }
                    (KeyCode::End, false, true) => {
                        // Ctrl+End: move to document end
                        let doc_end = self.state.document.len();
                        self.state.cursor.pos = doc_end;
                        self.state.cursor.preferred_x = None;
                        self.state.selection.anchor = doc_end;
                        self.state.selection.head = doc_end;
                        cursor_moved = true;
                    }

                    // ============================================================
                    // Left/Right: Word navigation + character navigation
                    // Word-mod: Alt on macOS, Ctrl on non-macOS
                    // ============================================================
                    (KeyCode::Left, _, _) => {
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
                    (KeyCode::Right, _, _) => {
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

                    // ============================================================
                    // Shift: Selection extension (Up/Down/Home/End)
                    // SPEC-HAL-109 Stage F: Policy-controlled vertical navigation
                    // ============================================================
                    (KeyCode::Up, true, false) if self.editing_policy.allow_vertical_navigation => {
                        move_up_select(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Down, true, false)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        move_down_select(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Home, true, false) => {
                        delegate::handle_home_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            true,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::End, true, false) => {
                        delegate::handle_end_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            true,
                        );
                        cursor_moved = true;
                    }

                    // ============================================================
                    // No modifiers: Basic navigation (Up/Down/Home/End)
                    // Note: Left/Right handled above with word navigation
                    // SPEC-HAL-109 Stage F: Policy-controlled vertical navigation
                    // ============================================================
                    (KeyCode::Up, false, false)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        move_up(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Down, false, false)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        move_down(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::Home, false, false) => {
                        delegate::handle_home_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            false,
                        );
                        cursor_moved = true;
                    }
                    (KeyCode::End, false, false) => {
                        delegate::handle_end_key(
                            &mut self.state.cursor,
                            &mut self.state.selection,
                            boundaries,
                            false,
                        );
                        cursor_moved = true;
                    }
                    // ============================================================
                    // PageUp / PageDown: Move cursor by viewport height
                    // ============================================================
                    (KeyCode::PageUp, false, false)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        let line_h = self.ascent + self.descent;
                        let lines_per_page = ((self.text_viewport().height / line_h).floor()
                            as usize)
                            .max(1)
                            .saturating_sub(1)
                            .max(1);
                        for _ in 0..lines_per_page {
                            move_up(
                                &mut self.state.cursor,
                                &mut self.state.selection,
                                boundaries,
                            );
                        }
                        cursor_moved = true;
                    }
                    (KeyCode::PageDown, false, false)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        let line_h = self.ascent + self.descent;
                        let lines_per_page = ((self.text_viewport().height / line_h).floor()
                            as usize)
                            .max(1)
                            .saturating_sub(1)
                            .max(1);
                        for _ in 0..lines_per_page {
                            move_down(
                                &mut self.state.cursor,
                                &mut self.state.selection,
                                boundaries,
                            );
                        }
                        cursor_moved = true;
                    }
                    (KeyCode::PageUp, true, false)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        let line_h = self.ascent + self.descent;
                        let lines_per_page = ((self.text_viewport().height / line_h).floor()
                            as usize)
                            .max(1)
                            .saturating_sub(1)
                            .max(1);
                        for _ in 0..lines_per_page {
                            move_up_select(
                                &mut self.state.cursor,
                                &mut self.state.selection,
                                boundaries,
                            );
                        }
                        cursor_moved = true;
                    }
                    (KeyCode::PageDown, true, false)
                        if self.editing_policy.allow_vertical_navigation =>
                    {
                        let line_h = self.ascent + self.descent;
                        let lines_per_page = ((self.text_viewport().height / line_h).floor()
                            as usize)
                            .max(1)
                            .saturating_sub(1)
                            .max(1);
                        for _ in 0..lines_per_page {
                            move_down_select(
                                &mut self.state.cursor,
                                &mut self.state.selection,
                                boundaries,
                            );
                        }
                        cursor_moved = true;
                    }
                    // ============================================================
                    // Editing keys (ignore modifiers for these)
                    // ============================================================
                    (KeyCode::Enter, _, _) if self.editing_policy.allow_newline => {
                        // SPEC-HAL-109 Stage F: Policy-controlled newline insertion
                        #[cfg(debug_assertions)]
                        let pre_doc_len = self.state.document.len();
                        #[cfg(debug_assertions)]
                        let pre_cursor_pos = self.state.cursor.pos;

                        let edit_kind = delegate::edit_kind_for_insert(&self.state.selection);
                        let before = self.capture_snapshot();
                        let timestamp = cx.frame_time_ms();
                        delegate::perform_text_insert(
                            self.state.document.content_mut(),
                            &mut self.state.selection,
                            &mut self.state.cursor,
                            boundaries,
                            "\n",
                        );
                        self.record_edit(before, edit_kind, timestamp);
                        self.state.document.mark_modified();
                        self.layout_dirty.set(true);
                        self.state.layout_cache.mark_stale();
                        self.rebuild_layout_cache_now(cx.text_engine());
                        cx.invalidate_layout();
                        // Typing MUST trigger focus visibility (caret follow)
                        self.needs_focus_visibility_update.set(true);

                        #[cfg(debug_assertions)]
                        {
                            let post_doc_len = self.state.document.len();
                            let post_cursor_pos = self.state.cursor.pos;
                            debug_assert!(
                                post_doc_len == pre_doc_len + 1,
                                "[TextArea Enter] POST: doc_len did not increase by 1! pre={}, post={}",
                                pre_doc_len,
                                post_doc_len
                            );
                            debug_assert!(
                                post_cursor_pos == pre_cursor_pos + 1,
                                "[TextArea Enter] POST: cursor did not advance by 1! pre={}, post={}",
                                pre_cursor_pos,
                                post_cursor_pos
                            );
                        }

                        cursor_moved = true;
                    }
                    (KeyCode::Backspace, _, _) => {
                        let edit_kind = delegate::edit_kind_for_backspace(&self.state.selection);
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
                        self.record_edit(before, edit_kind, timestamp);
                        self.state.document.mark_modified();
                        self.layout_dirty.set(true);
                        self.state.layout_cache.mark_stale();
                        self.rebuild_layout_cache_now(cx.text_engine());
                        cx.invalidate_layout();
                        cursor_moved = true;
                    }
                    (KeyCode::Delete, _, _) => {
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
                        self.record_edit(before, edit_kind, timestamp);
                        self.state.document.mark_modified();
                        self.layout_dirty.set(true);
                        self.state.layout_cache.mark_stale();
                        self.rebuild_layout_cache_now(cx.text_engine());
                        cx.invalidate_layout();
                        cursor_moved = true;
                    }
                    // Space is treated as text input (prevents activation fallback)
                    (KeyCode::Space, _, _) => {
                        let edit_kind = delegate::edit_kind_for_insert(&self.state.selection);
                        let before = self.capture_snapshot();
                        let timestamp = cx.frame_time_ms();
                        delegate::perform_text_insert(
                            self.state.document.content_mut(),
                            &mut self.state.selection,
                            &mut self.state.cursor,
                            boundaries,
                            " ",
                        );
                        self.record_edit(before, edit_kind, timestamp);
                        self.state.document.mark_modified();
                        self.layout_dirty.set(true);
                        self.state.layout_cache.mark_stale();
                        self.rebuild_layout_cache_now(cx.text_engine());
                        cx.invalidate_layout();
                        // Typing MUST trigger focus visibility (caret follow)
                        self.needs_focus_visibility_update.set(true);
                        self.reset_caret_blink(cx);
                        return true;
                    }
                    _ => {}
                }

                if cursor_moved {
                    self.reset_caret_blink(cx);
                    self.update_focus_visibility();
                }
                cursor_moved
            }
            Event::Text(text_event) if cx.keyboard_mode == KeyboardAuthorityMode::Capture => {
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

                // Undo: Cmd/Ctrl + Z
                if shortcut_mod
                    && !text_event.modifiers.shift
                    && (text_event.text == "z" || text_event.text == "Z")
                {
                    self.perform_undo(cx);
                    self.update_focus_visibility();
                    return true;
                }

                // Preserve native redo and admit both browser conventions.
                if crate::core::text_edit_policy::is_redo_shortcut(
                    text_event.modifiers,
                    &text_event.text,
                ) {
                    self.perform_redo(cx);
                    self.update_focus_visibility();
                    return true;
                }

                // Copy: Cmd/Ctrl + C
                if shortcut_mod && (text_event.text == "c" || text_event.text == "C") {
                    if let Some(selected_text) = delegate::get_selection_text(
                        self.state.document.text(),
                        &self.state.selection,
                    ) {
                        cx.clipboard_set_text(selected_text);
                    }
                    return true;
                }

                // Cut: Cmd/Ctrl + X
                if shortcut_mod && (text_event.text == "x" || text_event.text == "X") {
                    if let Some(selected_text) = delegate::get_selection_text(
                        self.state.document.text(),
                        &self.state.selection,
                    ) {
                        cx.clipboard_set_text(selected_text);

                        let before = self.capture_snapshot();
                        let timestamp = cx.frame_time_ms();
                        // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                        if self.layout_dirty.get() {
                            self.rebuild_layout_cache_now(cx.text_engine());
                        }
                        let boundaries = self.state.layout_cache.boundaries();
                        delegate::perform_cut(
                            self.state.document.content_mut(),
                            &mut self.state.selection,
                            &mut self.state.cursor,
                            boundaries,
                        );
                        self.record_edit(before, EditKind::Cut, timestamp);
                        self.state.document.mark_modified();
                        self.layout_dirty.set(true);
                        self.state.layout_cache.mark_stale();
                        self.rebuild_layout_cache_now(cx.text_engine());
                        cx.invalidate_layout();
                        self.update_focus_visibility();
                        self.reset_caret_blink(cx);
                    }
                    return true;
                }

                // Paste: Cmd/Ctrl + V
                // SPEC-HAL-109 Stage F: Policy-controlled newline normalization
                if shortcut_mod && (text_event.text == "v" || text_event.text == "V") {
                    if let Some(clipboard_text) = cx.clipboard_get_text() {
                        let paste_text = if self.editing_policy.normalize_paste_newlines {
                            delegate::normalize_paste_newlines(clipboard_text)
                        } else {
                            clipboard_text
                        };

                        if !paste_text.is_empty() {
                            let before = self.capture_snapshot();
                            let timestamp = cx.frame_time_ms();
                            // INV-TEXT-GEOM-PHASE: Rebuild cache before geometry read
                            if self.layout_dirty.get() {
                                self.rebuild_layout_cache_now(cx.text_engine());
                            }
                            let boundaries = self.state.layout_cache.boundaries();
                            delegate::perform_paste(
                                self.state.document.content_mut(),
                                &mut self.state.selection,
                                &mut self.state.cursor,
                                boundaries,
                                &paste_text,
                            );
                            self.record_edit(before, EditKind::Paste, timestamp);
                            self.state.document.mark_modified();
                            self.layout_dirty.set(true);
                            self.state.layout_cache.mark_stale();
                            self.rebuild_layout_cache_now(cx.text_engine());
                            cx.invalidate_layout();
                            // Typing MUST trigger focus visibility (caret follow)
                            self.needs_focus_visibility_update.set(true);
                            self.reset_caret_blink(cx);
                        }
                    }
                    return true;
                }

                // Skip other modifier combos
                if text_event.modifiers.ctrl || text_event.modifiers.meta {
                    return false;
                }

                // Regular text input
                let edit_kind = delegate::edit_kind_for_insert(&self.state.selection);
                let before = self.capture_snapshot();
                let timestamp = cx.frame_time_ms();
                // PERF: apply_insert_op does NOT use boundaries (documented as "unused,
                // for API consistency" in editing.rs). Skip cache rebuild to avoid O(n)
                // full reshaping on each char during key repeat. The single layout()
                // pass after event handling will do the necessary reshaping.
                delegate::perform_text_insert(
                    self.state.document.content_mut(),
                    &mut self.state.selection,
                    &mut self.state.cursor,
                    &[],
                    &text_event.text,
                );
                self.record_edit(before, edit_kind, timestamp);
                self.state.document.mark_modified();
                self.layout_dirty.set(true);
                self.state.layout_cache.mark_stale();
                self.rebuild_layout_cache_now(cx.text_engine());
                cx.invalidate_layout();
                // Typing MUST trigger focus visibility (caret follow)
                self.needs_focus_visibility_update.set(true);
                self.reset_caret_blink(cx);
                true
            }
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
                        let _ = text;
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
                        self.layout_dirty.set(true);
                        self.state.layout_cache.mark_stale();
                        self.rebuild_layout_cache_now(cx.text_engine());
                        cx.invalidate_layout();
                        // Typing MUST trigger focus visibility (caret follow)
                        self.needs_focus_visibility_update.set(true);
                        self.reset_caret_blink(cx);
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
        })();
        if observe {
            if revision != self.state.document.version() || self.has_active_selection()
                || scroll_before != self.state.scroll.borrow().target_offset() {
                self.snap_smooth_caret();
            } else if navigation && handled && !self.smooth_caret.snap && before.is_some() && self.motion_idle() {
                match (before, self.smooth_destination()) {
                    (Some(a), Some(b)) if a.revision == b.revision && a.line == b.line
                        && a.start == b.start && a.end == b.end && a.font == b.font => {
                        if a.x != b.x { self.smooth_caret.pending = Some(b); }
                    }
                    _ => self.snap_smooth_caret(),
                }
            }
        }
        handled
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        self.config.is_enabled
    }

    fn needs_animation(&self) -> bool {
        (self.config.is_enabled && self.caret_animation_demand.unwrap_or(self.focused))
            || (self.config.is_enabled && self.state.scroll.borrow().needs_animation())
            || self.smooth_caret.handle.is_some()
    }

    fn wants_scroll(&self) -> bool {
        self.config.is_enabled
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.focused = true;
        self.blink_caret_visible.set(true);
        self.blink_last_ms.set(cx.frame_time_ms());
        self.blink_accumulated_secs.set(0.0); // Reset dt-based timer
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.snap_smooth_caret();
        self.focused = false;
        self.blink_caret_visible.set(false);
        self.state.drag_mode = TextAreaDragMode::None;
        self.state.clear_composition();
        self.state.undo_history.resume();
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        self.hover = true;
        self.reset_caret_blink_on_activity(cx);
    }

    fn on_hover_leave(&mut self, _cx: &mut EventContext) {
        self.hover = false;
    }

    fn paint_overlay(&self, _cx: &mut PaintContext, _rect: RectF, _tree_widget_id: TreeWidgetId) {
        // SPEC-HAL-XXX: Scrollbars are now painted in paint() after end_clip().
        // This ensures they're outside the text_viewport clip.
        // paint_overlay is intentionally empty for TextArea.
    }

    /// SPEC-HAL-131A: Return self as AnchorResolver when layout cache is valid.
    ///
    /// Returns Some(self) only when:
    /// - layout cache is valid (layout_dirty is false)
    /// - line metrics are available (lines is not empty)
    ///
    /// Returns None otherwise, falling back to absolute scrolling.
    fn as_anchor_resolver(&self) -> Option<&dyn AnchorResolver> {
        // Cache must be fresh and have valid line metrics
        if !self.layout_dirty.get() && !self.state.layout_cache.lines().is_empty() {
            Some(self)
        } else {
            None
        }
    }

    fn touch_target(&self, _hit: crate::core::event::TouchHit) -> Option<crate::core::event::TouchTarget> {
        self.config.is_enabled.then_some(crate::core::event::TouchTarget::Editor)
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, tap: crate::core::event::TouchTap) -> bool {
        if self.layout_dirty.get() {
            self.rebuild_layout_cache_now(cx.text_engine());
        }
        let (scroll_x, scroll_y) = self.state.scroll.borrow().target_offset();
        let hit = hit_test(
            self.state.layout_cache.boundaries(),
            self.state.layout_cache.lines(),
            tap.local_position.x - self.padding + scroll_x,
            tap.local_position.y - self.padding + scroll_y,
        );
        self.state.cursor.pos = hit;
        self.state.cursor.preferred_x = None;
        self.state.selection = Selection::at(hit);
        self.state.drag_mode = TextAreaDragMode::None;
        self.reset_caret_blink(cx);
        cx.invalidate_paint();
        true
    }

    fn capture_eligible(&self) -> bool {
        self.config.is_enabled
    }

    fn on_capture_enter(&mut self, cx: &mut EventContext) {
        self.focused = true;
        self.reset_caret_blink_on_activity(cx);
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        self.snap_smooth_caret();
        // Keep focused — capture release doesn't blur
    }

    fn debug_type_name(&self) -> &'static str {
        "TextArea"
    }
}

// ============================================================================
// SPEC-HAL-131A: AnchorResolver Implementation
// ============================================================================

impl AnchorResolver for TextArea {
    /// Resolve anchor to logical position in content space.
    ///
    /// Anchor target_id.0 = logical line index.
    /// Returns Point(0.0, line_y(index) + target_offset.y).
    /// Returns None if index >= line_count.
    fn resolve_anchor(&self, anchor: &ScrollAnchor) -> Option<Point> {
        let line_index = anchor.target_id.0 as usize;
        let lines = self.state.layout_cache.lines();

        // Target line no longer exists
        if line_index >= lines.len() {
            return None;
        }

        // Compute Y position: line_y(index) + offset within line
        let y = self.state.layout_cache.line_y(line_index) + anchor.target_offset.y;

        // X is always 0.0 for vertical text scrolling
        Some(Point::new(0.0, y))
    }

    /// Find nearest valid anchor when target line is missing.
    ///
    /// Clamps to valid line range, preserving viewport_offset.
    /// Returns None if no lines exist (empty document).
    fn find_nearest_anchor(&self, anchor: &ScrollAnchor) -> Option<ScrollAnchor> {
        let line_count = self.state.layout_cache.lines().len();
        if line_count == 0 {
            return None;
        }

        let line_index = anchor.target_id.0 as usize;

        // Clamp to valid range
        let clamped_index = line_index.min(line_count - 1);

        // If already valid, return None (caller should use resolve_anchor)
        if clamped_index == line_index {
            return None;
        }

        // Return new anchor with clamped index, preserving viewport_offset
        Some(ScrollAnchor::new(
            AnchorId::new(clamped_index as u64),
            anchor.target_offset,
            anchor.viewport_offset,
        ))
    }
}

// ============================================================================
// Test Harness (SPEC-HAL-109 Stage J: Consolidated)
// ============================================================================

#[cfg(any(test, feature = "test-harness"))]
use crate::widgets::text_widget_test_harness::{TextWidgetTestHarness, TextWidgetTestHarnessRead};

#[cfg(any(test, feature = "test-harness"))]
impl TextWidgetTestHarness for TextArea {
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
impl TextWidgetTestHarnessRead for TextArea {
    fn cursor_ref(&self) -> &TextCursor {
        &self.state.cursor
    }

    fn selection_ref(&self) -> &Selection {
        &self.state.selection
    }
}

/// TextArea-specific test helpers (not shared with TextField).
#[cfg(any(test, feature = "test-harness"))]
impl TextArea {
    /// Get mutable access to internal state for testing.
    pub fn state_mut(&mut self) -> &mut TextAreaState {
        &mut self.state
    }

    /// Mark layout as dirty for testing anchor resolver conditions.
    pub fn mark_layout_dirty_for_test(&mut self) {
        self.layout_dirty.set(true);
    }

    /// Replace config for testing policy behavior.
    /// No validation, no derived state recomputation.
    pub fn set_config_for_test(&mut self, config: TextAreaConfig) {
        self.config = config;
    }

    /// Get cursor byte position for test assertions (legacy alias).
    pub fn cursor_byte_pos_for_test(&self) -> usize {
        self.state.cursor.pos
    }

    /// Get selection range for test assertions (legacy alias).
    /// Returns (anchor, head).
    pub fn selection_range_for_test(&self) -> (usize, usize) {
        (self.state.selection.anchor, self.state.selection.head)
    }

    // ========================================================================
    // Stage C Test Harness: Scroll Introspection
    // ========================================================================

    /// Get current scroll_y (target offset) for test assertions.
    pub fn scroll_y_for_test(&self) -> f32 {
        self.state.scroll.borrow().target_offset().1
    }

    /// Set scroll container state for testing Stage C.
    /// Sets viewport_height, content_height, and scroll_y directly.
    pub fn set_scroll_state_for_test(
        &mut self,
        viewport_height: f32,
        content_height: f32,
        scroll_y: f32,
    ) {
        use crate::core::math::Size;
        let mut scroll = self.state.scroll.borrow_mut();
        scroll.set_viewport_size(Size::new(200.0, viewport_height));
        scroll.set_content_size(Size::new(200.0, content_height));
        scroll.set_target_offset((0.0, scroll_y));
    }

    /// Get line height for test calculations.
    pub fn line_height_for_test(&self) -> f32 {
        self.ascent + self.descent
    }

    /// Set line data for testing (needed for line_y calculations).
    pub fn set_lines_for_test(&mut self, line_count: usize, line_height: f32) {
        self.state
            .layout_cache
            .set_lines_for_test(line_count, line_height);
    }

    /// K.INV1: Flush deferred focus visibility update.
    /// This simulates what layout() does after rebuilding the cache.
    /// Call this after handle_event in tests that expect scrolling to occur.
    /// K3.INV1: Also call ensure_focus_visible unconditionally for tests that
    /// directly manipulate cursor position (bypassing event handling).
    pub fn flush_focus_visibility_for_test(&self) {
        self.needs_focus_visibility_update.set(false);
        self.ensure_focus_visible();
    }

    // ========================================================================
    // Stage S2: Horizontal Scroll Test Harness
    // ========================================================================

    /// Get current scroll_x (target offset) for test assertions.
    pub fn scroll_x_for_test(&self) -> f32 {
        self.state.scroll.borrow().target_offset().0
    }

    /// Get current visual scroll_x for test assertions.
    /// S2.FIX: After jump_to_offset, visual_offset == target_offset.
    pub fn visual_scroll_x_for_test(&self) -> f32 {
        self.state.scroll.borrow().visual_offset().0
    }

    /// Set horizontal scroll container state for testing.
    /// Sets viewport_width, content_width, and scroll_x directly.
    pub fn set_hscroll_state_for_test(
        &mut self,
        viewport_width: f32,
        content_width: f32,
        scroll_x: f32,
    ) {
        use crate::core::math::Size;
        let mut scroll = self.state.scroll.borrow_mut();
        // Preserve existing vertical dimensions
        let existing_vp_h = scroll.viewport_size.height;
        let existing_ct_h = scroll.content_size.height;
        let existing_y = scroll.target_offset().1;
        scroll.set_viewport_size(Size::new(viewport_width, existing_vp_h.max(100.0)));
        scroll.set_content_size(Size::new(content_width, existing_ct_h.max(100.0)));
        // Use jump_to_offset so visual_offset == target_offset immediately
        scroll.jump_to_offset((scroll_x, existing_y));
    }

    // ========================================================================
    // Stage D Test Harness: Paint Inspection
    // ========================================================================

    /// Get the clip rect (text_viewport) used for painting.
    /// INV-D.CLIP1: Clip rect equals viewport in S_layout.
    pub fn clip_rect_for_test(&self) -> crate::core::math::RectF {
        self.text_viewport()
    }

    /// Compute caret rect in S_layout coordinates for given visual scroll.
    /// Returns None if blink_caret_visible is false.
    /// INV-D.CARET1-2: Caret geometry in S_content, transformed to S_layout.
    pub fn caret_rect_for_test(&self, scroll_y_visual: f32) -> Option<crate::core::math::RectF> {
        if !self.blink_caret_visible.get() {
            return None;
        }
        let text_viewport = self.text_viewport();
        let (caret_line, caret_x) = self.caret_position(self.state.cursor.pos);
        let line_y = self.state.layout_cache.line_y(caret_line);
        let caret_top = text_viewport.y + line_y - scroll_y_visual;
        let caret_height = self.ascent + self.descent;
        Some(crate::core::math::RectF::new(
            text_viewport.x + caret_x,
            caret_top,
            2.0, // caret thickness (simplified)
            caret_height,
        ))
    }

    /// Compute selection rects in S_layout coordinates for given visual scroll.
    /// INV-D.SEL1-2: Selection rects in S_content, transformed to S_layout.
    pub fn selection_rects_for_test(&self, scroll_y_visual: f32) -> Vec<crate::core::math::RectF> {
        let sel_start = self.state.selection.anchor.min(self.state.selection.head);
        let sel_end = self.state.selection.anchor.max(self.state.selection.head);

        if sel_start == sel_end {
            return Vec::new();
        }

        let text_viewport = self.text_viewport();
        let start_line = self.line_for_byte(sel_start);
        let end_line = self.line_for_byte(sel_end);
        let lines = self.state.layout_cache.lines();

        let mut rects = Vec::new();

        for line_idx in start_line..=end_line {
            let line = match lines.get(line_idx) {
                Some(l) => l,
                None => continue,
            };

            let line_y = self.state.layout_cache.line_y(line_idx);
            let rect_top = text_viewport.y + line_y - scroll_y_visual;

            // S1.INV2: Empty lines must have minimum selection width for visibility
            let line_width = self.line_end_x(line_idx);
            let is_empty_line = line_width < 0.01;
            let empty_sel_w = self.empty_line_selection_width;

            let (rect_left, rect_right) = if line_idx == start_line && line_idx == end_line {
                let start_x = self.x_for_byte(sel_start);
                let end_x = self.x_for_byte(sel_end);
                if is_empty_line && (end_x - start_x).abs() < 0.01 {
                    (start_x, start_x + empty_sel_w)
                } else {
                    (start_x, end_x)
                }
            } else if line_idx == start_line {
                let start_x = self.x_for_byte(sel_start);
                if is_empty_line {
                    (start_x, start_x + empty_sel_w)
                } else {
                    (start_x, line_width)
                }
            } else if line_idx == end_line {
                let end_x = self.x_for_byte(sel_end);
                if is_empty_line && end_x < 0.01 {
                    (0.0, empty_sel_w)
                } else {
                    (0.0, end_x)
                }
            } else {
                if is_empty_line {
                    (0.0, empty_sel_w)
                } else {
                    (0.0, line_width)
                }
            };

            let width = (rect_right - rect_left).max(0.0);
            if width > 0.01 {
                rects.push(crate::core::math::RectF::new(
                    text_viewport.x + rect_left,
                    rect_top,
                    width,
                    line.height,
                ));
            }
        }

        rects
    }

    /// Get current visual scroll offset for testing.
    pub fn visual_scroll_y_for_test(&self) -> f32 {
        self.state.scroll.borrow().visual_offset().1
    }

    /// Set visual scroll offset directly for testing Stage D.
    /// Allows testing visual vs target separation without animation.
    pub fn set_visual_scroll_for_test(&mut self, scroll_y: f32) {
        self.state
            .scroll
            .borrow_mut()
            .set_visual_offset_for_test((0.0, scroll_y));
    }

    /// Force caret visible for paint tests.
    pub fn set_blink_caret_visible_for_test(&mut self, visible: bool) {
        self.blink_caret_visible.set(visible);
    }

    /// Set layout rect for geometry tests.
    /// SPEC-HAL-141: Renamed from widget_rect to layout_rect (no longer Cell).
    pub fn set_widget_rect_for_test(&mut self, rect: crate::core::math::RectF) {
        self.layout_rect = rect;
    }

    // ========================================================================
    // Stage K.3 Test Harness: Auto-Height Origin
    // ========================================================================

    /// K3.INV1: Get visual_content_origin_y for test assertions.
    pub fn visual_content_origin_y_for_test(&self) -> f32 {
        self.visual_content_origin_y.get()
    }

    /// K3.INV1: Set visual_content_origin_y for test setup.
    pub fn set_visual_content_origin_y_for_test(&mut self, origin: f32) {
        self.visual_content_origin_y.set(origin);
    }

    /// K3.INV1: Compute effective vertical offset (what paint uses).
    /// In auto-height mode: visual_content_origin_y
    /// In scroll mode: scroll_y (visual)
    pub fn effective_vertical_offset_for_test(&self) -> f32 {
        if self.is_auto_height_mode() {
            self.visual_content_origin_y.get()
        } else {
            self.state.scroll.borrow().visual_offset().1
        }
    }

    /// K3.INV1: Check if in auto-height mode for test assertions.
    pub fn is_auto_height_mode_for_test(&self) -> bool {
        self.is_auto_height_mode()
    }

    // ========================================================================
    // Stage K.5 Test Harness: Document Content Manipulation
    // ========================================================================

    /// K5.INV1: Set document content without resetting cursor or origin.
    /// This allows tests to simulate mutations that would occur via handle_event.
    /// Unlike set_text(), this preserves cursor position and visual_content_origin_y.
    #[cfg(feature = "test-harness")]
    pub fn set_document_content_for_test(&mut self, text: impl Into<String>) {
        self.state.document.set_text(text);
        // Deliberately do NOT reset cursor or visual_content_origin_y
        // Tests are responsible for managing these after mutation
    }

    // ========================================================================
    // Stage K.6 Test Harness: Scenario Runner Support
    // ========================================================================

    /// K6: Check if layout is dirty (cache stale).
    pub fn layout_dirty_for_test(&self) -> bool {
        self.layout_dirty.get()
    }

    /// K6: Clear layout_dirty flag after manual cache rebuild.
    /// This simulates what layout() does after cache.rebuild().
    pub fn clear_layout_dirty_for_test(&self) {
        self.layout_dirty.set(false);
    }

    /// K6: Check if deferred focus visibility update is pending.
    pub fn needs_focus_visibility_update_for_test(&self) -> bool {
        self.needs_focus_visibility_update.get()
    }

    // ========================================================================
    // Scroll Semantics Test Harness
    // ========================================================================

    /// Get full scroll state for test assertions.
    /// Returns (content_size, viewport_size, target_offset, max_scroll).
    pub fn debug_scroll_state_for_test(
        &self,
    ) -> (
        crate::core::math::Size,
        crate::core::math::Size,
        (f32, f32),
        (f32, f32),
    ) {
        let scroll = self.state.scroll.borrow();
        (
            scroll.content_size,
            scroll.viewport_size,
            scroll.target_offset(),
            scroll.max_scroll(),
        )
    }

    /// Get height policy config for test assertions.
    /// Returns (min_height, max_height, fixed_height).
    pub fn debug_height_policy_for_test(&self) -> (f32, Option<f32>, Option<f32>) {
        (
            self.config.min_height,
            self.config.max_height,
            self.config.fixed_height,
        )
    }

    /// Get the cached bounded_height value for test assertions.
    /// This is the height preserved across Flex second pass.
    pub fn debug_bounded_height_for_test(&self) -> Option<f32> {
        self.bounded_height.get()
    }

    /// Get scrollbar drag states for test assertions.
    /// Returns (v_dragging, h_dragging).
    pub fn debug_scrollbar_dragging_for_test(&self) -> (bool, bool) {
        (self.scrollbar_v_dragging, self.scrollbar_h_dragging)
    }

    /// Get text viewport rect (content area excluding scrollbar gutters).
    pub fn debug_text_viewport_for_test(&self) -> crate::core::math::RectF {
        self.text_viewport()
    }

    /// Check if vertical scrollbar is visible.
    pub fn debug_v_scrollbar_visible_for_test(&self) -> bool {
        let scroll = self.state.scroll.borrow();
        let content_h = scroll.content_size.height;
        let viewport_h = scroll.viewport_size.height;
        self.scrollbar_v.is_visible(content_h, viewport_h)
    }

    /// Check if horizontal scrollbar is visible.
    pub fn debug_h_scrollbar_visible_for_test(&self) -> bool {
        let scroll = self.state.scroll.borrow();
        let content_w = scroll.content_size.width;
        let viewport_w = scroll.viewport_size.width;
        self.scrollbar_h.is_visible(content_w, viewport_w)
    }

    /// Set bounded_height directly for testing Flex second pass immunity.
    pub fn set_bounded_height_for_test(&mut self, height: Option<f32>) {
        self.bounded_height.set(height);
    }

    /// Call scroll_by on the scroll container (for testing wheel behavior).
    pub fn scroll_by_for_test(&mut self, dx: f32, dy: f32) {
        self.state.scroll.borrow_mut().scroll_by(dx, dy);
    }

    /// Call clamp_target on the scroll container.
    pub fn clamp_scroll_for_test(&mut self) {
        self.state.scroll.borrow_mut().clamp_target();
    }

    /// Set scroll content and viewport size directly.
    pub fn set_scroll_sizes_for_test(
        &mut self,
        content: crate::core::math::Size,
        viewport: crate::core::math::Size,
    ) {
        let mut scroll = self.state.scroll.borrow_mut();
        scroll.set_content_size(content);
        scroll.set_viewport_size(viewport);
    }

    /// Get scrollbar hover states for testing.
    /// Returns (v_hovered, h_hovered).
    pub fn debug_scrollbar_hover_for_test(&self) -> (bool, bool) {
        use crate::widgets::scrollbar::ScrollbarState;
        let v_hovered = self.scrollbar_v.state == ScrollbarState::Hovered;
        let h_hovered = self.scrollbar_h.state == ScrollbarState::Hovered;
        (v_hovered, h_hovered)
    }

    /// Test helper to simulate pointer down on scrollbar and return whether it hit.
    /// Returns true if the scrollbar consumed the event.
    pub fn test_scrollbar_v_hit(&mut self, local_x: f32, local_y: f32) -> bool {
        let pe = crate::core::event::PointerEvent {
            id: crate::core::event::PointerId::MOUSE,
            kind: crate::core::event::PointerEventKind::Down,
            button: Some(crate::core::event::PointerButton::Primary),
            global_pos: (local_x, local_y),
            local_pos: (local_x, local_y),
            modifiers: crate::core::event::Modifiers::default(),
            pointer_type: crate::core::event::PointerType::Mouse,
        };
        let mut cx = crate::core::widget::EventContext::new(
            crate::utils::id::TreeWidgetId(0),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            None,  // INV-TEXT-GEOM-PHASE: No text engine in test helper
            None,  // SPEC-HAL-021 Stage 2.5: Animation capability optional
            false, // has_pointer_capture
            false, // SPEC-HAL-268: Not relevant for test helpers
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );
        self.handle_scrollbar_down(&mut cx, &pe)
    }

    /// Test helper to simulate pointer down on horizontal scrollbar and return whether it hit.
    pub fn test_scrollbar_h_hit(&mut self, local_x: f32, local_y: f32) -> bool {
        let pe = crate::core::event::PointerEvent {
            id: crate::core::event::PointerId::MOUSE,
            kind: crate::core::event::PointerEventKind::Down,
            button: Some(crate::core::event::PointerButton::Primary),
            global_pos: (local_x, local_y),
            local_pos: (local_x, local_y),
            modifiers: crate::core::event::Modifiers::default(),
            pointer_type: crate::core::event::PointerType::Mouse,
        };
        let mut cx = crate::core::widget::EventContext::new(
            crate::utils::id::TreeWidgetId(0),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            None,  // INV-TEXT-GEOM-PHASE: No text engine in test helper
            None,  // SPEC-HAL-021 Stage 2.5: Animation capability optional
            false, // has_pointer_capture
            false, // SPEC-HAL-268: Not relevant for test helpers
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );
        self.handle_scrollbar_h_down(&mut cx, &pe)
    }

    /// Test helper to simulate pointer move for hover testing.
    pub fn test_scrollbar_hover_update(&mut self, local_x: f32, local_y: f32) {
        let pe = crate::core::event::PointerEvent {
            id: crate::core::event::PointerId::MOUSE,
            kind: crate::core::event::PointerEventKind::Move,
            button: None,
            global_pos: (local_x, local_y),
            local_pos: (local_x, local_y),
            modifiers: crate::core::event::Modifiers::default(),
            pointer_type: crate::core::event::PointerType::Mouse,
        };
        self.update_scrollbar_hover(&pe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paint::{Color, PaintSink, TextBackgroundSpan};
    use crate::core::text_engine::{GlyphRun, TextEngine, TextMetrics};
    use crate::core::theme::Theme;

    #[test]
    fn text_area_caret_demand_settles_after_release_without_stalling_scroll() {
        use crate::core::app::{FrameReason, FrameTime, RetainedSceneTestHarness};
        use crate::core::event::{
            InteractionDomains, PointerDevice, RawInputEvent, WheelDelta, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::icons::builtin_registry;
        use crate::core::input_fusion::InputFusion;
        use crate::core::rect_authority::UpdateAuthorityGuard;
        use crate::core::scene::{SceneNode, TextNode};
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::runtime::actions::ActionId;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{flex::Flex, label::Label};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        fn area(tree: &WidgetTree, node: NodeId) -> &TextArea {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn output<'a>(
            nodes: &'a [SceneNode],
            texts: &mut Vec<&'a TextNode>,
            carets: &mut Vec<RectF>,
            clips: &mut Vec<RectF>,
        ) {
            for node in nodes {
                match node {
                    SceneNode::Text(text) => texts.push(text),
                    SceneNode::Rect(rect) if rect.z_index == crate::core::paint::LAYER_OVERLAY => {
                        carets.push(rect.rect)
                    }
                    SceneNode::Clip(clip) => {
                        clips.push(clip.rect);
                        output(&clip.children, texts, carets, clips);
                    }
                    SceneNode::RetainedFragment(fragment) => {
                        output(&fragment.children, texts, carets, clips)
                    }
                    _ => {}
                }
            }
        }

        let content = (0..32)
            .map(|i| format!("Line {i:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(
            root,
            Box::new(TextArea::new().with_fixed_height(100.0).with_text(&content)),
            None,
            0,
        );
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let theme = Theme::default();
        let icons = builtin_registry();
        let mut text = FontManagerTextEngine::new();
        let mut router = EventRouter::new();
        let mut domains = InteractionDomains::new();
        let mut fusion = InputFusion::new();
        let mut bindings = BindingRegistry::new();
        let mut harness = RetainedSceneTestHarness::new();
        let started = Instant::now();
        let mut elapsed = Duration::ZERO;
        let mut frame_index = 0;
        macro_rules! frame {
            ($millis:expr) => {{
                let dt = Duration::from_millis($millis);
                elapsed += dt;
                frame_index += 1;
                harness.frame_with_gesture_sessions(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(320.0, 160.0)),
                    &theme,
                    &mut text,
                    &icons,
                    &FrameTime {
                        now: started + elapsed,
                        dt,
                        frame_index,
                        reason: FrameReason::Animation,
                    },
                    None,
                    &mut domains,
                    WindowId::PRIMARY,
                    &mut router,
                    &mut fusion,
                )
            }};
        }
        let state = |tree: &WidgetTree| {
            let value = area(tree, node);
            (
                value.text().to_owned(),
                value.state.cursor.pos,
                value.state.selection.anchor,
                value.state.selection.head,
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
        let projection =
            |nodes: &[SceneNode], expected_carets: usize, viewport: RectF, bounds: RectF| {
                let mut texts = Vec::new();
                let mut carets = Vec::new();
                let mut clips = Vec::new();
                output(nodes, &mut texts, &mut carets, &mut clips);
                assert_eq!(clips, vec![viewport, bounds]);
                assert_eq!(carets.len(), expected_carets);
                assert!(!texts.is_empty());
                for text in &texts {
                    assert_eq!(text.font_size, theme.typography.input_font_size);
                    assert_eq!(text.color, theme.colors.typography.primary.into());
                    assert_eq!(text.z_index, LAYER_CONTENT + 2);
                    assert!(text.backgrounds.is_empty());
                }
                for caret in carets {
                    assert_eq!(caret.width, theme.typography.caret_thickness);
                    assert!(caret.height > 0.0);
                }
                texts
                    .iter()
                    .map(|text| (text.text.clone(), text.position))
                    .collect::<Vec<_>>()
            };

        frame!(16);
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        frame!(16);
        router.reset_continuous_frames_request();
        let original_state = state(&tree);
        let original_geometry = geometry(&tree);
        let rect = original_geometry[1].2;
        let hit = |tree: &WidgetTree| {
            let hit = tree.hit_test(rect.x + 10.0, rect.y + 20.0).unwrap();
            (hit.widget_id, hit.local_pos, hit.absolute_bounds)
        };
        let original_hit = hit(&tree);
        assert_eq!(original_hit.0, owner);
        let viewport = area(&tree, node).clip_rect_for_test();
        assert!(area(&tree, node).state.scroll.borrow().max_scroll().1 > 24.51);
        assert_eq!(
            area(&tree, node).state.scroll.borrow().target_offset(),
            (0.0, 0.0)
        );
        assert_eq!(
            area(&tree, node).state.scroll.borrow().visual_offset(),
            (0.0, 0.0)
        );
        let peer_local = harness.local_fragment(peer_id).unwrap();
        let root_local = harness.local_fragment(root_id).unwrap();

        for _ in 0..2 {
            router.set_frame_time_ms(elapsed.as_millis() as u64);
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
            router.reset_continuous_frames_request();
            frame!(16);
            projection(&harness.local_fragment(owner).unwrap(), 1, viewport, rect);
            assert!(tree.node(node).unwrap().widget.needs_animation());
            assert!(tree.any_widget_needs_animation());
            frame!(530);
            projection(&harness.local_fragment(owner).unwrap(), 0, viewport, rect);
            assert!(tree.node(node).unwrap().widget.needs_animation());
            router.set_frame_time_ms(elapsed.as_millis() as u64);
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
            assert!(area(&tree, node).focused);
            assert_eq!(state(&tree), original_state);
            frame!(16);
            assert!(!area(&tree, node).state.scroll.borrow().needs_animation());
            assert!(area(&tree, node).focused);
            assert!(
                !tree.node(node).unwrap().widget.needs_animation(),
                "released TextArea retains focus but must settle caret animation demand"
            );
            assert!(!tree.any_widget_needs_animation());
            projection(&harness.local_fragment(owner).unwrap(), 0, viewport, rect);
            assert!(router.continuous_frames_requested());
            router.reset_continuous_frames_request();
            let settled = harness.local_fragment(owner).unwrap();
            let settled_geometry = geometry(&tree);
            let idle = frame!(16);
            assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
            assert!(Arc::ptr_eq(
                &settled,
                &harness.local_fragment(owner).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &peer_local,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_local,
                &harness.local_fragment(root_id).unwrap()
            ));
            assert_eq!(geometry(&tree), settled_geometry);
            assert!(!router.continuous_frames_requested());
        }

        let resting = projection(&harness.local_fragment(owner).unwrap(), 0, viewport, rect);
        let scroll_geometry = geometry(&tree);
        let mut events = Vec::new();
        fusion.process_raw(
            RawInputEvent::PointerWheel {
                window_id: WindowId::PRIMARY,
                time: started + elapsed,
                device: PointerDevice::Mouse,
                delta: WheelDelta::Pixels(0.0, -24.51),
                position: Point::new(rect.x + 30.0, rect.y + 30.0),
            },
            &mut events,
        );
        fusion.flush(&mut events);
        assert_eq!(events.len(), 1);
        router.set_frame_time_ms(elapsed.as_millis() as u64);
        for event in events {
            router.route_input_event(
                &mut tree,
                event,
                Some(root),
                &mut text,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
        }
        assert_eq!(
            area(&tree, node).state.scroll.borrow().target_offset(),
            (0.0, 24.51)
        );
        assert_eq!(
            area(&tree, node).state.scroll.borrow().visual_offset(),
            (0.0, 0.0)
        );
        assert!(area(&tree, node).state.scroll.borrow().needs_animation());
        assert!(tree.node(node).unwrap().widget.needs_animation());
        assert!(tree.any_widget_needs_animation());
        assert!(router.continuous_frames_requested());
        router.reset_continuous_frames_request();
        let mut terminal_seen = false;
        for step in 0..64 {
            let before = area(&tree, node).state.scroll.borrow().visual_offset();
            let old_local = harness.local_fragment(owner).unwrap();
            let painted = frame!(16);
            let after = area(&tree, node).state.scroll.borrow().visual_offset();
            assert!(after.1 > before.1);
            assert!(painted.scene_built);
            let current = harness.local_fragment(owner).unwrap();
            assert!(!Arc::ptr_eq(&old_local, &current));
            let displayed = projection(&current, 0, viewport, rect);
            assert_eq!(displayed.len(), resting.len());
            for ((actual_text, actual), (expected_text, initial)) in displayed.iter().zip(&resting)
            {
                assert_eq!(actual_text, expected_text);
                assert_eq!(actual.x, initial.x);
                assert_eq!(
                    actual.y,
                    initial.y - after.1.round(),
                    "retained text must consume the current snapped visual offset, including its terminal frame"
                );
            }
            assert!(Arc::ptr_eq(
                &peer_local,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_local,
                &harness.local_fragment(root_id).unwrap()
            ));
            assert_eq!(geometry(&tree), scroll_geometry);
            assert_eq!(hit(&tree), original_hit);
            assert_eq!(area(&tree, node).clip_rect_for_test(), viewport);
            assert_eq!(state(&tree), original_state);
            assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            if !area(&tree, node).state.scroll.borrow().needs_animation() {
                assert_eq!(after, (0.0, 24.51));
                assert_ne!(before.1.round(), after.1.round());
                assert!(!tree.node(node).unwrap().widget.needs_animation());
                assert!(!tree.any_widget_needs_animation());
                terminal_seen = true;
                break;
            }
            assert!(tree.node(node).unwrap().widget.needs_animation());
            assert!(tree.any_widget_needs_animation());
            if step == 0 {
                assert!(after.1 > 0.0 && after.1 < 24.51);
            }
        }
        assert!(
            terminal_seen,
            "ordinary Update frames must reach the real scroll target"
        );
        let terminal = harness.local_fragment(owner).unwrap();
        let idle = frame!(16);
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &terminal,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(!router.continuous_frames_requested());
        {
            let _update = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(None);
            cx.begin_widget_update_for_owner(owner);
            tree.node_mut(node).unwrap().widget.update(0.0, &mut cx);
            assert_eq!(cx.take_widget_invalidations(), (false, false, false));
        }
        assert!(tree.node(node).unwrap().widget.needs_animation());
        assert!(tree.any_widget_needs_animation());
        let republished = frame!(16);
        assert!(!republished.scene_built && !republished.needs_next_frame);
        assert!(!tree.node(node).unwrap().widget.needs_animation());
        assert!(!tree.any_widget_needs_animation());
        assert!(Arc::ptr_eq(
            &terminal,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_local,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_local,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert_eq!(geometry(&tree), scroll_geometry);
        assert_eq!(hit(&tree), original_hit);
        assert_eq!(state(&tree), original_state);
        assert_eq!(
            area(&tree, node).state.scroll.borrow().visual_offset(),
            (0.0, 24.51)
        );
        assert_eq!(
            area(&tree, node).state.scroll.borrow().target_offset(),
            (0.0, 24.51)
        );
    }

    struct NonFiniteMetricsEngine;

    impl TextEngine for NonFiniteMetricsEngine {
        fn measure(&mut self, _text: &str, _font_size: f32) -> (f32, f32) {
            (f32::NAN, f32::NAN)
        }

        fn metrics(&mut self, _text: &str, _font_size: f32) -> TextMetrics {
            TextMetrics {
                ascent: f32::NAN,
                descent: f32::NAN,
                line_gap: f32::NAN,
                line_height: f32::NAN,
            }
        }

        fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun {
            GlyphRun {
                binding: None,
                clusters: Vec::new(),
                glyphs: Vec::new(),
                width: 0.0,
                height: f32::NAN,
                text: text.to_string(),
                font_size,
                ascent: f32::NAN,
                descent: f32::NAN,
            }
        }

        fn draw(
            &mut self,
            _run: &GlyphRun,
            _x: f32,
            _y: f32,
            _color: Color,
            _z_index: crate::core::paint::ZIndex,
            _sink: &mut dyn PaintSink,
        ) {
        }

        fn draw_with_backgrounds(
            &mut self,
            _run: &GlyphRun,
            _x: f32,
            _y: f32,
            _color: Color,
            _backgrounds: Vec<TextBackgroundSpan>,
            _z_index: crate::core::paint::ZIndex,
            _sink: &mut dyn PaintSink,
        ) {
        }
    }

    fn layout_text_area(
        text_area: &mut TextArea,
        engine: &mut NonFiniteMetricsEngine,
        constraints: Constraints,
    ) -> LayoutResult {
        let mut tree = WidgetTree::new();
        let theme = Theme::default();
        let mut cx = LayoutContext::new(&mut tree, engine, &theme);
        text_area.layout(&mut cx, constraints, &[])
    }

    #[test]
    fn layout_respects_tight_height_constraint() {
        let mut text_area = TextArea::new();
        let mut engine = NonFiniteMetricsEngine;
        let result = layout_text_area(
            &mut text_area,
            &mut engine,
            Constraints::tight(Size::new(200.0, 420.0)),
        );

        assert_eq!(result.size.height, 420.0);
        assert_eq!(text_area.layout_rect.height, 420.0);

        let (_, viewport, _, _) = text_area.debug_scroll_state_for_test();
        assert_eq!(viewport.height, 420.0 - text_area.padding * 2.0);
    }

    #[test]
    fn unbounded_measurement_does_not_poison_rebuild_width() {
        let mut text_area = TextArea::new().with_text("[session]\nprofile = \"canary\"");
        let mut engine = NonFiniteMetricsEngine;

        layout_text_area(
            &mut text_area,
            &mut engine,
            Constraints::new(0.0, 240.0, 0.0, 160.0),
        );
        let finite_width = text_area.last_layout_width;
        assert!(finite_width > 100.0);

        layout_text_area(
            &mut text_area,
            &mut engine,
            Constraints::new(0.0, f32::INFINITY, 0.0, f32::INFINITY),
        );
        assert_eq!(text_area.last_layout_width, finite_width);

        text_area.state.document.mark_modified();
        text_area.layout_dirty.set(true);
        text_area.state.layout_cache.mark_stale();
        text_area.rebuild_layout_cache_now(&mut engine);

        assert_eq!(text_area.state.layout_cache.cached_width(), finite_width);
    }

    #[test]
    fn layout_returns_finite_size_with_non_finite_text_metrics() {
        let mut text_area = TextArea::new();
        let mut engine = NonFiniteMetricsEngine;
        let result = layout_text_area(
            &mut text_area,
            &mut engine,
            Constraints::new(0.0, 200.0, 0.0, 120.0),
        );

        assert!(result.size.width.is_finite());
        assert!(result.size.height.is_finite());
        assert!(text_area.layout_rect.width.is_finite());
        assert!(text_area.layout_rect.height.is_finite());
    }

    #[test]
    fn layout_does_not_reuse_non_finite_bounded_height() {
        let mut text_area = TextArea::new();
        text_area.bounded_height.set(Some(f32::NAN));

        let mut engine = NonFiniteMetricsEngine;
        let result = layout_text_area(
            &mut text_area,
            &mut engine,
            Constraints::new(0.0, 200.0, 0.0, f32::INFINITY),
        );

        assert!(result.size.height.is_finite());
        assert!(text_area.layout_rect.height.is_finite());
    }
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{FusedInputEvent, InteractionDomains, Modifiers, PhysicalKeyCode, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::core::icons::builtin_registry;
    use crate::core::scene::SceneNode;
    use crate::core::theme::Theme;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::runtime::actions::ActionId;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::{flex::Flex, label::Label};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    struct Fixture {
        tree: WidgetTree,
        root: NodeId,
        node: NodeId,
        owner: TreeWidgetId,
        peer: TreeWidgetId,
        text: FontManagerTextEngine,
        router: EventRouter,
        domains: InteractionDomains,
        harness: RetainedSceneTestHarness,
        time: Instant,
        now: f64,
        width: f32,
    }

    #[test]
    fn text_selection_palette_text_area_paints_selected_characters() {
        use crate::tests::_support::{assert_painted_text_backgrounds, text_selection_palette_cases};

        for (theme, expected) in text_selection_palette_cases() {
            let mut f = Fixture::new(false);
            f.key(
                KeyCode::Right,
                false,
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            );
            assert_eq!(f.area().selection_range_for_test(), (0, 1));
            f.tick_with_theme(0.002, &theme);
            let painted = f.harness.local_fragment(f.owner).unwrap();
            assert_painted_text_backgrounds(&painted, "abcd", &[expected]);
        }
    }

    impl Fixture {
        fn new(enabled: bool) -> Self {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()), None, 0);
            let node = tree.add_child(root, Box::new(TextArea::new()
                .with_text("abcd\nnext").with_fixed_height(90.0)
                .with_smooth_caret_prototype(enabled)), None, 0);
            let peer = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 0);
            let mut f = Self {
                owner: tree.widget_id_for_node(node).unwrap(),
                peer: tree.widget_id_for_node(peer).unwrap(),
                tree, root, node, text: FontManagerTextEngine::new(),
                router: EventRouter::new(), domains: InteractionDomains::new(),
                harness: RetainedSceneTestHarness::new(), time: Instant::now(),
                now: 0.0, width: 360.0,
            };
            f.tick(0.0);
            f.capture();
            f
        }

        fn area(&self) -> &TextArea {
            self.tree.node(self.node).unwrap().widget().as_any().downcast_ref().unwrap()
        }

        fn tick(&mut self, now: f64) {
            self.tick_with_theme(now, &Theme::default());
        }

        fn tick_with_theme(&mut self, now: f64, theme: &Theme) {
            self.now = now;
            self.harness.advance_transition_time(now);
            self.harness.frame_with_appearance(
                &mut self.tree, self.root, Constraints::tight(Size::new(self.width, 140.0)),
                theme, &mut self.text, &builtin_registry(), &self.domains,
                WindowId::PRIMARY,
            );
        }

        fn capture(&mut self) {
            if self.domains.primary_state().keyboard_mode == KeyboardAuthorityMode::Capture {
                assert!(self.router.handle_engine_action(&mut self.tree, ActionId::RELEASE_CAPTURE,
                    Some(self.root), self.domains.primary_state_mut(), &mut self.text, None));
            }
            self.router.request_focus_for_widget(&mut self.tree, self.owner, self.domains.primary_state_mut());
            assert!(self.router.handle_engine_action(&mut self.tree, ActionId::ENTER_WIDGET,
                Some(self.root), self.domains.primary_state_mut(), &mut self.text, None));
            self.tick(self.now + 0.001);
        }

        fn route(&mut self, event: FusedInputEvent) {
            self.router.set_frame_time_ms((self.now * 1000.0) as u64);
            self.router.route_input_event(&mut self.tree, event, Some(self.root), &mut self.text,
                Some(self.harness.animation_context()), self.domains.primary_state_mut(),
                &mut BindingRegistry::new());
            self.time += Duration::from_nanos(1);
        }

        fn key(&mut self, key_code: KeyCode, repeat: bool, modifiers: Modifiers) {
            self.route(FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY, time: self.time, key_code,
                physical_key: PhysicalKeyCode::Unknown, reservation: None, modifiers, repeat,
            });
        }

        fn insert(&mut self) {
            let time = self.time;
            self.route(FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY, time, key_code: KeyCode::Character('5'),
                physical_key: PhysicalKeyCode::Digit5, reservation: None,
                modifiers: Modifiers::default(), repeat: false,
            });
            self.route(FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY, time, text: "5".into(),
                modifiers: Modifiers::default(),
            });
        }

        fn canonical_x(&self) -> f32 {
            self.area().caret_position(self.area().state.cursor.pos).1
        }

        fn painted_x(&self) -> f32 {
            fn find(nodes: &[SceneNode], clip: Option<RectF>) -> Option<(RectF, RectF)> {
                for node in nodes {
                    let found = match node {
                        SceneNode::Rect(n) if n.z_index == crate::core::paint::LAYER_OVERLAY =>
                            Some((n.rect, clip.expect("caret inside actual text viewport clip"))),
                        SceneNode::Clip(n) => find(&n.children, Some(n.rect)),
                        SceneNode::RetainedFragment(n) => find(&n.children, clip),
                        _ => None,
                    };
                    if found.is_some() { return found; }
                }
                None
            }
            let local = self.harness.local_fragment(self.owner).unwrap();
            let (caret, clip) = find(&local, None).expect("real captured TextArea caret");
            assert!(caret.left() >= clip.left() && caret.right() <= clip.right());
            assert!(caret.top() >= clip.top() && caret.bottom() <= clip.bottom());
            let viewport = self.area().text_viewport_from_rect(
                self.tree.get_absolute_paint_rect(self.node).unwrap());
            caret.x - viewport.x
        }
    }

    #[test]
    fn textarea_same_line_motion_routed_retained_owner() {
        let mut f = Fixture::new(true);
        assert!(f.area().smooth_caret_prototype_enabled());
        let source = f.area().text().to_owned();
        let peer = f.harness.local_fragment(f.peer).unwrap();
        let start = f.canonical_x();
        f.key(KeyCode::Right, false, Modifiers::default());
        assert_eq!(f.area().state.cursor.pos, 1, "logical navigation is immediate");
        assert_eq!(f.area().text(), source);
        let destination = f.canonical_x();
        f.tick(0.010);
        assert!(f.area().smooth_caret.handle.is_some());
        assert_eq!(f.painted_x(), start, "same-frame start is the current static caret");
        f.tick(0.030);
        let middle = f.painted_x();
        assert!(middle > start && middle < destination);
        f.tick(0.05000001);
        assert_eq!(f.painted_x(), destination);
        assert!(f.area().smooth_caret.handle.is_none());
        assert!(Arc::ptr_eq(&peer, &f.harness.local_fragment(f.peer).unwrap()));

        f.key(KeyCode::Right, false, Modifiers::default());
        f.tick(0.080);
        f.tick(0.100);
        let displayed = f.painted_x();
        let old = f.area().smooth_caret.handle.unwrap();
        f.key(KeyCode::Right, false, Modifiers::default());
        f.tick(0.100);
        assert_eq!(f.painted_x(), displayed, "retarget has no jump or backlog");
        assert_ne!(f.area().smooth_caret.handle, Some(old));
        assert!(!f.harness.animation_context().is_active(old));

        f.key(KeyCode::End, false, Modifiers::default());
        f.tick(0.200);
        let handle = f.area().smooth_caret.handle;
        f.key(KeyCode::End, false, Modifiers::default());
        f.tick(0.200);
        assert_eq!(f.area().smooth_caret.handle, handle, "no-op must not restart/cancel");
        f.insert();
        f.key(KeyCode::Left, false, Modifiers::default());
        assert_eq!(f.area().text(), "abcd5\nnext");
        f.tick(0.210);
        assert!(f.area().smooth_caret.handle.is_none(), "content snap dominates same-batch navigation");
        assert_eq!(f.painted_x(), f.canonical_x());
        f.tick(0.211);
        f.key(KeyCode::Left, false, Modifiers::default());
        f.tick(0.220);
        assert!(f.area().smooth_caret.handle.is_some());
        f.key(KeyCode::Left, true, Modifiers::default());
        f.tick(0.230);
        assert!(f.area().smooth_caret.handle.is_none());
        assert_eq!(f.painted_x(), f.canonical_x());

        f.key(KeyCode::Right, false, Modifiers::default());
        f.tick(0.240);
        assert!(f.area().smooth_caret.handle.is_some());
        f.width = 250.0;
        f.tree.mark_layout_dirty(f.root);
        f.tick(0.250);
        assert_eq!(f.painted_x(), f.canonical_x(), "first changed-geometry paint suppresses old projection");
        f.tick(0.251);
        assert!(f.area().smooth_caret.handle.is_none());
        f.key(KeyCode::End, false, Modifiers::default());
        f.tick(0.300);
        f.tick(0.366);
        f.key(KeyCode::Right, false, Modifiers::default());
        f.tick(0.370);
        assert_eq!(f.area().smooth_destination().unwrap().line, 1);
        assert!(f.area().smooth_caret.handle.is_none(), "cross-line navigation is static");
        f.key(KeyCode::Right, false, Modifiers { shift: true, ..Modifiers::default() });
        f.tick(0.371);
        assert!(f.area().has_active_selection());
        assert!(f.area().smooth_caret.handle.is_none());

        f.key(KeyCode::Left, false, Modifiers::default());
        f.tick(0.372);
        f.tick(0.373);
        f.key(KeyCode::Right, false, Modifiers::default());
        f.tick(0.380);
        assert!(f.area().smooth_caret.handle.is_some());
        assert!(f.router.handle_engine_action(&mut f.tree, ActionId::RELEASE_CAPTURE,
            Some(f.root), f.domains.primary_state_mut(), &mut f.text, None));
        f.tick(0.381);
        assert!(f.area().focused, "real Capture release preserves Focus");
        assert!(f.area().smooth_caret.handle.is_none());
        assert!(!f.area().needs_animation());

        f.capture();
        f.key(KeyCode::Right, false, Modifiers::default());
        f.tick(0.400);
        let retired = f.area().smooth_caret.handle.unwrap();
        f.tree.mark_layout_dirty(f.root);
        f.tick(0.410);
        assert_eq!(f.area().smooth_caret.handle, Some(retired), "same-instance layout lease preserves owner");
        f.tree.replace_widget_only(f.node, Box::new(TextArea::new()
            .with_text("wxyz\nnext").with_fixed_height(90.0).with_smooth_caret_prototype(true)));
        assert_eq!(f.tree.widget_id_for_node(f.node), Some(f.owner));
        f.tick(0.412);
        assert!(!f.harness.animation_context().is_active(retired));
        f.capture();
        f.tick(0.414);
        f.key(KeyCode::Right, false, Modifiers::default());
        f.tick(0.420);
        let successor = f.area().smooth_caret.handle.unwrap();
        assert_ne!(successor, retired);
        f.tick(0.430);
        assert_eq!(f.area().smooth_caret.handle, Some(successor));

        let mut plain = Fixture::new(false);
        assert!(!TextArea::new().smooth_caret_prototype_enabled());
        plain.key(KeyCode::Right, false, Modifiers::default());
        plain.tick(0.010);
        assert_eq!(plain.painted_x(), plain.canonical_x());
        assert!(plain.area().smooth_caret.handle.is_none());
        assert!(plain.area().smooth_caret.projection.is_none());
        assert!(plain.area().motion_font.is_none());
    }
}

