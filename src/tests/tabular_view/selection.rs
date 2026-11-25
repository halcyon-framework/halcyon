// =============================================================================
// TabularSubstrate Selection Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-178: Selection Semantics (Single + Range).
//
// Key invariants under test:
// - INV-178-1: Selection unit is row (RowId). No cell-level selection.
// - INV-178-2: Selection never mutates focus.
// - INV-178-3: Selection is identity-sticky across sort/scroll.
// - INV-178-4: Anchor set on SelectRow/first ExtendToRow, cleared on ClearSelection/edit.
// - INV-178-5: Beginning edit clears selection.
// - INV-178-6: Selection changes are versioned strictly monotonic.
// - INV-178-7: Extend range computed over effective_row_order().
// - INV-178-8: No RowModel/ColumnModel/geometry mutation.
//
// All tests use only public APIs.
//
// LEGACY READ — selection assertions use selection_state() (deprecated mirror).
// Will migrate to ordered_selection_state accessors when available.
// =============================================================================

use crate::widgets::tabular_view::{
    Column, ColumnId, EditIntent, FocusIntent, NavigationDirection, RowComparator, RowId,
    SelectionIntent, SelectionState, SortDirection, SortIntent, TabularIntent, TabularSubstrate,
};
use std::cmp::Ordering;

// =============================================================================
// Test Comparators
// =============================================================================

/// Comparator that reverses natural order.
struct ReverseComparator;
impl RowComparator for ReverseComparator {
    fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
        Some(b.cmp(&a))
    }
}

// =============================================================================
// Test 1: SelectionState starts empty
// =============================================================================

#[test]
fn test_selection_state_initial() {
    let state = SelectionState::new();
    assert!(!state.has_selection());
    assert!(state.anchor().is_none());
    assert!(state.selected_rows().is_empty());
    assert_eq!(state.version(), 0);
    assert_eq!(state.count(), 0);
}

// =============================================================================
// Test 2: SelectRow sets anchor and selects single row (INV-178-1)
// =============================================================================

#[test]
fn test_select_row() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[2],
    }));

    assert!(view.legacy_selection_state().has_selection());
    assert_eq!(view.legacy_selection_state().anchor(), Some(ids[2]));
    assert_eq!(view.legacy_selection_state().selected_rows(), &[ids[2]]);
    assert_eq!(view.legacy_selection_state().count(), 1);
    assert!(view.legacy_selection_state().is_selected(ids[2]));
    assert!(!view.legacy_selection_state().is_selected(ids[0]));
}

// =============================================================================
// Test 3: SelectRow clears previous selection
// =============================================================================

#[test]
fn test_select_row_clears_previous() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Select row 1
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[1],
    }));
    assert_eq!(view.legacy_selection_state().count(), 1);

    // Select row 3 — should clear previous
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[3],
    }));
    assert_eq!(view.legacy_selection_state().count(), 1);
    assert_eq!(view.legacy_selection_state().selected_rows(), &[ids[3]]);
    assert!(!view.legacy_selection_state().is_selected(ids[1]));
}

// =============================================================================
// Test 4: Selection never mutates focus (INV-178-2)
// =============================================================================

#[test]
fn test_selection_does_not_mutate_focus() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Set focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[0],
        column_id: col_id,
    }));
    let focus_version = view.focus_state().version();
    let focused = view.focus_state().focused();

    // Perform selection operations
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[2],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[4],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[1],
    }));

    // Focus unchanged
    assert_eq!(view.focus_state().version(), focus_version);
    assert_eq!(view.focus_state().focused(), focused);
}

// =============================================================================
// Test 5: ExtendToRow with no anchor sets anchor and selects target
// =============================================================================

#[test]
fn test_extend_no_anchor() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[3],
    }));

    assert_eq!(view.legacy_selection_state().anchor(), Some(ids[3]));
    assert_eq!(view.legacy_selection_state().selected_rows(), &[ids[3]]);
}

// =============================================================================
// Test 6: ExtendToRow with anchor selects range (INV-178-7)
// =============================================================================

#[test]
fn test_extend_range() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Set anchor via SelectRow
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[1],
    }));

    // Extend to row 3 — should select rows 1, 2, 3
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[3],
    }));

    assert_eq!(view.legacy_selection_state().anchor(), Some(ids[1]));
    assert_eq!(view.legacy_selection_state().count(), 3);
    assert!(view.legacy_selection_state().is_selected(ids[1]));
    assert!(view.legacy_selection_state().is_selected(ids[2]));
    assert!(view.legacy_selection_state().is_selected(ids[3]));
    assert!(!view.legacy_selection_state().is_selected(ids[0]));
    assert!(!view.legacy_selection_state().is_selected(ids[4]));
}

// =============================================================================
// Test 7: ExtendToRow in reverse direction
// =============================================================================

#[test]
fn test_extend_range_reverse() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Anchor at row 3
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[3],
    }));

    // Extend to row 1 (reverse) — should select rows 1, 2, 3
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[1],
    }));

    assert_eq!(view.legacy_selection_state().count(), 3);
    assert!(view.legacy_selection_state().is_selected(ids[1]));
    assert!(view.legacy_selection_state().is_selected(ids[2]));
    assert!(view.legacy_selection_state().is_selected(ids[3]));
}

// =============================================================================
// Test 8: ToggleRow adds and removes
// =============================================================================

#[test]
fn test_toggle_row() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Toggle on
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[1],
    }));
    assert!(view.legacy_selection_state().is_selected(ids[1]));
    // ToggleRow does not set anchor (anchor is for range-selection operations).

    // Toggle another on
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[3],
    }));
    assert!(view.legacy_selection_state().is_selected(ids[1]));
    assert!(view.legacy_selection_state().is_selected(ids[3]));
    assert_eq!(view.legacy_selection_state().count(), 2);

    // Toggle first off
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[1],
    }));
    assert!(!view.legacy_selection_state().is_selected(ids[1]));
    assert!(view.legacy_selection_state().is_selected(ids[3]));
    assert_eq!(view.legacy_selection_state().count(), 1);
}

// =============================================================================
// Test 9: ClearSelection clears all (INV-178-4)
// =============================================================================

#[test]
fn test_clear_selection() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..3)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[0],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[2],
    }));
    assert_eq!(view.legacy_selection_state().count(), 3);
    assert!(view.legacy_selection_state().anchor().is_some());

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ClearSelection));

    assert!(!view.legacy_selection_state().has_selection());
    assert!(view.legacy_selection_state().anchor().is_none());
    assert_eq!(view.legacy_selection_state().count(), 0);
}

// =============================================================================
// Test 10: ClearSelection when already empty is no-op (no version bump)
// =============================================================================

#[test]
fn test_clear_selection_noop_when_empty() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_row(RowId::new());

    let version_before = view.legacy_selection_state().version();
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ClearSelection));
    assert_eq!(view.legacy_selection_state().version(), version_before);
}

// =============================================================================
// Test 11: Beginning edit clears selection (INV-178-5)
// =============================================================================

#[test]
fn test_edit_begin_clears_selection() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Focus and select
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[2],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[0],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[3],
    }));
    assert_eq!(view.legacy_selection_state().count(), 4);

    // Begin edit — selection must be cleared
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: ids[2],
        column_id: col_id,
    }));

    assert!(!view.legacy_selection_state().has_selection());
    assert!(view.legacy_selection_state().anchor().is_none());
    assert!(view.is_editing());
}

// =============================================================================
// Test 12: Version increments strictly monotonic (INV-178-6)
// =============================================================================

#[test]
fn test_version_monotonic() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..3)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    assert_eq!(view.legacy_selection_state().version(), 0);

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[0],
    }));
    assert_eq!(view.legacy_selection_state().version(), 1);

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[2],
    }));
    assert_eq!(view.legacy_selection_state().version(), 2);

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[1],
    }));
    assert_eq!(view.legacy_selection_state().version(), 3);

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ClearSelection));
    assert_eq!(view.legacy_selection_state().version(), 4);
}

// =============================================================================
// Test 13: Selection identity-sticky across sort (INV-178-3)
// =============================================================================

#[test]
fn test_selection_sticky_across_sort() {
    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Select rows 1 and 3
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[1],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[3],
    }));
    let sel_version = view.legacy_selection_state().version();

    // Apply sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    }));

    // Selection unchanged (identity-sticky)
    assert_eq!(view.legacy_selection_state().version(), sel_version);
    assert!(view.legacy_selection_state().is_selected(ids[1]));
    assert!(view.legacy_selection_state().is_selected(ids[3]));
    assert_eq!(view.legacy_selection_state().count(), 2);
}

// =============================================================================
// Test 15: Selection identity-sticky across scroll (INV-178-3)
// =============================================================================

#[test]
fn test_selection_sticky_across_scroll() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..20)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Select row 5
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[5],
    }));
    let sel_version = view.legacy_selection_state().version();

    // Scroll far (simulated via set_scroll_y)
    view.set_scroll_y(500.0);

    // Selection unchanged
    assert_eq!(view.legacy_selection_state().version(), sel_version);
    assert!(view.legacy_selection_state().is_selected(ids[5]));
}

// =============================================================================
// Test 16: ExtendToRow uses sorted order (INV-178-7)
// =============================================================================

#[test]
fn test_extend_uses_sorted_order() {
    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Apply reverse sort: effective order = 4, 3, 2, 1, 0
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    }));

    // Anchor at ids[4] (position 0 in sorted order)
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[4],
    }));

    // Extend to ids[2] (position 2 in sorted order)
    // In reversed order: 4, 3, 2 — should select ids[4], ids[3], ids[2]
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[2],
    }));

    assert_eq!(view.legacy_selection_state().count(), 3);
    assert!(view.legacy_selection_state().is_selected(ids[4]));
    assert!(view.legacy_selection_state().is_selected(ids[3]));
    assert!(view.legacy_selection_state().is_selected(ids[2]));
    assert!(!view.legacy_selection_state().is_selected(ids[1]));
    assert!(!view.legacy_selection_state().is_selected(ids[0]));
}

// =============================================================================
// Test 18: Focus and selection are independent
// =============================================================================

#[test]
fn test_focus_selection_independent() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Focus on row 0
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[0],
        column_id: col_id,
    }));

    // Select row 3 — focus stays on row 0
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[3],
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[0]);
    assert!(view.legacy_selection_state().is_selected(ids[3]));

    // Navigate focus — selection unchanged
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[1]);
    assert!(view.legacy_selection_state().is_selected(ids[3]));
    assert!(!view.legacy_selection_state().is_selected(ids[1]));
}

// =============================================================================
// Test 19: Anchor correctness — toggle removes anchor
// =============================================================================

#[test]
fn test_anchor_cleared_when_toggle_removes_anchor_row() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..3)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Toggle on ids[0] — ToggleRow does not set anchor.
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[0],
    }));
    assert!(view.legacy_selection_state().is_selected(ids[0]));
    assert_eq!(view.legacy_selection_state().anchor(), None);

    // Toggle on ids[2]
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[2],
    }));
    assert!(view.legacy_selection_state().is_selected(ids[2]));
    assert_eq!(view.legacy_selection_state().anchor(), None);

    // Toggle off ids[0] — ids[2] remains selected, anchor still None.
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[0],
    }));
    assert!(!view.legacy_selection_state().is_selected(ids[0]));
    assert!(view.legacy_selection_state().is_selected(ids[2]));
    assert_eq!(view.legacy_selection_state().anchor(), None);
}

// =============================================================================
// Test 20: Selection during edit cancels edit first
// =============================================================================

#[test]
fn test_selection_during_edit_cancels_edit() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..3)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Focus and begin edit
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[0],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: ids[0],
        column_id: col_id,
    }));
    assert!(view.is_editing());

    // Selection intent — should cancel edit, then select
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[2],
    }));

    assert!(!view.is_editing());
    assert!(view.legacy_selection_state().is_selected(ids[2]));
}

// =============================================================================
// Test 21: ExtendToRow replaces selection with new range
// =============================================================================

#[test]
fn test_extend_replaces_selection() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Anchor at 1, extend to 3 → rows 1,2,3
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[1],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[3],
    }));
    assert_eq!(view.legacy_selection_state().count(), 3);

    // Extend to 0 → rows 0,1 (replaces previous range)
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[0],
    }));
    assert_eq!(view.legacy_selection_state().count(), 2);
    assert!(view.legacy_selection_state().is_selected(ids[0]));
    assert!(view.legacy_selection_state().is_selected(ids[1]));
    assert!(!view.legacy_selection_state().is_selected(ids[2]));
    assert!(!view.legacy_selection_state().is_selected(ids[3]));
}

// =============================================================================
// Test 22: Sort intent does not clear selection
// =============================================================================

#[test]
fn test_sort_does_not_clear_selection() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..3)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[1],
    }));

    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    }));

    assert!(view.legacy_selection_state().is_selected(ids[1]));
}

// =============================================================================
// Test 24: No RowModel mutation from selection (INV-178-8)
// =============================================================================

#[test]
fn test_no_row_model_mutation() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    let ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    let row_count_before = view.body().row_model().len();

    // Perform various selection operations
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: ids[0],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: ids[4],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: ids[2],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ClearSelection));

    assert_eq!(view.body().row_model().len(), row_count_before);
    // All row IDs intact
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(view.body().row_model().get(i), Some(id));
    }
}

// =============================================================================
// Test 26: Edit begin with no selection is no-op for selection version
// =============================================================================

#[test]
fn test_edit_begin_no_selection_noop() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let row_id = RowId::new();
    view.add_row(row_id);

    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_id,
    }));

    let sel_version = view.legacy_selection_state().version();

    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id,
        column_id: col_id,
    }));

    // No selection existed, so version should not change
    assert_eq!(view.legacy_selection_state().version(), sel_version);
}
