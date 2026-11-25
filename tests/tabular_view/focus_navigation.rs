// =============================================================================
// TabularSubstrate Focus + Navigation Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-175: Cell focus and keyboard navigation as intents.
//
// Key invariants under test:
// - Focus set via FocusIntent only (no direct mutation in hit-test or paint).
// - Navigation resolves using effective row order (sort aware).
// - Sort intents never mutate FocusState.
// - Navigation clamps to bounds, never panics.
// - Deterministic default when focus is None (top-left visible cell).
//
// All tests use only public APIs.
// =============================================================================

use halcyon::widgets::tabular_view::{
    Column, ColumnId, EditIntent, FocusIntent, FocusState, NavigationDirection, RowComparator,
    RowId, SortDirection, SortIntent, TabularIntent, TabularSubstrate,
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
// Test 1: FocusState starts unfocused
// =============================================================================

#[test]
fn test_focus_state_initial() {
    let state = FocusState::new();
    assert!(!state.is_focused());
    assert!(state.focused().is_none());
    assert!(state.anchor().is_none());
    assert_eq!(state.version(), 0);
}

// =============================================================================
// Test 2: Focus set via RequestFocus intent
// =============================================================================

#[test]
fn test_focus_set_via_intent() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let row_id = RowId::new();
    view.add_row(row_id);

    assert!(!view.focus_state().is_focused());

    // Apply focus intent
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_id,
    }));

    assert!(view.focus_state().is_focused());
    let focused = view.focus_state().focused().unwrap();
    assert_eq!(focused.row_id, row_id);
    assert_eq!(focused.column_id, col_id);
    assert_eq!(view.focus_state().version(), 1);
}

// =============================================================================
// Test 3: Focus cleared via RequestClearFocus
// =============================================================================

#[test]
fn test_focus_clear_via_intent() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let row_id = RowId::new();
    view.add_row(row_id);

    // Set then clear focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_id,
    }));
    assert!(view.focus_state().is_focused());

    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestClearFocus));
    assert!(!view.focus_state().is_focused());
    assert_eq!(view.focus_state().version(), 2);
}

// =============================================================================
// Test 4: Clear focus when already unfocused is no-op
// =============================================================================

#[test]
fn test_clear_focus_when_unfocused_is_noop() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_row(RowId::new());

    let version_before = view.focus_state().version();
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestClearFocus));
    assert_eq!(view.focus_state().version(), version_before);
}

// =============================================================================
// Test 5: Arrow Down navigates to next row
// =============================================================================

#[test]
fn test_navigate_down() {
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

    // Navigate down
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));

    let focused = view.focus_state().focused().unwrap();
    assert_eq!(focused.row_id, ids[1], "Down from row 0 should go to row 1");
    assert_eq!(focused.column_id, col_id);
}

// =============================================================================
// Test 6: Arrow Up navigates to previous row
// =============================================================================

#[test]
fn test_navigate_up() {
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

    // Focus on row 2
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[2],
        column_id: col_id,
    }));

    // Navigate up
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Up,
    }));

    let focused = view.focus_state().focused().unwrap();
    assert_eq!(focused.row_id, ids[1], "Up from row 2 should go to row 1");
}

// =============================================================================
// Test 7: Arrow Left/Right navigates across columns
// =============================================================================

#[test]
fn test_navigate_left_right() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));
    view.add_column(Column::new(200.0));
    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    let row_id = RowId::new();
    view.add_row(row_id);

    // Focus on column 0
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_ids[0],
    }));

    // Navigate right
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Right,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[1]);

    // Navigate right again
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Right,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[2]);

    // Navigate left
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Left,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[1]);
}

// =============================================================================
// Test 8: Navigation clamps at bounds
// =============================================================================

#[test]
fn test_navigation_clamps_at_bounds() {
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

    // Focus on first row, navigate up → stays at first
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[0],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Up,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[0]);

    // Focus on last row, navigate down → stays at last
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[2],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[2]);

    // Left at first column → stays
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Left,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_id);
}

// =============================================================================
// Test 9: Home/End navigates to first/last column
// =============================================================================

#[test]
fn test_navigate_home_end() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));
    view.add_column(Column::new(200.0));
    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    let row_id = RowId::new();
    view.add_row(row_id);

    // Focus on middle column
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_ids[1],
    }));

    // Home → first column
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Home,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[0]);

    // End → last column
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::End,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[2]);
}

// =============================================================================
// Test 10: No focus → navigation defaults to top-left
// =============================================================================

#[test]
fn test_navigate_no_focus_defaults_top_left() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));
    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    let ids: Vec<RowId> = (0..3)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    assert!(!view.focus_state().is_focused());

    // Navigate down from no focus → focuses top-left then moves down
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));

    let focused = view.focus_state().focused().unwrap();
    // From (0,0), Down → (1,0)
    assert_eq!(focused.row_id, ids[1]);
    assert_eq!(focused.column_id, col_ids[0]);
}

// =============================================================================
// Test 11: Sort active — navigation uses sorted order
// =============================================================================

#[test]
fn test_navigate_uses_sorted_order() {
    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..4)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Apply reverse sort (row indices: 3, 2, 1, 0)
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    }));

    // Focus on what would be "first" in sorted order (index 3 = ids[3])
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[3],
        column_id: col_id,
    }));

    // Navigate down in sorted order: 3→2
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        ids[2],
        "Down in reversed sort: ids[3] → ids[2]"
    );

    // Navigate down again: 2→1
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        ids[1],
        "Down in reversed sort: ids[2] → ids[1]"
    );
}

// =============================================================================
// Test 12: Sort intent does not mutate FocusState
// =============================================================================

#[test]
fn test_sort_intent_does_not_mutate_focus() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let row_id = RowId::new();
    view.add_row(row_id);

    // Set focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_id,
    }));
    let focus_version = view.focus_state().version();
    let focused_cell = view.focus_state().focused();

    // Apply sort intent
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    }));

    // Focus unchanged
    assert_eq!(view.focus_state().version(), focus_version);
    assert_eq!(view.focus_state().focused(), focused_cell);
}

// =============================================================================
// Test 13: Hit-test does not mutate focus
// =============================================================================

#[test]
fn test_hit_test_does_not_mutate_focus() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let row_id = RowId::new();
    view.add_row(row_id);

    // Set focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_id,
    }));
    let focus_version = view.focus_state().version();

    // Perform hit-tests (body and header)
    let _ = view.body_hit_test(50.0, 30.0);
    let _ = view.header().hit_test(50.0, 12.0);

    // Focus unchanged
    assert_eq!(view.focus_state().version(), focus_version);
    assert!(view.focus_state().is_focused());
}

// =============================================================================
// Test 14: Navigation with empty row model
// =============================================================================

#[test]
fn test_navigate_empty_rows_no_panic() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));

    // No rows — navigation should not panic, focus stays None
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert!(!view.focus_state().is_focused());
}

// =============================================================================
// Test 15: Navigation with no columns
// =============================================================================

#[test]
fn test_navigate_no_columns_no_panic() {
    let mut view = TabularSubstrate::new();
    view.add_row(RowId::new());

    // No columns — navigation should not panic
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Right,
    }));
    assert!(!view.focus_state().is_focused());
}

// =============================================================================
// Test 16: PageDown navigates by page size
// =============================================================================

#[test]
fn test_navigate_page_down() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..20)
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

    // PageDown with no viewport set → default page size (10)
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::PageDown,
    }));

    let focused = view.focus_state().focused().unwrap();
    assert_eq!(
        focused.row_id, ids[10],
        "PageDown from 0 with page=10 → row 10"
    );
}

// =============================================================================
// Test 17: PageUp clamps at top
// =============================================================================

#[test]
fn test_navigate_page_up_clamps() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..20)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Focus on row 3, PageUp with default page=10 → clamp to 0
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[3],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::PageUp,
    }));

    let focused = view.focus_state().focused().unwrap();
    assert_eq!(
        focused.row_id, ids[0],
        "PageUp from 3 with page=10 → clamp to 0"
    );
}

// =============================================================================
// Test 18: Version increments only on actual focus change
// =============================================================================

#[test]
fn test_version_increments_on_change() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let row_id = RowId::new();
    view.add_row(row_id);

    assert_eq!(view.focus_state().version(), 0);

    // Focus → version 1
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_id,
    }));
    assert_eq!(view.focus_state().version(), 1);

    // Navigate down on single row → clamps, still changes focus (same cell but version bumps)
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    // resolve_navigation returns Some(same cell) → version still increments
    assert_eq!(view.focus_state().version(), 2);
}

// =============================================================================
// Test 19: Hidden column skipped during navigation (INV-175-4)
// =============================================================================

#[test]
fn test_navigate_skips_hidden_column() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    let row_id = RowId::new();
    view.add_row(row_id);

    // Hide middle column
    view.set_column_visible(col_ids[1], false);

    // Focus on first column
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_ids[0],
    }));

    // Navigate right — should skip hidden col_ids[1], land on col_ids[2]
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Right,
    }));

    assert_eq!(
        view.focus_state().focused().unwrap().column_id,
        col_ids[2],
        "Right from col 0 with col 1 hidden → col 2"
    );

    // Navigate left — should skip hidden col_ids[1], land on col_ids[0]
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Left,
    }));

    assert_eq!(
        view.focus_state().focused().unwrap().column_id,
        col_ids[0],
        "Left from col 2 with col 1 hidden → col 0"
    );
}

// =============================================================================
// Test 20: Home/End respect hidden columns (INV-175-4)
// =============================================================================

#[test]
fn test_home_end_respect_hidden_columns() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    let row_id = RowId::new();
    view.add_row(row_id);

    // Hide first and last columns
    view.set_column_visible(col_ids[0], false);
    view.set_column_visible(col_ids[3], false);

    // Focus on col 1
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_ids[1],
    }));

    // Home → first visible column (col_ids[1])
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Home,
    }));
    assert_eq!(
        view.focus_state().focused().unwrap().column_id,
        col_ids[1],
        "Home → first visible column"
    );

    // End → last visible column (col_ids[2])
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::End,
    }));
    assert_eq!(
        view.focus_state().focused().unwrap().column_id,
        col_ids[2],
        "End → last visible column"
    );
}

// =============================================================================
// Test 21: Virtualization-safe — focus works without viewport realization (INV-175-5)
// =============================================================================

#[test]
fn test_focus_independent_of_realization() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let ids: Vec<RowId> = (0..100)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // No viewport set, no realization. Navigation should still work.
    // Focus on row 0
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[0],
        column_id: col_id,
    }));
    assert!(view.focus_state().is_focused());

    // Navigate down 50 times — all unrealized rows, focus should still work
    for _ in 0..50 {
        view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
            from: None,
            direction: NavigationDirection::Down,
        }));
    }

    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        ids[50],
        "Focus should reach row 50 regardless of realization"
    );
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_id);
}

// =============================================================================
// Test 22: Focus is identity-sticky, not index-based (INV-175-5)
// =============================================================================

#[test]
fn test_focus_sticky_across_sort() {
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

    // Focus on ids[2] — natural index 2
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[2],
        column_id: col_id,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[2]);

    // Apply reverse sort — ids[2] moves to a different position in order
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    }));

    // Focus still on ids[2] (identity-sticky, NOT moved to index 2's new occupant)
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        ids[2],
        "Focus must remain on same RowId after sort"
    );

    // Navigate down from ids[2] in reversed order: 4,3,2,1,0 → ids[2] is at position 2
    // Down from position 2 → position 3 → ids[1]
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        ids[1],
        "Down from ids[2] in reverse order → ids[1]"
    );
}

// =============================================================================
// Test 23: Navigation during edit cancels edit then navigates (INV-175-8)
// =============================================================================

#[test]
fn test_navigate_during_edit_cancels_then_navigates() {
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

    // Focus on row 2
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[2],
        column_id: col_id,
    }));

    // Begin edit on row 2
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: ids[2],
        column_id: col_id,
    }));
    assert!(view.is_editing());

    // Navigate down — should cancel edit, then navigate
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));

    // Edit must be cancelled
    assert!(!view.is_editing());
    // Focus should have moved (cancel restores pre-edit focus, then navigate proceeds)
    assert!(view.focus_state().is_focused());
}

// =============================================================================
// Test 24: RequestFocus during edit cancels edit (INV-175-8)
// =============================================================================

#[test]
fn test_request_focus_during_edit_cancels_edit() {
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

    // Focus and edit row 0
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[0],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Edit(EditIntent::RequestBeginEdit {
        row_id: ids[0],
        column_id: col_id,
    }));
    assert!(view.is_editing());

    // RequestFocus on different cell — should cancel edit, then focus
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[2],
        column_id: col_id,
    }));

    assert!(!view.is_editing());
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[2]);
}

// =============================================================================
// Test 25: All 8 directions with 3 columns and 12 rows
// =============================================================================

#[test]
fn test_all_directions_multi_column_multi_row() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    let ids: Vec<RowId> = (0..12)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Start at row 5, col 1 (middle)
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[5],
        column_id: col_ids[1],
    }));

    // Up → row 4
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Up,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[4]);
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[1]);

    // Down → row 5
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[5]);

    // Left → col 0
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Left,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[0]);

    // Right → col 1
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Right,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[1]);

    // Home → col 0
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Home,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[0]);

    // End → col 2
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::End,
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[2]);

    // PageDown (default page=10) from row 5 → row 11 (clamped to last)
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::PageDown,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[11]);

    // PageUp from row 11 → row 1 (11-10=1)
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::PageUp,
    }));
    assert_eq!(view.focus_state().focused().unwrap().row_id, ids[1]);
}

// =============================================================================
// Test 26: Version monotonic — never decreases, never skips
// =============================================================================

#[test]
fn test_version_strictly_monotonic() {
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

    let mut prev_version = view.focus_state().version();

    // Each accepted focus change increments by exactly 1
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: ids[0],
        column_id: col_id,
    }));
    assert_eq!(view.focus_state().version(), prev_version + 1);
    prev_version = view.focus_state().version();

    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Down,
    }));
    assert_eq!(view.focus_state().version(), prev_version + 1);
    prev_version = view.focus_state().version();

    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestClearFocus));
    assert_eq!(view.focus_state().version(), prev_version + 1);
    prev_version = view.focus_state().version();

    // Clear when already None → no increment
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestClearFocus));
    assert_eq!(view.focus_state().version(), prev_version);
}

// =============================================================================
// Test 27: Navigation on hidden-column-focused cell falls back correctly
// =============================================================================

#[test]
fn test_focus_sticky_across_column_hide() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(100.0));
    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    let row_id = RowId::new();
    view.add_row(row_id);

    // Focus on middle column
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id,
        column_id: col_ids[1],
    }));
    assert_eq!(view.focus_state().focused().unwrap().column_id, col_ids[1]);

    // Hide the focused column
    view.set_column_visible(col_ids[1], false);

    // Focus still on col_ids[1] (identity-sticky)
    assert_eq!(
        view.focus_state().focused().unwrap().column_id,
        col_ids[1],
        "Focus stays on hidden column (sticky)"
    );

    // Navigate right — col_ids[1] not in visible set, fallback to col pos 0, then Right → pos 1
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: None,
        direction: NavigationDirection::Right,
    }));

    let focused = view.focus_state().focused().unwrap();
    // Should be on a visible column, not col_ids[1]
    assert_ne!(focused.column_id, col_ids[1]);
}
