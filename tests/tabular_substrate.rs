// =============================================================================
// TabularSubstrate Substrate Invariant Tests (Black-Box)
// =============================================================================
//
// SPEC-HAL-164 substrate verification via PUBLIC API only.
//
// These are INTEGRATION tests that cannot access pub(crate) internals.
// For white-box engine tests, see src/tests/tabular_substrate.rs
//
// Invariants verified:

#![allow(deprecated)]
// - Focus/Selection via intents work through public API
// - State accessors report correct values
// - No geometry, body, or realization helper access
//
// LEGACY READ — selection assertions use selection_state() (deprecated mirror).
// Will migrate to ordered_selection_state accessors when available.
// =============================================================================

use halcyon::widgets::tabular_view::TabularSubstrate;
use halcyon::widgets::tabular_view::column::Column;
use halcyon::widgets::tabular_view::intents::{
    FocusIntent, NavigationDirection, SelectionIntent, TabularIntent,
};
use halcyon::widgets::tabular_view::row::RowId;

const ROW_COUNT: usize = 100;
const COLUMN_WIDTH: f32 = 200.0;

/// Create a TabularSubstrate with 100 rows, single column.
fn create_view() -> (TabularSubstrate, Vec<RowId>) {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(COLUMN_WIDTH));

    let row_ids: Vec<RowId> = (0..ROW_COUNT).map(|_| RowId::new()).collect();
    for &id in &row_ids {
        view.add_row(id);
    }

    (view, row_ids)
}

// =============================================================================
// INV-API-1: Focus via Intent
// =============================================================================

#[test]
fn inv_api_focus_request() {
    let (mut view, row_ids) = create_view();
    let col_id = view.column_model().columns()[0].id;

    // Initially no focus
    assert!(view.focus_state().focused().is_none());

    // Request focus on row 50
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[50],
        column_id: col_id,
    }));

    let focused = view.focus_state().focused().expect("Focus should be set");
    assert_eq!(focused.row_id, row_ids[50]);
    assert_eq!(focused.column_id, col_id);
}

#[test]
fn inv_api_focus_navigation() {
    let (mut view, row_ids) = create_view();
    let col_id = view.column_model().columns()[0].id;

    // Focus row 10
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[10],
        column_id: col_id,
    }));

    // Navigate down 5 times
    for _ in 0..5 {
        view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
            from: view
                .focus_state()
                .focused()
                .map(|c| (c.row_id, c.column_id)),
            direction: NavigationDirection::Down,
        }));
    }

    let focused = view.focus_state().focused().expect("Focus should exist");
    let focus_idx = row_ids.iter().position(|&r| r == focused.row_id).unwrap();
    assert_eq!(
        focus_idx, 15,
        "After 5 Down from row 10, should be at row 15"
    );
}

// =============================================================================
// INV-API-2: Selection via Intent
// =============================================================================

#[test]
fn inv_api_select_row() {
    let (mut view, row_ids) = create_view();

    // Initially no selection
    assert!(!view.legacy_selection_state().has_selection());

    // Select row 25
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[25],
    }));

    assert!(view.legacy_selection_state().is_selected(row_ids[25]));
    assert_eq!(view.legacy_selection_state().count(), 1);
}

#[test]
fn inv_api_range_selection() {
    let (mut view, row_ids) = create_view();

    // Select row 20 (sets anchor)
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[20],
    }));

    // Extend to row 30
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: row_ids[30],
    }));

    // Should have 11 rows selected (20-30 inclusive)
    assert_eq!(view.legacy_selection_state().count(), 11);

    for i in 20..=30 {
        assert!(
            view.legacy_selection_state().is_selected(row_ids[i]),
            "Row {} should be selected",
            i
        );
    }
}

#[test]
fn inv_api_clear_selection() {
    let (mut view, row_ids) = create_view();

    // Select some rows
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[10],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: row_ids[20],
    }));
    assert!(view.legacy_selection_state().has_selection());

    // Clear
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ClearSelection));
    assert!(!view.legacy_selection_state().has_selection());
    assert_eq!(view.legacy_selection_state().count(), 0);
}

// =============================================================================
// INV-API-3: Focus/Selection Independence
// =============================================================================

#[test]
fn inv_api_focus_selection_independence() {
    let (mut view, row_ids) = create_view();
    let col_id = view.column_model().columns()[0].id;

    // Select row 30
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[30],
    }));

    // Focus row 50
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[50],
        column_id: col_id,
    }));

    // Both should be independent
    assert!(
        view.legacy_selection_state().is_selected(row_ids[30]),
        "Selection should remain"
    );
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        row_ids[50],
        "Focus should be at row 50"
    );

    // Navigate focus
    for _ in 0..10 {
        view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
            from: view
                .focus_state()
                .focused()
                .map(|c| (c.row_id, c.column_id)),
            direction: NavigationDirection::Down,
        }));
    }

    // Selection unchanged
    assert!(
        view.legacy_selection_state().is_selected(row_ids[30]),
        "Selection should not change with focus navigation"
    );
    assert_eq!(view.legacy_selection_state().count(), 1);
}
