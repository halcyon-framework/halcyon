//! SPEC-HAL-055: Undo/Redo for Text Editing
//!
//! Provides undo/redo history tracking for text editing operations.
//! Stage 1: Single-line TextField support with full string snapshots.

use crate::core::editing::{Selection, TextCursor};

/// Default merge window for consecutive typing operations (milliseconds).
pub const DEFAULT_MERGE_WINDOW_MS: u64 = 300;

/// The kind of edit operation, used to determine merge eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Character insertion (typing) - can merge with adjacent inserts
    Insert,
    /// Backspace deletion - can merge with adjacent backspaces
    Backspace,
    /// Forward delete - can merge with adjacent deletes
    Delete,
    /// Paste operation - never merges
    Paste,
    /// Cut operation - never merges
    Cut,
    /// Selection deletion (e.g., typing over selection) - never merges
    SelectionDelete,
    /// IME composition commit - never merges, always atomic
    ImeCommit,
}

impl EditKind {
    /// Check if this edit kind can merge with another.
    fn can_merge_with(&self, other: &EditKind) -> bool {
        match (self, other) {
            (EditKind::Insert, EditKind::Insert) => true,
            (EditKind::Backspace, EditKind::Backspace) => true,
            (EditKind::Delete, EditKind::Delete) => true,
            _ => false,
        }
    }
}

/// A snapshot of text editing state for undo/redo.
#[derive(Debug, Clone)]
pub struct EditSnapshot {
    /// The document text
    pub text: String,
    /// Cursor position
    pub cursor_pos: usize,
    /// Selection anchor
    pub selection_anchor: usize,
    /// Selection head
    pub selection_head: usize,
}

impl EditSnapshot {
    /// Create a snapshot from current state.
    pub fn capture(text: &str, cursor: &TextCursor, selection: &Selection) -> Self {
        Self {
            text: text.to_string(),
            cursor_pos: cursor.pos,
            selection_anchor: selection.anchor,
            selection_head: selection.head,
        }
    }

    /// Restore state from this snapshot.
    ///
    /// Clamps cursor/selection positions to valid UTF-8 boundaries.
    pub fn restore(&self, text: &mut String, cursor: &mut TextCursor, selection: &mut Selection) {
        *text = self.text.clone();

        // Clamp positions to valid range
        let len = text.len();
        let clamped_cursor = self.cursor_pos.min(len);
        let clamped_anchor = self.selection_anchor.min(len);
        let clamped_head = self.selection_head.min(len);

        // Ensure positions are at valid UTF-8 char boundaries
        cursor.pos = clamp_to_char_boundary(text, clamped_cursor);
        selection.anchor = clamp_to_char_boundary(text, clamped_anchor);
        selection.head = clamp_to_char_boundary(text, clamped_head);

        // Reset preferred_x since text/position changed
        cursor.preferred_x = None;
    }
}

/// Clamp a byte position to the nearest valid UTF-8 char boundary.
fn clamp_to_char_boundary(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        return text.len();
    }
    // If already at a char boundary, return as-is
    if text.is_char_boundary(pos) {
        return pos;
    }
    // Scan backward to find valid boundary
    let mut p = pos;
    while p > 0 && !text.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// A single undo transaction recording before/after states.
#[derive(Debug, Clone)]
pub struct UndoTransaction {
    /// State before the edit
    pub before: EditSnapshot,
    /// State after the edit
    pub after: EditSnapshot,
    /// Kind of edit (for merge decisions)
    pub kind: EditKind,
    /// Timestamp when edit occurred (from unified time source)
    pub timestamp_ms: u64,
}

impl UndoTransaction {
    /// Create a new transaction.
    pub fn new(
        before: EditSnapshot,
        after: EditSnapshot,
        kind: EditKind,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            before,
            after,
            kind,
            timestamp_ms,
        }
    }

    /// Check if this transaction can merge with a new edit.
    ///
    /// Merge rules:
    /// - Same edit kind (Insert+Insert, Backspace+Backspace, Delete+Delete)
    /// - Within merge window
    /// - Cursor position is contiguous (for inserts: after.cursor == new_before.cursor)
    /// - No selection was active before either edit
    pub fn can_merge(
        &self,
        new_before: &EditSnapshot,
        new_kind: EditKind,
        new_timestamp_ms: u64,
        merge_window_ms: u64,
    ) -> bool {
        // Must be same kind and mergeable type
        if !self.kind.can_merge_with(&new_kind) {
            return false;
        }

        // Must be within merge window
        if new_timestamp_ms.saturating_sub(self.timestamp_ms) > merge_window_ms {
            return false;
        }

        // Original edit must not have had a selection
        if self.before.selection_anchor != self.before.selection_head {
            return false;
        }

        // New edit must not have a selection
        if new_before.selection_anchor != new_before.selection_head {
            return false;
        }

        // Cursor must be contiguous
        match new_kind {
            EditKind::Insert => {
                // After last insert, cursor should be at new_before cursor
                self.after.cursor_pos == new_before.cursor_pos
            }
            EditKind::Backspace => {
                // After backspace, cursor moves back; new backspace starts there
                self.after.cursor_pos == new_before.cursor_pos
            }
            EditKind::Delete => {
                // After delete, cursor stays; new delete starts there
                self.after.cursor_pos == new_before.cursor_pos
            }
            _ => false,
        }
    }

    /// Merge a new edit into this transaction.
    ///
    /// Updates `after` to the new state, keeps original `before`, updates timestamp.
    pub fn merge(&mut self, new_after: EditSnapshot, new_timestamp_ms: u64) {
        self.after = new_after;
        self.timestamp_ms = new_timestamp_ms;
    }
}

/// Undo/redo history for text editing.
///
/// Maintains separate undo and redo stacks. Redo stack is cleared
/// on any new edit after undo.
///
/// Stage 1.1: IME-safe integration via suspend/resume.
#[derive(Debug, Clone)]
pub struct UndoHistory {
    /// Stack of undo transactions (most recent at end)
    undo_stack: Vec<UndoTransaction>,
    /// Stack of redo transactions (most recent at end)
    redo_stack: Vec<UndoTransaction>,
    /// Maximum number of undo levels (0 = unlimited)
    max_levels: usize,
    /// Merge window for consecutive edits (milliseconds)
    merge_window_ms: u64,
    /// SPEC-HAL-055 Stage 1.1: Suspended during IME composition
    suspended: bool,
}

impl Default for UndoHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoHistory {
    /// Create a new empty history with default settings.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_levels: 100,
            merge_window_ms: DEFAULT_MERGE_WINDOW_MS,
            suspended: false,
        }
    }

    /// Create history with custom settings.
    pub fn with_settings(max_levels: usize, merge_window_ms: u64) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_levels,
            merge_window_ms,
            suspended: false,
        }
    }

    // =========================================================================
    // SPEC-HAL-055 Stage 1.1: IME suspension
    // =========================================================================

    /// Suspend history recording (for IME composition).
    ///
    /// While suspended, `record()` is a no-op.
    pub fn suspend(&mut self) {
        self.suspended = true;
    }

    /// Resume history recording (after IME composition ends).
    ///
    /// Does NOT auto-record; caller must explicitly record if needed.
    pub fn resume(&mut self) {
        self.suspended = false;
    }

    /// Check if history recording is suspended.
    pub fn is_suspended(&self) -> bool {
        self.suspended
    }

    // =========================================================================
    // Core operations
    // =========================================================================

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear redo stack (called on new edits).
    pub fn clear_redo(&mut self) {
        self.redo_stack.clear();
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Record an edit, potentially merging with the previous transaction.
    ///
    /// Returns true if a new transaction was created, false if merged.
    /// Returns false immediately if history is suspended (IME composition).
    pub fn record(
        &mut self,
        before: EditSnapshot,
        after: EditSnapshot,
        kind: EditKind,
        timestamp_ms: u64,
    ) -> bool {
        // SPEC-HAL-055 Stage 1.1: No-op during IME composition
        if self.suspended {
            return false;
        }

        // Clear redo on any new edit
        self.clear_redo();

        // Try to merge with previous transaction
        if let Some(last) = self.undo_stack.last_mut() {
            if last.can_merge(&before, kind, timestamp_ms, self.merge_window_ms) {
                last.merge(after, timestamp_ms);
                return false;
            }
        }

        // Create new transaction
        let tx = UndoTransaction::new(before, after, kind, timestamp_ms);
        self.undo_stack.push(tx);

        // Enforce max levels
        if self.max_levels > 0 && self.undo_stack.len() > self.max_levels {
            self.undo_stack.remove(0);
        }

        true
    }

    /// Perform undo, returning the state to restore.
    ///
    /// Returns None if undo stack is empty.
    pub fn undo(&mut self) -> Option<&EditSnapshot> {
        let tx = self.undo_stack.pop()?;

        // Move to redo stack
        self.redo_stack.push(tx);

        // Return reference to the before state (now in redo stack)
        self.redo_stack.last().map(|t| &t.before)
    }

    /// Perform redo, returning the state to restore.
    ///
    /// Returns None if redo stack is empty.
    pub fn redo(&mut self) -> Option<&EditSnapshot> {
        let tx = self.redo_stack.pop()?;

        // Move to undo stack
        self.undo_stack.push(tx);

        // Return reference to the after state (now in undo stack)
        self.undo_stack.last().map(|t| &t.after)
    }

    /// Get current undo stack depth.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get current redo stack depth.
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cursor(pos: usize) -> TextCursor {
        TextCursor::at(pos)
    }

    fn make_selection(anchor: usize, head: usize) -> Selection {
        Selection { anchor, head }
    }

    #[test]
    fn test_basic_undo_redo() {
        let mut history = UndoHistory::new();

        let before = EditSnapshot::capture("hello", &make_cursor(5), &make_selection(5, 5));
        let after = EditSnapshot::capture("hello!", &make_cursor(6), &make_selection(6, 6));

        history.record(before, after, EditKind::Insert, 1000);

        assert!(history.can_undo());
        assert!(!history.can_redo());

        let restored = history.undo().unwrap();
        assert_eq!(restored.text, "hello");
        assert_eq!(restored.cursor_pos, 5);

        assert!(!history.can_undo());
        assert!(history.can_redo());

        let restored = history.redo().unwrap();
        assert_eq!(restored.text, "hello!");
        assert_eq!(restored.cursor_pos, 6);
    }

    #[test]
    fn test_insert_merge() {
        let mut history = UndoHistory::new();

        // First insert: "" -> "a"
        let before1 = EditSnapshot::capture("", &make_cursor(0), &make_selection(0, 0));
        let after1 = EditSnapshot::capture("a", &make_cursor(1), &make_selection(1, 1));
        history.record(before1, after1, EditKind::Insert, 100);

        // Second insert: "a" -> "ab" (within merge window, contiguous)
        let before2 = EditSnapshot::capture("a", &make_cursor(1), &make_selection(1, 1));
        let after2 = EditSnapshot::capture("ab", &make_cursor(2), &make_selection(2, 2));
        let merged = !history.record(before2, after2, EditKind::Insert, 200);

        assert!(merged);
        assert_eq!(history.undo_depth(), 1);

        // Single undo should restore to empty
        let restored = history.undo().unwrap();
        assert_eq!(restored.text, "");
    }

    #[test]
    fn test_no_merge_after_pause() {
        let mut history = UndoHistory::new();

        // First insert
        let before1 = EditSnapshot::capture("", &make_cursor(0), &make_selection(0, 0));
        let after1 = EditSnapshot::capture("a", &make_cursor(1), &make_selection(1, 1));
        history.record(before1, after1, EditKind::Insert, 100);

        // Second insert after merge window
        let before2 = EditSnapshot::capture("a", &make_cursor(1), &make_selection(1, 1));
        let after2 = EditSnapshot::capture("ab", &make_cursor(2), &make_selection(2, 2));
        let merged = !history.record(before2, after2, EditKind::Insert, 1200);

        assert!(!merged);
        assert_eq!(history.undo_depth(), 2);
    }

    #[test]
    fn test_paste_never_merges() {
        let mut history = UndoHistory::new();

        // First paste
        let before1 = EditSnapshot::capture("", &make_cursor(0), &make_selection(0, 0));
        let after1 = EditSnapshot::capture("hello", &make_cursor(5), &make_selection(5, 5));
        history.record(before1, after1, EditKind::Paste, 100);

        // Second paste (even contiguous and in window)
        let before2 = EditSnapshot::capture("hello", &make_cursor(5), &make_selection(5, 5));
        let after2 = EditSnapshot::capture("helloworld", &make_cursor(10), &make_selection(10, 10));
        let merged = !history.record(before2, after2, EditKind::Paste, 200);

        assert!(!merged);
        assert_eq!(history.undo_depth(), 2);
    }

    #[test]
    fn test_redo_cleared_on_new_edit() {
        let mut history = UndoHistory::new();

        // Edit 1
        let before1 = EditSnapshot::capture("", &make_cursor(0), &make_selection(0, 0));
        let after1 = EditSnapshot::capture("a", &make_cursor(1), &make_selection(1, 1));
        history.record(before1, after1, EditKind::Insert, 100);

        // Undo
        history.undo();
        assert!(history.can_redo());

        // New edit should clear redo
        let before2 = EditSnapshot::capture("", &make_cursor(0), &make_selection(0, 0));
        let after2 = EditSnapshot::capture("b", &make_cursor(1), &make_selection(1, 1));
        history.record(before2, after2, EditKind::Insert, 2000);

        assert!(!history.can_redo());
    }

    #[test]
    fn test_clamp_to_char_boundary() {
        let text = "héllo"; // é is 2 bytes
        assert_eq!(clamp_to_char_boundary(text, 0), 0);
        assert_eq!(clamp_to_char_boundary(text, 1), 1);
        assert_eq!(clamp_to_char_boundary(text, 2), 1); // middle of é, clamp back
        assert_eq!(clamp_to_char_boundary(text, 3), 3);
        assert_eq!(clamp_to_char_boundary(text, 100), 6);
    }
}
