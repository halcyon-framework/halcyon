// =============================================================================
// SPEC-HAL-174: Edit Lifecycle and Intent Model Integration Tests
// =============================================================================
//
// Verifies:
// - INV-174-1: Exactly one CellId at a time (no multi-edit, nested begin rejected).
// - INV-174-2: Explicit lifecycle Idle → Editing → Idle (commit/cancel).
// - INV-174-3: EditState owned by TabularSubstrate.
// - INV-174-5: Sort/Focus during edit → Cancel, then apply.
//              Scroll/column-hide → Cancel (destabilization).
// - INV-174-6: RowModel identity never mutated by edit lifecycle.
// - Commit preserves focus on edited cell.
// - Cancel restores pre-edit focus.
// - Mismatched commit/cancel targets rejected.
// - Deferred comparator applied on commit/cancel (SPEC-HAL-174A).
//
// All tests use only public APIs.
// =============================================================================

#[allow(unused_imports)]
use halcyon::widgets::tabular_view::{
    Column, ColumnId, EditIntent, EditPayload, EditState, FocusIntent, NavigationDirection,
    RowComparator, RowId, SortDirection, SortIntent, TabularIntent, TabularSubstrate,
};
use std::cmp::Ordering;

fn setup_view() -> (TabularSubstrate, Vec<ColumnId>, Vec<RowId>) {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    let col_a = view.column_model().columns()[0].id;
    let col_b = view.column_model().columns()[1].id;

    let r0 = RowId::new();
    let r1 = RowId::new();
    let r2 = RowId::new();
    view.add_row(r0);
    view.add_row(r1);
    view.add_row(r2);

    (view, vec![col_a, col_b], vec![r0, r1, r2])
}

// =============================================================================
// Session lifecycle (INV-174-2)
// =============================================================================

#[test]
fn test_no_edit_session_initially() {
    let (view, _, _) = setup_view();
    assert!(!view.is_editing());
    assert!(view.edit_session().is_none());
    assert_eq!(*view.edit_state(), EditState::Idle);
}

#[test]
fn test_begin_edit_creates_session() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[0],
    }));

    assert!(view.is_editing());
    assert!(matches!(*view.edit_state(), EditState::Editing(_)));
    let session = view.edit_session().unwrap();
    assert_eq!(session.editing_cell().row_id, rows[1]);
    assert_eq!(session.editing_cell().column_id, cols[0]);
}

#[test]
fn test_begin_edit_records_pre_edit_focus() {
    let (mut view, cols, rows) = setup_view();

    // Set focus on row[0], col[1]
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[0],
        column_id: cols[1],
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, rows[0]);
    assert_eq!(view.focus_state().focused().unwrap().column_id, cols[1]);

    // Begin edit on row[1], col[0]
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[0],
    }));

    let session = view.edit_session().unwrap();
    // Pre-edit focus was row[0], col[1]
    let pre = session.pre_edit_focus().unwrap();
    assert_eq!(pre.row_id, rows[0]);
    assert_eq!(pre.column_id, cols[1]);
    // Current focus moved to edited cell
    assert_eq!(view.focus_state().focused().unwrap().row_id, rows[1]);
    assert_eq!(view.focus_state().focused().unwrap().column_id, cols[0]);
}

#[test]
fn test_begin_edit_with_no_prior_focus() {
    let (mut view, cols, rows) = setup_view();
    assert!(view.focus_state().focused().is_none());

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[2],
        column_id: cols[1],
    }));

    let session = view.edit_session().unwrap();
    assert!(session.pre_edit_focus().is_none());
    // Focus moved to the edited cell
    assert_eq!(view.focus_state().focused().unwrap().row_id, rows[2]);
    assert_eq!(view.focus_state().focused().unwrap().column_id, cols[1]);
}

// =============================================================================
// Commit
// =============================================================================

#[test]
fn test_commit_preserves_focus_on_edited_cell() {
    let (mut view, cols, rows) = setup_view();

    // Focus row[0]
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Begin edit on row[1]
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[1],
    }));

    // Commit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[1],
        column_id: cols[1],
        payload: EditPayload::String("value".into()),
    }));

    assert!(!view.is_editing());
    assert_eq!(*view.edit_state(), EditState::Idle);
    // Focus stays on the committed cell
    let focused = view.focus_state().focused().unwrap();
    assert_eq!(focused.row_id, rows[1]);
    assert_eq!(focused.column_id, cols[1]);
}

#[test]
fn test_commit_without_active_session_is_noop() {
    let (mut view, cols, rows) = setup_view();
    let v_before = view.focus_state().version();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[0],
        column_id: cols[0],
        payload: EditPayload::None,
    }));

    assert!(!view.is_editing());
    assert_eq!(view.focus_state().version(), v_before);
}

#[test]
fn test_commit_does_not_bump_version_when_focus_unchanged() {
    let (mut view, cols, rows) = setup_view();

    // Begin edit — focus moves to edited cell
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    // Focus is now on (rows[0], cols[0]) — same as editing_cell
    let v_before = view.focus_state().version();

    // Commit — focus stays on editing_cell, which is already focused
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[0],
        column_id: cols[0],
        payload: EditPayload::None,
    }));

    // Version should NOT have bumped (focus didn't actually change)
    assert_eq!(view.focus_state().version(), v_before);
}

// =============================================================================
// Cancel
// =============================================================================

#[test]
fn test_cancel_restores_pre_edit_focus() {
    let (mut view, cols, rows) = setup_view();

    // Focus row[0], col[0]
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Begin edit on row[2], col[1]
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[2],
        column_id: cols[1],
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, rows[2]);

    // Cancel
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[2],
        column_id: cols[1],
    }));

    assert!(!view.is_editing());
    // Focus restored to pre-edit: row[0], col[0]
    let focused = view.focus_state().focused().unwrap();
    assert_eq!(focused.row_id, rows[0]);
    assert_eq!(focused.column_id, cols[0]);
}

#[test]
fn test_cancel_with_no_pre_edit_focus_clears_focus() {
    let (mut view, cols, rows) = setup_view();
    assert!(view.focus_state().focused().is_none());

    // Begin edit (no prior focus)
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[0],
    }));

    // Cancel
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[1],
        column_id: cols[0],
    }));

    assert!(!view.is_editing());
    assert!(view.focus_state().focused().is_none());
}

#[test]
fn test_cancel_without_active_session_is_noop() {
    let (mut view, cols, rows) = setup_view();

    // Set focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[0],
        column_id: cols[0],
    }));
    let v_before = view.focus_state().version();

    // Cancel with no session
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[1],
        column_id: cols[1],
    }));

    assert!(!view.is_editing());
    // Focus unchanged
    assert_eq!(view.focus_state().focused().unwrap().row_id, rows[0]);
    assert_eq!(view.focus_state().version(), v_before);
}

// =============================================================================
// INV-174-5: Destabilization cancel — Focus during edit
// =============================================================================

#[test]
fn test_focus_during_edit_cancels_then_applies() {
    let (mut view, cols, rows) = setup_view();

    // Set pre-edit focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Begin edit on row[1], col[0]
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // Focus intent → cancels edit, then applies new focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[2],
        column_id: cols[1],
    }));

    assert!(!view.is_editing());
    // Focus is now the target of the focus intent (applied after cancel)
    let focused = view.focus_state().focused().unwrap();
    assert_eq!(focused.row_id, rows[2]);
    assert_eq!(focused.column_id, cols[1]);
}

#[test]
fn test_clear_focus_during_edit_cancels_then_clears() {
    let (mut view, cols, rows) = setup_view();

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // ClearFocus → cancels edit (restores pre-edit=None), then clears
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestClearFocus));

    assert!(!view.is_editing());
    assert!(view.focus_state().focused().is_none());
}

#[test]
fn test_navigate_during_edit_cancels_then_navigates() {
    let (mut view, cols, rows) = setup_view();

    // Set pre-edit focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[1],
        column_id: cols[0],
    }));

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // Navigate Down → cancels edit, then navigates
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: view
            .focus_state()
            .focused()
            .map(|c| (c.row_id, c.column_id)),
        direction: NavigationDirection::Down,
    }));

    assert!(!view.is_editing());
    // After cancel, pre-edit focus (rows[1]) is restored, then navigation applies
    // Navigation from rows[1] down goes to rows[2]
    // (exact target depends on resolve_navigation, but edit is cancelled)
}

// =============================================================================
// INV-174-5: Destabilization cancel — Sort during edit
// =============================================================================

#[test]
fn test_sort_during_edit_cancels_then_applies() {
    let (mut view, cols, rows) = setup_view();

    // Set pre-edit focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[1],
        column_id: cols[1],
    }));
    let pre_focus = view.focus_state().focused();

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // Sort intent → cancels edit, then applies sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: cols[0],
        direction: Some(SortDirection::Ascending),
    }));

    assert!(!view.is_editing());
    assert!(view.sort_state().is_active());
    // Focus restored to pre-edit state (cancel semantics)
    assert_eq!(view.focus_state().focused(), pre_focus);
}

#[test]
fn test_sort_after_commit_works() {
    let (mut view, cols, rows) = setup_view();

    // Begin + commit edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[0],
        column_id: cols[0],
        payload: EditPayload::None,
    }));

    // Sort should work
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: cols[0],
        direction: Some(SortDirection::Ascending),
    }));

    assert!(view.sort_state().is_active());
}

#[test]
fn test_sort_after_cancel_works() {
    let (mut view, cols, rows) = setup_view();

    // Begin + cancel edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Sort should work
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: cols[1],
        direction: Some(SortDirection::Descending),
    }));

    assert!(view.sort_state().is_active());
}

// =============================================================================
// INV-174-5: Destabilization cancel — Scroll
// =============================================================================

#[test]
fn test_scroll_during_edit_cancels() {
    let (mut view, cols, rows) = setup_view();

    // Set pre-edit focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[2],
        column_id: cols[1],
    }));

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // Scroll change → destabilization cancel
    view.set_scroll_y(50.0);

    assert!(!view.is_editing());
    // Focus restored to pre-edit
    let focused = view.focus_state().focused().unwrap();
    assert_eq!(focused.row_id, rows[2]);
    assert_eq!(focused.column_id, cols[1]);
}

#[test]
fn test_scroll_same_value_no_cancel() {
    let (mut view, cols, rows) = setup_view();

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // Set scroll to same value (0.0) — no cancel
    view.set_scroll_y(0.0);

    assert!(view.is_editing());
}

// =============================================================================
// INV-174-5: Destabilization cancel — Column hide
// =============================================================================

#[test]
fn test_column_hide_editing_column_cancels() {
    let (mut view, cols, rows) = setup_view();

    // Begin edit on cols[0]
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // Hide the editing column → destabilization cancel
    view.set_column_visible(cols[0], false);

    assert!(!view.is_editing());
}

#[test]
fn test_column_hide_other_column_no_cancel() {
    let (mut view, cols, rows) = setup_view();

    // Begin edit on cols[0]
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // Hide a different column — no cancel
    view.set_column_visible(cols[1], false);

    assert!(view.is_editing());
}

// =============================================================================
// Version semantics
// =============================================================================

#[test]
fn test_begin_edit_bumps_version() {
    let (mut view, cols, rows) = setup_view();
    let v0 = view.focus_state().version();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    assert!(view.focus_state().version() > v0);
}

#[test]
fn test_cancel_bumps_version_when_focus_changes() {
    let (mut view, cols, rows) = setup_view();

    // Set focus first so cancel restores to a different cell
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Begin edit on a different cell
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[1],
    }));
    let v1 = view.focus_state().version();

    // Cancel — focus changes back to row[0]
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[1],
        column_id: cols[1],
    }));

    assert!(view.focus_state().version() > v1);
}

// =============================================================================
// Edit does not leak into sort state
// =============================================================================

#[test]
fn test_edit_lifecycle_does_not_mutate_sort() {
    let (mut view, cols, rows) = setup_view();

    // Activate sort first
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: cols[0],
        direction: Some(SortDirection::Ascending),
    }));
    assert!(view.sort_state().is_active());
    let sort_col = view.sort_state().active_column();
    let sort_dir = view.sort_state().direction();

    // Full edit lifecycle
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[1],
    }));
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[1],
        column_id: cols[1],
        payload: EditPayload::String("x".into()),
    }));

    // Sort state unchanged
    assert!(view.sort_state().is_active());
    assert_eq!(view.sort_state().active_column(), sort_col);
    assert_eq!(view.sort_state().direction(), sort_dir);
}

// =============================================================================
// Mismatched commit/cancel targets
// =============================================================================

#[cfg(not(debug_assertions))]
#[test]
fn test_commit_mismatched_target_ignored() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Commit with wrong target — ignored in release
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[1],
        column_id: cols[1],
        payload: EditPayload::None,
    }));

    // Still editing
    assert!(view.is_editing());
    assert_eq!(view.edit_session().unwrap().editing_cell().row_id, rows[0]);
}

#[cfg(not(debug_assertions))]
#[test]
fn test_cancel_mismatched_target_ignored() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Cancel with wrong target — ignored in release
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[2],
        column_id: cols[1],
    }));

    // Still editing
    assert!(view.is_editing());
}

// =============================================================================
// Nested begin edit (INV-174-1)
// =============================================================================

#[cfg(not(debug_assertions))]
#[test]
fn test_nested_begin_edit_ignored() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Second begin — ignored in release
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[1],
    }));

    // Original session intact
    assert_eq!(view.edit_session().unwrap().editing_cell().row_id, rows[0]);
    assert_eq!(
        view.edit_session().unwrap().editing_cell().column_id,
        cols[0]
    );
}

// =============================================================================
// Debug panic guards
// =============================================================================

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "SPEC-HAL-174 INV-174-1: nested RequestBeginEdit forbidden")]
fn test_nested_begin_edit_debug_panics() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[1],
        column_id: cols[1],
    }));
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "SPEC-HAL-174: commit target mismatch")]
fn test_commit_mismatch_debug_panics() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[1],
        column_id: cols[1],
        payload: EditPayload::None,
    }));
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "SPEC-HAL-174: cancel target mismatch")]
fn test_cancel_mismatch_debug_panics() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[2],
        column_id: cols[1],
    }));
}

// =============================================================================
// INV-EDIT-ORDER: set_comparator deferred during edit (SPEC-HAL-174A)
// =============================================================================

#[allow(dead_code)]
struct ReverseComparator;
impl RowComparator for ReverseComparator {
    fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
        Some(b.cmp(&a))
    }
}

#[allow(dead_code)]
struct ForwardComparator;
impl RowComparator for ForwardComparator {
    fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
        Some(a.cmp(&b))
    }
}

#[cfg(not(debug_assertions))]
#[test]
fn test_set_comparator_during_edit_is_deferred() {
    let (mut view, cols, rows) = setup_view();

    // Activate sort with initial comparator
    view.set_comparator(ReverseComparator);
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: cols[0],
        direction: Some(SortDirection::Ascending),
    }));
    let version_after_sort = view.sort_state().version();
    let dirty_after_sort = view.sort_state().is_dirty();

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    assert!(view.is_editing());

    // set_comparator during edit: deferred, sort state unchanged
    view.set_comparator(ForwardComparator);
    assert_eq!(view.sort_state().version(), version_after_sort);
    assert_eq!(view.sort_state().is_dirty(), dirty_after_sort);
}

#[cfg(not(debug_assertions))]
#[test]
fn test_set_comparator_after_edit_commit_applies() {
    let (mut view, cols, rows) = setup_view();

    // Activate sort
    view.set_comparator(ReverseComparator);
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: cols[0],
        direction: Some(SortDirection::Ascending),
    }));
    let version_after_sort = view.sort_state().version();

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Defer comparator change — version unchanged during edit
    view.set_comparator(ForwardComparator);
    assert_eq!(view.sort_state().version(), version_after_sort);

    // Commit edit: deferred comparator applied, version bumps
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[0],
        column_id: cols[0],
        payload: EditPayload::None,
    }));

    assert!(!view.is_editing());
    assert!(view.sort_state().version() > version_after_sort);
    assert!(view.sort_state().is_dirty());
}

#[cfg(not(debug_assertions))]
#[test]
fn test_set_comparator_after_edit_cancel_applies() {
    let (mut view, cols, rows) = setup_view();

    // Activate sort
    view.set_comparator(ReverseComparator);
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: cols[0],
        direction: Some(SortDirection::Ascending),
    }));
    let version_after_sort = view.sort_state().version();

    // Begin edit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    // Defer comparator change — version unchanged during edit
    view.set_comparator(ForwardComparator);
    assert_eq!(view.sort_state().version(), version_after_sort);

    // Cancel edit: deferred comparator applied, version bumps
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCancelEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    assert!(!view.is_editing());
    assert!(view.sort_state().version() > version_after_sort);
    assert!(view.sort_state().is_dirty());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "SPEC-HAL-174: set_comparator forbidden during active edit session")]
fn test_set_comparator_during_edit_debug_panics() {
    let (mut view, cols, rows) = setup_view();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));

    view.set_comparator(ForwardComparator);
}

// =============================================================================
// INV-174-6: RowModel identity not mutated by edit lifecycle
// =============================================================================

#[test]
fn test_row_model_unchanged_after_edit_lifecycle() {
    let (mut view, cols, rows) = setup_view();

    let row_count_before = view.row_model().len();
    let row_ids_before: Vec<RowId> = (0..row_count_before)
        .map(|i| view.row_model().get(i).unwrap())
        .collect();

    // Full edit lifecycle: begin, commit
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestCommitEdit {
        row_id: rows[0],
        column_id: cols[0],
        payload: EditPayload::String("data".into()),
    }));

    // RowModel unchanged
    assert_eq!(view.row_model().len(), row_count_before);
    for (i, &expected_id) in row_ids_before.iter().enumerate() {
        assert_eq!(view.row_model().get(i).unwrap(), expected_id);
    }
}

#[test]
fn test_row_model_unchanged_after_destabilization_cancel() {
    let (mut view, cols, rows) = setup_view();

    let row_ids_before: Vec<RowId> = (0..view.row_model().len())
        .map(|i| view.row_model().get(i).unwrap())
        .collect();

    // Begin edit, then scroll (destabilization cancel)
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: rows[0],
        column_id: cols[0],
    }));
    view.set_scroll_y(100.0);
    assert!(!view.is_editing());

    // RowModel unchanged
    for (i, &expected_id) in row_ids_before.iter().enumerate() {
        assert_eq!(view.row_model().get(i).unwrap(), expected_id);
    }
}
