// SPEC-HAL-305: Console Widget — High-Throughput Colored Log Display
//
// Architecture: Console (container) → ScrollArea → ConsoleBody
// Shared state via Rc<RefCell<ConsoleState>>.
// Same pattern as OrderBook (SPEC-HAL-303).
//
// Storage: SlidingWindow<ConsoleLine> — Halcyon's ring buffer
// (crate::core::sliding_window). Auto-evicts oldest on push.
// FORBID-305-5: Do NOT use VecDeque.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT, LAYER_OVERLAY, TextBackgroundSpan, ZIndex};
use crate::core::sliding_window::SlidingWindow;
use crate::core::text_selection::{find_word_boundaries, hit_test_run, x_for_offset};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, ScrollSyncInvalidation, UpdateContext, Widget,
    WidgetDowncast,
};
use crate::core::widget_tree::{NodeId, OverlayHitRegion};
use crate::engines::virtualization::VirtualizationEngine;
use crate::model::tabular::row_heights::RowHeights;
use crate::utils::id::TreeWidgetId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::scroll_area::ScrollArea;
use crossbeam_channel::Receiver;

const CONSOLE_TEXT_LAYER: ZIndex = LAYER_CONTENT + 1;

// ─── Text Model (SPEC-HAL-305 §3) ────────────────────────

/// Visual style for a console span.
/// SPEC-HAL-305 §3: Resolved at paint time (INV-305-4).
#[derive(Clone, Debug, Default)]
pub struct ConsoleStyle {
    /// Foreground color. None = level default or theme primary.
    pub fg: Option<Color>,
    /// Background color. None = transparent.
    pub bg: Option<Color>,
    /// Bold — renders as lightened color (20%).
    /// Real bold font variant requires font switching (future).
    pub bold: bool,
    /// Italic — no-op until font variant switching is available.
    /// Field exists for API stability (SPEC-HAL-305 §3).
    pub italic: bool,
    /// Dimmed — alpha multiplied by 0.5.
    pub dim: bool,
    /// Underline — 1px line below text.
    pub underline: bool,
}

/// A single styled text span within a console line.
#[derive(Clone, Debug)]
pub struct ConsoleSpan {
    /// Text content.
    pub text: String,
    /// Visual style.
    pub style: ConsoleStyle,
}

/// A single line of console output.
#[derive(Clone, Debug)]
pub struct ConsoleLine {
    /// Styled text spans.
    pub spans: Vec<ConsoleSpan>,
    /// Optional log level (determines default color when span.fg is None).
    pub level: Option<ConsoleLevel>,
}

impl Default for ConsoleLine {
    fn default() -> Self {
        Self {
            spans: Vec::new(),
            level: None,
        }
    }
}

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl ConsoleLevel {
    /// Short tag string for display.
    pub fn tag(self) -> &'static str {
        match self {
            ConsoleLevel::Trace => "TRACE",
            ConsoleLevel::Debug => "DEBUG",
            ConsoleLevel::Info => "INFO",
            ConsoleLevel::Warn => "WARN",
            ConsoleLevel::Error => "ERROR",
        }
    }
}

/// Parse text with ANSI SGR escape sequences into styled spans.
///
/// Supports: Reset (0), Bold (1), Dim (2), Italic (3), Underline (4),
/// Standard FG (30-37), Bright FG (90-97), Standard BG (40-47), Bright BG (100-107),
/// 256-color: 38;5;N / 48;5;N, 24-bit RGB: 38;2;R;G;B / 48;2;R;G;B.
pub fn parse_ansi(input: &str) -> Vec<ConsoleSpan> {
    let mut spans = Vec::new();
    let mut current_style = ConsoleStyle::default();
    let mut current_text = String::new();
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            if !current_text.is_empty() {
                spans.push(ConsoleSpan {
                    text: std::mem::take(&mut current_text),
                    style: current_style.clone(),
                });
            }
            i += 2;
            let mut params = Vec::new();
            let mut param_buf = String::new();
            while i < len {
                let c = bytes[i] as char;
                if c.is_ascii_digit() {
                    param_buf.push(c);
                    i += 1;
                } else if c == ';' {
                    params.push(param_buf.parse::<u32>().unwrap_or(0));
                    param_buf.clear();
                    i += 1;
                } else {
                    if !param_buf.is_empty() {
                        params.push(param_buf.parse::<u32>().unwrap_or(0));
                    }
                    if c == 'm' {
                        apply_sgr_params(&params, &mut current_style);
                    }
                    i += 1;
                    break;
                }
            }
        } else {
            // Regular character — consume full UTF-8 codepoint.
            let rest = &input[i..];
            let ch = rest.chars().next().unwrap();
            current_text.push(ch);
            i += ch.len_utf8();
        }
    }

    if !current_text.is_empty() {
        spans.push(ConsoleSpan {
            text: current_text,
            style: current_style,
        });
    }

    spans.retain(|s| !s.text.is_empty());
    spans
}

const ANSI_COLORS: [u32; 8] = [
    0x000000, 0xCC0000, 0x00CC00, 0xCCCC00, 0x3333CC, 0xCC00CC, 0x00CCCC, 0xCCCCCC,
];

const ANSI_BRIGHT_COLORS: [u32; 8] = [
    0x555555, 0xFF5555, 0x55FF55, 0xFFFF55, 0x5555FF, 0xFF55FF, 0x55FFFF, 0xFFFFFF,
];

fn apply_sgr_params(params: &[u32], style: &mut ConsoleStyle) {
    if params.is_empty() {
        *style = ConsoleStyle::default();
        return;
    }
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => *style = ConsoleStyle::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            n @ 30..=37 => {
                style.fg = Some(Color::from_hex(ANSI_COLORS[(n - 30) as usize]));
            }
            39 => style.fg = None,
            n @ 40..=47 => {
                style.bg = Some(Color::from_hex(ANSI_COLORS[(n - 40) as usize]));
            }
            49 => style.bg = None,
            n @ 90..=97 => {
                style.fg = Some(Color::from_hex(ANSI_BRIGHT_COLORS[(n - 90) as usize]));
            }
            n @ 100..=107 => {
                style.bg = Some(Color::from_hex(ANSI_BRIGHT_COLORS[(n - 100) as usize]));
            }
            38 => {
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            style.fg = Some(ansi_256_to_color(params[i + 2]));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            let r = params[i + 2].min(255) as f32 / 255.0;
                            let g = params[i + 3].min(255) as f32 / 255.0;
                            let b = params[i + 4].min(255) as f32 / 255.0;
                            style.fg = Some(Color::from_srgb(r, g, b, 1.0));
                            i += 4;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
            }
            48 => {
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            style.bg = Some(ansi_256_to_color(params[i + 2]));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            let r = params[i + 2].min(255) as f32 / 255.0;
                            let g = params[i + 3].min(255) as f32 / 255.0;
                            let b = params[i + 4].min(255) as f32 / 255.0;
                            style.bg = Some(Color::from_srgb(r, g, b, 1.0));
                            i += 4;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn ansi_256_to_color(n: u32) -> Color {
    match n {
        0..=7 => Color::from_hex(ANSI_COLORS[n as usize]),
        8..=15 => Color::from_hex(ANSI_BRIGHT_COLORS[(n - 8) as usize]),
        16..=231 => {
            let n = n - 16;
            let r = ((n / 36) * 51) as f32 / 255.0;
            let g = (((n % 36) / 6) * 51) as f32 / 255.0;
            let b = ((n % 6) * 51) as f32 / 255.0;
            Color::from_srgb(r, g, b, 1.0)
        }
        232..=255 => {
            let v = (8 + (n - 232) * 10).min(255) as f32 / 255.0;
            Color::from_srgb(v, v, v, 1.0)
        }
        _ => Color::WHITE,
    }
}

// NOTE: Level-default colors are resolved at paint time inside
// ConsoleBody::paint using theme colors for Trace/Debug/Info and
// hardcoded colors for Warn/Error. There is NO default_color()
// method — the widget must have theme access to resolve correctly
// (dark mode, custom themes). FORBID-305-4.

// ─── Console Config ──────────────────────────────────────

/// How long entries handle overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleOverflowMode {
    /// Horizontal scroll for long entries. Fixed row height.
    HorizontalScroll,
    /// Wrap entries at viewport edge. Variable row height.
    /// Requires Phase 6 variable-height virtualization.
    Wrap,
}

impl Default for ConsoleOverflowMode {
    fn default() -> Self {
        ConsoleOverflowMode::HorizontalScroll
    }
}

/// SPEC-HAL-305 §12: Interaction policy governing Console behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsolePolicy {
    /// Scroll only. No text selection, no search.
    OutputOnly,
    /// OutputOnly + text selection via click+drag. Ctrl+C copies selection.
    Selectable,
    /// Selectable + Ctrl+F search bar. Level/source filter.
    Interactive,
}

impl Default for ConsolePolicy {
    fn default() -> Self {
        ConsolePolicy::OutputOnly
    }
}

/// SPEC-HAL-305 §11.5: Console viewing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleViewMode {
    /// Auto-follow tail. Live data from SlidingWindow.
    Live,
    /// Frozen snapshot. User scrolls freely within cloned history.
    Review,
}

/// SPEC-HAL-305 §13.2: Position within console text.
/// Two-level: entry index + byte offset within concatenated span text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsoleTextPosition {
    pub entry_index: usize,
    pub byte_offset: usize,
}

impl PartialOrd for ConsoleTextPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ConsoleTextPosition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.entry_index
            .cmp(&other.entry_index)
            .then(self.byte_offset.cmp(&other.byte_offset))
    }
}

/// SPEC-HAL-305 §13.2: Text selection across console entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleTextSelection {
    pub anchor: ConsoleTextPosition,
    pub head: ConsoleTextPosition,
}

impl ConsoleTextSelection {
    /// Ordered (start, end) where start <= end.
    pub fn ordered(&self) -> (ConsoleTextPosition, ConsoleTextPosition) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

pub struct ConsoleConfig {
    /// SPEC-HAL-305 §12: Interaction policy. Default: OutputOnly.
    pub policy: ConsolePolicy,
    /// Maximum lines in ring buffer. Default: 10_000.
    /// INV-305-0: Bounded memory (enforced by SlidingWindow).
    pub capacity: usize,
    /// Row height in logical pixels. Default: 16.0.
    pub row_height: f32,
    /// Font size. Default: 12.0.
    pub font_size: f32,
    /// Left padding. Default: 8.0.
    pub padding_left: f32,
    /// If true, widget signals needs_animation().
    pub live: bool,
    /// How long entries handle viewport overflow.
    /// Default: HorizontalScroll.
    pub overflow: ConsoleOverflowMode,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            policy: ConsolePolicy::default(),
            capacity: 10_000,
            row_height: 16.0,
            font_size: 12.0,
            padding_left: 8.0,
            live: true,
            overflow: ConsoleOverflowMode::default(),
        }
    }
}

// ─── Console State (shared between Console + ConsoleBody) ─

pub(crate) struct ConsoleState {
    /// INV-305-0: Ring buffer. SlidingWindow auto-evicts oldest.
    pub lines: SlidingWindow<ConsoleLine>,
    pub config: ConsoleConfig,
    /// Scroll Y offset from ScrollArea (set by Console in layout).
    pub scroll_y: f32,
    /// Viewport height (set by Console in layout).
    pub viewport_h: f32,
    /// Viewport width (set by Console in layout).
    pub viewport_w: f32,
    /// Visible range engine. Uses RowHeights for O(1) fixed / O(log N) variable.
    pub virtualization: VirtualizationEngine,
    /// Height oracle. Fixed-height mode for Phase 1.
    pub row_heights: RowHeights,
    /// Identity sorted indices — [0, 1, 2, ..., len-1].
    /// Resized on line count change. No sorting.
    pub sorted_indices: Vec<usize>,
    /// Topology stamp — incremented on push/clear.
    pub topology_stamp: u64,
    /// Cached visible range (first, last_exclusive, y_offset).
    pub visible_range: (usize, usize, f32),
    /// Maximum content width across all measured entries (for horizontal scroll).
    pub max_content_width: f32,
    /// SPEC-HAL-305 §11.5: Current viewing mode.
    pub view_mode: ConsoleViewMode,
    /// SPEC-HAL-305 §11.5: Frozen snapshot of entire window (Review mode only).
    pub snapshot: Option<Vec<ConsoleLine>>,
    /// Monotonic push counter. Incremented on every push.
    pub push_count: u64,
    /// Push generation represented by `row_heights`.
    row_heights_push_count: u64,
    /// Active view represented by `row_heights`.
    row_heights_view_mode: ConsoleViewMode,
    /// A visible wrapped entry replaced an estimate with an exact height.
    wrap_height_correction_pending: bool,
    /// Push count at the moment the snapshot was taken.
    pub snapshot_push_count: u64,
    /// SPEC-HAL-305 §13.2: Active text selection.
    pub text_selection: Option<ConsoleTextSelection>,
    /// Horizontal scroll offset (synced from ScrollArea for hit-testing).
    pub scroll_x: f32,
}

impl ConsoleState {
    fn new(config: ConsoleConfig) -> Self {
        let capacity = config.capacity;
        let row_height = config.row_height;
        Self {
            lines: SlidingWindow::new(capacity),
            config,
            scroll_y: 0.0,
            viewport_h: 0.0,
            viewport_w: 0.0,
            virtualization: VirtualizationEngine::new(),
            row_heights: RowHeights::with_fixed_height(row_height),
            sorted_indices: Vec::new(),
            topology_stamp: 0,
            visible_range: (0, 0, 0.0),
            max_content_width: 0.0,
            view_mode: ConsoleViewMode::Live,
            snapshot: None,
            push_count: 0,
            row_heights_push_count: 0,
            row_heights_view_mode: ConsoleViewMode::Live,
            wrap_height_correction_pending: false,
            snapshot_push_count: 0,
            text_selection: None,
            scroll_x: 0.0,
        }
    }

    /// Line count for the active view (snapshot in Review, live in Live).
    pub fn effective_line_count(&self) -> usize {
        match &self.snapshot {
            Some(snap) => snap.len(),
            None => self.lines.len(),
        }
    }

    /// Sync sorted_indices to match current line count, then compute visible range.
    pub fn compute_visible_range(&mut self) {
        let line_count = self.effective_line_count();

        // Resize identity indices if line count changed.
        if self.sorted_indices.len() != line_count {
            self.sorted_indices.clear();
            self.sorted_indices.extend(0..line_count);
            self.topology_stamp += 1;
        }

        let viewport_bottom = self.scroll_y + self.viewport_h;
        self.visible_range = self.virtualization.compute_visible_range(
            self.scroll_y,
            viewport_bottom,
            &self.row_heights,
            &self.sorted_indices,
            self.topology_stamp,
        );
    }

    fn reset_wrap_row_heights(&mut self) {
        let line_count = self.effective_line_count();
        let estimate = self.config.row_height;
        self.row_heights = RowHeights::with_variable_heights();
        for _ in 0..line_count {
            self.row_heights.push_with_height(estimate);
        }
        self.row_heights_push_count = self.push_count;
        self.row_heights_view_mode = self.view_mode;
        self.topology_stamp += 1;
    }

    fn sync_wrap_row_heights(&mut self, width_changed: bool) -> bool {
        if width_changed
            || self.row_heights.fixed_height().is_some()
            || self.row_heights_view_mode != self.view_mode
        {
            self.reset_wrap_row_heights();
            return true;
        }

        let line_count = self.effective_line_count();
        let estimate = self.config.row_height;
        if self.view_mode == ConsoleViewMode::Review {
            let changed = self.row_heights.len() != line_count;
            if changed {
                self.reset_wrap_row_heights();
            } else {
                self.row_heights_push_count = self.push_count;
            }
            return changed;
        }

        let Ok(new_entries) =
            usize::try_from(self.push_count.saturating_sub(self.row_heights_push_count))
        else {
            self.reset_wrap_row_heights();
            return true;
        };
        let old_len = self.row_heights.len();
        let Some(pre_eviction_len) = old_len.checked_add(new_entries) else {
            self.reset_wrap_row_heights();
            return true;
        };
        let evicted = pre_eviction_len.saturating_sub(line_count);
        if evicted > old_len || pre_eviction_len.saturating_sub(evicted) != line_count {
            self.reset_wrap_row_heights();
            return true;
        }

        for _ in 0..evicted {
            self.row_heights.remove(0);
        }
        for _ in 0..new_entries {
            self.row_heights.push_with_height(estimate);
        }
        self.row_heights_push_count = self.push_count;
        evicted > 0 || new_entries > 0
    }

    fn total_content_height(&self) -> f32 {
        (0..self.effective_line_count())
            .map(|index| self.row_heights.height_at(index))
            .sum()
    }
}

// ─── Console Update (channel-based) ──────────────────────

pub enum ConsoleUpdate {
    Line(ConsoleLine),
    Clear,
}

// ─── Console Widget (container) ──────────────────────────

pub struct Console {
    id: WidgetInternalId,
    pub(crate) state: Rc<RefCell<ConsoleState>>,
    /// ScrollArea child NodeId (for programmatic scroll).
    scroll_area_id: Option<NodeId>,
    /// Pending scroll-to-bottom request (applied in layout).
    pending_scroll_to_bottom: bool,
    /// INV-305-7: manual_scroll lives on Console container, not shared state.
    manual_scroll: bool,
    /// Channel receiver for cross-thread updates.
    update_rx: Option<Receiver<ConsoleUpdate>>,
    /// Track line count to detect new content in layout.
    /// Push count at last step 2 detection. Detects new content at capacity.
    last_push_count: u64,
    /// True when update() drained content this frame. Cleared next frame.
    has_pending_activity: bool,
    /// Value last passed to scroll_to_y in step 3.
    /// Used to detect external (user) scroll: if ScrollArea target
    /// differs from this value, the user scrolled.
    last_set_target: f32,
    /// Cached pill rect for hit-testing (set in paint_overlay, read in handle_event).
    /// In LOCAL coordinates (relative to Console's origin).
    review_pill_rect: Cell<Option<RectF>>,
    /// Review indicator: current opacity (0.0 = hidden, 1.0 = visible).
    indicator_alpha: f32,
    /// Review indicator: countdown timer before fade starts (seconds).
    indicator_timer: f32,
    /// Review indicator: mouse is hovering directly over the indicator.
    indicator_hovered: bool,
    /// One-shot: reset horizontal scroll on next step 3 execution.
    pending_horizontal_reset: bool,
    /// Text selection drag in progress.
    selection_dragging: bool,
    /// True if Review mode was entered automatically for selection stability.
    /// Return to Live when selection is cleared.
    selection_auto_review: bool,
}

impl Console {
    pub fn new() -> Self {
        Self::with_config(ConsoleConfig::default())
    }

    pub fn with_config(config: ConsoleConfig) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state: Rc::new(RefCell::new(ConsoleState::new(config))),
            scroll_area_id: None,
            pending_scroll_to_bottom: true,
            manual_scroll: false,
            update_rx: None,
            last_push_count: 0,
            has_pending_activity: true,
            last_set_target: 0.0,
            review_pill_rect: Cell::new(None),
            indicator_alpha: 0.0,
            indicator_timer: 0.0,
            indicator_hovered: false,
            pending_horizontal_reset: false,
            selection_dragging: false,
            selection_auto_review: false,
        }
    }

    pub fn with_capacity(self, capacity: usize) -> Self {
        {
            let mut state = self.state.borrow_mut();
            state.config.capacity = capacity;
            state.lines = SlidingWindow::new(capacity);
        }
        self
    }

    /// Create the inner body widget (shares state via Rc).
    pub fn create_body(&self) -> ConsoleBody {
        ConsoleBody::new(Rc::clone(&self.state))
    }

    /// Create a channel for cross-thread updates.
    /// Returns (self, sender).
    pub fn with_update_channel(mut self) -> (Self, crossbeam_channel::Sender<ConsoleUpdate>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.update_rx = Some(rx);
        (self, tx)
    }

    /// Set the ScrollArea NodeId (for auto-scroll-to-bottom).
    pub fn set_scroll_area(&mut self, id: NodeId) {
        self.scroll_area_id = Some(id);
    }

    /// Push a line directly (same-thread).
    /// SlidingWindow auto-evicts oldest when at capacity.
    pub fn push_line(&self, line: ConsoleLine) {
        let mut state = self.state.borrow_mut();
        state.lines.push(line);
        state.push_count += 1;
    }

    /// Convenience: push a plain text line with optional level.
    pub fn push_text(&self, text: &str, level: Option<ConsoleLevel>) {
        self.push_line(ConsoleLine {
            spans: vec![ConsoleSpan {
                text: text.to_string(),
                style: ConsoleStyle::default(),
            }],
            level,
        });
    }

    /// Clear all lines by replacing with a fresh SlidingWindow.
    pub fn clear(&self) {
        let mut state = self.state.borrow_mut();
        let capacity = state.config.capacity;
        state.lines = SlidingWindow::new(capacity);
    }

    /// Exact hit-test for wrap mode: collect visible lines, compute exact Y walk.
    /// Returns (content_x, content_y, font_size, is_wrap, entry_index, entry_top_y, line).
    fn hit_test_at(
        &self,
        local_pos: (f32, f32),
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) -> (f32, f32, f32, bool, usize, f32, Option<ConsoleLine>) {
        let state = self.state.borrow();
        let content_x = local_pos.0 + state.scroll_x - state.config.padding_left;
        let content_y = local_pos.1 + state.scroll_y;
        let font_size = state.config.font_size;
        let is_wrap = state.config.overflow == ConsoleOverflowMode::Wrap;
        let row_h = state.config.row_height;
        let line_count = state.effective_line_count();
        let visible_range = state.visible_range;
        let available_width = (state.viewport_w - state.config.padding_left).max(1.0);

        if !is_wrap {
            // Fixed height: simple O(1).
            let ei = (content_y / row_h).floor().max(0.0) as usize;
            let ei = ei.min(line_count.saturating_sub(1));
            let entry_top_y = ei as f32 * row_h;
            let line = match &state.snapshot {
                Some(snap) => snap.get(ei).cloned(),
                None => state.lines.get(ei).cloned(),
            };
            return (
                content_x,
                content_y,
                font_size,
                is_wrap,
                ei,
                entry_top_y,
                line,
            );
        }

        // Wrap mode: collect visible lines, drop state, walk with exact heights.
        let (first, last, _) = visible_range;
        let first = first.min(line_count);
        let last = last.min(line_count);

        let mut visible_lines: Vec<(usize, ConsoleLine)> = Vec::new();
        for i in first..last {
            let line = match &state.snapshot {
                Some(snap) => snap.get(i).cloned(),
                None => state.lines.get(i).cloned(),
            };
            if let Some(line) = line {
                visible_lines.push((i, line));
            }
        }
        drop(state);

        let (ei, entry_top_y) = entry_index_from_y_exact(
            content_y,
            line_count,
            true,
            row_h,
            visible_range,
            &visible_lines,
            available_width,
            font_size,
            text_engine,
        );

        let line = visible_lines
            .iter()
            .find(|(idx, _)| *idx == ei)
            .map(|(_, l)| l.clone());

        (
            content_x,
            content_y,
            font_size,
            is_wrap,
            ei,
            entry_top_y,
            line,
        )
    }

    /// Change font size at runtime.
    pub fn set_font_size(&self, size: f32) {
        let mut state = self.state.borrow_mut();
        state.config.font_size = size;
        state.config.row_height = (size + 4.0).ceil();
        state.topology_stamp += 1;
        state.max_content_width = 0.0;
        let new_row_h = state.config.row_height;
        if state.config.overflow == ConsoleOverflowMode::Wrap {
            state.reset_wrap_row_heights();
        } else if state.row_heights.fixed_height().is_some() {
            state.row_heights.set_fixed_height(new_row_h);
        }
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

/// Find entry index from content-space Y using variable row heights.
/// Returns (entry_index, entry_top_y).
/// Find entry index from content-space Y.
///
/// In wrap mode, walks visible entries with exact wrapped heights
/// (same computation as paint). In non-wrap mode, O(1) division.
///
/// Returns (entry_index, entry_top_y).
///
/// Requires: lines already cloned from state (caller handles RefCell).
fn entry_index_from_y_exact(
    content_y: f32,
    line_count: usize,
    is_wrap: bool,
    fixed_row_height: f32,
    visible_range: (usize, usize, f32),
    visible_lines: &[(usize, ConsoleLine)],
    available_width: f32,
    font_size: f32,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
) -> (usize, f32) {
    if line_count == 0 {
        return (0, 0.0);
    }

    if !is_wrap {
        let ei = (content_y / fixed_row_height).floor().max(0.0) as usize;
        let ei = ei.min(line_count.saturating_sub(1));
        return (ei, ei as f32 * fixed_row_height);
    }

    // Walk visible entries with exact heights.
    let (_, _, first_y_offset) = visible_range;
    let mut y = first_y_offset;

    for (idx, line) in visible_lines {
        let wrapped = compute_wrapped_lines(line, available_width, font_size, text_engine);
        let exact_h = wrapped.len().max(1) as f32 * fixed_row_height;

        if content_y < y + exact_h {
            return (*idx, y);
        }
        y += exact_h;
    }

    // Click below all visible entries — return last one.
    if let Some((idx, _)) = visible_lines.last() {
        (*idx, y - fixed_row_height)
    } else {
        let ei = (content_y / fixed_row_height).floor().max(0.0) as usize;
        let ei = ei.min(line_count.saturating_sub(1));
        (ei, ei as f32 * fixed_row_height)
    }
}

/// A segment of text within a wrapped visual line.
#[derive(Clone)]
pub(crate) struct WrappedSegment {
    span_index: usize,
    byte_start: usize,
    byte_end: usize,
    x_offset: f32,
    width: f32,
}

/// A single visual line within a wrapped entry.
#[derive(Clone)]
pub(crate) struct WrappedLine {
    segments: Vec<WrappedSegment>,
}

/// SPEC-PM-016 (Console port): a shaped run plus the frame generation in which
/// it was last used. Mirrors ListView's `CachedRun`. Keyed by the exact shaped
/// text; `gen` drives hard-cap LRU eviction, and a font-size change clears the
/// whole cache.
struct CachedConsoleRun {
    run: crate::core::text_engine::GlyphRun,
    r#gen: u64,
}

const CONSOLE_SHAPE_CACHE_MAX_ENTRIES: usize = 4096;

#[derive(Hash, Eq, PartialEq)]
struct ConsoleWrapCacheKey {
    available_width_bits: u32,
    font_size_bits: u32,
    spans: String,
}

struct CachedWrappedConsoleLine {
    lines: Vec<WrappedLine>,
    r#gen: u64,
}

fn trim_console_shape_cache(cache: &mut std::collections::HashMap<String, CachedConsoleRun>) {
    if cache.len() <= CONSOLE_SHAPE_CACHE_MAX_ENTRIES {
        return;
    }

    let remove_count = cache.len() - CONSOLE_SHAPE_CACHE_MAX_ENTRIES;
    let mut oldest: Vec<(String, u64)> = cache
        .iter()
        .map(|(key, entry)| (key.clone(), entry.r#gen))
        .collect();
    oldest.sort_unstable_by_key(|(_, generation)| *generation);

    for (key, _) in oldest.into_iter().take(remove_count) {
        cache.remove(&key);
    }
}

fn console_wrap_cache_key(
    line: &ConsoleLine,
    available_width: f32,
    font_size: f32,
) -> ConsoleWrapCacheKey {
    let mut spans = String::new();
    spans.push_str(&line.spans.len().to_string());
    spans.push('|');
    for span in &line.spans {
        spans.push_str(&span.text.len().to_string());
        spans.push(':');
        spans.push_str(&span.text);
        spans.push('|');
    }

    ConsoleWrapCacheKey {
        available_width_bits: available_width.to_bits(),
        font_size_bits: font_size.to_bits(),
        spans,
    }
}

/// Ensure `text` is shaped in `cache` (shaping only on a miss), stamp the entry
/// with the current frame `gen`, and return its width. No clone — used by the
/// wrap-measurement pass, which needs only widths.
fn cached_shape_width(
    cache: &mut std::collections::HashMap<String, CachedConsoleRun>,
    r#gen: u64,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
    text: &str,
    font_size: f32,
) -> f32 {
    if let Some(entry) = cache.get_mut(text) {
        entry.r#gen = r#gen;
        return entry.run.width;
    }
    let run = text_engine.shape(text, font_size);
    let width = run.width;
    cache.insert(text.to_string(), CachedConsoleRun { run, r#gen });
    width
}

/// Ensure `text` is shaped in `cache` (shaping only on a miss), stamp the entry
/// with the current frame `gen`, and return an owned clone of the run. Cloning
/// keeps the caller free of any borrow into the cache (`GlyphRun: Clone`), which
/// keeps the paint borrow choreography trivial; the clone is cheap relative to
/// shaping.
fn cached_shape(
    cache: &mut std::collections::HashMap<String, CachedConsoleRun>,
    r#gen: u64,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
    text: &str,
    font_size: f32,
) -> crate::core::text_engine::GlyphRun {
    if let Some(entry) = cache.get_mut(text) {
        entry.r#gen = r#gen;
        return entry.run.clone();
    }
    let run = text_engine.shape(text, font_size);
    cache.insert(
        text.to_string(),
        CachedConsoleRun {
            run: run.clone(),
            r#gen,
        },
    );
    run
}

/// Compute wrapped visual lines for a console entry (uncached convenience form
/// for cold callers — hit-testing and measurement). Delegates to the cached
/// form with a throwaway cache so the wrap logic stays single-sourced.
fn compute_wrapped_lines(
    line: &ConsoleLine,
    available_width: f32,
    font_size: f32,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
) -> Vec<WrappedLine> {
    let mut throwaway = std::collections::HashMap::new();
    compute_wrapped_lines_cached(
        line,
        available_width,
        font_size,
        text_engine,
        &mut throwaway,
        0,
    )
}

/// SINGLE SOURCE OF TRUTH for wrap decisions. Shapes each word and whitespace
/// run through `cache` (keyed by text), reusing shapes computed on prior frames.
fn compute_wrapped_lines_cached(
    line: &ConsoleLine,
    available_width: f32,
    font_size: f32,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
    cache: &mut std::collections::HashMap<String, CachedConsoleRun>,
    r#gen: u64,
) -> Vec<WrappedLine> {
    let mut lines: Vec<WrappedLine> = Vec::new();
    let mut current_line = WrappedLine {
        segments: Vec::new(),
    };
    let mut x = 0.0_f32;

    for (span_idx, span) in line.spans.iter().enumerate() {
        let span_text = &span.text;
        let mut pos = 0_usize;

        while pos < span_text.len() {
            let remaining = &span_text[pos..];
            let word_end = remaining
                .find(char::is_whitespace)
                .unwrap_or(remaining.len());

            if word_end > 0 {
                let word = &remaining[..word_end];
                let word_width = cached_shape_width(cache, r#gen, text_engine, word, font_size);

                if x > 0.0 && x + word_width > available_width {
                    lines.push(std::mem::replace(
                        &mut current_line,
                        WrappedLine {
                            segments: Vec::new(),
                        },
                    ));
                    x = 0.0;
                }

                current_line.segments.push(WrappedSegment {
                    span_index: span_idx,
                    byte_start: pos,
                    byte_end: pos + word_end,
                    x_offset: x,
                    width: word_width,
                });
                x += word_width;
                pos += word_end;
            }

            if pos >= span_text.len() {
                break;
            }

            let remaining = &span_text[pos..];
            let ws_end = remaining
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(remaining.len());
            if ws_end > 0 {
                let ws = &remaining[..ws_end];
                let ws_width = cached_shape_width(cache, r#gen, text_engine, ws, font_size);

                current_line.segments.push(WrappedSegment {
                    span_index: span_idx,
                    byte_start: pos,
                    byte_end: pos + ws_end,
                    x_offset: x,
                    width: ws_width,
                });
                x += ws_width;
                pos += ws_end;
            }
        }
    }

    lines.push(current_line);
    if lines.is_empty() {
        lines.push(WrappedLine {
            segments: Vec::new(),
        });
    }

    lines
}

fn console_scene_run(
    text: &str,
    width: f32,
    font_size: f32,
    metrics: crate::core::text_engine::TextMetrics,
) -> crate::core::text_engine::GlyphRun {
    crate::core::text_engine::GlyphRun {
        binding: None,
        clusters: Vec::new(),
        glyphs: Vec::new(),
        width,
        height: metrics.line_height,
        text: text.to_string(),
        font_size,
        ascent: metrics.ascent,
        descent: metrics.descent,
    }
}

fn cached_wrapped_lines(
    line: &ConsoleLine,
    available_width: f32,
    font_size: f32,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
    shape_cache: &mut std::collections::HashMap<String, CachedConsoleRun>,
    wrap_cache: &mut std::collections::HashMap<ConsoleWrapCacheKey, CachedWrappedConsoleLine>,
    r#gen: u64,
) -> Vec<WrappedLine> {
    let key = console_wrap_cache_key(line, available_width, font_size);
    if let Some(entry) = wrap_cache.get_mut(&key) {
        entry.r#gen = r#gen;
        return entry.lines.clone();
    }

    let lines = compute_wrapped_lines_cached(
        line,
        available_width,
        font_size,
        text_engine,
        shape_cache,
        r#gen,
    );
    wrap_cache.insert(
        key,
        CachedWrappedConsoleLine {
            lines: lines.clone(),
            r#gen,
        },
    );
    lines
}

/// Hit-test byte offset within a wrapped console entry.
fn byte_offset_in_wrapped_entry(
    line: &ConsoleLine,
    content_x: f32,
    within_entry_y: f32,
    available_width: f32,
    base_line_height: f32,
    font_size: f32,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
) -> usize {
    let wrapped = compute_wrapped_lines(line, available_width, font_size, text_engine);
    let target_line = (within_entry_y / base_line_height).floor().max(0.0) as usize;
    let target_line = target_line.min(wrapped.len().saturating_sub(1));

    let span_byte_starts: Vec<usize> = {
        let mut offsets = Vec::with_capacity(line.spans.len());
        let mut cum = 0_usize;
        for span in &line.spans {
            offsets.push(cum);
            cum += span.text.len();
        }
        offsets
    };

    let wline = &wrapped[target_line];
    if wline.segments.is_empty() {
        return line.spans.iter().map(|s| s.text.len()).sum();
    }

    for seg in &wline.segments {
        if content_x < seg.x_offset + seg.width {
            let local_x = content_x - seg.x_offset;
            let seg_text = &line.spans[seg.span_index].text[seg.byte_start..seg.byte_end];
            let run = text_engine.shape(seg_text, font_size);
            let off = hit_test_run(&run, local_x);
            return span_byte_starts[seg.span_index] + seg.byte_start + off;
        }
    }

    let last = wline.segments.last().unwrap();
    span_byte_starts[last.span_index] + last.byte_end
}

/// Extract selected text as plain string. Entries joined by newlines.
fn extract_selection_text(
    selection: &ConsoleTextSelection,
    get_line: impl Fn(usize) -> Option<ConsoleLine>,
) -> String {
    let (start, end) = selection.ordered();
    let mut result = String::new();

    for i in start.entry_index..=end.entry_index {
        let line = match get_line(i) {
            Some(l) => l,
            None => continue,
        };
        let full_text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
        let slice_start = if i == start.entry_index {
            start.byte_offset
        } else {
            0
        };
        let slice_end = if i == end.entry_index {
            end.byte_offset
        } else {
            full_text.len()
        };
        let slice_start = slice_start.min(full_text.len());
        let slice_end = slice_end.min(full_text.len());

        if let Some(s) = full_text.get(slice_start..slice_end) {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(s);
        }
    }
    result
}

impl Widget for Console {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width;

        // Store viewport height for ConsoleBody's visible range computation.
        // ConsoleBody receives unbounded max_height from ScrollArea — that is
        // the content constraint, NOT the viewport. The viewport is Console's
        // own height.
        //
        // Mirror List (INV-LIST2-VIEWPORT-SIZED-WIDGET): adopt only a finite,
        // positive viewport height. During a Flex natural-size measure pass
        // (INV-FLEX-MEASURE-UNBOUNDED) the parent forwards max_height = INFINITY;
        // keep the last known viewport so the rest of layout — including the
        // auto-scroll target — never operates on a bogus height (which would
        // collapse the scroll target to 0).
        let wrap_width_changed = {
            let mut state = self.state.borrow_mut();
            let width_changed = state.config.overflow == ConsoleOverflowMode::Wrap
                && (state.viewport_w - w).abs() > 0.5;
            if constraints.max_height.is_finite() && constraints.max_height > 0.0 {
                state.viewport_h = constraints.max_height;
            }
            state.viewport_w = w;
            width_changed
        };
        let h = self.state.borrow().viewport_h;

        let (wrap_geometry_changed, height_correction_pending) = {
            let mut state = self.state.borrow_mut();
            let geometry_changed = state.config.overflow == ConsoleOverflowMode::Wrap
                && state.sync_wrap_row_heights(wrap_width_changed);
            let correction_pending = state.wrap_height_correction_pending;
            state.wrap_height_correction_pending = false;
            (geometry_changed, correction_pending)
        };
        if height_correction_pending && !self.manual_scroll {
            self.pending_scroll_to_bottom = true;
        }

        // Decide whether the subtree needs fresh content geometry before laying
        // it out. Dirtying it after this point would leave retained paint with
        // invalid completed geometry.
        let content_changed = self.state.borrow().push_count != self.last_push_count;
        if self.pending_scroll_to_bottom
            || (content_changed && !self.manual_scroll)
            || wrap_geometry_changed
        {
            if let Some(sa_id) = self.scroll_area_id {
                _cx.tree.mark_layout_dirty(sa_id);
                for child_id in _cx.tree.children(sa_id) {
                    _cx.tree.mark_layout_dirty(child_id);
                }
            }
        }

        // Layout ScrollArea child.
        if !children.is_empty() {
            let body_constraints = Constraints::new(w, w, h, h);
            _cx.layout_child(children[0], body_constraints);
            _cx.set_child_rect(children[0], RectF::new(0.0, 0.0, w, h));
        }

        // Auto-scroll needs a viewport height. `h` is the CACHED viewport (kept
        // from the last finite pass above), so it remains valid even when THIS
        // pass is an unbounded Flex measure — and that is exactly the pass a
        // streamed-line relayout arrives on, so the scroll logic must run then,
        // not be skipped. Bail only before any finite viewport has ever been
        // seen (cached h still 0), when there is nothing to scroll to yet.
        if h <= 0.0 {
            return LayoutResult::fit(Size::new(w, h));
        }

        // ── Auto-scroll logic ──────────────────────────────
        //
        // CRITICAL ORDER:
        // 1. Detect manual scroll FIRST (read target BEFORE we touch anything)
        // 2. Detect new content → set pending only if !manual_scroll
        // 3. Execute pending scroll
        //
        // If step 1 ran AFTER step 3, auto-scroll would override the user's
        // drag position before detection. That's the scrollbar drag bug.

        // Step 1: Detect manual scroll.
        // SKIP when pending_scroll_to_bottom is set — the scroll position is stale
        // and about to be reset by step 3. Detecting manual scroll on stale state
        // would immediately re-lock after return-to-Live.
        if !self.pending_scroll_to_bottom {
            if let Some(sa_id) = self.scroll_area_id {
                if let Some(node) = _cx.tree.node(sa_id) {
                    if let Some(sa) = node.widget.as_any().downcast_ref::<ScrollArea>() {
                        let target_y = sa.target_scroll_y();
                        self.state.borrow_mut().scroll_x = sa.target_scroll_x();
                        let state = self.state.borrow();
                        let content_h = if state.config.overflow == ConsoleOverflowMode::Wrap {
                            state.total_content_height()
                        } else {
                            state.effective_line_count() as f32 * state.config.row_height
                        };
                        let max_scroll = (content_h - h).max(0.0);
                        let at_bottom = max_scroll < 1.0
                            || (max_scroll - target_y) < state.config.row_height * 1.5;
                        let user_scrolled = (target_y - self.last_set_target).abs() > 0.5;

                        if user_scrolled && !at_bottom {
                            self.manual_scroll = true;
                        } else if at_bottom && self.manual_scroll {
                            self.manual_scroll = false;
                        }
                    }
                }
            }
        }

        // Step 1b: Enter Review mode when manual scroll detected.
        // INV-305-10: Full-window clone — data source's concern.
        // INV-305-11: Clone does not block live window.
        if self.manual_scroll {
            let view_mode = self.state.borrow().view_mode;
            if view_mode == ConsoleViewMode::Live {
                let mut state = self.state.borrow_mut();
                let entries: Vec<ConsoleLine> = state.lines.iter().cloned().collect();
                let count = entries.len();
                state.snapshot_push_count = state.push_count;
                state.snapshot = Some(entries);
                state.view_mode = ConsoleViewMode::Review;
                // Reset virtualization indices for snapshot dimensions.
                state.sorted_indices.clear();
                state.sorted_indices.extend(0..count);
                state.topology_stamp += 1;
                drop(state);

                self.indicator_alpha = 1.0;
                self.indicator_timer = 2.0;
                self.indicator_hovered = false;
            }
        }

        // Step 2: Detect new content.
        {
            let state = self.state.borrow();
            let push_count = state.push_count;
            drop(state);
            if push_count != self.last_push_count {
                self.last_push_count = push_count;
                if !self.manual_scroll {
                    self.pending_scroll_to_bottom = true;
                }
            }
        }

        // Step 3: Execute pending scroll-to-bottom (one-shot — INV-305-12).
        if self.pending_scroll_to_bottom {
            self.pending_scroll_to_bottom = false;

            // Compute target FIRST, drop state borrow.
            let target = {
                let state = self.state.borrow();
                let content_h = if state.config.overflow == ConsoleOverflowMode::Wrap {
                    state.total_content_height()
                } else {
                    state.effective_line_count() as f32 * state.config.row_height
                };
                (content_h - h).max(0.0)
            }; // state dropped here.

            // Capture horizontal reset flag before clearing it.
            let do_horizontal_reset = self.pending_horizontal_reset;
            if do_horizontal_reset {
                self.pending_horizontal_reset = false;
            }

            // Execute scroll.
            if let Some(sa_id) = self.scroll_area_id {
                if let Some(node) = _cx.tree.node_mut(sa_id) {
                    if let Some(sa) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                        if do_horizontal_reset {
                            sa.scroll_to(0.0, target);
                        } else {
                            sa.scroll_to_y(target);
                        }
                        self.last_set_target = target;
                    }
                }
            }

            // Sync visible_range to match the new scroll position.
            {
                let mut state = self.state.borrow_mut();
                state.scroll_y = target;
                if do_horizontal_reset {
                    state.scroll_x = 0.0;
                }
                state.compute_visible_range();
            }
        }

        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene
            .push_rect(rect, cx.theme.colors.surfaces.content, 0, None);
    }

    fn update(&mut self, _dt: f32, _cx: &mut UpdateContext) {
        self.has_pending_activity = false;

        // Drain update channel.
        let updates: Vec<ConsoleUpdate> = self
            .update_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();

        if !updates.is_empty() {
            self.has_pending_activity = true;
        }

        let mut new_lines = false;
        for update in updates {
            match update {
                ConsoleUpdate::Line(line) => {
                    self.push_line(line);
                    new_lines = true;
                }
                ConsoleUpdate::Clear => {
                    self.clear();
                }
            }
        }

        if new_lines && !self.manual_scroll {
            self.pending_scroll_to_bottom = true;
        }
        if self.has_pending_activity {
            if self.manual_scroll {
                _cx.invalidate_paint();
            } else {
                _cx.invalidate_layout();
            }
        }

        if self.state.borrow().view_mode == ConsoleViewMode::Review {
            // INV-305-14: this is the sole cue and return-to-Live control for
            // the frozen snapshot, so it remains visible for all of Review.
            self.indicator_alpha = 1.0;
            self.indicator_timer = 0.0;
        } else {
            self.indicator_alpha = 0.0;
            self.indicator_timer = 0.0;
        }
    }

    fn needs_animation(&self) -> bool {
        self.state.borrow().config.live
            || self.has_pending_activity
            || self.pending_scroll_to_bottom
            || self.indicator_alpha > 0.001
            || self.indicator_timer > 0.0
            || self.selection_dragging
            || self.update_rx.as_ref().map_or(false, |rx| !rx.is_empty())
    }

    fn handle_event(&mut self, _cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let policy = self.state.borrow().config.policy;
        let is_selectable =
            policy == ConsolePolicy::Selectable || policy == ConsolePolicy::Interactive;

        match event {
            // ── PointerDown ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == crate::core::event::PointerEventKind::Down =>
            {
                // 1. Indicator click (Review mode only).
                if self.state.borrow().view_mode == ConsoleViewMode::Review {
                    if let Some(indicator_rect) = self.review_pill_rect.get() {
                        let (lx, ly) = pe.local_pos;
                        if lx >= indicator_rect.x
                            && lx <= indicator_rect.x + indicator_rect.width
                            && ly >= indicator_rect.y
                            && ly <= indicator_rect.y + indicator_rect.height
                        {
                            self.manual_scroll = false;
                            self.pending_scroll_to_bottom = true;
                            {
                                let mut state = self.state.borrow_mut();
                                state.snapshot = None;
                                state.view_mode = ConsoleViewMode::Live;
                                state.topology_stamp += 1;
                                state.max_content_width = 0.0;

                                state.text_selection = None;
                            }
                            self.indicator_alpha = 0.0;
                            self.indicator_timer = 0.0;
                            self.indicator_hovered = false;
                            self.pending_horizontal_reset = true;
                            self.selection_dragging = false;
                            self.selection_auto_review = false;
                            _cx.invalidate_layout();
                            return true;
                        }
                    }
                }

                // 2. Selection start (Selectable, primary button).
                if is_selectable && pe.button == Some(PointerButton::Primary) {
                    let (content_x, cy_val, font_size, is_wrap, entry_index, entry_top_y, line) =
                        self.hit_test_at(pe.local_pos, _cx.text_engine());

                    let byte_offset = if let Some(ref line) = line {
                        if is_wrap {
                            let state = self.state.borrow();
                            let within_y = cy_val - entry_top_y;
                            let available_width =
                                (state.viewport_w - state.config.padding_left).max(1.0);
                            let base_lh = state.config.row_height;
                            drop(state);
                            byte_offset_in_wrapped_entry(
                                line,
                                content_x,
                                within_y,
                                available_width,
                                base_lh,
                                font_size,
                                _cx.text_engine(),
                            )
                        } else {
                            let mut cum_x = 0.0_f32;
                            let mut cum_bytes = 0_usize;
                            let mut found = None;
                            for span in &line.spans {
                                let run = _cx.text_engine().shape(&span.text, font_size);
                                if content_x < cum_x + run.width {
                                    let off = hit_test_run(&run, content_x - cum_x);
                                    found = Some(cum_bytes + off);
                                    break;
                                }
                                cum_x += run.width;
                                cum_bytes += span.text.len();
                            }
                            found.unwrap_or(cum_bytes)
                        }
                    } else {
                        0
                    };

                    let pos = ConsoleTextPosition {
                        entry_index,
                        byte_offset,
                    };

                    let selection = if pe.modifiers.shift {
                        let existing = self.state.borrow().text_selection.clone();
                        match existing {
                            Some(sel) => ConsoleTextSelection {
                                anchor: sel.anchor,
                                head: pos,
                            },
                            None => ConsoleTextSelection {
                                anchor: pos,
                                head: pos,
                            },
                        }
                    } else {
                        ConsoleTextSelection {
                            anchor: pos,
                            head: pos,
                        }
                    };

                    self.state.borrow_mut().text_selection = Some(selection);
                    self.selection_dragging = true;
                    _cx.capture_pointer(PointerButton::Primary);

                    // Auto-enter Review mode for selection stability.
                    if self.state.borrow().view_mode == ConsoleViewMode::Live {
                        let mut state = self.state.borrow_mut();
                        let entries: Vec<ConsoleLine> = state.lines.iter().cloned().collect();
                        let count = entries.len();
                        state.snapshot_push_count = state.push_count;
                        state.snapshot = Some(entries);
                        state.view_mode = ConsoleViewMode::Review;
                        state.sorted_indices.clear();
                        state.sorted_indices.extend(0..count);
                        state.topology_stamp += 1;
                        self.selection_auto_review = true;
                        self.manual_scroll = true;
                        _cx.invalidate_layout();
                    }

                    return true;
                }

                false
            }

            // ── DoubleClick: select word ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == crate::core::event::PointerEventKind::DoubleClick =>
            {
                if !is_selectable || pe.button != Some(PointerButton::Primary) {
                    return false;
                }

                let (content_x, cy_val, font_size, is_wrap, entry_index, entry_top_y, line) =
                    self.hit_test_at(pe.local_pos, _cx.text_engine());

                if let Some(ref line) = line {
                    let full_text: String = line.spans.iter().map(|s| s.text.as_str()).collect();

                    let byte_offset = if is_wrap {
                        let state = self.state.borrow();
                        let within_y = cy_val - entry_top_y;
                        let available_width =
                            (state.viewport_w - state.config.padding_left).max(1.0);
                        let base_lh = state.config.row_height;
                        drop(state);
                        byte_offset_in_wrapped_entry(
                            line,
                            content_x,
                            within_y,
                            available_width,
                            base_lh,
                            font_size,
                            _cx.text_engine(),
                        )
                    } else {
                        let mut cum_x = 0.0_f32;
                        let mut cum_bytes = 0_usize;
                        let mut found = full_text.len();
                        for span in &line.spans {
                            let run = _cx.text_engine().shape(&span.text, font_size);
                            if content_x < cum_x + run.width {
                                found = cum_bytes + hit_test_run(&run, content_x - cum_x);
                                break;
                            }
                            cum_x += run.width;
                            cum_bytes += span.text.len();
                        }
                        found
                    };

                    let (word_start, word_end) = find_word_boundaries(&full_text, byte_offset);

                    let selection = ConsoleTextSelection {
                        anchor: ConsoleTextPosition {
                            entry_index,
                            byte_offset: word_start,
                        },
                        head: ConsoleTextPosition {
                            entry_index,
                            byte_offset: word_end,
                        },
                    };
                    self.state.borrow_mut().text_selection = Some(selection);
                    self.selection_dragging = false;
                    return true;
                }

                false
            }

            // ── TripleClick: select entire entry ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == crate::core::event::PointerEventKind::TripleClick =>
            {
                if !is_selectable || pe.button != Some(PointerButton::Primary) {
                    return false;
                }

                let (_, _, _, _, entry_index, _, line) =
                    self.hit_test_at(pe.local_pos, _cx.text_engine());
                let total_bytes = line
                    .as_ref()
                    .map(|l| l.spans.iter().map(|s| s.text.len()).sum::<usize>())
                    .unwrap_or(0);

                let selection = ConsoleTextSelection {
                    anchor: ConsoleTextPosition {
                        entry_index,
                        byte_offset: 0,
                    },
                    head: ConsoleTextPosition {
                        entry_index,
                        byte_offset: total_bytes,
                    },
                };
                self.state.borrow_mut().text_selection = Some(selection);
                self.selection_dragging = false;
                true
            }

            // ── PointerMove ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == crate::core::event::PointerEventKind::Move =>
            {
                // Indicator hover (existing).
                if self.state.borrow().view_mode == ConsoleViewMode::Review {
                    self.indicator_timer = 2.0;
                    self.indicator_alpha = 1.0;
                    if let Some(indicator_rect) = self.review_pill_rect.get() {
                        let (lx, ly) = pe.local_pos;
                        self.indicator_hovered = lx >= indicator_rect.x
                            && lx <= indicator_rect.x + indicator_rect.width
                            && ly >= indicator_rect.y
                            && ly <= indicator_rect.y + indicator_rect.height;
                    }
                }

                // Selection drag.
                if self.selection_dragging && is_selectable {
                    let (content_x, cy_val, font_size, is_wrap, entry_index, entry_top_y, line) =
                        self.hit_test_at(pe.local_pos, _cx.text_engine());

                    let byte_offset = if let Some(ref line) = line {
                        if is_wrap {
                            let state = self.state.borrow();
                            let within_y = cy_val - entry_top_y;
                            let available_width =
                                (state.viewport_w - state.config.padding_left).max(1.0);
                            let base_lh = state.config.row_height;
                            drop(state);
                            byte_offset_in_wrapped_entry(
                                line,
                                content_x,
                                within_y,
                                available_width,
                                base_lh,
                                font_size,
                                _cx.text_engine(),
                            )
                        } else {
                            let mut cum_x = 0.0_f32;
                            let mut cum_bytes = 0_usize;
                            let mut found = None;
                            for span in &line.spans {
                                let run = _cx.text_engine().shape(&span.text, font_size);
                                if content_x < cum_x + run.width {
                                    let off = hit_test_run(&run, content_x - cum_x);
                                    found = Some(cum_bytes + off);
                                    break;
                                }
                                cum_x += run.width;
                                cum_bytes += span.text.len();
                            }
                            found.unwrap_or(cum_bytes)
                        }
                    } else {
                        0
                    };

                    let pos = ConsoleTextPosition {
                        entry_index,
                        byte_offset,
                    };
                    let mut state = self.state.borrow_mut();
                    if let Some(ref mut sel) = state.text_selection {
                        sel.head = pos;
                    }
                }

                false
            }

            // ── Scroll during selection drag: don't consume ──
            Event::Pointer(pe)
                if matches!(pe.kind, crate::core::event::PointerEventKind::Scroll { .. }) =>
            {
                false
            }

            // ── PointerUp ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == crate::core::event::PointerEventKind::Up =>
            {
                if self.selection_dragging && pe.button == Some(PointerButton::Primary) {
                    self.selection_dragging = false;
                    _cx.release_pointer();
                    return true;
                }
                false
            }

            // ── Keyboard ──
            Event::Key(ke) if phase == EventPhase::Target && ke.is_down && !ke.is_repeat => {
                match ke.key_code {
                    KeyCode::Space | KeyCode::End => {
                        self.manual_scroll = false;
                        self.pending_scroll_to_bottom = true;
                        {
                            let mut state = self.state.borrow_mut();
                            state.snapshot = None;
                            state.view_mode = ConsoleViewMode::Live;
                            state.topology_stamp += 1;
                            state.max_content_width = 0.0;

                            state.text_selection = None;
                        }
                        self.indicator_alpha = 0.0;
                        self.indicator_timer = 0.0;
                        self.indicator_hovered = false;
                        self.pending_horizontal_reset = true;
                        self.selection_dragging = false;
                        self.selection_auto_review = false;
                        _cx.invalidate_layout();
                        true
                    }
                    KeyCode::Escape => {
                        let mut state = self.state.borrow_mut();
                        if state.text_selection.is_some() {
                            state.text_selection = None;
                            // Return to Live if Review was entered for selection.
                            if self.selection_auto_review {
                                state.snapshot = None;
                                state.view_mode = ConsoleViewMode::Live;
                                state.topology_stamp += 1;
                                state.max_content_width = 0.0;
                                self.selection_auto_review = false;
                                self.manual_scroll = false;
                                self.pending_scroll_to_bottom = true;
                                self.pending_horizontal_reset = true;
                                _cx.invalidate_layout();
                            }
                            return true;
                        }
                        false
                    }
                    // Debug: font size cycling.
                    KeyCode::Character('[') => {
                        let current = self.state.borrow().config.font_size;
                        let sizes = [10.0_f32, 12.0, 14.0, 16.0, 18.0, 20.0];
                        if let Some(idx) = sizes.iter().rposition(|&s| s < current) {
                            self.set_font_size(sizes[idx]);
                        }
                        true
                    }
                    KeyCode::Character(']') => {
                        let current = self.state.borrow().config.font_size;
                        let sizes = [10.0_f32, 12.0, 14.0, 16.0, 18.0, 20.0];
                        if let Some(idx) = sizes.iter().position(|&s| s > current) {
                            self.set_font_size(sizes[idx]);
                        }
                        true
                    }
                    _ => false,
                }
            }
            // ── Text input (Cmd/Ctrl+C, Cmd/Ctrl+A) ──
            // SPEC-HAL-033: Text events carry the character. KeyCode::Character
            // does not fire reliably on all platforms.
            Event::Text(text_event) if phase == EventPhase::Target => {
                if !is_selectable {
                    return false;
                }

                #[cfg(target_os = "macos")]
                let shortcut_mod = text_event.modifiers.meta;
                #[cfg(not(target_os = "macos"))]
                let shortcut_mod = text_event.modifiers.ctrl;

                if !shortcut_mod {
                    return false;
                }

                match text_event.text.as_str() {
                    "c" | "C" => {
                        let state = self.state.borrow();
                        if let Some(ref sel) = state.text_selection {
                            if !sel.is_empty() {
                                let text = extract_selection_text(sel, |i| match &state.snapshot {
                                    Some(snap) => snap.get(i).cloned(),
                                    None => state.lines.get(i).cloned(),
                                });
                                drop(state);
                                if !text.is_empty() {
                                    _cx.clipboard_set_text(&text);
                                }
                                return true;
                            }
                        }
                        false
                    }
                    "a" | "A" => {
                        let state = self.state.borrow();
                        let line_count = state.effective_line_count();
                        if line_count == 0 {
                            return false;
                        }
                        let last_idx = line_count - 1;
                        let last_line = match &state.snapshot {
                            Some(snap) => snap.get(last_idx).cloned(),
                            None => state.lines.get(last_idx).cloned(),
                        };
                        let last_byte = last_line
                            .map(|l| l.spans.iter().map(|s| s.text.len()).sum::<usize>())
                            .unwrap_or(0);
                        drop(state);

                        let selection = ConsoleTextSelection {
                            anchor: ConsoleTextPosition {
                                entry_index: 0,
                                byte_offset: 0,
                            },
                            head: ConsoleTextPosition {
                                entry_index: last_idx,
                                byte_offset: last_byte,
                            },
                        };
                        self.state.borrow_mut().text_selection = Some(selection);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn layout_depends_on_descendant_scroll(&self) -> bool {
        true
    }

    /// SPEC-PM-005 / Widget trait contract: a virtualizing widget that derives
    /// its viewport from `constraints.max_height` MUST request a bounded main
    /// axis. Console stores `viewport_h` from `max_height` and computes its
    /// auto-scroll target from it; without this, a parent Flex
    /// (INV-FLEX-MEASURE-UNBOUNDED) measures Console with `max_height = INFINITY`,
    /// collapsing the scroll target to 0 (snap-to-top) and defeating
    /// manual-scroll detection. List/ListView return true for the same reason.
    fn needs_viewport_constraint(&self) -> bool {
        true
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        let state = self.state.borrow();
        if state.view_mode != ConsoleViewMode::Review || self.indicator_alpha < 0.01 {
            self.review_pill_rect.set(None);
            return;
        }

        let alpha = self.indicator_alpha;
        let new_count = state.push_count.saturating_sub(state.snapshot_push_count);

        let count_label = if new_count == 1 {
            format!("\u{2193}  {} new entry", new_count)
        } else {
            format!("\u{2193}  {} new entries", new_count)
        };

        let font_size = 13.0;
        let count_run = cx.text_engine.shape(&count_label, font_size);
        let font_metrics = cx.text_engine.font_metrics(font_size);

        let pad_h = 20.0;
        let pad_v = 8.0;
        let indicator_w = count_run.width + pad_h * 2.0;
        let indicator_h = font_size + pad_v * 2.0;

        // Centered horizontally, 16px above bottom edge (clear of scrollbar).
        let indicator_x = rect.x + (rect.width - indicator_w) / 2.0;
        let indicator_y = rect.y + rect.height - indicator_h - 16.0;

        // Background + text in ONE call via TextBackgroundSpan.
        // Same pattern as TooltipSurface (tooltip_surface.rs:207-221).
        // Background span renders BEFORE glyphs within the text pipeline.
        // No z-order fight between separate rect and text passes.
        let accent = cx.theme.colors.accents.primary;
        let text_y =
            indicator_y + (indicator_h - (font_metrics.ascent + font_metrics.descent.abs())) / 2.0;
        let text_color = Color::WHITE.with_alpha(alpha);
        cx.scene.push_text_with_backgrounds(
            &count_run,
            indicator_x + pad_h,
            text_y,
            text_color,
            vec![TextBackgroundSpan {
                x_offset: -pad_h,
                y_offset: -(text_y - indicator_y),
                width: indicator_w,
                height: indicator_h,
                color: accent.with_alpha(0.9 * alpha),
            }],
            LAYER_OVERLAY + 1,
            cx.scene_key,
        );

        // Cache LOCAL rect for hit-test + hover.
        let local_rect = RectF::new(
            indicator_x - rect.x,
            indicator_y - rect.y,
            indicator_w,
            indicator_h,
        );
        self.review_pill_rect.set(Some(local_rect));

        // Overlay hit region for click priority over scrollbar.
        cx.register_overlay_hit_region(OverlayHitRegion {
            abs_rect: RectF::new(indicator_x, indicator_y, indicator_w, indicator_h),
            owner_abs_rect: rect,
            widget_id: tree_widget_id,
        });
    }

    fn debug_type_name(&self) -> &'static str {
        "Console"
    }
}

impl WidgetDowncast for Console {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ─── Console Body (inner paint widget) ───────────────────

pub struct ConsoleBody {
    id: WidgetInternalId,
    pub(crate) state: Rc<RefCell<ConsoleState>>,
    /// SPEC-PM-016 (Console port): per-text shaped-run cache. Console's paint is
    /// forced every frame in always-on hosts (e.g. the Showcase status bar), so
    /// without this it re-wraps and re-shapes every visible entry each frame.
    /// Keyed by the exact shaped text; reused across frames; entries untouched
    /// since the previous frame are evicted via `shape_gen`. `RefCell` because
    /// paint takes `&self`.
    shape_cache: RefCell<std::collections::HashMap<String, CachedConsoleRun>>,
    /// SPEC-HAL-344: cached wrap segmentation for exact entry text at the active
    /// width/font. Stores only wrapped segment geometry; GlyphRuns remain owned
    /// by `shape_cache`.
    wrap_cache: RefCell<std::collections::HashMap<ConsoleWrapCacheKey, CachedWrappedConsoleLine>>,
    /// Monotonic frame generation, bumped at the start of each paint and stamped
    /// onto every cache entry touched that frame; drives eviction.
    shape_gen: Cell<u64>,
    /// Font size the cached runs were shaped at. A change clears the cache.
    shaped_font_size: Cell<f32>,
    /// Exact paint-time measurements need one following layout pass to update
    /// ScrollArea's extent and the virtualization prefix sums.
    height_layout_pending: Cell<bool>,
}

impl ConsoleBody {
    pub(crate) fn new(state: Rc<RefCell<ConsoleState>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state,
            shape_cache: RefCell::new(std::collections::HashMap::new()),
            wrap_cache: RefCell::new(std::collections::HashMap::new()),
            shape_gen: Cell::new(0),
            shaped_font_size: Cell::new(0.0),
            height_layout_pending: Cell::new(false),
        }
    }
}

impl Widget for ConsoleBody {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let mut state = self.state.borrow_mut();
        let viewport_width = constraints.max_width;
        let line_count = state.effective_line_count();
        let is_wrap = state.config.overflow == ConsoleOverflowMode::Wrap;

        // viewport_h is written by Console::layout (container).
        // Do NOT use constraints.max_height — ScrollArea passes unbounded.

        if is_wrap {
            state.compute_visible_range();
            let content_h = state.total_content_height();

            LayoutResult::new(
                Size::new(viewport_width, content_h),
                Size::new(viewport_width, content_h),
            )
        } else {
            // HorizontalScroll mode — existing fixed-height path.
            let content_h = line_count as f32 * state.config.row_height;
            state.compute_visible_range();

            let content_w = state.max_content_width.max(viewport_width);

            LayoutResult::new(
                Size::new(viewport_width, content_h),
                Size::new(content_w, content_h),
            )
        }
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let state = self.state.borrow();
        let row_h = state.config.row_height;
        let font_size = state.config.font_size;
        let pad_left = state.config.padding_left;
        let is_wrap = state.config.overflow == ConsoleOverflowMode::Wrap;

        // SPEC-PM-016 / SPEC-HAL-344: bump the frame generation, clear shaped
        // runs on font-size changes, and otherwise keep them until the hard LRU
        // cap evicts oldest entries. Wrap segmentation remains visible-set scoped.
        let shape_gen = self.shape_gen.get().wrapping_add(1);
        self.shape_gen.set(shape_gen);
        let font_size_changed = (self.shaped_font_size.get() - font_size).abs() > f32::EPSILON;
        {
            let mut sc = self.shape_cache.borrow_mut();
            if font_size_changed {
                sc.clear();
                self.shaped_font_size.set(font_size);
            }
            trim_console_shape_cache(&mut sc);
        }
        {
            let mut wc = self.wrap_cache.borrow_mut();
            if font_size_changed {
                wc.clear();
            } else {
                wc.retain(|_, entry| entry.r#gen + 1 >= shape_gen);
            }
        }

        if state.effective_line_count() == 0 {
            return;
        }

        let (first, last, first_y_offset) = state.visible_range;
        let line_count = state.effective_line_count();
        let first = first.min(line_count);
        let last = last.min(line_count);

        if is_wrap {
            let sel_color = cx.theme.text_selection_color();
            let font_metrics = cx.text_engine.font_metrics(font_size);
            let text_y_offset = (row_h - (font_metrics.ascent + font_metrics.descent.abs())) / 2.0;
            let available_width = (rect.width - pad_left).max(1.0);

            // Collect visible entries, drop state borrow.
            struct VisEntry {
                index: usize,
                line: ConsoleLine,
                sel_range: Option<(usize, usize)>,
            }
            let mut vis_entries: Vec<VisEntry> = Vec::new();
            let first_y_offset_val = first_y_offset;

            for i in first..last {
                let line_ref = match &state.snapshot {
                    Some(snap) => snap.get(i).cloned(),
                    None => state.lines.get(i).cloned(),
                };
                if let Some(line) = line_ref {
                    let sel_range = state.text_selection.as_ref().and_then(|sel| {
                        if sel.is_empty() {
                            return None;
                        }
                        let (s, e) = sel.ordered();
                        if i < s.entry_index || i > e.entry_index {
                            return None;
                        }
                        let sb = if i == s.entry_index { s.byte_offset } else { 0 };
                        let eb = if i == e.entry_index {
                            e.byte_offset
                        } else {
                            usize::MAX
                        };
                        Some((sb, eb))
                    });
                    vis_entries.push(VisEntry {
                        index: i,
                        line,
                        sel_range,
                    });
                }
            }
            drop(state);

            // Paint with on-the-fly wrapping. Accumulate exact heights.
            let mut entry_y = rect.y + first_y_offset_val;
            let mut measured_heights = Vec::with_capacity(vis_entries.len());

            for vis in &vis_entries {
                let wrapped = {
                    let mut sc = self.shape_cache.borrow_mut();
                    let mut wc = self.wrap_cache.borrow_mut();
                    cached_wrapped_lines(
                        &vis.line,
                        available_width,
                        font_size,
                        cx.text_engine,
                        &mut sc,
                        &mut wc,
                        shape_gen,
                    )
                };
                let exact_height = wrapped.len().max(1) as f32 * row_h;
                measured_heights.push((vis.index, exact_height));

                let level_color = match vis.line.level {
                    Some(ConsoleLevel::Trace) => cx.theme.colors.typography.muted,
                    Some(ConsoleLevel::Debug) => cx.theme.colors.typography.secondary,
                    Some(ConsoleLevel::Info) => cx.theme.colors.typography.primary,
                    Some(ConsoleLevel::Warn) => Color::from_hex(0xF39C12),
                    Some(ConsoleLevel::Error) => Color::from_hex(0xE74C3C),
                    None => cx.theme.colors.typography.primary,
                };

                // Precompute span byte offsets.
                let span_byte_starts: Vec<usize> = {
                    let mut offsets = Vec::new();
                    let mut cum = 0_usize;
                    for span in &vis.line.spans {
                        offsets.push(cum);
                        cum += span.text.len();
                    }
                    offsets
                };

                for (line_idx, wrapped_line) in wrapped.iter().enumerate() {
                    let y = entry_y + line_idx as f32 * row_h;

                    for seg in &wrapped_line.segments {
                        let span = &vis.line.spans[seg.span_index];
                        let seg_text = &span.text[seg.byte_start..seg.byte_end];
                        let x = rect.x + pad_left + seg.x_offset;

                        let mut color = span.style.fg.unwrap_or(level_color);
                        if span.style.bold {
                            color = color.lighten(0.2);
                        }
                        if span.style.dim {
                            color = color.with_alpha(color.a * 0.5);
                        }

                        let global_byte_start = span_byte_starts[seg.span_index] + seg.byte_start;
                        let global_byte_end = span_byte_starts[seg.span_index] + seg.byte_end;
                        let mut backgrounds = Vec::new();
                        let mut shaped_run = None;

                        if let Some((sel_start, sel_end)) = vis.sel_range {
                            if sel_end > global_byte_start && sel_start < global_byte_end {
                                let run = {
                                    let mut sc = self.shape_cache.borrow_mut();
                                    cached_shape(
                                        &mut sc,
                                        shape_gen,
                                        cx.text_engine,
                                        seg_text,
                                        font_size,
                                    )
                                };

                                let os = sel_start.max(global_byte_start) - global_byte_start;
                                let oe = sel_end.min(global_byte_end) - global_byte_start;
                                let x0 = x_for_offset(&run, os);
                                let x1 = x_for_offset(&run, oe);
                                if (x1 - x0) > 0.01 {
                                    backgrounds.push(TextBackgroundSpan {
                                        x_offset: x0,
                                        y_offset: -(text_y_offset),
                                        width: x1 - x0,
                                        height: row_h,
                                        color: sel_color,
                                    });
                                }
                                shaped_run = Some(run);
                            }
                        }

                        let run = shaped_run.unwrap_or_else(|| {
                            console_scene_run(seg_text, seg.width, font_size, font_metrics)
                        });

                        if backgrounds.is_empty() {
                            cx.text_engine.draw(
                                &run,
                                x,
                                y + text_y_offset,
                                color,
                                CONSOLE_TEXT_LAYER,
                                cx.scene,
                            );
                        } else {
                            cx.text_engine.draw_with_backgrounds(
                                &run,
                                x,
                                y + text_y_offset,
                                color,
                                backgrounds,
                                CONSOLE_TEXT_LAYER,
                                cx.scene,
                            );
                        }
                    }
                }

                entry_y += exact_height;
            }

            let mut state = self.state.borrow_mut();
            let mut changed = false;
            for (index, measured_height) in measured_heights {
                if (state.row_heights.height_at(index) - measured_height).abs() > 0.01 {
                    state.row_heights.set_height(index, measured_height);
                    changed = true;
                }
            }
            if changed {
                state.wrap_height_correction_pending = true;
                self.height_layout_pending.set(true);
            }
            return;
        }

        // ── HorizontalScroll mode ──
        let font_metrics = cx.text_engine.font_metrics(font_size);
        let text_y_offset = (row_h - (font_metrics.ascent + font_metrics.descent.abs())) / 2.0;
        let mut tracked_max_w: f32 = 0.0;

        let sel_color = cx.theme.text_selection_color();

        for i in first..last {
            let line = match &state.snapshot {
                Some(snap) => snap.get(i),
                None => state.lines.get(i),
            };
            if let Some(line) = line {
                let y = rect.y + i as f32 * row_h;
                let level_color = match line.level {
                    Some(ConsoleLevel::Trace) => cx.theme.colors.typography.muted,
                    Some(ConsoleLevel::Debug) => cx.theme.colors.typography.secondary,
                    Some(ConsoleLevel::Info) => cx.theme.colors.typography.primary,
                    Some(ConsoleLevel::Warn) => Color::from_hex(0xF39C12),
                    Some(ConsoleLevel::Error) => Color::from_hex(0xE74C3C),
                    None => cx.theme.colors.typography.primary,
                };
                let sel_range = state.text_selection.as_ref().and_then(|sel| {
                    if sel.is_empty() {
                        return None;
                    }
                    let (s, e) = sel.ordered();
                    if i < s.entry_index || i > e.entry_index {
                        return None;
                    }
                    let sb = if i == s.entry_index { s.byte_offset } else { 0 };
                    let eb = if i == e.entry_index {
                        e.byte_offset
                    } else {
                        usize::MAX
                    };
                    Some((sb, eb))
                });

                let mut x = rect.x + pad_left;
                let mut cumulative_bytes = 0_usize;

                for span in &line.spans {
                    let mut color = span.style.fg.unwrap_or(level_color);
                    if span.style.bold {
                        color = color.lighten(0.2);
                    }
                    if span.style.dim {
                        color = color.with_alpha(color.a * 0.5);
                    }

                    let run = {
                        let mut sc = self.shape_cache.borrow_mut();
                        cached_shape(&mut sc, shape_gen, cx.text_engine, &span.text, font_size)
                    };
                    let span_width = run.width;
                    let span_byte_start = cumulative_bytes;
                    let span_byte_end = cumulative_bytes + span.text.len();

                    if let Some(bg) = span.style.bg {
                        cx.scene.push_rect(
                            RectF::new(x, y, span_width, row_h),
                            bg,
                            LAYER_CONTENT,
                            None,
                        );
                    }

                    let mut backgrounds = Vec::new();
                    if let Some((sel_start, sel_end)) = sel_range {
                        if sel_end > span_byte_start && sel_start < span_byte_end {
                            let os = sel_start.max(span_byte_start) - span_byte_start;
                            let oe = sel_end.min(span_byte_end) - span_byte_start;
                            let x0 = x_for_offset(&run, os);
                            let x1 = x_for_offset(&run, oe);
                            if (x1 - x0) > 0.01 {
                                backgrounds.push(TextBackgroundSpan {
                                    x_offset: x0,
                                    y_offset: -(text_y_offset),
                                    width: x1 - x0,
                                    height: row_h,
                                    color: sel_color,
                                });
                            }
                        }
                    }

                    if backgrounds.is_empty() {
                        cx.text_engine.draw(
                            &run,
                            x,
                            y + text_y_offset,
                            color,
                            CONSOLE_TEXT_LAYER,
                            cx.scene,
                        );
                    } else {
                        cx.text_engine.draw_with_backgrounds(
                            &run,
                            x,
                            y + text_y_offset,
                            color,
                            backgrounds,
                            CONSOLE_TEXT_LAYER,
                            cx.scene,
                        );
                    }

                    x += span_width;
                    cumulative_bytes = span_byte_end;
                }

                let entry_width = x - rect.x;
                if entry_width > tracked_max_w {
                    tracked_max_w = entry_width;
                }
            }
        }
        drop(state);

        let mut state = self.state.borrow_mut();
        if tracked_max_w > state.max_content_width {
            state.max_content_width = tracked_max_w;
        }
    }

    fn sync_scroll(&mut self, scroll_y: f32) -> bool {
        let mut state = self.state.borrow_mut();
        if (state.scroll_y - scroll_y).abs() < 0.001 {
            return false;
        }
        state.scroll_y = scroll_y;
        state.compute_visible_range();
        true
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        if self.height_layout_pending.replace(false) {
            cx.invalidate_layout();
        }
    }

    fn needs_update(&self) -> bool {
        self.height_layout_pending.get()
    }

    fn scroll_sync_invalidation(&self) -> ScrollSyncInvalidation {
        ScrollSyncInvalidation::Paint
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "ConsoleBody"
    }
}

impl WidgetDowncast for ConsoleBody {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod text_z_order_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::scene::{RectNode, SceneNode, TextNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;

    fn collect<'a>(
        nodes: &'a [SceneNode],
        texts: &mut Vec<&'a TextNode>,
        rects: &mut Vec<&'a RectNode>,
    ) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => texts.push(text),
                SceneNode::Rect(rect) => rects.push(rect),
                SceneNode::RetainedFragment(fragment) => collect(&fragment.children, texts, rects),
                SceneNode::Clip(clip) => collect(&clip.children, texts, rects),
                SceneNode::Layer(layer) => collect(&layer.children, texts, rects),
                SceneNode::Scroll(scroll) => collect(&scroll.children, texts, rects),
                _ => {}
            }
        }
    }

    fn mount_console(console: Console) -> (WidgetTree, NodeId) {
        let body = console.create_body();

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(console), None, 0);
        let scroll_area = tree.add_child(
            root,
            Box::new(ScrollArea::new_unwired().with_scrollbars(false)),
            None,
            0,
        );
        let body = tree.add_child(scroll_area, Box::new(body), None, 0);
        tree.node_mut(scroll_area)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body);
        tree.node_mut(root)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Console>()
            .unwrap()
            .set_scroll_area(scroll_area);

        (tree, root)
    }

    #[test]
    fn text_selection_palette_console_paints_both_overflow_modes() {
        use crate::tests::_support::{assert_painted_text_backgrounds, text_selection_palette_cases};

        let content = "console";
        for overflow in [
            ConsoleOverflowMode::HorizontalScroll,
            ConsoleOverflowMode::Wrap,
        ] {
            for (theme, expected) in text_selection_palette_cases() {
                let console = Console::with_config(ConsoleConfig {
                    policy: ConsolePolicy::Selectable,
                    overflow,
                    ..ConsoleConfig::default()
                });
                console.push_text(content, None);
                console.state.borrow_mut().text_selection = Some(ConsoleTextSelection {
                    anchor: ConsoleTextPosition {
                        entry_index: 0,
                        byte_offset: 0,
                    },
                    head: ConsoleTextPosition {
                        entry_index: 0,
                        byte_offset: content.len(),
                    },
                });
                let (mut tree, root) = mount_console(console);
                let frame = RetainedSceneTestHarness::new().frame(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(320.0, 120.0)),
                    &theme,
                    &mut FontManagerTextEngine::new(),
                    &IconRegistry::new(&[]),
                );
                assert_painted_text_backgrounds(&frame.scene_root, content, &[expected]);
            }
        }
    }

    #[test]
    fn explicit_text_z_console_text_is_above_owner_background() {
        let console = Console::new();
        console.push_text("visible console text", None);
        let (mut tree, root) = mount_console(console);
        let mut harness = RetainedSceneTestHarness::new();
        let mut text_engine = FontManagerTextEngine::new();
        let frame = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(320.0, 120.0)),
            &Theme::default(),
            &mut text_engine,
            &IconRegistry::new(&[]),
        );

        let mut texts = Vec::new();
        let mut rects = Vec::new();
        collect(&frame.scene_root, &mut texts, &mut rects);
        let text = texts
            .iter()
            .find(|text| text.text == "visible console text")
            .expect("console body must paint its line");
        assert_eq!(text.z_index, CONSOLE_TEXT_LAYER);
        assert!(rects.iter().any(|rect| rect.z_index == 0));
        assert!(rects.iter().all(|rect| rect.z_index < text.z_index));
    }
}
