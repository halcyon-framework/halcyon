//! SPEC-HAL-052 Stage 1-15: Cluster Map, Caret Boundaries, Movement, Selection, Word, Line, Paragraph Selection, Editing, Vertical Nav, Hit-Testing, Paragraph Movement, Multi-Paragraph Navigation & Soft-Wrap Grid
//!
//! Editing substrate: cluster boundary extraction, legal caret positions,
//! cursor movement operations, selection model, word/line/paragraph selection,
//! editing ops (delete/insert/word-delete), vertical navigation, hit-testing,
//! paragraph-bound movement with selection collapse, multi-paragraph navigation,
//! and soft-wrap cursor grid for deterministic visual line movement.

use crate::core::paragraph::Line;
use std::collections::HashMap;

/// A cluster boundary representing a legal caret position.
///
/// Clusters are the atomic units of text that should not be split when
/// placing a caret. Examples: single ASCII chars, combining mark sequences,
/// ligatures, emoji with ZWJ sequences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterBoundary {
    /// Byte offset where this cluster starts (inclusive)
    pub byte_start: usize,
    /// Byte offset where this cluster ends (exclusive)
    pub byte_end: usize,
    /// Visual line index (0-based)
    pub line_index: usize,
    /// X-position of this cluster inside the line (logical pixels)
    pub x_logical: f32,
}

/// Cursor affinity (gravity) for ambiguous caret positions.
///
/// When a caret lands between two cluster boundaries that represent visually
/// ambiguous positions, affinity determines which side the caret logically attaches to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorAffinity {
    /// Caret logically attaches to the left boundary.
    /// Selection collapse resolves toward selection.start().
    #[default]
    Left,
    /// Caret logically attaches to the right boundary.
    /// Selection collapse resolves toward selection.end().
    Right,
}

/// A text cursor representing a caret position.
///
/// The position is always a legal caret position (cluster boundary).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextCursor {
    /// Legal caret position, always a cluster boundary
    pub pos: usize,
    /// Preferred X position for vertical movement (preserved across up/down)
    pub preferred_x: Option<f32>,
    /// Cursor affinity (gravity) for ambiguous positions
    pub affinity: CursorAffinity,
}

impl TextCursor {
    /// Create a new cursor at position 0 with Left affinity.
    pub fn new() -> Self {
        Self {
            pos: 0,
            preferred_x: None,
            affinity: CursorAffinity::Left,
        }
    }

    /// Create a cursor at the specified position with Left affinity.
    pub fn at(pos: usize) -> Self {
        Self {
            pos,
            preferred_x: None,
            affinity: CursorAffinity::Left,
        }
    }

    /// Create a cursor at the specified position with specified affinity.
    pub fn at_with_affinity(pos: usize, affinity: CursorAffinity) -> Self {
        Self {
            pos,
            preferred_x: None,
            affinity,
        }
    }

    /// Move cursor one cluster to the left.
    ///
    /// If at first boundary, cursor stays in place.
    /// Sets affinity = Left.
    pub fn move_left(&mut self, boundaries: &[ClusterBoundary]) {
        self.preferred_x = None;
        self.affinity = CursorAffinity::Left;
        let i = find_index_of_pos(boundaries, self.pos);
        if i == 0 {
            return;
        }
        self.pos = boundaries[i - 1].byte_start;
    }

    /// Move cursor one cluster to the right.
    ///
    /// If at last boundary (sentinel), cursor stays in place.
    /// Sets affinity = Right.
    pub fn move_right(&mut self, boundaries: &[ClusterBoundary]) {
        self.preferred_x = None;
        self.affinity = CursorAffinity::Right;
        let i = find_index_of_pos(boundaries, self.pos);
        if i == boundaries.len() - 1 {
            return;
        }
        self.pos = boundaries[i + 1].byte_start;
    }

    /// Move cursor to the start of the current line.
    /// Sets affinity = Left.
    pub fn move_to_start_of_line(&mut self, boundaries: &[ClusterBoundary]) {
        self.preferred_x = None;
        self.affinity = CursorAffinity::Left;
        let i = find_index_of_pos(boundaries, self.pos);
        let current_line = boundaries[i].line_index;

        // Scan backward while previous boundary has same line
        let mut target = i;
        while target > 0 && boundaries[target - 1].line_index == current_line {
            target -= 1;
        }
        self.pos = boundaries[target].byte_start;
    }

    /// Move cursor to the end of the current line.
    /// Sets affinity = Right.
    pub fn move_to_end_of_line(&mut self, boundaries: &[ClusterBoundary]) {
        self.preferred_x = None;
        self.affinity = CursorAffinity::Right;
        let i = find_index_of_pos(boundaries, self.pos);
        let current_line = boundaries[i].line_index;

        // Scan forward while next boundary has same line
        let mut target = i;
        while target + 1 < boundaries.len() && boundaries[target + 1].line_index == current_line {
            target += 1;
        }
        self.pos = boundaries[target].byte_start;
    }

    /// Move cursor up one line, preserving horizontal position.
    ///
    /// Delegates to the canonical Stage-10 implementation.
    /// Selection is not used (Stage-02 cursor-only API).
    pub fn move_up(&mut self, boundaries: &[ClusterBoundary]) {
        let mut dummy_sel = Selection::at(self.pos);
        move_up(self, &mut dummy_sel, boundaries);
    }

    /// Move cursor down one line, preserving horizontal position.
    ///
    /// Delegates to the canonical Stage-10 implementation.
    /// Selection is not used (Stage-02 cursor-only API).
    pub fn move_down(&mut self, boundaries: &[ClusterBoundary]) {
        let mut dummy_sel = Selection::at(self.pos);
        move_down(self, &mut dummy_sel, boundaries);
    }

    /// Collapse the cursor to a specific position.
    ///
    /// Resets preferred_x and sets pos to the given position.
    /// Preserves current affinity.
    pub fn collapse_to(&mut self, pos: usize) {
        self.pos = pos;
        self.preferred_x = None;
    }

    /// Collapse the cursor to a specific position with specified affinity.
    ///
    /// Resets preferred_x and sets pos and affinity.
    pub fn collapse_to_with_affinity(&mut self, pos: usize, affinity: CursorAffinity) {
        self.pos = pos;
        self.preferred_x = None;
        self.affinity = affinity;
    }
}

impl Default for TextCursor {
    fn default() -> Self {
        Self::new()
    }
}

/// A text selection with anchor and head positions.
///
/// Both anchor and head must always be valid cluster boundary byte_start values.
/// Selection range is half-open: [start, end) where start = min(anchor, head).
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    /// Anchor position (where selection started)
    pub anchor: usize,
    /// Head position (where cursor is, may be before or after anchor)
    pub head: usize,
}

impl Selection {
    /// Create a new collapsed selection at position 0.
    pub fn new() -> Self {
        Self { anchor: 0, head: 0 }
    }

    /// Create a collapsed selection at the specified position.
    pub fn at(pos: usize) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    /// Clear the selection and collapse to a single position.
    pub fn clear(&mut self, pos: usize) {
        self.anchor = pos;
        self.head = pos;
    }

    /// Extend the selection by moving the head to a new position.
    pub fn extend_to(&mut self, head: usize) {
        self.head = head;
    }

    /// Check if the selection is empty (anchor equals head).
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Get the start of the selection (minimum of anchor and head).
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// Get the end of the selection (maximum of anchor and head).
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::new()
    }
}

/// A word boundary representing a range of text that forms a "word".
///
/// Used for double-click word selection and word-drag expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordBoundary {
    /// Byte index at word start
    pub start: usize,
    /// Byte index at word end (exclusive)
    pub end: usize,
}

/// A line boundary representing a range of text that forms a logical line.
///
/// Used for triple-click line selection and line-drag expansion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineBoundary {
    /// Byte index of first cluster in line
    pub start: usize,
    /// Byte index AFTER last cluster in line (or sentinel)
    pub end: usize,
}

/// A paragraph boundary representing a range of text that forms a paragraph.
///
/// Used for quadruple-click paragraph selection and paragraph-drag expansion.
/// A paragraph is a maximal run of consecutive non-empty lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParagraphBoundary {
    /// Byte index of first cluster in paragraph
    pub start: usize,
    /// Byte index AFTER last cluster in paragraph (or sentinel)
    pub end: usize,
}

/// Token kind for word selection tokenization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenKind {
    /// Letters, digits, underscore, internal apostrophes
    Word,
    /// Non-whitespace, non-word symbols
    Punctuation,
    /// Spaces, tabs, etc.
    Whitespace,
}

/// Classify a character's token kind.
///
/// Apostrophe is classified based on context (see `classify_char_in_context`).
fn classify_char(ch: char) -> TokenKind {
    if ch.is_whitespace() {
        TokenKind::Whitespace
    } else if is_word_char(ch) {
        TokenKind::Word
    } else {
        TokenKind::Punctuation
    }
}

/// Classify a character at a boundary index, handling apostrophe context.
///
/// Apostrophe is Word if surrounded by word chars, otherwise Punctuation.
fn classify_char_in_context(boundaries: &[ClusterBoundary], text: &str, idx: usize) -> TokenKind {
    let ch = char_at(text, boundaries[idx].byte_start);

    if ch == '\'' {
        // Check if apostrophe is internal to a word
        let has_word_left = idx > 0 && {
            let prev = &boundaries[idx - 1];
            prev.byte_start != prev.byte_end && is_word_char(char_at(text, prev.byte_start))
        };
        let has_word_right = idx + 1 < boundaries.len() && {
            let next = &boundaries[idx + 1];
            next.byte_start != next.byte_end && is_word_char(char_at(text, next.byte_start))
        };
        if has_word_left && has_word_right {
            TokenKind::Word
        } else {
            TokenKind::Punctuation
        }
    } else {
        classify_char(ch)
    }
}

/// Find the word at a given hit position.
///
/// Returns a WordBoundary where start <= hit < end (unless hit is on whitespace/punctuation).
/// Uses 3-way tokenization: Word, Punctuation, Whitespace.
///
/// # Arguments
/// - `boundaries`: legal cluster boundaries
/// - `text`: the source text string
/// - `hit`: caret boundary byte_start where the click occurred
///
/// # Returns
/// A WordBoundary covering the token (word, punctuation block, or whitespace run) at the hit position.
pub fn find_word_at(boundaries: &[ClusterBoundary], text: &str, hit: usize) -> WordBoundary {
    if boundaries.is_empty() || text.is_empty() {
        return WordBoundary { start: 0, end: 0 };
    }

    // Find the index of the hit position in boundaries
    let hit_idx = match boundaries.iter().position(|b| b.byte_start == hit) {
        Some(idx) => idx,
        None => {
            return WordBoundary {
                start: hit,
                end: hit,
            };
        }
    };

    // Handle sentinel (end of text)
    if boundaries[hit_idx].byte_start == boundaries[hit_idx].byte_end {
        return WordBoundary {
            start: hit,
            end: hit,
        };
    }

    // Get the token kind at hit position
    let hit_kind = classify_char_in_context(boundaries, text, hit_idx);

    // Scan left to find start of token run
    let mut left = hit_idx;
    while left > 0 {
        let prev_idx = left - 1;
        // Don't cross sentinel
        if boundaries[prev_idx].byte_start == boundaries[prev_idx].byte_end {
            break;
        }
        let prev_kind = classify_char_in_context(boundaries, text, prev_idx);
        if prev_kind != hit_kind {
            break;
        }
        left = prev_idx;
    }

    // Scan right to find end of token run
    let mut right = hit_idx;
    while right + 1 < boundaries.len() {
        let next_idx = right + 1;
        // Don't cross sentinel
        if boundaries[next_idx].byte_start == boundaries[next_idx].byte_end {
            break;
        }
        let next_kind = classify_char_in_context(boundaries, text, next_idx);
        if next_kind != hit_kind {
            break;
        }
        right = next_idx;
    }

    // Compute byte range
    let start = boundaries[left].byte_start;
    let end = if right + 1 < boundaries.len() {
        boundaries[right + 1].byte_start
    } else {
        boundaries[right].byte_end
    };

    WordBoundary { start, end }
}

/// Check if a character is a "word character".
///
/// Word characters are: letters, marks, numbers, underscore.
/// Apostrophe is handled specially in find_word_at.
fn is_word_char(ch: char) -> bool {
    ch.is_alphabetic() || ch.is_numeric() || ch == '_' || is_unicode_mark(ch)
}

/// Check if character is a Unicode mark (combining character).
fn is_unicode_mark(ch: char) -> bool {
    // Mark categories: Mn (non-spacing), Mc (spacing combining), Me (enclosing)
    matches!(
        ch,
        '\u{0300}'..='\u{036F}'   // Combining Diacritical Marks
        | '\u{1AB0}'..='\u{1AFF}' // Combining Diacritical Marks Extended
        | '\u{1DC0}'..='\u{1DFF}' // Combining Diacritical Marks Supplement
        | '\u{20D0}'..='\u{20FF}' // Combining Diacritical Marks for Symbols
        | '\u{FE20}'..='\u{FE2F}' // Combining Half Marks
    )
}

/// Get the character at a byte index in text.
///
/// Assumes byte_index is a valid UTF-8 boundary.
fn char_at(text: &str, byte_index: usize) -> char {
    text[byte_index..].chars().next().unwrap_or('\0')
}

// ============================================================================
// Line Selection Functions (Stage 5)
// ============================================================================

/// Find the line boundary for a given line index.
///
/// Returns a LineBoundary where:
/// - `start` is the byte_start of the first cluster on that line
/// - `end` is the byte_start of the first boundary on the next line,
///   or the sentinel's byte_start for the last line
///
/// # Arguments
/// - `boundaries`: legal cluster boundaries
/// - `line_index`: the visual line index to find
///
/// # Returns
/// A LineBoundary covering the entire logical line.
pub fn find_line_boundary(boundaries: &[ClusterBoundary], line_index: usize) -> LineBoundary {
    if boundaries.is_empty() {
        return LineBoundary { start: 0, end: 0 };
    }

    // Find first boundary on this line
    let first_on_line = boundaries.iter().position(|b| b.line_index == line_index);

    match first_on_line {
        Some(first_idx) => {
            let start = boundaries[first_idx].byte_start;

            // Find the first boundary on the next line, or use sentinel
            let end = boundaries
                .iter()
                .find(|b| b.line_index > line_index)
                .map(|b| b.byte_start)
                .unwrap_or_else(|| {
                    // No next line - use sentinel (last boundary)
                    boundaries.last().map(|b| b.byte_start).unwrap_or(start)
                });

            LineBoundary { start, end }
        }
        None => {
            // Empty line or line_index out of range
            // Find the smallest byte_start whose line_index > line_index
            let next_line_boundary = boundaries.iter().find(|b| b.line_index > line_index);

            match next_line_boundary {
                Some(b) => LineBoundary {
                    start: b.byte_start,
                    end: b.byte_start,
                },
                None => {
                    // Last line or beyond - use sentinel
                    let sentinel_start = boundaries.last().map(|b| b.byte_start).unwrap_or(0);
                    LineBoundary {
                        start: sentinel_start,
                        end: sentinel_start,
                    }
                }
            }
        }
    }
}

/// Handle triple-click to select an entire line.
///
/// # Arguments
/// - `boundaries`: legal cluster boundaries
/// - `click_pos`: the byte position where the triple-click occurred
///
/// # Returns
/// A tuple of (Selection, TextCursor) covering the entire line.
pub fn triple_click_selection(
    boundaries: &[ClusterBoundary],
    click_pos: usize,
) -> (Selection, TextCursor) {
    if boundaries.is_empty() {
        return (Selection::at(0), TextCursor::at(0));
    }

    // Find the index of the click position
    let click_idx = boundaries
        .iter()
        .position(|b| b.byte_start == click_pos)
        .unwrap_or(0);

    // Get the line index at click position
    let line_index = boundaries[click_idx].line_index;

    // Get the line boundary
    let lb = find_line_boundary(boundaries, line_index);

    // Create selection covering the line
    let mut selection = Selection::new();
    selection.anchor = lb.start;
    selection.head = lb.end;

    // Create cursor at end of line
    let mut cursor = TextCursor::new();
    cursor.collapse_to(lb.end);

    (selection, cursor)
}

/// Update selection during triple-click + drag (line-drag mode).
///
/// Expands selection by whole lines based on drag direction.
///
/// # Arguments
/// - `initial_lb`: the LineBoundary of the initially triple-clicked line
/// - `initial_line_index`: the line index where triple-click occurred
/// - `current_pos`: the current drag position (byte offset)
/// - `boundaries`: legal cluster boundaries
///
/// # Returns
/// A Selection covering from initial line to current line, by whole lines.
pub fn line_drag_update(
    initial_lb: LineBoundary,
    initial_line_index: usize,
    current_pos: usize,
    boundaries: &[ClusterBoundary],
) -> Selection {
    if boundaries.is_empty() {
        return Selection::at(0);
    }

    // Find the line index at current_pos
    let current_idx = boundaries
        .iter()
        .position(|b| b.byte_start == current_pos)
        .unwrap_or(0);

    // Clamp to valid boundary
    let current_idx = current_idx.min(boundaries.len() - 1);
    let drag_line_index = boundaries[current_idx].line_index;

    // Get boundary for drag line
    let drag_lb = find_line_boundary(boundaries, drag_line_index);

    let mut selection = Selection::new();

    if drag_line_index >= initial_line_index {
        // Forward drag (downwards or same line)
        selection.anchor = initial_lb.start;
        selection.head = drag_lb.end;
    } else {
        // Backward drag (upwards)
        selection.anchor = initial_lb.end;
        selection.head = drag_lb.start;
    }

    selection
}

// ============================================================================
// Paragraph Selection Functions (Stage 6)
// ============================================================================

/// Check if a line is empty (paragraph separator).
///
/// An empty line has its first boundary's byte_start == byte_end.
fn is_empty_line(boundaries: &[ClusterBoundary], line_index: usize) -> bool {
    // Find first boundary on this line
    for b in boundaries {
        if b.line_index == line_index {
            // Empty line = boundary where byte_start == byte_end
            return b.byte_start == b.byte_end;
        }
    }
    // No boundaries on this line - treat as empty
    true
}

/// Find the paragraph boundary for a given click position.
///
/// A paragraph is a maximal run of consecutive non-empty lines.
/// Empty lines (byte_start == byte_end) act as paragraph separators.
///
/// # Arguments
/// - `boundaries`: legal cluster boundaries
/// - `click_pos`: the byte position where the click occurred
///
/// # Returns
/// A ParagraphBoundary covering the entire paragraph.
pub fn find_paragraph_boundary(
    boundaries: &[ClusterBoundary],
    click_pos: usize,
) -> ParagraphBoundary {
    if boundaries.is_empty() {
        return ParagraphBoundary { start: 0, end: 0 };
    }

    // Find the index of the click position
    let click_idx = boundaries
        .iter()
        .position(|b| b.byte_start == click_pos)
        .unwrap_or(0);

    let click_line = boundaries[click_idx].line_index;

    // Find max line index
    let max_line = boundaries.last().map(|b| b.line_index).unwrap_or(0);

    // Scan upward to find paragraph start
    let mut para_start_line = click_line;
    if click_line > 0 {
        for li in (0..click_line).rev() {
            if is_empty_line(boundaries, li) {
                // Found empty line - paragraph starts after it
                break;
            }
            para_start_line = li;
        }
    }

    // Scan downward to find paragraph end
    let mut para_end_line = click_line;
    for li in (click_line + 1)..=max_line {
        if is_empty_line(boundaries, li) {
            // Found empty line - paragraph ends before it
            break;
        }
        para_end_line = li;
    }

    // Get byte boundaries
    let start_lb = find_line_boundary(boundaries, para_start_line);
    let end_lb = find_line_boundary(boundaries, para_end_line);

    ParagraphBoundary {
        start: start_lb.start,
        end: end_lb.end,
    }
}

/// Handle quadruple-click to select an entire paragraph.
///
/// # Arguments
/// - `boundaries`: legal cluster boundaries
/// - `click_pos`: the byte position where the quadruple-click occurred
///
/// # Returns
/// A tuple of (Selection, TextCursor) covering the entire paragraph.
pub fn quadruple_click_selection(
    boundaries: &[ClusterBoundary],
    click_pos: usize,
) -> (Selection, TextCursor) {
    if boundaries.is_empty() {
        return (Selection::at(0), TextCursor::at(0));
    }

    let pb = find_paragraph_boundary(boundaries, click_pos);

    let mut selection = Selection::new();
    selection.anchor = pb.start;
    selection.head = pb.end;

    let mut cursor = TextCursor::new();
    cursor.collapse_to(pb.end);

    (selection, cursor)
}

/// Get the paragraph index for a given line index.
///
/// Counts paragraphs by counting empty lines before this line.
fn get_paragraph_index(boundaries: &[ClusterBoundary], line_index: usize) -> usize {
    let mut para_idx = 0;
    for li in 0..line_index {
        if is_empty_line(boundaries, li) {
            para_idx += 1;
        }
    }
    para_idx
}

/// Update selection during quadruple-click + drag (paragraph-drag mode).
///
/// Expands selection by whole paragraphs based on drag direction.
///
/// # Arguments
/// - `initial_pb`: the ParagraphBoundary of the initially quadruple-clicked paragraph
/// - `initial_par_index`: the paragraph index where quadruple-click occurred
/// - `current_pos`: the current drag position (byte offset)
/// - `boundaries`: legal cluster boundaries
///
/// # Returns
/// A Selection covering from initial paragraph to current paragraph, by whole paragraphs.
pub fn paragraph_drag_update(
    initial_pb: ParagraphBoundary,
    initial_par_index: usize,
    current_pos: usize,
    boundaries: &[ClusterBoundary],
) -> Selection {
    if boundaries.is_empty() {
        return Selection::at(0);
    }

    // Find the line index at current_pos
    let current_idx = boundaries
        .iter()
        .position(|b| b.byte_start == current_pos)
        .unwrap_or(0);

    let current_idx = current_idx.min(boundaries.len() - 1);
    let drag_line_index = boundaries[current_idx].line_index;

    // Get paragraph boundary and index for drag position
    let drag_pb = find_paragraph_boundary(boundaries, current_pos);
    let drag_par_index = get_paragraph_index(boundaries, drag_line_index);

    let mut selection = Selection::new();

    if drag_par_index >= initial_par_index {
        // Forward drag (downwards or same paragraph)
        selection.anchor = initial_pb.start;
        selection.head = drag_pb.end;
    } else {
        // Backward drag (upwards)
        selection.anchor = initial_pb.end;
        selection.head = drag_pb.start;
    }

    selection
}

// ============================================================================
// Editing Ops (Stage 7)
// ============================================================================

/// Basic text editing operations.
///
/// These operate on the text buffer using cluster boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditOp {
    /// Delete cluster before caret (backspace key)
    Backspace,
    /// Delete cluster at caret (delete key)
    Delete,
}

/// Apply a text editing operation.
///
/// Modifies the text buffer, selection, and cursor according to the operation.
/// Caller must regenerate boundaries after this function returns.
///
/// # Arguments
/// - `text`: the text buffer to modify
/// - `selection`: the current selection (will be collapsed after deletion)
/// - `cursor`: the current cursor position (will be updated)
/// - `boundaries`: the current cluster boundaries (used for finding cluster extents)
/// - `op`: the editing operation to apply
pub fn apply_edit_op(
    text: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
    op: TextEditOp,
) {
    // If selection is non-empty, delete the selection regardless of op
    if !selection.is_empty() {
        let start = selection.start();
        let end = selection.end();

        // Remove the selected range
        text.replace_range(start..end, "");

        // Collapse cursor and selection to start
        cursor.pos = start;
        cursor.preferred_x = None;
        selection.anchor = start;
        selection.head = start;

        return;
    }

    // Selection is empty - use cursor.pos for backspace/delete
    match op {
        TextEditOp::Backspace => {
            // If at start, do nothing
            if cursor.pos == 0 {
                return;
            }

            // Find the cluster whose byte_end == cursor.pos
            let cluster = boundaries.iter().find(|b| b.byte_end == cursor.pos);

            if let Some(cluster) = cluster {
                let byte_start = cluster.byte_start;

                // Remove [byte_start .. cursor.pos]
                text.replace_range(byte_start..cursor.pos, "");

                // Update cursor and selection
                cursor.pos = byte_start;
                cursor.preferred_x = None;
                selection.anchor = byte_start;
                selection.head = byte_start;
            }
        }
        TextEditOp::Delete => {
            // If at sentinel (end of text), do nothing
            let is_at_sentinel = boundaries
                .last()
                .map(|b| b.byte_start == cursor.pos && b.byte_start == b.byte_end)
                .unwrap_or(true);

            if is_at_sentinel {
                return;
            }

            // Find the boundary whose byte_start == cursor.pos
            let cluster = boundaries.iter().find(|b| b.byte_start == cursor.pos);

            if let Some(cluster) = cluster {
                let byte_end = cluster.byte_end;

                // Remove [cursor.pos .. byte_end]
                text.replace_range(cursor.pos..byte_end, "");

                // cursor.pos stays the same
                cursor.preferred_x = None;
                selection.anchor = cursor.pos;
                selection.head = cursor.pos;
            }
        }
    }
}

/// Apply a text insertion operation.
///
/// Inserts the given string at the cursor position, replacing any selection.
/// Caller must regenerate boundaries after this function returns.
///
/// # Arguments
/// - `text`: the text buffer to modify
/// - `selection`: the current selection (will be collapsed after insertion)
/// - `cursor`: the current cursor position (will be updated)
/// - `boundaries`: the current cluster boundaries (unused, for API consistency)
/// - `s`: the string to insert
#[allow(unused_variables)]
pub fn apply_insert_op(
    text: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
    s: &str,
) {
    if !selection.is_empty() {
        // Replace selection with inserted text
        let start = selection.start();
        let end = selection.end();

        // Remove the selected range
        text.replace_range(start..end, "");

        // Insert the new text at start
        text.insert_str(start, s);

        // Update cursor and selection
        cursor.pos = start + s.len();
        cursor.preferred_x = None;
        selection.anchor = cursor.pos;
        selection.head = cursor.pos;
    } else {
        // Simple insertion at caret
        let pos = cursor.pos;

        // Insert the new text
        text.insert_str(pos, s);

        // Update cursor and selection
        cursor.pos = pos + s.len();
        cursor.preferred_x = None;
        selection.anchor = cursor.pos;
        selection.head = cursor.pos;
    }
}

// ============================================================================
// Word-Wise Deletion (Stage 9)
// ============================================================================

/// Check if a character is a word character for word-wise deletion.
///
/// Word characters are: Unicode letters, digits, underscore.
fn is_word_char_for_delete(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Scan backward to find the start of the word (or separator run) at/before pos.
///
/// Returns the byte index where deletion should start for Ctrl+Backspace.
fn scan_word_start(text: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }

    // Get the character just before pos
    let char_before = text[..pos].chars().last();

    match char_before {
        None => 0,
        Some(ch) => {
            if is_word_char_for_delete(ch) {
                // We're after a word character - scan back through word chars
                let mut idx = pos;
                for (i, c) in text[..pos].char_indices().rev() {
                    if !is_word_char_for_delete(c) {
                        // Check for apostrophe special case
                        if c == '\'' && i > 0 {
                            // Check if surrounded by word chars
                            let before_apos = text[..i].chars().last();
                            if before_apos.map(is_word_char_for_delete).unwrap_or(false) {
                                idx = i;
                                continue;
                            }
                        }
                        break;
                    }
                    idx = i;
                }
                idx
            } else {
                // We're after a separator - skip separators, then delete word
                let mut idx = pos;

                // First, skip back over separators
                for (i, c) in text[..pos].char_indices().rev() {
                    if is_word_char_for_delete(c) {
                        idx = i + c.len_utf8();
                        break;
                    }
                    idx = i;
                }

                // If we hit a word, delete it too
                if idx > 0 {
                    let word_end = idx;
                    for (i, c) in text[..word_end].char_indices().rev() {
                        if !is_word_char_for_delete(c) {
                            if c == '\'' && i > 0 {
                                let before_apos = text[..i].chars().last();
                                if before_apos.map(is_word_char_for_delete).unwrap_or(false) {
                                    idx = i;
                                    continue;
                                }
                            }
                            break;
                        }
                        idx = i;
                    }
                }

                idx
            }
        }
    }
}

/// Scan forward to find the end of the word (or separator run) at/after pos.
///
/// Returns the byte index where deletion should end for Ctrl+Delete.
fn scan_word_end(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        return text.len();
    }

    // Get the character at pos
    let char_at = text[pos..].chars().next();

    match char_at {
        None => text.len(),
        Some(ch) => {
            if is_word_char_for_delete(ch) {
                // We're at a word character - scan forward through word chars
                let mut idx = pos;
                for (i, c) in text[pos..].char_indices() {
                    let abs_idx = pos + i;
                    if !is_word_char_for_delete(c) {
                        // Check for apostrophe special case
                        if c == '\'' {
                            let after_apos = text[abs_idx + c.len_utf8()..].chars().next();
                            if after_apos.map(is_word_char_for_delete).unwrap_or(false) {
                                continue;
                            }
                        }
                        idx = abs_idx;
                        return idx;
                    }
                    idx = abs_idx + c.len_utf8();
                }
                idx
            } else {
                // We're at a separator - skip separators, then delete word
                let mut idx = pos;

                // First, skip forward over separators
                for (i, c) in text[pos..].char_indices() {
                    let abs_idx = pos + i;
                    if is_word_char_for_delete(c) {
                        idx = abs_idx;
                        break;
                    }
                    idx = abs_idx + c.len_utf8();
                }

                // If we hit a word, delete it too
                if idx < text.len() {
                    let word_start = idx;
                    for (i, c) in text[word_start..].char_indices() {
                        let abs_idx = word_start + i;
                        if !is_word_char_for_delete(c) {
                            if c == '\'' {
                                let after_apos = text[abs_idx + c.len_utf8()..].chars().next();
                                if after_apos.map(is_word_char_for_delete).unwrap_or(false) {
                                    continue;
                                }
                            }
                            idx = abs_idx;
                            return idx;
                        }
                        idx = abs_idx + c.len_utf8();
                    }
                }

                idx
            }
        }
    }
}

/// Snap a byte position to the nearest cluster boundary (start).
fn snap_to_cluster_start(boundaries: &[ClusterBoundary], pos: usize) -> usize {
    // Find the largest byte_start <= pos
    let mut best = 0;
    for b in boundaries {
        if b.byte_start <= pos {
            best = b.byte_start;
        } else {
            break;
        }
    }
    best
}

/// Snap a byte position to the nearest cluster boundary (end).
fn snap_to_cluster_end(boundaries: &[ClusterBoundary], pos: usize) -> usize {
    // Find the smallest byte_end >= pos
    for b in boundaries {
        if b.byte_end >= pos {
            return b.byte_end;
        }
    }
    // Fallback to last boundary
    boundaries.last().map(|b| b.byte_end).unwrap_or(pos)
}

/// Apply word-wise backward deletion (Ctrl+Backspace).
///
/// If selection is non-empty, deletes the selection.
/// Otherwise, deletes from start of word to cursor position.
pub fn apply_delete_word_back(
    text: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
) {
    // If selection is non-empty, delete it
    if !selection.is_empty() {
        let start = selection.start();
        let end = selection.end();

        text.replace_range(start..end, "");

        cursor.pos = start;
        cursor.preferred_x = None;
        selection.anchor = start;
        selection.head = start;
        return;
    }

    // Selection is empty - word-wise delete backward
    if cursor.pos == 0 {
        return;
    }

    // Scan to find word start
    let logical_start = scan_word_start(text, cursor.pos);

    // Snap to cluster boundaries
    let start = snap_to_cluster_start(boundaries, logical_start);
    let end = cursor.pos;

    if start >= end {
        return;
    }

    // Delete the range
    text.replace_range(start..end, "");

    // Update cursor and selection
    cursor.pos = start;
    cursor.preferred_x = None;
    selection.anchor = start;
    selection.head = start;
}

/// Apply word-wise forward deletion (Ctrl+Delete).
///
/// If selection is non-empty, deletes the selection.
/// Otherwise, deletes from cursor position to end of word.
pub fn apply_delete_word_forward(
    text: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
) {
    // If selection is non-empty, delete it
    if !selection.is_empty() {
        let start = selection.start();
        let end = selection.end();

        text.replace_range(start..end, "");

        cursor.pos = start;
        cursor.preferred_x = None;
        selection.anchor = start;
        selection.head = start;
        return;
    }

    // Selection is empty - word-wise delete forward
    if cursor.pos >= text.len() {
        return;
    }

    // Scan to find word end
    let logical_end = scan_word_end(text, cursor.pos);

    // Snap to cluster boundaries
    let start = cursor.pos;
    let end = snap_to_cluster_end(boundaries, logical_end);

    if start >= end {
        return;
    }

    // Delete the range
    text.replace_range(start..end, "");

    // cursor.pos stays the same
    cursor.preferred_x = None;
    selection.anchor = cursor.pos;
    selection.head = cursor.pos;
}

// ============================================================================
// Vertical Navigation (Stage 10)
// ============================================================================

/// Find the cluster index whose byte_start == pos.
///
/// Returns 0 if pos is not found (no panic).
/// If pos == text.len(), returns the sentinel index naturally.
pub fn find_cluster_index(boundaries: &[ClusterBoundary], pos: usize) -> usize {
    boundaries
        .iter()
        .position(|b| b.byte_start == pos)
        .unwrap_or(0)
}

/// Find the byte_start of the cluster on line_index closest to preferred_x.
///
/// Returns 0 if no clusters exist on that line.
pub fn find_best_x_match(
    boundaries: &[ClusterBoundary],
    line_index: usize,
    preferred_x: f32,
) -> usize {
    let mut best_byte_start = 0;
    let mut best_distance = f32::MAX;

    for b in boundaries {
        if b.line_index == line_index {
            let dist = (b.x_logical - preferred_x).abs();
            if dist < best_distance {
                best_distance = dist;
                best_byte_start = b.byte_start;
            }
        }
    }

    best_byte_start
}

/// Check if a line has any clusters.
fn line_has_clusters(boundaries: &[ClusterBoundary], line_index: usize) -> bool {
    boundaries.iter().any(|b| b.line_index == line_index)
}

// ============================================================================
// Soft-Wrap Cursor Grid (Stage 15)
// ============================================================================

/// A canonical soft-wrap cursor grid for deterministic visual line movement.
///
/// This struct maps boundary indices to visual lines, enabling consistent
/// Up/Down navigation across soft-wrapped text, ligatures, and mixed-width clusters.
///
/// The grid is computed from boundaries only — no external layout data required.
#[derive(Debug, Clone)]
pub struct SoftWrapGrid {
    /// Boundary indices of the first cluster of each visual line
    line_start_indices: Vec<usize>,
    /// Boundary indices of the last cluster of each visual line
    line_end_indices: Vec<usize>,
}

impl SoftWrapGrid {
    /// Build a SoftWrapGrid from cluster boundaries.
    ///
    /// Rules:
    /// - A new visual line begins whenever boundaries[i].line_index changes
    /// - End of visual line = last cluster with that line_index
    /// - Sentinel belongs to last visual line
    pub fn from_boundaries(boundaries: &[ClusterBoundary]) -> Self {
        if boundaries.is_empty() {
            return Self {
                line_start_indices: Vec::new(),
                line_end_indices: Vec::new(),
            };
        }

        let mut line_start_indices = Vec::new();
        let mut line_end_indices = Vec::new();

        let mut current_line = boundaries[0].line_index;
        line_start_indices.push(0);

        for (i, b) in boundaries.iter().enumerate() {
            if b.line_index != current_line {
                // Previous line ended at i-1
                line_end_indices.push(i - 1);
                // New line starts at i
                line_start_indices.push(i);
                current_line = b.line_index;
            }
        }

        // Last line ends at the last boundary (sentinel)
        line_end_indices.push(boundaries.len() - 1);

        Self {
            line_start_indices,
            line_end_indices,
        }
    }

    /// Get the number of visual lines in the grid.
    pub fn line_count(&self) -> usize {
        self.line_start_indices.len()
    }

    /// Find the visual line index for a given boundary index.
    ///
    /// Returns the line number (0-based) that contains the boundary at index `idx`.
    pub fn find_visual_line_for_index(&self, idx: usize) -> usize {
        // Linear scan for the line containing this index
        for (line, &start) in self.line_start_indices.iter().enumerate() {
            let end = self.line_end_indices[line];
            if idx >= start && idx <= end {
                return line;
            }
        }
        // Fallback to last line
        self.line_start_indices.len().saturating_sub(1)
    }

    /// Get the boundary index of the first cluster on the given visual line.
    ///
    /// Returns None if line is out of range.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.line_start_indices.get(line).copied()
    }

    /// Get the boundary index of the last cluster on the given visual line.
    ///
    /// Returns None if line is out of range.
    pub fn line_end(&self, line: usize) -> Option<usize> {
        self.line_end_indices.get(line).copied()
    }

    /// Find the best matching boundary index on the given visual line for preferred_x.
    ///
    /// Returns the boundary index of the cluster closest to preferred_x on that line.
    /// Returns None if line is out of range.
    pub fn find_best_match(
        &self,
        boundaries: &[ClusterBoundary],
        line: usize,
        preferred_x: f32,
    ) -> Option<usize> {
        let start = self.line_start(line)?;
        let end = self.line_end(line)?;

        let mut best_idx = start;
        let mut best_distance = f32::MAX;

        for idx in start..=end {
            if idx < boundaries.len() {
                let dist = (boundaries[idx].x_logical - preferred_x).abs();
                if dist < best_distance {
                    best_distance = dist;
                    best_idx = idx;
                }
            }
        }

        Some(best_idx)
    }
}

/// Move cursor up one visual line, clearing selection.
///
/// Uses SoftWrapGrid for deterministic visual line navigation.
/// If at first visual line, moves caret to start of that line.
/// Preserves affinity (Stage-13 requirement).
pub fn move_up(cursor: &mut TextCursor, selection: &mut Selection, boundaries: &[ClusterBoundary]) {
    if boundaries.is_empty() {
        return;
    }

    let grid = SoftWrapGrid::from_boundaries(boundaries);
    let i = find_cluster_index(boundaries, cursor.pos);

    // Initialize preferred_x if not set
    if cursor.preferred_x.is_none() {
        cursor.preferred_x = Some(boundaries[i].x_logical);
    }
    let px = cursor.preferred_x.unwrap();

    let current_line = grid.find_visual_line_for_index(i);

    if current_line == 0 {
        // At first visual line - move to start of line
        // SPEC-HAL-107 §7.4: Do NOT overwrite preferred_x at edge
        if let Some(start_idx) = grid.line_start(0) {
            cursor.pos = boundaries[start_idx].byte_start;
        }
        selection.clear(cursor.pos);
        return;
    }

    // Move to previous line
    let target_line = current_line - 1;
    if let Some(target_idx) = grid.find_best_match(boundaries, target_line, px) {
        cursor.pos = boundaries[target_idx].byte_start;
    }
    // preferred_x is retained

    selection.clear(cursor.pos);
}

/// Move cursor down one visual line, clearing selection.
///
/// Uses SoftWrapGrid for deterministic visual line navigation.
/// If at last visual line, clamps to that line (no-op but clears selection).
/// Preserves affinity (Stage-13 requirement).
pub fn move_down(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    if boundaries.is_empty() {
        return;
    }

    let grid = SoftWrapGrid::from_boundaries(boundaries);
    let i = find_cluster_index(boundaries, cursor.pos);

    // Initialize preferred_x if not set
    if cursor.preferred_x.is_none() {
        cursor.preferred_x = Some(boundaries[i].x_logical);
    }
    let px = cursor.preferred_x.unwrap();

    let current_line = grid.find_visual_line_for_index(i);
    let max_line = grid.line_count().saturating_sub(1);

    if current_line >= max_line {
        // At last visual line - clamp (no-op on position, but clear selection)
        selection.clear(cursor.pos);
        return;
    }

    // Move to next line
    let target_line = current_line + 1;
    if let Some(target_idx) = grid.find_best_match(boundaries, target_line, px) {
        cursor.pos = boundaries[target_idx].byte_start;
    }
    // preferred_x is retained

    selection.clear(cursor.pos);
}

/// Move cursor up one visual line, extending selection (Shift+Up).
///
/// Uses SoftWrapGrid for deterministic visual line navigation.
/// If selection was empty before the move, anchor is set to old cursor position.
/// Anchor stays frozen; only head (and cursor.pos) move.
/// Preserves affinity (Stage-13 requirement).
pub fn move_up_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    if boundaries.is_empty() {
        return;
    }

    let old_pos = cursor.pos;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = old_pos;
    }

    let grid = SoftWrapGrid::from_boundaries(boundaries);
    let i = find_cluster_index(boundaries, cursor.pos);

    // Initialize preferred_x if not set
    if cursor.preferred_x.is_none() {
        cursor.preferred_x = Some(boundaries[i].x_logical);
    }
    let px = cursor.preferred_x.unwrap();

    let current_line = grid.find_visual_line_for_index(i);

    if current_line == 0 {
        // At first visual line - move to start of line
        if let Some(start_idx) = grid.line_start(0) {
            cursor.pos = boundaries[start_idx].byte_start;
            cursor.preferred_x = Some(boundaries[start_idx].x_logical);
        }
        selection.head = cursor.pos;
        return;
    }

    // Move to previous line
    let target_line = current_line - 1;
    if let Some(target_idx) = grid.find_best_match(boundaries, target_line, px) {
        cursor.pos = boundaries[target_idx].byte_start;
    }

    selection.head = cursor.pos;
}

/// Move cursor down one visual line, extending selection (Shift+Down).
///
/// Uses SoftWrapGrid for deterministic visual line navigation.
/// If selection was empty before the move, anchor is set to old cursor position.
/// Anchor stays frozen; only head (and cursor.pos) move.
/// Preserves affinity (Stage-13 requirement).
///
/// Special case: If at last visual line, extend selection to the end of the line
/// (to the last cluster boundary on that line). This ensures Shift+Down at bottom
/// still does something useful.
pub fn move_down_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    if boundaries.is_empty() {
        return;
    }

    let old_pos = cursor.pos;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = old_pos;
    }

    let grid = SoftWrapGrid::from_boundaries(boundaries);
    let i = find_cluster_index(boundaries, cursor.pos);

    // Initialize preferred_x if not set
    if cursor.preferred_x.is_none() {
        cursor.preferred_x = Some(boundaries[i].x_logical);
    }
    let px = cursor.preferred_x.unwrap();

    let current_line = grid.find_visual_line_for_index(i);
    let max_line = grid.line_count().saturating_sub(1);

    if current_line >= max_line {
        // At last visual line - extend selection to end of line
        // This makes Shift+Down at bottom useful: it selects to line end
        if let Some(end_idx) = grid.line_end(current_line) {
            cursor.pos = boundaries[end_idx].byte_start;
            // Update preferred_x to line end position
            cursor.preferred_x = Some(boundaries[end_idx].x_logical);
        }
        selection.head = cursor.pos;
        return;
    }

    // Move to next line
    let target_line = current_line + 1;
    if let Some(target_idx) = grid.find_best_match(boundaries, target_line, px) {
        cursor.pos = boundaries[target_idx].byte_start;
    }

    selection.head = cursor.pos;
}

// ============================================================================
// Paragraph-Bound Movement & Selection Collapse (Stage 12)
// ============================================================================

/// Move cursor to start of current line, clearing selection (Home key).
///
/// Updates preferred_x to the new position's x_logical.
/// Sets affinity = Left.
pub fn move_home(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.affinity = CursorAffinity::Left;

    if boundaries.is_empty() {
        return;
    }

    let i = find_cluster_index(boundaries, cursor.pos);
    let current_line = boundaries[i].line_index;

    // Find first cluster on this line
    let first_on_line = boundaries.iter().find(|b| b.line_index == current_line);
    if let Some(b) = first_on_line {
        cursor.pos = b.byte_start;
        cursor.preferred_x = Some(b.x_logical);
    }

    selection.clear(cursor.pos);
}

/// Move cursor to end of current line, clearing selection (End key).
///
/// Moves to the last cluster on the line (not sentinel unless on last line).
/// Updates preferred_x to the new position's x_logical.
/// Sets affinity = Right.
pub fn move_end(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.affinity = CursorAffinity::Right;

    if boundaries.is_empty() {
        return;
    }

    let i = find_cluster_index(boundaries, cursor.pos);
    let current_line = boundaries[i].line_index;

    // Find last cluster on this line
    let mut target_idx = i;
    for (idx, b) in boundaries.iter().enumerate() {
        if b.line_index == current_line {
            target_idx = idx;
        } else if b.line_index > current_line {
            break;
        }
    }

    cursor.pos = boundaries[target_idx].byte_start;
    cursor.preferred_x = Some(boundaries[target_idx].x_logical);

    selection.clear(cursor.pos);
}

/// Move cursor to absolute start of paragraph, clearing selection (Ctrl+Home).
///
/// In single-paragraph model, this is byte 0.
/// Sets affinity = Left.
pub fn move_to_paragraph_start(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.pos = 0;
    cursor.preferred_x = boundaries.first().map(|b| b.x_logical);
    cursor.affinity = CursorAffinity::Left;

    selection.clear(cursor.pos);
}

/// Move cursor to absolute end of paragraph, clearing selection (Ctrl+End).
///
/// In single-paragraph model, this is the sentinel position (buffer.len()).
/// Sets affinity = Right.
pub fn move_to_paragraph_end(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.affinity = CursorAffinity::Right;

    if boundaries.is_empty() {
        cursor.pos = 0;
        cursor.preferred_x = None;
        selection.clear(0);
        return;
    }

    // Sentinel is last boundary
    let sentinel = boundaries.last().unwrap();
    cursor.pos = sentinel.byte_start;
    cursor.preferred_x = Some(sentinel.x_logical);

    selection.clear(cursor.pos);
}

/// Move cursor to start of current line, extending selection (Shift+Home).
/// Sets affinity = Left.
pub fn move_home_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.affinity = CursorAffinity::Left;

    if boundaries.is_empty() {
        return;
    }

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    let i = find_cluster_index(boundaries, cursor.pos);
    let current_line = boundaries[i].line_index;

    // Find first cluster on this line
    let first_on_line = boundaries.iter().find(|b| b.line_index == current_line);
    if let Some(b) = first_on_line {
        cursor.pos = b.byte_start;
        cursor.preferred_x = Some(b.x_logical);
    }

    selection.head = cursor.pos;
}

/// Move cursor to end of current line, extending selection (Shift+End).
/// Sets affinity = Right.
pub fn move_end_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.affinity = CursorAffinity::Right;

    if boundaries.is_empty() {
        return;
    }

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    let i = find_cluster_index(boundaries, cursor.pos);
    let current_line = boundaries[i].line_index;

    // Find last cluster on this line
    let mut target_idx = i;
    for (idx, b) in boundaries.iter().enumerate() {
        if b.line_index == current_line {
            target_idx = idx;
        } else if b.line_index > current_line {
            break;
        }
    }

    cursor.pos = boundaries[target_idx].byte_start;
    cursor.preferred_x = Some(boundaries[target_idx].x_logical);

    selection.head = cursor.pos;
}

/// Move cursor to paragraph start, extending selection (Ctrl+Shift+Home).
/// Sets affinity = Left.
pub fn move_to_paragraph_start_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.affinity = CursorAffinity::Left;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    cursor.pos = 0;
    cursor.preferred_x = boundaries.first().map(|b| b.x_logical);

    selection.head = cursor.pos;
}

/// Move cursor to paragraph end, extending selection (Ctrl+Shift+End).
/// Sets affinity = Right.
pub fn move_to_paragraph_end_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.affinity = CursorAffinity::Right;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    if boundaries.is_empty() {
        cursor.pos = 0;
        cursor.preferred_x = None;
        selection.head = 0;
        return;
    }

    let sentinel = boundaries.last().unwrap();
    cursor.pos = sentinel.byte_start;
    cursor.preferred_x = Some(sentinel.x_logical);

    selection.head = cursor.pos;
}

/// Move cursor left, collapsing selection if present (Left arrow).
///
/// If selection exists, collapses to selection.start() with affinity = Left.
/// Otherwise, moves one cluster left with affinity = Left.
///
/// K2.INV3: Left arrow at line start moves to previous line's newline.
pub fn move_left_collapse(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Left;

    if !selection.is_empty() {
        // Collapse to start of selection
        cursor.pos = selection.start();
        selection.clear(cursor.pos);
        return;
    }

    // Normal left movement
    // K2.INV3: Allow crossing lines when moving to a newline boundary.
    // This replaces the old constraint that blocked all line crossings.
    let i = find_cluster_index(boundaries, cursor.pos);
    if i > 0 {
        let target = &boundaries[i - 1];
        let current = &boundaries[i];

        // K2.INV3: Allow movement if:
        // - Same line (normal horizontal movement), OR
        // - Target is a single-byte boundary on previous line (newline)
        let same_line = current.line_index == target.line_index;
        let is_newline_target =
            target.byte_end == target.byte_start + 1 && target.line_index < current.line_index;

        if same_line || is_newline_target {
            cursor.pos = target.byte_start;
        }
        // else: at non-newline line boundary (soft wrap), noop
    }
    selection.clear(cursor.pos);
}

/// Move cursor right, collapsing selection if present (Right arrow).
///
/// If selection exists, collapses to selection.end() with affinity = Right.
/// Otherwise, moves one cluster right with affinity = Right.
///
/// K2.INV3: Right arrow at newline boundary moves to next line start.
pub fn move_right_collapse(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Right;

    if !selection.is_empty() {
        // Collapse to end of selection
        cursor.pos = selection.end();
        selection.clear(cursor.pos);
        return;
    }

    // Normal right movement
    // K2.INV3: Allow crossing lines when on a newline boundary (byte_end = byte_start + 1
    // and next boundary is on a different line). This replaces the old constraint that
    // blocked all line crossings (SPEC-HAL-107 §7.1 was for single-line TextField).
    let i = find_cluster_index(boundaries, cursor.pos);
    if i + 1 < boundaries.len() {
        let current = &boundaries[i];
        let target = &boundaries[i + 1];

        // K2.INV3: Allow movement if:
        // - Same line (normal horizontal movement), OR
        // - On a single-byte boundary crossing to next line (newline traversal)
        let same_line = current.line_index == target.line_index;
        let is_newline_crossing =
            current.byte_end == current.byte_start + 1 && target.line_index > current.line_index;

        if same_line || is_newline_crossing {
            cursor.pos = target.byte_start;
        }
        // else: at non-newline line boundary (soft wrap), noop
    }
    selection.clear(cursor.pos);
}

/// Move cursor left, extending selection (Shift+Left).
/// Sets affinity = Left.
pub fn move_left_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Left;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    let i = find_cluster_index(boundaries, cursor.pos);
    if i > 0 {
        cursor.pos = boundaries[i - 1].byte_start;
    }

    selection.head = cursor.pos;
}

/// Move cursor right, extending selection (Shift+Right).
/// Sets affinity = Right.
pub fn move_right_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Right;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    let i = find_cluster_index(boundaries, cursor.pos);
    if i + 1 < boundaries.len() {
        cursor.pos = boundaries[i + 1].byte_start;
    }

    selection.head = cursor.pos;
}

/// Move cursor to previous word boundary, clearing selection (Ctrl+Left).
///
/// Clamps at paragraph start (byte 0).
/// Sets affinity = Left.
pub fn move_word_left(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    text: &str,
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Left;

    if cursor.pos == 0 {
        selection.clear(0);
        return;
    }

    // Find word start using existing scan_word_start logic
    let target = scan_word_start(text, cursor.pos);
    // Snap to cluster boundary
    cursor.pos = snap_to_cluster_start(boundaries, target);

    selection.clear(cursor.pos);
}

/// Move cursor to next word boundary, clearing selection (Ctrl+Right).
///
/// Clamps at paragraph end (buffer.len()).
/// Sets affinity = Right.
pub fn move_word_right(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    text: &str,
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Right;

    if cursor.pos >= text.len() {
        selection.clear(cursor.pos);
        return;
    }

    // Find word end using existing scan_word_end logic
    let target = scan_word_end(text, cursor.pos);
    // Snap to cluster boundary
    cursor.pos = snap_to_cluster_end(boundaries, target);

    selection.clear(cursor.pos);
}

/// Move cursor to previous word boundary, extending selection (Ctrl+Shift+Left).
/// Sets affinity = Left.
pub fn move_word_left_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    text: &str,
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Left;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    if cursor.pos == 0 {
        selection.head = 0;
        return;
    }

    let target = scan_word_start(text, cursor.pos);
    cursor.pos = snap_to_cluster_start(boundaries, target);

    selection.head = cursor.pos;
}

/// Move cursor to next word boundary, extending selection (Ctrl+Shift+Right).
/// Sets affinity = Right.
pub fn move_word_right_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    text: &str,
) {
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Right;

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    if cursor.pos >= text.len() {
        selection.head = cursor.pos;
        return;
    }

    let target = scan_word_end(text, cursor.pos);
    cursor.pos = snap_to_cluster_end(boundaries, target);

    selection.head = cursor.pos;
}

// ============================================================================
// Multi-Paragraph Navigation (Stage 14)
// ============================================================================

/// Move cursor to start of current paragraph, or previous paragraph if already at start (Ctrl+Up).
///
/// Canonical editor behavior:
/// - If cursor is NOT at start of current paragraph → move to paragraph start
/// - If cursor IS at start of current paragraph → move to start of previous paragraph
///
/// Clears preferred_x (block movement). Preserves affinity.
pub fn ctrl_move_up(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;

    if boundaries.is_empty() {
        return;
    }

    // Find current paragraph
    let pb = find_paragraph_boundary(boundaries, cursor.pos);

    if cursor.pos > pb.start {
        // Not at paragraph start - move to start of current paragraph
        cursor.pos = pb.start;
    } else {
        // At paragraph start - move to start of previous paragraph
        if pb.start == 0 {
            // Already at document start
            cursor.pos = 0;
        } else {
            // Find previous paragraph by looking at position before current paragraph start
            let prev_pos = boundaries
                .iter()
                .rev()
                .find(|b| b.byte_start < pb.start)
                .map(|b| b.byte_start)
                .unwrap_or(0);
            let prev_pb = find_paragraph_boundary(boundaries, prev_pos);
            cursor.pos = prev_pb.start;
        }
    }

    selection.clear(cursor.pos);
}

/// Move cursor to end of current paragraph, or start of next paragraph if already at end (Ctrl+Down).
///
/// Canonical editor behavior:
/// - If cursor is NOT at end of current paragraph → move to paragraph end
/// - If cursor IS at end of current paragraph → move to start of next paragraph
///
/// Clears preferred_x (block movement). Preserves affinity.
pub fn ctrl_move_down(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;

    if boundaries.is_empty() {
        return;
    }

    // Find current paragraph
    let pb = find_paragraph_boundary(boundaries, cursor.pos);
    let sentinel_pos = boundaries.last().map(|b| b.byte_start).unwrap_or(0);

    if cursor.pos < pb.end {
        // Not at paragraph end - move to end of current paragraph
        cursor.pos = pb.end;
    } else {
        // At paragraph end - move to start of next paragraph
        if pb.end >= sentinel_pos {
            // Already at document end
            cursor.pos = sentinel_pos;
        } else {
            // Find next paragraph by looking at position after current paragraph end
            let next_pos = boundaries
                .iter()
                .find(|b| b.byte_start > pb.end)
                .map(|b| b.byte_start)
                .unwrap_or(sentinel_pos);
            let next_pb = find_paragraph_boundary(boundaries, next_pos);
            cursor.pos = next_pb.start;
        }
    }

    selection.clear(cursor.pos);
}

/// Move cursor to start of current paragraph with selection (Ctrl+Shift+Up).
///
/// Extends selection while moving. Uses same logic as ctrl_move_up.
/// Clears preferred_x (block movement). Preserves affinity.
pub fn ctrl_move_up_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;

    if boundaries.is_empty() {
        return;
    }

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    // Find current paragraph
    let pb = find_paragraph_boundary(boundaries, cursor.pos);

    if cursor.pos > pb.start {
        // Not at paragraph start - move to start of current paragraph
        cursor.pos = pb.start;
    } else {
        // At paragraph start - move to start of previous paragraph
        if pb.start == 0 {
            cursor.pos = 0;
        } else {
            let prev_pos = boundaries
                .iter()
                .rev()
                .find(|b| b.byte_start < pb.start)
                .map(|b| b.byte_start)
                .unwrap_or(0);
            let prev_pb = find_paragraph_boundary(boundaries, prev_pos);
            cursor.pos = prev_pb.start;
        }
    }

    selection.head = cursor.pos;
}

/// Move cursor to end of current paragraph with selection (Ctrl+Shift+Down).
///
/// Extends selection while moving. Uses same logic as ctrl_move_down.
/// Clears preferred_x (block movement). Preserves affinity.
pub fn ctrl_move_down_select(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
) {
    cursor.preferred_x = None;

    if boundaries.is_empty() {
        return;
    }

    // If selection was empty, anchor at current position
    if selection.is_empty() {
        selection.anchor = cursor.pos;
    }

    // Find current paragraph
    let pb = find_paragraph_boundary(boundaries, cursor.pos);
    let sentinel_pos = boundaries.last().map(|b| b.byte_start).unwrap_or(0);

    if cursor.pos < pb.end {
        // Not at paragraph end - move to end of current paragraph
        cursor.pos = pb.end;
    } else {
        // At paragraph end - move to start of next paragraph
        if pb.end >= sentinel_pos {
            cursor.pos = sentinel_pos;
        } else {
            let next_pos = boundaries
                .iter()
                .find(|b| b.byte_start > pb.end)
                .map(|b| b.byte_start)
                .unwrap_or(sentinel_pos);
            let next_pb = find_paragraph_boundary(boundaries, next_pos);
            cursor.pos = next_pb.start;
        }
    }

    selection.head = cursor.pos;
}

// ============================================================================
// Hit-Testing & Visual Mapping (Stage 11)
// ============================================================================
//
// These helpers operate in an abstract line/cluster space, not in device pixels.
// The paragraph/layout layer is responsible for converting real device coordinates
// into the line-slot Y-space before calling these functions.

/// Hit-test a click at (x, y) coordinates to find the nearest caret position.
///
/// Returns the byte-index (cluster byte_start) closest to the click.
///
/// Coordinate spaces:
/// - `x` is in the same logical coordinate system as `ClusterBoundary::x_logical`
/// - `y` is in line-slot units (see `hit_test_line` for details)
/// - Mapping from real device pixels to these spaces is the caller's responsibility
///
/// Rules:
/// - y is converted to target line_index via `hit_test_line`
/// - If y is above all lines → returns first boundary's byte_start
/// - If y is below all lines → returns sentinel's byte_start
/// - x is matched to nearest cluster on that line via `find_best_x_match`
///
/// Note: Callers should set cursor.affinity = CursorAffinity::Left after using this.
/// See `hit_test_click` for an integrated version.
pub fn hit_test_x_y(boundaries: &[ClusterBoundary], x: f32, y: f32) -> usize {
    if boundaries.is_empty() {
        return 0;
    }

    // Find target line from y coordinate
    let target_line = hit_test_line(boundaries, y);

    // Find best x match on that line (uses Stage-10 logic)
    find_best_x_match(boundaries, target_line, x)
}

/// Hit-test a click and update cursor/selection accordingly.
///
/// Performs hit-testing at (x, y) and updates the cursor position with proper affinity.
/// Mouse clicks always set affinity = Left (industry standard).
///
/// # Arguments
/// - `cursor`: The cursor to update
/// - `selection`: The selection to collapse to the new position
/// - `boundaries`: Cluster boundaries for hit-testing
/// - `x`: X coordinate in logical space
/// - `y`: Y coordinate in line-slot units
pub fn hit_test_click(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    x: f32,
    y: f32,
) {
    let pos = hit_test_x_y(boundaries, x, y);
    cursor.pos = pos;
    cursor.preferred_x = None;
    cursor.affinity = CursorAffinity::Left;
    selection.clear(pos);
}

/// Hit-test y coordinate to find the target line_index.
///
/// The `y` parameter is in **line-slot units**, not device pixels.
/// - Line 0 occupies y ∈ [0, LINE_SLOT_HEIGHT)
/// - Line 1 occupies y ∈ [LINE_SLOT_HEIGHT, 2*LINE_SLOT_HEIGHT)
/// - And so on...
///
/// The caller (paragraph/layout layer) is responsible for mapping real device
/// pixel coordinates into this line-slot space before calling this function.
///
/// Rules:
/// - If y < 0 → returns line 0
/// - If y > last_line → returns last line
/// - Otherwise → computes `y / LINE_SLOT_HEIGHT`, then searches for nearest non-empty line
pub fn hit_test_line(boundaries: &[ClusterBoundary], y: f32) -> usize {
    if boundaries.is_empty() {
        return 0;
    }

    // Find max line index
    let max_line = boundaries.last().map(|b| b.line_index).unwrap_or(0);

    // Early out: only one line
    if max_line == 0 {
        return 0;
    }

    // LINE_SLOT_HEIGHT defines the hit-testing Y-space unit.
    // One "slot" per line index. Caller maps real pixels to this space.
    const LINE_SLOT_HEIGHT: f32 = 20.0;

    if y < 0.0 {
        return 0;
    }

    let estimated_line = (y / LINE_SLOT_HEIGHT) as usize;

    if estimated_line > max_line {
        return max_line;
    }

    // Verify the estimated line has clusters, search for nearest if not
    if line_has_clusters(boundaries, estimated_line) {
        return estimated_line;
    }

    // Search for nearest non-empty line
    let mut search_up = estimated_line.saturating_sub(1);
    let mut search_down = estimated_line + 1;

    loop {
        if search_down <= max_line && line_has_clusters(boundaries, search_down) {
            return search_down;
        }
        if search_up > 0 && line_has_clusters(boundaries, search_up) {
            return search_up;
        }
        if search_up == 0 && line_has_clusters(boundaries, 0) {
            return 0;
        }
        if search_down > max_line && search_up == 0 {
            break;
        }
        search_up = search_up.saturating_sub(1);
        search_down += 1;
    }

    // Fallback: return max_line (sentinel line)
    max_line
}

/// Get the visual cluster for a given byte position.
///
/// Round-trip mapping for cursor painting.
///
/// Rules:
/// - Find boundary where byte_start == pos
/// - If not found, return nearest boundary below or sentinel if beyond end
/// - Works for any legal caret pos (including after last cluster)
pub fn visual_cluster_for_pos(boundaries: &[ClusterBoundary], pos: usize) -> ClusterBoundary {
    if boundaries.is_empty() {
        return ClusterBoundary {
            byte_start: 0,
            byte_end: 0,
            line_index: 0,
            x_logical: 0.0,
        };
    }

    // Try exact match first
    if let Some(b) = boundaries.iter().find(|b| b.byte_start == pos) {
        return *b;
    }

    // Find nearest boundary at or after pos
    if let Some(b) = boundaries.iter().find(|b| b.byte_start >= pos) {
        return *b;
    }

    // Beyond end - return last boundary (sentinel)
    *boundaries.last().unwrap()
}

// ============================================================================
// Helper functions for cursor movement
// ============================================================================

/// Find the index of the boundary with the given byte_start position.
///
/// Panics if pos is not a valid boundary position.
fn find_index_of_pos(boundaries: &[ClusterBoundary], pos: usize) -> usize {
    boundaries
        .iter()
        .position(|b| b.byte_start == pos)
        .expect("Cursor.pos must always be a valid boundary")
}

impl ClusterBoundary {
    /// Build cluster boundaries from laid-out lines.
    ///
    /// Uses a strict 2-pass algorithm:
    /// - Pass 1: Collect all cluster byte_starts, deduplicate, sort, derive byte_ends
    /// - Pass 2: Walk lines/runs/glyphs in visual order, emit boundaries
    ///
    /// # Arguments
    /// - `lines`: Laid-out lines from ParagraphLayoutEngine
    /// - `text`: The original text string
    ///
    /// # Returns
    /// Vector of ClusterBoundary in visual order, including end-of-text sentinel.
    pub fn build_from_lines(lines: &[Line], text: &str) -> Vec<ClusterBoundary> {
        if text.is_empty() {
            // Empty text: just the sentinel at position 0
            return vec![ClusterBoundary {
                byte_start: 0,
                byte_end: 0,
                line_index: 0,
                x_logical: 0.0,
            }];
        }

        // ====================================================================
        // PASS 1: Logical extents
        // ====================================================================
        // Collect all cluster byte_starts from all runs/glyphs
        let mut byte_starts: Vec<usize> = Vec::new();

        for line in lines {
            for run in &line.runs {
                for glyph in &run.glyphs {
                    let byte_start = run.range.start + glyph.cluster as usize;
                    byte_starts.push(byte_start);
                }
            }
        }

        // Insert 0 always
        byte_starts.push(0);

        // K2.INV1: Include all newline positions as boundaries
        // Newlines are not in any glyph run but must be explicit boundaries
        for (i, ch) in text.char_indices() {
            if ch == '\n' {
                byte_starts.push(i);
            }
        }

        // Deduplicate and sort ascending
        byte_starts.sort_unstable();
        byte_starts.dedup();

        // Build logical extents map: start -> end
        let mut logical_extents: HashMap<usize, usize> = HashMap::new();
        for (i, &start) in byte_starts.iter().enumerate() {
            let end = if i + 1 < byte_starts.len() {
                byte_starts[i + 1]
            } else {
                text.len()
            };
            logical_extents.insert(start, end);
        }

        // ====================================================================
        // PASS 2: Visual walk
        // ====================================================================
        // Walk lines -> runs -> glyphs in the order returned by ParagraphLayoutEngine.
        // For each consecutive group of glyphs with the same cluster number,
        // emit a ClusterBoundary.

        let mut boundaries: Vec<ClusterBoundary> = Vec::new();
        let mut seen_byte_starts: std::collections::HashSet<usize> =
            std::collections::HashSet::new();

        let last_line_index = if lines.is_empty() { 0 } else { lines.len() - 1 };
        let mut last_line_width: f32 = 0.0;
        // K2.INV1: Track text offset to handle empty lines correctly
        let mut current_text_offset: usize = 0;

        for (line_index, line) in lines.iter().enumerate() {
            let mut x_offset: f32 = 0.0;
            let mut line_max_byte_end: usize = current_text_offset;

            for run in &line.runs {
                let mut glyph_idx = 0;

                // Track max byte covered by this line's runs
                line_max_byte_end = line_max_byte_end.max(run.range.end);

                while glyph_idx < run.glyphs.len() {
                    let glyph = &run.glyphs[glyph_idx];
                    let cluster_byte_start = run.range.start + glyph.cluster as usize;
                    let cluster_x = x_offset;

                    // Consume all glyphs with the same cluster number
                    let mut cluster_advance: f32 = 0.0;
                    while glyph_idx < run.glyphs.len() {
                        let g = &run.glyphs[glyph_idx];
                        if run.range.start + g.cluster as usize != cluster_byte_start {
                            break;
                        }
                        cluster_advance += g.advance;
                        glyph_idx += 1;
                    }

                    // Only emit if we haven't seen this byte_start yet (visual order)
                    if !seen_byte_starts.contains(&cluster_byte_start) {
                        seen_byte_starts.insert(cluster_byte_start);

                        let cluster_byte_end = logical_extents
                            .get(&cluster_byte_start)
                            .copied()
                            .unwrap_or(text.len());

                        boundaries.push(ClusterBoundary {
                            byte_start: cluster_byte_start,
                            byte_end: cluster_byte_end,
                            line_index,
                            x_logical: cluster_x,
                        });
                    }

                    x_offset += cluster_advance;
                }
            }

            // K2.INV1: Add explicit newline boundary if present at end of this line
            // Newlines are NOT part of any glyph run but must be addressable boundaries
            if let Some(&ch) = text.as_bytes().get(line_max_byte_end) {
                if ch == b'\n' && !seen_byte_starts.contains(&line_max_byte_end) {
                    seen_byte_starts.insert(line_max_byte_end);
                    boundaries.push(ClusterBoundary {
                        byte_start: line_max_byte_end,
                        byte_end: line_max_byte_end + 1,
                        line_index,
                        x_logical: x_offset, // At end of line content
                    });
                }
                // Advance past the newline for the next line
                current_text_offset = line_max_byte_end + 1;
            } else {
                // No newline at end (last line), advance to end of content
                current_text_offset = line_max_byte_end;
            }

            last_line_width = x_offset;
        }

        // If position 0 wasn't emitted (can happen with empty lines), add it
        if !seen_byte_starts.contains(&0) && !text.is_empty() {
            boundaries.insert(
                0,
                ClusterBoundary {
                    byte_start: 0,
                    byte_end: logical_extents.get(&0).copied().unwrap_or(text.len()),
                    line_index: 0,
                    x_logical: 0.0,
                },
            );
        }

        // Add end-of-text sentinel
        boundaries.push(ClusterBoundary {
            byte_start: text.len(),
            byte_end: text.len(),
            line_index: last_line_index,
            x_logical: last_line_width,
        });

        // K2.INV2: Sort boundaries by byte_start for correct caret progression
        // move_left/move_right assume boundaries[i-1] and boundaries[i+1] are adjacent positions
        boundaries.sort_by_key(|b| b.byte_start);

        boundaries
    }
}

/// Find the boundary index at a given byte offset.
///
/// # Arguments
/// - `boundaries`: Cluster boundaries from `build_from_lines`
/// - `byte_offset`: The byte offset to find
///
/// # Returns
/// Index into `boundaries` of the matching or nearest boundary.
pub fn find_boundary_at(boundaries: &[ClusterBoundary], byte_offset: usize) -> Option<usize> {
    if boundaries.is_empty() {
        return None;
    }

    // First, try to find a boundary where:
    //   byte_start <= byte_offset < byte_end
    // OR
    //   byte_offset == byte_start (handles sentinel where byte_start == byte_end)
    for (i, b) in boundaries.iter().enumerate() {
        if byte_offset == b.byte_start {
            return Some(i);
        }
        if b.byte_start < byte_offset && byte_offset < b.byte_end {
            return Some(i);
        }
    }

    // No exact match - find boundary with smallest byte-distance
    let mut best_idx = 0;
    let mut best_distance = usize::MAX;

    for (i, b) in boundaries.iter().enumerate() {
        // Distance to the boundary's range
        let dist = if byte_offset < b.byte_start {
            b.byte_start - byte_offset
        } else if byte_offset >= b.byte_end {
            byte_offset - b.byte_end + 1
        } else {
            0
        };

        if dist < best_distance {
            best_distance = dist;
            best_idx = i;
        }
    }

    Some(best_idx)
}

/// Hit test to find the caret position at a given (x, y) coordinate.
///
/// Stage 1 minimal implementation:
/// - Uses average line height for line selection
/// - Returns byte_start of cluster with closest x_logical
///
/// # Arguments
/// - `boundaries`: Cluster boundaries from `build_from_lines`
/// - `lines`: Laid-out lines (for height calculation)
/// - `x`: X coordinate in logical pixels
/// - `y`: Y coordinate in logical pixels
///
/// # Returns
/// Byte offset (caret position) for the hit location.
pub fn hit_test(boundaries: &[ClusterBoundary], lines: &[Line], x: f32, y: f32) -> usize {
    if boundaries.is_empty() {
        return 0;
    }

    if lines.is_empty() {
        // Fallback: return first boundary
        return boundaries.first().map(|b| b.byte_start).unwrap_or(0);
    }

    // SPEC-HAL-107 §6.3.6: Use per-line vertical bounds, not average-height mapping
    // Compute cumulative y_offset and find line containing y
    let mut y_offset = 0.0_f32;
    let mut target_line = 0;

    for (i, line) in lines.iter().enumerate() {
        let line_bottom = y_offset + line.height;

        if y < line_bottom {
            // y is above or within this line's bounds
            target_line = i;
            break;
        }

        y_offset = line_bottom;
        target_line = i; // Keep updating to last line for y below all lines
    }

    // Collect boundaries on the target line
    let line_boundaries: Vec<(usize, &ClusterBoundary)> = boundaries
        .iter()
        .enumerate()
        .filter(|(_, b)| b.line_index == target_line)
        .collect();

    if line_boundaries.is_empty() {
        // No boundaries on target line - find nearest non-empty line
        let mut nearest_line = target_line;
        for dist in 1..lines.len() {
            if target_line >= dist {
                let below = target_line - dist;
                if boundaries.iter().any(|b| b.line_index == below) {
                    nearest_line = below;
                    break;
                }
            }
            if target_line + dist < lines.len() {
                let above = target_line + dist;
                if boundaries.iter().any(|b| b.line_index == above) {
                    nearest_line = above;
                    break;
                }
            }
        }

        let fallback_boundaries: Vec<(usize, &ClusterBoundary)> = boundaries
            .iter()
            .enumerate()
            .filter(|(_, b)| b.line_index == nearest_line)
            .collect();

        if fallback_boundaries.is_empty() {
            return boundaries.first().map(|b| b.byte_start).unwrap_or(0);
        }

        // Find closest x on fallback line
        return find_closest_x(&fallback_boundaries, x);
    }

    // Find cluster with closest x_logical to x
    find_closest_x(&line_boundaries, x)
}

/// Helper to find the boundary with closest x_logical to the given x.
fn find_closest_x(line_boundaries: &[(usize, &ClusterBoundary)], x: f32) -> usize {
    let mut best_byte_start = 0;
    let mut best_distance = f32::MAX;

    for (_, b) in line_boundaries {
        let dist = (b.x_logical - x).abs();
        if dist < best_distance {
            best_distance = dist;
            best_byte_start = b.byte_start;
        }
    }

    best_byte_start
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paragraph::{
        LayoutParams, Paragraph, ParagraphLayoutEngine, Run, ShapedGlyph, TextStyle,
    };
    use crate::core::text_engine::TextMetrics;
    use crate::core::text_shaper::TextShaper;
    use crate::platform::system_font_provider::load_legacy_embedded_inter;

    fn make_test_shaper() -> TextShaper {
        let font_data = load_legacy_embedded_inter().data;
        TextShaper::from_data(font_data).expect("bundled Inter must be a valid font")
    }

    fn make_simple_line(glyphs: Vec<ShapedGlyph>, range: std::ops::Range<usize>) -> Line {
        let width: f32 = glyphs.iter().map(|g| g.advance).sum();
        Line {
            runs: vec![Run {
                range,
                style: TextStyle::default(),
                font_id: 0,
                glyphs,
                metrics: TextMetrics {
                    ascent: 14.0,
                    descent: 4.0,
                    line_gap: 0.0,
                    line_height: 18.0,
                },
                width,
            }],
            width,
            ascent: 14.0,
            descent: 4.0,
            height: 18.0,
            baseline: 14.0,
        }
    }

    // ========================================================================
    // ASCII Tests
    // ========================================================================

    #[test]
    fn test_ascii_hello() {
        let text = "hello";
        // Simulate 5 glyphs, each 10px wide, cluster 0,1,2,3,4
        let glyphs: Vec<ShapedGlyph> = (0..5)
            .map(|i| ShapedGlyph {
                glyph_id: 100 + i,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: i,
            })
            .collect();

        let line = make_simple_line(glyphs, 0..5);
        let lines = vec![line];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Should have 6 boundaries: 0,1,2,3,4 + sentinel at 5
        assert_eq!(boundaries.len(), 6, "Expected 6 boundaries for 'hello'");

        // Check positions
        assert_eq!(boundaries[0].byte_start, 0);
        assert_eq!(boundaries[0].byte_end, 1);
        assert_eq!(boundaries[0].x_logical, 0.0);

        assert_eq!(boundaries[1].byte_start, 1);
        assert_eq!(boundaries[1].byte_end, 2);
        assert_eq!(boundaries[1].x_logical, 10.0);

        assert_eq!(boundaries[4].byte_start, 4);
        assert_eq!(boundaries[4].byte_end, 5);
        assert_eq!(boundaries[4].x_logical, 40.0);

        // Sentinel
        assert_eq!(boundaries[5].byte_start, 5);
        assert_eq!(boundaries[5].byte_end, 5);
        assert_eq!(boundaries[5].x_logical, 50.0);
    }

    #[test]
    fn test_empty_text() {
        let text = "";
        let lines: Vec<Line> = vec![];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        assert_eq!(boundaries.len(), 1, "Empty text should have sentinel only");
        assert_eq!(boundaries[0].byte_start, 0);
        assert_eq!(boundaries[0].byte_end, 0);
    }

    // ========================================================================
    // Combining Marks Tests
    // ========================================================================

    #[test]
    fn test_combining_marks_eee() {
        // "ééé" - each é is 'e' + combining acute (U+0301)
        // In UTF-8: e=0x65, acute=0xCC 0x81 (2 bytes)
        // So each é is 3 bytes: positions 0-3, 3-6, 6-9
        let text = "ééé";
        assert_eq!(text.len(), 6); // Actually é in NFC is 2 bytes (0xC3 0xA9)

        // Simulate 3 clusters, each representing one é
        // Clusters at byte 0, 2, 4 (each é is 2 bytes in NFC)
        let glyphs: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 200,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 200,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 2,
            },
            ShapedGlyph {
                glyph_id: 200,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 4,
            },
        ];

        let line = make_simple_line(glyphs, 0..6);
        let lines = vec![line];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Should have 4 boundaries: 0, 2, 4, sentinel at 6
        assert_eq!(boundaries.len(), 4, "Expected 4 boundaries for 'ééé'");

        assert_eq!(boundaries[0].byte_start, 0);
        assert_eq!(boundaries[0].byte_end, 2);

        assert_eq!(boundaries[1].byte_start, 2);
        assert_eq!(boundaries[1].byte_end, 4);

        assert_eq!(boundaries[2].byte_start, 4);
        assert_eq!(boundaries[2].byte_end, 6);

        // Sentinel
        assert_eq!(boundaries[3].byte_start, 6);
    }

    // ========================================================================
    // Emoji Tests
    // ========================================================================

    #[test]
    fn test_emoji_family() {
        // 👨‍👩‍👧‍👦 - family emoji with ZWJ sequences
        // This is typically rendered as a single glyph
        let text = "👨‍👩‍👧‍👦";
        let text_len = text.len(); // 25 bytes

        // Simulate: single cluster for entire family emoji
        let glyphs: Vec<ShapedGlyph> = vec![ShapedGlyph {
            glyph_id: 300,
            x_offset: 0.0,
            y_offset: 0.0,
            advance: 20.0,
            cluster: 0,
        }];

        let line = make_simple_line(glyphs, 0..text_len);
        let lines = vec![line];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Should have 2 boundaries: cluster at 0 + sentinel
        assert_eq!(boundaries.len(), 2, "Family emoji should be one cluster");
        assert_eq!(boundaries[0].byte_start, 0);
        assert_eq!(boundaries[0].byte_end, text_len);
        assert_eq!(boundaries[1].byte_start, text_len);
    }

    #[test]
    fn test_emoji_sequence() {
        // "🙂👍" - two separate emoji
        let text = "🙂👍";
        // 🙂 = 4 bytes, 👍 = 4 bytes
        assert_eq!(text.len(), 8);

        let glyphs: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 301,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 15.0,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 302,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 15.0,
                cluster: 4,
            },
        ];

        let line = make_simple_line(glyphs, 0..8);
        let lines = vec![line];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        assert_eq!(
            boundaries.len(),
            3,
            "Two emoji should be 2 clusters + sentinel"
        );
        assert_eq!(boundaries[0].byte_start, 0);
        assert_eq!(boundaries[0].byte_end, 4);
        assert_eq!(boundaries[1].byte_start, 4);
        assert_eq!(boundaries[1].byte_end, 8);
        assert_eq!(boundaries[2].byte_start, 8);
    }

    // ========================================================================
    // Mixed Runs Tests
    // ========================================================================

    #[test]
    fn test_mixed_ascii_emoji() {
        // "Hi👋" - ASCII + emoji
        let text = "Hi👋";
        // H=1, i=1, 👋=4 bytes
        assert_eq!(text.len(), 6);

        let glyphs: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 100,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 101,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 1,
            },
            ShapedGlyph {
                glyph_id: 400,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 20.0,
                cluster: 2,
            },
        ];

        let line = make_simple_line(glyphs, 0..6);
        let lines = vec![line];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        assert_eq!(boundaries.len(), 4, "Expected 4 boundaries");
        assert_eq!(boundaries[0].byte_start, 0);
        assert_eq!(boundaries[0].byte_end, 1);
        assert_eq!(boundaries[1].byte_start, 1);
        assert_eq!(boundaries[1].byte_end, 2);
        assert_eq!(boundaries[2].byte_start, 2);
        assert_eq!(boundaries[2].byte_end, 6);
    }

    // ========================================================================
    // Multi-line Tests
    // ========================================================================

    #[test]
    fn test_multiline_wrap() {
        // Simulate "abc def" wrapped to two lines: "abc " and "def"
        let text = "abc def";

        // Line 1: "abc " (bytes 0-4)
        let glyphs1: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 100,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 101,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 1,
            },
            ShapedGlyph {
                glyph_id: 102,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 2,
            },
            ShapedGlyph {
                glyph_id: 103,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 5.0,
                cluster: 3,
            }, // space
        ];

        // Line 2: "def" (bytes 4-7)
        let glyphs2: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 104,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 0,
            }, // cluster relative to run
            ShapedGlyph {
                glyph_id: 105,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 1,
            },
            ShapedGlyph {
                glyph_id: 106,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 2,
            },
        ];

        let line1 = make_simple_line(glyphs1, 0..4);
        let mut line2 = make_simple_line(glyphs2, 4..7);
        // Adjust run range for line2
        line2.runs[0].range = 4..7;

        let lines = vec![line1, line2];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Should have boundaries at: 0, 1, 2, 3, 4, 5, 6, sentinel at 7
        assert_eq!(boundaries.len(), 8, "Expected 8 boundaries for 'abc def'");

        // Check line indices
        assert_eq!(boundaries[0].line_index, 0); // 'a'
        assert_eq!(boundaries[3].line_index, 0); // ' '
        assert_eq!(boundaries[4].line_index, 1); // 'd'
        assert_eq!(boundaries[6].line_index, 1); // 'f'
        assert_eq!(boundaries[7].line_index, 1); // sentinel
    }

    // ========================================================================
    // find_boundary_at Tests
    // ========================================================================

    #[test]
    fn test_find_boundary_at_exact() {
        let text = "hello";
        let glyphs: Vec<ShapedGlyph> = (0..5)
            .map(|i| ShapedGlyph {
                glyph_id: 100 + i,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: i,
            })
            .collect();

        let line = make_simple_line(glyphs, 0..5);
        let boundaries = ClusterBoundary::build_from_lines(&[line], text);

        // Exact boundary positions
        assert_eq!(find_boundary_at(&boundaries, 0), Some(0));
        assert_eq!(find_boundary_at(&boundaries, 1), Some(1));
        assert_eq!(find_boundary_at(&boundaries, 4), Some(4));
        assert_eq!(find_boundary_at(&boundaries, 5), Some(5)); // sentinel
    }

    #[test]
    fn test_find_boundary_at_within_cluster() {
        // "ééé" - clusters at 0, 2, 4
        let text = "ééé";
        let glyphs: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 200,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 200,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 2,
            },
            ShapedGlyph {
                glyph_id: 200,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 4,
            },
        ];

        let line = make_simple_line(glyphs, 0..6);
        let boundaries = ClusterBoundary::build_from_lines(&[line], text);

        // Offset 1 is within cluster [0,2), should return boundary 0
        let idx = find_boundary_at(&boundaries, 1);
        assert_eq!(idx, Some(0));

        // Offset 3 is within cluster [2,4), should return boundary 1
        let idx = find_boundary_at(&boundaries, 3);
        assert_eq!(idx, Some(1));
    }

    // ========================================================================
    // hit_test Tests
    // ========================================================================

    #[test]
    fn test_hit_test_basic() {
        let text = "hello";
        let glyphs: Vec<ShapedGlyph> = (0..5)
            .map(|i| ShapedGlyph {
                glyph_id: 100 + i,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: i,
            })
            .collect();

        let line = make_simple_line(glyphs, 0..5);
        let lines = vec![line];
        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Click at x=0, y=0 should hit position 0
        assert_eq!(hit_test(&boundaries, &lines, 0.0, 0.0), 0);

        // Click at x=15, y=0 should be closest to position 1 (x=10) or 2 (x=20)
        let pos = hit_test(&boundaries, &lines, 15.0, 0.0);
        assert!(pos == 1 || pos == 2, "Should hit near middle of 'e' or 'l'");

        // Click at x=45, y=0 should hit position 4 (x=40) or sentinel (x=50)
        let pos = hit_test(&boundaries, &lines, 45.0, 0.0);
        assert!(pos == 4 || pos == 5);
    }

    #[test]
    fn test_hit_test_multiline() {
        let text = "ab\ncd";

        // Line 1: "ab" (0-2), line 2: "cd" (3-5)
        // Note: newline at position 2 is not rendered

        let glyphs1: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 100,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 101,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 1,
            },
        ];

        let glyphs2: Vec<ShapedGlyph> = vec![
            ShapedGlyph {
                glyph_id: 102,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 103,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: 1,
            },
        ];

        let line1 = make_simple_line(glyphs1, 0..2);
        let mut line2 = make_simple_line(glyphs2, 3..5);
        line2.runs[0].range = 3..5;

        let lines = vec![line1, line2];
        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Click on first line (y=5, within line height 18)
        let pos = hit_test(&boundaries, &lines, 5.0, 5.0);
        assert!(pos == 0 || pos == 1, "Should hit line 0");

        // Click on second line (y=20, after first line)
        let pos = hit_test(&boundaries, &lines, 5.0, 20.0);
        assert!(pos == 3 || pos == 4, "Should hit line 1");
    }

    // ========================================================================
    // Visual ordering preservation test
    // ========================================================================

    #[test]
    fn test_visual_order_preserved() {
        let text = "hello";
        let glyphs: Vec<ShapedGlyph> = (0..5)
            .map(|i| ShapedGlyph {
                glyph_id: 100 + i,
                x_offset: 0.0,
                y_offset: 0.0,
                advance: 10.0,
                cluster: i,
            })
            .collect();

        let line = make_simple_line(glyphs, 0..5);
        let boundaries = ClusterBoundary::build_from_lines(&[line], text);

        // Verify x_logical increases monotonically (visual order)
        for i in 1..boundaries.len() {
            assert!(
                boundaries[i].x_logical >= boundaries[i - 1].x_logical
                    || boundaries[i].line_index > boundaries[i - 1].line_index,
                "Visual order should be preserved"
            );
        }
    }

    // ========================================================================
    // Integration test with real shaper
    // ========================================================================

    #[test]
    fn test_with_real_shaper() {
        let shaper = make_test_shaper();

        let text = "Hello World!";
        let para = Paragraph::new(text, TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);
        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Should have boundaries for each character + sentinel
        assert!(
            boundaries.len() >= text.chars().count() + 1,
            "Should have at least one boundary per char + sentinel"
        );

        // First boundary at 0
        assert_eq!(boundaries[0].byte_start, 0);

        // Last boundary (sentinel) at text.len()
        assert_eq!(boundaries.last().unwrap().byte_start, text.len());

        // Verify no overlaps
        for i in 0..boundaries.len() - 1 {
            let b = &boundaries[i];
            assert!(
                b.byte_start < b.byte_end
                    || (b.byte_start == b.byte_end && b.byte_start == text.len()),
                "Boundary should have valid range"
            );
        }
    }

    #[test]
    fn test_byte_alignment() {
        let shaper = make_test_shaper();

        // Test with multi-byte characters
        let text = "café";
        let para = Paragraph::new(text, TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);
        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // All byte_start values should be valid UTF-8 boundaries
        for b in &boundaries {
            assert!(
                text.is_char_boundary(b.byte_start),
                "byte_start {} should be char boundary",
                b.byte_start
            );
            assert!(
                text.is_char_boundary(b.byte_end),
                "byte_end {} should be char boundary",
                b.byte_end
            );
        }
    }
}
