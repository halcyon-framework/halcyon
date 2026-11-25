// SPEC-HAL-109 Stage F: Shared editing logic for TextField and TextArea
//
// ============================================================================
// DELEGATE CONTRACT (non-negotiable)
// ============================================================================
//
// Delegate functions:
//   - NEVER touch policy (TextEditingPolicy decisions belong to widgets)
//   - NEVER resume/suspend undo history (except handle_ime_preedit/cancel which
//     are the canonical IME state machine transitions)
//   - NEVER capture snapshots (widget captures before, delegate mutates)
//   - NEVER invalidate layout (widget calls cx.invalidate_layout())
//   - ONLY mutate what is explicitly passed in
//
// INV-F.D1: All state passed explicitly (no hidden state, no &mut self)
// INV-F.D2: Delegates consume policy values, never decide behavior
// INV-F.D3: Call graph must remain structurally identical in both widgets
//
// Violation of this contract = immediate revert.
// ============================================================================
//
// ============================================================================
// SPEC-HAL-159: TEXT OWNERSHIP INVARIANTS (Negative Constraints)
// ============================================================================
//
// These invariants define what text widgets MUST NOT do. Violations are
// structural defects that cannot be "fixed later" — they indicate incorrect
// architecture.
//
// INV-TEXT-NO-DOC-MUTATE-PAINT:
//   Widgets MUST NOT mutate document (TextDocument) during paint.
//   Document mutations occur ONLY during event handlers.
//
// INV-TEXT-NO-SEL-MUTATE-PAINT:
//   Widgets MUST NOT mutate selection (Selection, TextCursor.pos) during paint.
//   Selection mutations occur ONLY during event handlers.
//
// INV-TEXT-NO-DOC-MUTATE-LAYOUT:
//   Layout phase MUST NOT mutate document content.
//   Layout may only read document to compute layout cache.
//
// INV-TEXT-CACHE-LAYOUT-ONLY:
//   Text layout cache (lines, boundaries, metrics) may ONLY be written during
//   layout phase. Paint and event handlers read-only.
//
// INV-TEXT-BLINK-VISUAL-ONLY:
//   Caret blink state (blink_last_ms, blink_caret_visible) is visual-only.
//   Must use AllowPaintMutationGuard when mutated during paint.
//   Field names MUST use `blink_` prefix per INV-WIDGET-VISUAL-PREFIX.
//
// INV-TEXT-SINGLE-OWNER:
//   Each text concern has exactly one owner:
//   - Document content: Input/edit reducers (event phase)
//   - Selection/cursor position: Input/commands (event phase)
//   - Layout cache: Layout phase
//   - Blink state: Paint phase (with guard)
//
// Enforcement:
//   - Grep: no document mutation in paint()
//   - Grep: no selection mutation in paint()
//   - Grep: blink fields use blink_* prefix
//   - Code review: new text state requires explicit phase documentation
//
// ============================================================================

use std::cell::Cell;

use crate::core::editing::{
    ClusterBoundary, Selection, TextCursor, TextEditOp, apply_edit_op, apply_insert_op, move_end,
    move_end_select, move_home, move_home_select, move_left_collapse, move_left_select,
    move_right_collapse, move_right_select, move_word_left, move_word_left_select, move_word_right,
    move_word_right_select,
};
use crate::core::editing_history::{EditKind, EditSnapshot, UndoHistory};
use crate::core::event::Modifiers;

// ============================================================================
// Debug Assertions (zero cost in release)
// ============================================================================

/// Assert cursor position is valid for document length.
#[inline]
fn debug_assert_cursor_in_bounds(cursor: &TextCursor, doc_len: usize) {
    debug_assert!(
        cursor.pos <= doc_len,
        "cursor.pos ({}) must be <= doc_len ({})",
        cursor.pos,
        doc_len
    );
}

/// Assert selection bounds are valid for document length.
#[inline]
fn debug_assert_selection_in_bounds(selection: &Selection, doc_len: usize) {
    debug_assert!(
        selection.anchor <= doc_len,
        "selection.anchor ({}) must be <= doc_len ({})",
        selection.anchor,
        doc_len
    );
    debug_assert!(
        selection.head <= doc_len,
        "selection.head ({}) must be <= doc_len ({})",
        selection.head,
        doc_len
    );
}

/// Assert cursor position lies on a cluster boundary.
#[inline]
fn debug_assert_cursor_on_boundary(cursor: &TextCursor, boundaries: &[ClusterBoundary]) {
    if boundaries.is_empty() {
        // Empty document: cursor must be at 0
        debug_assert!(cursor.pos == 0, "cursor.pos must be 0 for empty boundaries");
        return;
    }
    // Cursor can be at byte_start of any boundary, or at byte_end of the last boundary (end of doc)
    let on_boundary = boundaries.iter().any(|b| b.byte_start == cursor.pos)
        || cursor.pos == boundaries.last().map(|b| b.byte_end).unwrap_or(0);
    debug_assert!(
        on_boundary,
        "cursor.pos ({}) must be on a cluster boundary",
        cursor.pos
    );
}

/// Assert cursor and selection are coherent after mutation.
/// After editing operations, cursor.pos should equal selection.head.
#[inline]
fn debug_assert_cursor_selection_coherent(cursor: &TextCursor, selection: &Selection) {
    debug_assert!(
        cursor.pos == selection.head,
        "cursor.pos ({}) must equal selection.head ({}) after mutation",
        cursor.pos,
        selection.head
    );
}

// ============================================================================
// Caret Blink
// ============================================================================

/// Update caret blink state based on elapsed time.
pub fn update_caret_blink(
    focused: bool,
    last_blink_ms: &Cell<u64>,
    caret_visible: &Cell<bool>,
    now_ms: u64,
    blink_interval_ms: u64,
) {
    if !focused {
        return;
    }
    let last_ms = last_blink_ms.get();
    if now_ms.saturating_sub(last_ms) >= blink_interval_ms {
        // SPEC-HAL-146: Open paint-mutation allow window for caret blink updates
        let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
        crate::core::rect_authority::assert_paint_mutation_authorized(
            "update_caret_blink: caret_visible/last_blink_ms",
        );
        caret_visible.set(!caret_visible.get());
        last_blink_ms.set(now_ms);
    }
}

/// Reset caret to visible state.
pub fn reset_caret_blink(last_blink_ms: &Cell<u64>, caret_visible: &Cell<bool>, now_ms: u64) {
    caret_visible.set(true);
    last_blink_ms.set(now_ms);
}

// ============================================================================
// Undo/Redo Pipeline
// ============================================================================

/// Capture current editing state as a snapshot.
pub fn capture_snapshot(text: &str, cursor: &TextCursor, selection: &Selection) -> EditSnapshot {
    EditSnapshot::capture(text, cursor, selection)
}

/// Record an edit in undo history.
/// Call AFTER the mutation is complete, passing the before snapshot.
pub fn record_edit(
    undo_history: &mut UndoHistory,
    text: &str,
    cursor: &TextCursor,
    selection: &Selection,
    before: EditSnapshot,
    kind: EditKind,
    timestamp_ms: u64,
) {
    let after = EditSnapshot::capture(text, cursor, selection);
    undo_history.record(before, after, kind, timestamp_ms);
}

/// Perform undo operation.
/// Returns true if undo was performed.
/// Caller must call mark_modified(), invalidate_layout(), reset_caret_blink().
pub fn perform_undo(
    ime_snapshot: &mut Option<EditSnapshot>,
    undo_history: &mut UndoHistory,
    document_content: &mut String,
    cursor: &mut TextCursor,
    selection: &mut Selection,
    layout_dirty: &Cell<bool>,
) -> bool {
    // Clear IME composition
    *ime_snapshot = None;
    undo_history.resume();

    if let Some(snapshot) = undo_history.undo() {
        let snapshot = snapshot.clone();
        snapshot.restore(document_content, cursor, selection);
        layout_dirty.set(true);
        true
    } else {
        false
    }
}

/// Perform redo operation.
/// Returns true if redo was performed.
/// Caller must call mark_modified(), invalidate_layout(), reset_caret_blink().
pub fn perform_redo(
    ime_snapshot: &mut Option<EditSnapshot>,
    undo_history: &mut UndoHistory,
    document_content: &mut String,
    cursor: &mut TextCursor,
    selection: &mut Selection,
    layout_dirty: &Cell<bool>,
) -> bool {
    // Clear IME composition
    *ime_snapshot = None;
    undo_history.resume();

    if let Some(snapshot) = undo_history.redo() {
        let snapshot = snapshot.clone();
        snapshot.restore(document_content, cursor, selection);
        layout_dirty.set(true);
        true
    } else {
        false
    }
}

// ============================================================================
// Horizontal Navigation
// ============================================================================

/// Handle Left arrow key with all modifier combinations.
pub fn handle_left_key(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    text: &str,
    modifiers: &Modifiers,
) {
    #[cfg(target_os = "macos")]
    let word_mod = modifiers.alt;
    #[cfg(not(target_os = "macos"))]
    let word_mod = modifiers.ctrl;

    if word_mod && modifiers.shift {
        move_word_left_select(cursor, selection, boundaries, text);
    } else if word_mod {
        move_word_left(cursor, selection, boundaries, text);
    } else if modifiers.shift {
        move_left_select(cursor, selection, boundaries);
    } else {
        move_left_collapse(cursor, selection, boundaries);
    }

    debug_assert_cursor_in_bounds(cursor, text.len());
    debug_assert_selection_in_bounds(selection, text.len());
    debug_assert_cursor_on_boundary(cursor, boundaries);
    debug_assert_cursor_selection_coherent(cursor, selection);
}

/// Handle Right arrow key with all modifier combinations.
pub fn handle_right_key(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    text: &str,
    modifiers: &Modifiers,
) {
    #[cfg(target_os = "macos")]
    let word_mod = modifiers.alt;
    #[cfg(not(target_os = "macos"))]
    let word_mod = modifiers.ctrl;

    if word_mod && modifiers.shift {
        move_word_right_select(cursor, selection, boundaries, text);
    } else if word_mod {
        move_word_right(cursor, selection, boundaries, text);
    } else if modifiers.shift {
        move_right_select(cursor, selection, boundaries);
    } else {
        move_right_collapse(cursor, selection, boundaries);
    }

    debug_assert_cursor_in_bounds(cursor, text.len());
    debug_assert_selection_in_bounds(selection, text.len());
    debug_assert_cursor_on_boundary(cursor, boundaries);
    debug_assert_cursor_selection_coherent(cursor, selection);
}

/// Handle Home key with shift modifier.
pub fn handle_home_key(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    shift: bool,
) {
    if shift {
        move_home_select(cursor, selection, boundaries);
    } else {
        move_home(cursor, selection, boundaries);
    }

    debug_assert_cursor_on_boundary(cursor, boundaries);
    debug_assert_cursor_selection_coherent(cursor, selection);
}

/// Handle End key with shift modifier.
pub fn handle_end_key(
    cursor: &mut TextCursor,
    selection: &mut Selection,
    boundaries: &[ClusterBoundary],
    shift: bool,
) {
    if shift {
        move_end_select(cursor, selection, boundaries);
    } else {
        move_end(cursor, selection, boundaries);
    }

    debug_assert_cursor_on_boundary(cursor, boundaries);
    debug_assert_cursor_selection_coherent(cursor, selection);
}

// ============================================================================
// Clipboard Operations
// ============================================================================

/// Get selected text for copy operation.
/// Returns None if no selection.
pub fn get_selection_text<'a>(text: &'a str, selection: &Selection) -> Option<&'a str> {
    if selection.anchor == selection.head {
        return None;
    }
    let sel_start = selection.anchor.min(selection.head);
    let sel_end = selection.anchor.max(selection.head);
    text.get(sel_start..sel_end)
}

/// Perform cut operation - delete selection.
/// Call AFTER copying to clipboard.
/// Caller must record edit with EditKind::Cut.
pub fn perform_cut(
    document_content: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
) {
    apply_edit_op(
        document_content,
        selection,
        cursor,
        boundaries,
        TextEditOp::Delete,
    );

    debug_assert_cursor_in_bounds(cursor, document_content.len());
    debug_assert_selection_in_bounds(selection, document_content.len());
    debug_assert_cursor_selection_coherent(cursor, selection);
}

/// Normalize clipboard text for single-line paste.
pub fn normalize_paste_newlines(mut text: String) -> String {
    text = text.replace("\r\n", "\n");
    text = text.replace('\r', "\n");
    text = text.replace('\n', " ");
    text
}

/// Perform paste insertion.
/// Caller must record edit with EditKind::Paste.
pub fn perform_paste(
    document_content: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
    text: &str,
) {
    apply_insert_op(document_content, selection, cursor, boundaries, text);

    debug_assert_cursor_in_bounds(cursor, document_content.len());
    debug_assert_selection_in_bounds(selection, document_content.len());
    debug_assert_cursor_selection_coherent(cursor, selection);
}

// ============================================================================
// Text Insertion
// ============================================================================

/// Perform text insertion.
/// Caller must record edit.
pub fn perform_text_insert(
    document_content: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
    text: &str,
) {
    apply_insert_op(document_content, selection, cursor, boundaries, text);

    debug_assert_cursor_in_bounds(cursor, document_content.len());
    debug_assert_selection_in_bounds(selection, document_content.len());
    debug_assert_cursor_selection_coherent(cursor, selection);
}

/// Determine edit kind for text insertion based on selection state.
pub fn edit_kind_for_insert(selection: &Selection) -> EditKind {
    if selection.anchor != selection.head {
        EditKind::SelectionDelete
    } else {
        EditKind::Insert
    }
}

/// Determine edit kind for backspace based on selection state.
pub fn edit_kind_for_backspace(selection: &Selection) -> EditKind {
    if selection.anchor != selection.head {
        EditKind::SelectionDelete
    } else {
        EditKind::Backspace
    }
}

/// Determine edit kind for delete based on selection state.
pub fn edit_kind_for_delete(selection: &Selection) -> EditKind {
    if selection.anchor != selection.head {
        EditKind::SelectionDelete
    } else {
        EditKind::Delete
    }
}

/// Perform backspace operation.
/// Caller must record edit.
pub fn perform_backspace(
    document_content: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
) {
    apply_edit_op(
        document_content,
        selection,
        cursor,
        boundaries,
        TextEditOp::Backspace,
    );

    debug_assert_cursor_in_bounds(cursor, document_content.len());
    debug_assert_selection_in_bounds(selection, document_content.len());
    debug_assert_cursor_selection_coherent(cursor, selection);
}

/// Perform delete operation.
/// Caller must record edit.
pub fn perform_delete(
    document_content: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
) {
    apply_edit_op(
        document_content,
        selection,
        cursor,
        boundaries,
        TextEditOp::Delete,
    );

    debug_assert_cursor_in_bounds(cursor, document_content.len());
    debug_assert_selection_in_bounds(selection, document_content.len());
    debug_assert_cursor_selection_coherent(cursor, selection);
}

// ============================================================================
// IME Handling
// ============================================================================

/// Handle IME PreEdit event.
/// Returns true if composition was just started (for first preedit only).
pub fn handle_ime_preedit(
    ime_snapshot: &mut Option<EditSnapshot>,
    undo_history: &mut UndoHistory,
    text: &str,
    cursor: &TextCursor,
    selection: &Selection,
) -> bool {
    if ime_snapshot.is_none() {
        *ime_snapshot = Some(EditSnapshot::capture(text, cursor, selection));
        undo_history.suspend();
        true
    } else {
        false
    }
}

/// Perform IME commit insertion.
///
/// # Preconditions (hard requirements)
/// - Widget MUST have called `undo_history.resume()` before this function
/// - Widget MUST have computed `before` snapshot before any mutable borrow
/// - Widget MUST record edit with EditKind::ImeCommit after this returns
///
/// This delegate only performs the insert - it does NOT touch undo history.
pub fn perform_ime_commit_insert(
    document_content: &mut String,
    selection: &mut Selection,
    cursor: &mut TextCursor,
    boundaries: &[ClusterBoundary],
    commit_text: &str,
) {
    apply_insert_op(document_content, selection, cursor, boundaries, commit_text);

    debug_assert_cursor_in_bounds(cursor, document_content.len());
    debug_assert_selection_in_bounds(selection, document_content.len());
    debug_assert_cursor_selection_coherent(cursor, selection);
}

/// Handle IME Cancel event.
pub fn handle_ime_cancel(ime_snapshot: &mut Option<EditSnapshot>, undo_history: &mut UndoHistory) {
    *ime_snapshot = None;
    undo_history.resume();
}

// ============================================================================
// Select All
// ============================================================================

/// Select all text in document.
pub fn select_all(cursor: &mut TextCursor, selection: &mut Selection, text_len: usize) {
    selection.anchor = 0;
    selection.head = text_len;
    cursor.pos = text_len;

    debug_assert_cursor_in_bounds(cursor, text_len);
    debug_assert_selection_in_bounds(selection, text_len);
    debug_assert_cursor_selection_coherent(cursor, selection);
}
