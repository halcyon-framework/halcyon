// SPEC-HAL-315: Read-Only Text Selection Subsystem
//
// Composable, policy-governed, read-only text selection for non-editable
// text regions. Widgets opt in by embedding ReadOnlyTextSelection and
// forwarding pointer events.
//
// Hit-test and rendering functions are free functions operating on
// GlyphRun data — they do NOT extend the TextEngine trait.
//
// INV-315-1: anchor/head always valid byte offsets within text.
// INV-315-2: Selection state never modified during layout/paint.
// INV-315-5: No persistent references to GlyphRun or TextEngine.
// FORBID-315-1: No text mutation.
// FORBID-315-2: No cursor rendering.

use crate::core::editing::Selection;
use crate::core::paint::{Color, TextBackgroundSpan};
use crate::core::text_engine::GlyphRun;

// =============================================================================
// Policy
// =============================================================================

/// Controls when read-only text selection is available.
///
/// SPEC-HAL-315 §4.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSelectionPolicy {
    /// No selection. Text is purely visual. Default.
    Disabled,

    /// Selection available when the widget has focus or capture.
    Enabled,

    /// Selection available only in Capture mode.
    /// Prevents accidental selection during casual navigation.
    OnCapture,

    /// Selection available only while a modifier key (Shift) is held.
    OnModifier,
}

impl Default for TextSelectionPolicy {
    fn default() -> Self {
        Self::Disabled
    }
}

// =============================================================================
// Drag State
// =============================================================================

/// Pointer drag state for selection gestures.
///
/// SPEC-HAL-315 §4.3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionDrag {
    /// No drag in progress.
    Idle,
    /// Character-level drag (single click + drag).
    Character,
    /// Word-level drag (double-click + drag).
    /// Stores the anchor word boundaries so drag expansion
    /// always includes the original double-clicked word.
    Word {
        anchor_start: usize,
        anchor_end: usize,
    },
    /// Line-level drag (triple-click + drag). Selects all text.
    Line,
}

// =============================================================================
// ReadOnlyTextSelection
// =============================================================================

/// Read-only text selection state for a single text region.
///
/// SPEC-HAL-315 §4.2
///
/// Reuses `core::editing::Selection` (anchor/head byte offsets).
/// The text content is stored for clipboard extraction. Updated
/// by the widget whenever the underlying text changes.
///
/// INV-315-1: anchor and head are always UTF-8 boundaries in the stored text.
pub struct ReadOnlyTextSelection {
    /// Current selection range (may be empty: anchor == head).
    selection: Selection,

    /// The text content this selection refers to.
    text: String,

    /// Active policy.
    policy: TextSelectionPolicy,

    /// Drag state for pointer gestures.
    drag: SelectionDrag,
}

impl ReadOnlyTextSelection {
    /// Create with the given policy and initial text.
    pub fn new(policy: TextSelectionPolicy, text: String) -> Self {
        Self {
            selection: Selection::new(),
            text,
            policy,
            drag: SelectionDrag::Idle,
        }
    }

    /// Create with Disabled policy and empty text.
    pub fn disabled() -> Self {
        Self::new(TextSelectionPolicy::Disabled, String::new())
    }

    /// Clear the selection and end the drag when the source bytes change.
    pub fn set_text(&mut self, text: String) {
        if self.text != text {
            self.text = text;
            self.clear();
        }
    }

    /// Set policy. Clears selection if new policy is Disabled.
    pub fn set_policy(&mut self, policy: TextSelectionPolicy) {
        self.policy = policy;
        if policy == TextSelectionPolicy::Disabled {
            self.clear();
        }
    }

    /// Current policy.
    pub fn policy(&self) -> TextSelectionPolicy {
        self.policy
    }

    // =========================================================================
    // Gesture Handling (called by widget from handle_event)
    // =========================================================================

    /// Pointer down. Begins selection if policy allows.
    ///
    /// `byte_offset`: hit-tested position from `hit_test_run`.
    /// `click_count`: 1 = character, 2 = word, 3 = line/all.
    /// `shift`: true if Shift held (extend from anchor).
    /// `in_capture`: true if widget is in Capture mode.
    /// `modifier_held`: true if the modifier key for OnModifier policy is held.
    ///
    /// Returns true if the gesture was accepted.
    pub fn pointer_down(
        &mut self,
        byte_offset: usize,
        click_count: u32,
        shift: bool,
        in_capture: bool,
        modifier_held: bool,
    ) -> bool {
        if !self.is_allowed(in_capture, modifier_held) {
            return false;
        }

        let offset = self.text.floor_char_boundary(byte_offset);

        match click_count {
            1 => {
                if shift {
                    // Extend from current anchor.
                    self.selection.extend_to(offset);
                } else {
                    // New selection starting point.
                    self.selection.clear(offset);
                }
                self.drag = SelectionDrag::Character;
            }
            2 => {
                // Word selection.
                let (ws, we) = find_word_boundaries(&self.text, offset);
                self.selection.anchor = ws;
                self.selection.head = we;
                self.drag = SelectionDrag::Word {
                    anchor_start: ws,
                    anchor_end: we,
                };
            }
            _ => {
                // Triple-click or higher: select all.
                self.selection.anchor = 0;
                self.selection.head = self.text.len();
                self.drag = SelectionDrag::Line;
            }
        }

        true
    }

    /// Pointer move during drag. Extends selection.
    /// `byte_offset`: current hit-tested position.
    pub fn pointer_move(&mut self, byte_offset: usize) {
        let offset = self.text.floor_char_boundary(byte_offset);

        match self.drag {
            SelectionDrag::Idle => {}
            SelectionDrag::Character => {
                self.selection.extend_to(offset);
            }
            SelectionDrag::Word {
                anchor_start,
                anchor_end,
            } => {
                // Word-level drag: always include the anchor word.
                let (ws, we) = find_word_boundaries(&self.text, offset);
                if offset < anchor_start {
                    self.selection.anchor = anchor_end;
                    self.selection.head = ws;
                } else {
                    self.selection.anchor = anchor_start;
                    self.selection.head = we;
                }
            }
            SelectionDrag::Line => {
                // Line-level: stay at select-all.
            }
        }
    }

    /// Pointer up. Finalizes drag.
    pub fn pointer_up(&mut self) {
        self.drag = SelectionDrag::Idle;
    }

    /// Whether a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag != SelectionDrag::Idle
    }

    /// Select all text. Typically bound to Ctrl+A.
    pub fn select_all(&mut self) {
        self.selection.anchor = 0;
        self.selection.head = self.text.len();
    }

    /// Clear selection (collapse to empty).
    pub fn clear(&mut self) {
        self.selection.clear(0);
        self.drag = SelectionDrag::Idle;
    }

    /// HAL390 §16: the deferred owner restores an exact logical interval,
    /// without gestures, clamping, geometry or capture authority.
    pub(crate) fn restore_markdown_read_handoff(
        &mut self,
        text: &str,
        anchor: usize,
        head: usize,
    ) -> bool {
        if self.text != text || !text.is_char_boundary(anchor) || !text.is_char_boundary(head) {
            return false;
        }
        self.selection.anchor = anchor;
        self.selection.head = head;
        self.drag = SelectionDrag::Idle;
        true
    }

    // =========================================================================
    // Query
    // =========================================================================

    /// Whether there is a non-empty selection.
    pub fn has_selection(&self) -> bool {
        !self.selection.is_empty()
    }

    /// Get the selected text substring.
    /// Returns empty string if no selection.
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.start()..self.selection.end()]
    }

    /// Get the selection range (start, end) in byte offsets.
    /// A collapsed range gives the current insertion position.
    pub fn range(&self) -> (usize, usize) {
        (self.selection.start(), self.selection.end())
    }

    /// Get the raw Selection (for advanced use).
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Get the stored text reference.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Copy selected text to clipboard. Returns the selected string
    /// or None if selection is empty.
    pub fn copy_to_clipboard(&self) -> Option<String> {
        if self.selection.is_empty() {
            None
        } else {
            Some(self.selected_text().to_string())
        }
    }

    // =========================================================================
    // Policy Enforcement
    // =========================================================================

    /// Check if selection is currently allowed under the active policy.
    fn is_allowed(&self, in_capture: bool, modifier_held: bool) -> bool {
        match self.policy {
            TextSelectionPolicy::Disabled => false,
            TextSelectionPolicy::Enabled => true,
            TextSelectionPolicy::OnCapture => in_capture,
            TextSelectionPolicy::OnModifier => modifier_held,
        }
    }
}

// =============================================================================
// Hit-Test Free Functions
// =============================================================================

/// Map a pixel X offset within a shaped run to a byte offset in the
/// original text.
///
/// Returns the byte offset of the character boundary nearest to `x`.
/// If `x` is past the end of the run, returns `run.text.len()`.
///
/// SPEC-HAL-315 §5.1
///
/// Algorithm ported from Console's `byte_offset_at_x` (proven in production).
pub fn hit_test_run(run: &GlyphRun, target_x: f32) -> usize {
    if !run.clusters.is_empty() {
        let mut best = (f32::INFINITY, 0);
        for cluster in &run.clusters {
            let (leading, trailing) =
                if cluster.direction == crate::core::text_shaper::TextDirection::LeftToRight {
                    (cluster.source.start, cluster.source.end)
                } else {
                    (cluster.source.end, cluster.source.start)
                };
            if target_x >= cluster.x_start && target_x <= cluster.x_end {
                return if target_x < (cluster.x_start + cluster.x_end) * 0.5 {
                    leading
                } else {
                    trailing
                };
            }
            for (x, offset) in [(cluster.x_start, leading), (cluster.x_end, trailing)] {
                let distance = (target_x - x).abs();
                if distance <= best.0 {
                    best = (distance, offset);
                }
            }
        }
        return best.1;
    }
    let text = &run.text;
    let char_byte_offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let mut x = 0.0_f32;

    for (glyph_idx, glyph) in run.glyphs.iter().enumerate() {
        let mid = x + glyph.advance / 2.0;
        if target_x < mid {
            return char_byte_offsets
                .get(glyph_idx)
                .copied()
                .unwrap_or(text.len());
        }
        x += glyph.advance;
    }
    text.len()
}

/// Map a byte offset within the original text to a pixel X offset
/// from the run's origin.
///
/// Returns 0.0 for offset 0, run.width for offset >= text.len().
///
/// SPEC-HAL-315 §5.2
///
/// Algorithm ported from Console's `x_offset_for_byte` (proven in production).
pub fn x_for_offset(run: &GlyphRun, byte_offset: usize) -> f32 {
    if !run.clusters.is_empty() {
        let offset = byte_offset.min(run.text.len());
        if let Some(cluster) = run
            .clusters
            .iter()
            .find(|cluster| cluster.source.start == offset || cluster.source.contains(&offset))
        {
            return if cluster.direction == crate::core::text_shaper::TextDirection::LeftToRight {
                cluster.x_start
            } else {
                cluster.x_end
            };
        }
        if let Some(cluster) = run
            .clusters
            .iter()
            .find(|cluster| cluster.source.end == offset)
        {
            return if cluster.direction == crate::core::text_shaper::TextDirection::LeftToRight {
                cluster.x_end
            } else {
                cluster.x_start
            };
        }
    }
    let text = &run.text;
    if byte_offset == 0 {
        return 0.0;
    }
    if byte_offset >= text.len() {
        return run.width;
    }
    let mut x = 0.0_f32;
    for (glyph_idx, (char_byte, _)) in text.char_indices().enumerate() {
        if char_byte >= byte_offset {
            return x;
        }
        if let Some(glyph) = run.glyphs.get(glyph_idx) {
            x += glyph.advance;
        }
    }
    run.width
}

/// Compute highlight background spans for a selection range within a shaped run.
///
/// Returns a single `TextBackgroundSpan` covering the selected characters.
/// For LTR text, this is always one contiguous span.
///
/// SPEC-HAL-315 §5.3
///
/// `start`/`end`: byte offsets (start <= end).
/// `line_height`: height of the highlight rect.
/// `y_offset`: vertical offset from the text origin (0.0 for single-line).
/// `color`: highlight background color.
pub fn selection_spans(
    run: &GlyphRun,
    start: usize,
    end: usize,
    line_height: f32,
    y_offset: f32,
    color: Color,
) -> Vec<TextBackgroundSpan> {
    if start >= end {
        return Vec::new();
    }
    if !run.clusters.is_empty() {
        let mut spans: Vec<TextBackgroundSpan> = Vec::new();
        for cluster in run
            .clusters
            .iter()
            .filter(|cluster| cluster.source.start < end && start < cluster.source.end)
        {
            if cluster.x_end <= cluster.x_start {
                continue;
            }
            if let Some(span) = spans.last_mut() {
                let previous_end = span.x_offset + span.width;
                if cluster.x_start
                    <= previous_end
                        + run
                            .binding
                            .as_ref()
                            .map_or(0.0, |binding| binding.cluster_gap().value())
                        + 0.001
                {
                    span.width = cluster.x_end - span.x_offset;
                    continue;
                }
            }
            spans.push(TextBackgroundSpan {
                x_offset: cluster.x_start,
                y_offset,
                width: cluster.x_end - cluster.x_start,
                height: line_height,
                color,
            });
        }
        return spans;
    }
    let x_start = x_for_offset(run, start);
    let x_end = x_for_offset(run, end);
    let width = x_end - x_start;
    if width <= 0.0 {
        return Vec::new();
    }
    vec![TextBackgroundSpan {
        x_offset: x_start,
        y_offset,
        width,
        height: line_height,
        color,
    }]
}

// =============================================================================
// Word Boundary Detection
// =============================================================================

/// Find word boundaries around a byte offset in plain text.
///
/// Returns (word_start, word_end) byte offsets.
///
/// Three character classes: whitespace, alphanumeric/underscore, other punctuation.
/// Scans left and right from the hit position until the class changes.
///
/// Algorithm ported from Console's `find_word_boundaries` (proven in production).
pub fn find_word_boundaries(text: &str, byte_offset: usize) -> (usize, usize) {
    if text.is_empty() {
        return (0, 0);
    }
    let byte_offset = byte_offset.min(text.len());

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return (0, 0);
    }

    let char_idx = chars
        .iter()
        .rposition(|&(i, _)| i <= byte_offset)
        .unwrap_or(0);

    let classify = |ch: char| -> u8 {
        if ch.is_whitespace() {
            0
        } else if ch.is_alphanumeric() || ch == '_' {
            1
        } else {
            2
        }
    };

    let hit_class = classify(chars[char_idx].1);

    let mut left = char_idx;
    while left > 0 && classify(chars[left - 1].1) == hit_class {
        left -= 1;
    }

    let mut right = char_idx;
    while right + 1 < chars.len() && classify(chars[right + 1].1) == hit_class {
        right += 1;
    }

    let start = chars[left].0;
    let end = if right + 1 < chars.len() {
        chars[right + 1].0
    } else {
        text.len()
    };

    (start, end)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn ui_typography_selection_uses_source_cluster_edges() {
        use crate::core::text_engine::TextEngine;
        let mut fonts = crate::platform::font_manager::FontManager::new();
        let font_id = fonts
            .load_font_memory(
                std::sync::Arc::new(
                    std::fs::read("/System/Library/Fonts/Supplemental/STIXGeneral.otf").unwrap(),
                ),
                None,
            )
            .unwrap();
        let mut engine =
            crate::platform::font_manager_text_engine::FontManagerTextEngine::with_face_catalog(
                fonts.face_catalog(),
                font_id,
            )
            .unwrap();
        let style = crate::core::paragraph::TextStyle::default()
            .with_size(24.0)
            .with_cluster_gap(4.0)
            .unwrap();
        for text in ["ffi e\u{301} Привет", "office", "e\u{301}x"] {
            let run = engine.shape_styled(text, Some(&style), 13.0).unwrap();
            assert!(
                run.clusters
                    .iter()
                    .any(|cluster| text[cluster.source.clone()].chars().count() > 1)
            );
            for cluster in &run.clusters {
                let middle = (cluster.x_start + cluster.x_end) * 0.5;
                assert_eq!(
                    hit_test_run(&run, middle),
                    cluster.source.end,
                    "midpoint chooses visual trailing edge"
                );
                assert_eq!(hit_test_run(&run, cluster.x_start), cluster.source.start);
                let spans = selection_spans(
                    &run,
                    cluster.source.start,
                    cluster.source.start + 1,
                    run.height,
                    0.0,
                    Color::WHITE,
                );
                assert_eq!(spans.len(), 1);
                assert!((spans[0].x_offset - cluster.x_start).abs() < 0.001);
                assert!((spans[0].width - (cluster.x_end - cluster.x_start)).abs() < 0.001);
            }
            let all = selection_spans(&run, 0, text.len(), run.height, 0.0, Color::WHITE);
            assert_eq!(all.len(), 1);
            assert!((all[0].width - run.width).abs() < 0.001);
            let mut selection =
                ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, text.into());
            selection.select_all();
            assert_eq!(selection.selected_text(), text);
        }
    }

    // --- Policy tests ---

    #[test]
    fn disabled_rejects_all_gestures() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Disabled, "hello world".into());
        assert!(!sel.pointer_down(3, 1, false, false, false));
        assert!(!sel.has_selection());
    }

    #[test]
    fn enabled_accepts_gestures() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello world".into());
        assert!(sel.pointer_down(0, 1, false, false, false));
        sel.pointer_move(5);
        sel.pointer_up();
        assert!(sel.has_selection());
        assert_eq!(sel.selected_text(), "hello");
    }

    #[test]
    fn on_capture_requires_capture() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::OnCapture, "hello world".into());
        // Not in capture — rejected.
        assert!(!sel.pointer_down(0, 1, false, false, false));
        // In capture — accepted.
        assert!(sel.pointer_down(0, 1, false, true, false));
    }

    #[test]
    fn on_modifier_requires_modifier() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::OnModifier, "hello world".into());
        // No modifier — rejected.
        assert!(!sel.pointer_down(0, 1, false, false, false));
        // Modifier held — accepted.
        assert!(sel.pointer_down(0, 1, false, false, true));
    }

    // --- Selection mechanics ---

    #[test]
    fn click_clears_previous_selection() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello world".into());
        sel.pointer_down(0, 1, false, false, false);
        sel.pointer_move(5);
        sel.pointer_up();
        assert!(sel.has_selection());

        // New click — clears.
        sel.pointer_down(6, 1, false, false, false);
        sel.pointer_up();
        assert!(!sel.has_selection());
    }

    #[test]
    fn shift_click_extends() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello world".into());
        sel.pointer_down(0, 1, false, false, false);
        sel.pointer_up();
        // Shift+click at 5 — extends from anchor 0.
        sel.pointer_down(5, 1, true, false, false);
        sel.pointer_up();
        assert_eq!(sel.selected_text(), "hello");
    }

    #[test]
    fn select_all() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello world".into());
        sel.select_all();
        assert_eq!(sel.selected_text(), "hello world");
    }

    #[test]
    fn readonly_selection_replacement_preserves_utf8_boundaries() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "aβ long".into());
        assert!(sel.pointer_down(1, 1, false, false, false));
        sel.pointer_move(usize::MAX);
        assert_eq!(sel.selected_text(), "β long");

        sel.set_text("aβ long".into());
        assert_eq!(sel.range(), (1, 8));
        assert!(sel.is_dragging());

        sel.set_text("γ界".into());
        assert_eq!(sel.range(), (0, 0));
        assert!(!sel.is_dragging());
        assert_eq!(sel.copy_to_clipboard(), None);
        sel.pointer_move(usize::MAX);
        assert_eq!(sel.selected_text(), "");

        assert!(sel.pointer_down(3, 1, false, false, false));
        assert_eq!(sel.range(), (2, 2));
        sel.pointer_move(1);
        assert_eq!(sel.selected_text(), "γ");
        sel.pointer_move(usize::MAX);
        sel.pointer_up();
        assert_eq!(sel.copy_to_clipboard().as_deref(), Some("界"));
    }

    #[test]
    fn copy_to_clipboard_empty_returns_none() {
        let sel = ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello".into());
        assert!(sel.copy_to_clipboard().is_none());
    }

    #[test]
    fn copy_to_clipboard_with_selection() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello world".into());
        sel.pointer_down(0, 1, false, false, false);
        sel.pointer_move(5);
        sel.pointer_up();
        assert_eq!(sel.copy_to_clipboard(), Some("hello".into()));
    }

    // --- Word boundary tests ---

    #[test]
    fn word_boundary_simple() {
        let (s, e) = find_word_boundaries("hello world", 2);
        assert_eq!(&"hello world"[s..e], "hello");
    }

    #[test]
    fn word_boundary_space() {
        let (s, e) = find_word_boundaries("hello world", 5);
        assert_eq!(&"hello world"[s..e], " ");
    }

    #[test]
    fn word_boundary_end() {
        let (s, e) = find_word_boundaries("hello", 5);
        // At end of text — should return last word.
        assert_eq!(s, 0);
        assert_eq!(e, 5);
    }

    #[test]
    fn word_boundary_empty() {
        let (s, e) = find_word_boundaries("", 0);
        assert_eq!((s, e), (0, 0));
    }

    #[test]
    fn word_boundary_punctuation() {
        let (s, e) = find_word_boundaries("a.b", 1);
        assert_eq!(&"a.b"[s..e], ".");
    }

    // --- Double-click word selection ---

    #[test]
    fn double_click_selects_word() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello world".into());
        sel.pointer_down(2, 2, false, false, false);
        sel.pointer_up();
        assert_eq!(sel.selected_text(), "hello");
    }

    // --- Triple-click select all ---

    #[test]
    fn triple_click_selects_all() {
        let mut sel =
            ReadOnlyTextSelection::new(TextSelectionPolicy::Enabled, "hello world".into());
        sel.pointer_down(2, 3, false, false, false);
        sel.pointer_up();
        assert_eq!(sel.selected_text(), "hello world");
    }
}
