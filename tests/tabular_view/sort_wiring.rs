// =============================================================================
// TabularSubstrate Sort Wiring Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-171: Sort state management and intent handling.
// All tests use only public APIs via TabularSubstrate.
// =============================================================================

use halcyon::widgets::tabular_view::{
    Column, ColumnId, NullComparator, RowComparator, SortDirection, SortIntent, SortState,
    TabularIntent, TabularSubstrate,
};
use std::cmp::Ordering;

// =============================================================================
// SortState Unit Tests (via public API)
// =============================================================================

#[test]
fn test_sort_state_new_is_unsorted() {
    let state = SortState::new();
    assert!(state.active_column().is_none());
    assert!(state.direction().is_none());
    assert!(!state.is_active());
    assert!(!state.is_dirty());
}

#[test]
fn test_sort_state_apply_sort_intent() {
    let mut state = SortState::new();
    let col_id = ColumnId::new();

    // First sort: sets column and default direction
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    });

    assert_eq!(state.active_column(), Some(col_id));
    assert_eq!(state.direction(), Some(SortDirection::Ascending));
    assert!(state.is_dirty());
    assert_eq!(state.version(), 1);
}

#[test]
fn test_sort_state_toggle_direction() {
    let mut state = SortState::new();
    let col_id = ColumnId::new();

    // First sort: ascending
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    });
    assert_eq!(state.direction(), Some(SortDirection::Ascending));

    // Second sort same column: toggle to descending
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    });
    assert_eq!(state.direction(), Some(SortDirection::Descending));
    assert_eq!(state.version(), 2);

    // Third sort same column: toggle back to ascending
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    });
    assert_eq!(state.direction(), Some(SortDirection::Ascending));
}

#[test]
fn test_sort_state_switch_column() {
    let mut state = SortState::new();
    let col1 = ColumnId::new();
    let col2 = ColumnId::new();

    // Sort by col1 descending
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col1,
        direction: Some(SortDirection::Descending),
    });
    assert_eq!(state.active_column(), Some(col1));
    assert_eq!(state.direction(), Some(SortDirection::Descending));

    // Switch to col2: resets to ascending
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col2,
        direction: None,
    });
    assert_eq!(state.active_column(), Some(col2));
    assert_eq!(state.direction(), Some(SortDirection::Ascending));
}

#[test]
fn test_sort_state_clear() {
    let mut state = SortState::new();
    let col_id = ColumnId::new();

    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    });
    assert!(state.is_active());

    state.apply_intent(&SortIntent::RequestClearSort { column_id: col_id });
    assert!(!state.is_active());
    assert!(state.active_column().is_none());
}

#[test]
fn test_null_comparator_preserves_order() {
    let order = halcyon::widgets::tabular_view::sort::compute_sorted_order(
        5,
        &SortState::new(),
        &NullComparator,
    );
    assert_eq!(order, vec![0, 1, 2, 3, 4]);
}

#[test]
fn test_sorted_order_with_comparator() {
    struct ReverseComparator;
    impl RowComparator for ReverseComparator {
        fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
            // Reverse natural order
            Some(b.cmp(&a))
        }
    }

    let mut state = SortState::new();
    let col_id = ColumnId::new();
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    });

    let order =
        halcyon::widgets::tabular_view::sort::compute_sorted_order(5, &state, &ReverseComparator);
    assert_eq!(order, vec![4, 3, 2, 1, 0]);
}

#[test]
fn test_sorted_order_descending() {
    struct NaturalComparator;
    impl RowComparator for NaturalComparator {
        fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
            Some(a.cmp(&b))
        }
    }

    let mut state = SortState::new();
    let col_id = ColumnId::new();
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Descending),
    });

    let order =
        halcyon::widgets::tabular_view::sort::compute_sorted_order(5, &state, &NaturalComparator);
    assert_eq!(order, vec![4, 3, 2, 1, 0]);
}

#[test]
fn test_sorted_order_determinism() {
    struct DeterministicComparator;
    impl RowComparator for DeterministicComparator {
        fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
            // Sort by index mod 3
            Some((a % 3).cmp(&(b % 3)))
        }
    }

    let mut state = SortState::new();
    let col_id = ColumnId::new();
    state.apply_intent(&SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    });

    let order1 = halcyon::widgets::tabular_view::sort::compute_sorted_order(
        6,
        &state,
        &DeterministicComparator,
    );
    let order2 = halcyon::widgets::tabular_view::sort::compute_sorted_order(
        6,
        &state,
        &DeterministicComparator,
    );

    assert_eq!(order1, order2);
}

// =============================================================================
// TabularSubstrate Sort Integration Tests
// =============================================================================

#[test]
fn test_tabular_view_sort_state_initial() {
    let view = TabularSubstrate::new();
    assert!(!view.sort_state().is_active());
    assert!(view.sort_state().active_column().is_none());
}

#[test]
fn test_tabular_view_sort_toggle_direction_same_column() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    // First sort: ascending
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));
    assert_eq!(view.sort_state().active_column(), Some(col_id));
    assert_eq!(
        view.sort_state().direction(),
        Some(SortDirection::Ascending)
    );

    // Second sort: toggle to descending
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));
    assert_eq!(
        view.sort_state().direction(),
        Some(SortDirection::Descending)
    );
}

#[test]
fn test_tabular_view_sort_switch_column_resets_direction() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));
    let col1 = view.column_model().columns()[0].id;
    let col2 = view.column_model().columns()[1].id;

    // Sort by col1 descending
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col1,
        direction: Some(SortDirection::Descending),
    }));
    assert_eq!(
        view.sort_state().direction(),
        Some(SortDirection::Descending)
    );

    // Switch to col2: resets to ascending
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col2,
        direction: None,
    }));
    assert_eq!(view.sort_state().active_column(), Some(col2));
    assert_eq!(
        view.sort_state().direction(),
        Some(SortDirection::Ascending)
    );
}

#[test]
fn test_tabular_view_sort_clear() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    // Activate sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));
    assert!(view.sort_state().is_active());

    // Clear sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestClearSort {
        column_id: col_id,
    }));
    assert!(!view.sort_state().is_active());
}
