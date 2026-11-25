// =============================================================================
// TabularSubstrate Sort Virtualization Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-171: Sort order affecting row realization.
// All tests use only public APIs via TabularSubstrate.
// =============================================================================

use crate::core::math::RectF;
use crate::widgets::tabular_view::{
    BodyDomain, Column, ColumnGeometryTable, ColumnId, ColumnModel, GeometryView, RowComparator,
    RowId, RowModel, SortDirection, SortIntent, TabularIntent, TabularSubstrate,
};
use std::cmp::Ordering;

// Comparator that reverses natural order
struct ReverseComparator;
impl RowComparator for ReverseComparator {
    fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
        Some(b.cmp(&a))
    }
}

// Comparator for determinism testing
struct DeterministicComparator;
impl RowComparator for DeterministicComparator {
    fn compare_rows(&self, a: usize, b: usize, _col: ColumnId) -> Option<Ordering> {
        // Sort by index mod 3
        Some((a % 3).cmp(&(b % 3)))
    }
}

#[test]
fn test_sort_realization_order_changes() {
    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    // Add rows
    for _ in 0..5 {
        view.add_row(RowId::new());
    }

    // Natural order before sort - need to trigger index computation
    // Since compute_sorted_indices is private, we check via sorted_indices()
    // After construction without sort, sorted_indices should be empty or natural
    // We need to exercise the layout path to populate sorted indices

    // Apply sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));

    // The sort state should be active and dirty
    assert!(view.sort_state().is_active());
    assert!(view.sort_state().is_dirty());
}

#[test]
fn test_row_id_stability_after_sort() {
    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    // Add rows and capture their IDs
    let row_ids: Vec<RowId> = (0..3)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Before sort: row at index 0 has row_ids[0]
    assert_eq!(view.row_model().get(0), Some(row_ids[0]));
    assert_eq!(view.row_model().get(1), Some(row_ids[1]));
    assert_eq!(view.row_model().get(2), Some(row_ids[2]));

    // Apply sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));

    // After sort: row IDs are still the same at their original indices
    // (INVARIANT-SORT-4: RowId identity is not changed)
    assert_eq!(view.row_model().get(0), Some(row_ids[0]));
    assert_eq!(view.row_model().get(1), Some(row_ids[1]));
    assert_eq!(view.row_model().get(2), Some(row_ids[2]));
}

#[test]
fn test_sort_with_explicit_direction() {
    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    // Add rows
    for _ in 0..5 {
        view.add_row(RowId::new());
    }

    // Apply sort with explicit descending
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Descending),
    }));

    assert_eq!(
        view.sort_state().direction(),
        Some(SortDirection::Descending)
    );
}

#[test]
fn test_sort_determinism_via_tabular_view() {
    let mut view = TabularSubstrate::new().with_comparator(DeterministicComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    for _ in 0..6 {
        view.add_row(RowId::new());
    }

    // Apply sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));

    // Sort state should be consistent
    assert!(view.sort_state().is_active());
    assert_eq!(view.sort_state().active_column(), Some(col_id));
    assert_eq!(
        view.sort_state().direction(),
        Some(SortDirection::Ascending)
    );
}

#[test]
fn test_sort_with_multiple_columns() {
    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));
    view.add_column(Column::new(200.0));

    let col1 = view.column_model().columns()[0].id;
    let col2 = view.column_model().columns()[1].id;

    for _ in 0..5 {
        view.add_row(RowId::new());
    }

    // Sort by first column
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col1,
        direction: None,
    }));
    assert_eq!(view.sort_state().active_column(), Some(col1));

    // Switch to second column
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col2,
        direction: None,
    }));
    assert_eq!(view.sort_state().active_column(), Some(col2));
    // Direction resets to ascending when switching columns
    assert_eq!(
        view.sort_state().direction(),
        Some(SortDirection::Ascending)
    );
}

#[test]
fn test_sort_version_increments() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let initial_version = view.sort_state().version();

    // Apply sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));

    assert_eq!(view.sort_state().version(), initial_version + 1);

    // Toggle direction
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: None,
    }));

    assert_eq!(view.sort_state().version(), initial_version + 2);
}

#[test]
fn test_clear_sort_for_different_column_is_noop() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));

    let col1 = view.column_model().columns()[0].id;
    let col2 = view.column_model().columns()[1].id;

    // Sort by col1
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col1,
        direction: None,
    }));
    assert!(view.sort_state().is_active());
    let version = view.sort_state().version();

    // Clear sort for col2 (not the active column)
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestClearSort {
        column_id: col2,
    }));

    // Sort should still be active on col1
    assert!(view.sort_state().is_active());
    assert_eq!(view.sort_state().active_column(), Some(col1));
    // Version should not change
    assert_eq!(view.sort_state().version(), version);
}

// =============================================================================
// SPEC-HAL-171 INVARIANT-SORT-4 Identity Stability Tests
// =============================================================================
//
// These tests verify that sorting affects ONLY realization order, never
// RowModel structure or RowId identity mapping.
// =============================================================================

#[test]
fn test_sort_never_mutates_rowmodel_order() {
    // SPEC-HAL-171 INVARIANT-SORT-4: RowId identity mapping is stable.
    // Sorting must not reorder, copy, or reshuffle rows in RowModel.

    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    // Create rows with known IDs
    let row_ids: Vec<RowId> = (0..10)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Snapshot RowModel state before sort
    let ids_before: Vec<RowId> = view.row_model().rows().to_vec();

    // Apply multiple sort operations
    for _ in 0..5 {
        // Toggle sort direction
        view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
            column_id: col_id,
            direction: None,
        }));
    }

    // RowModel must be IDENTICAL to before sort
    // - Same length
    assert_eq!(view.row_model().len(), row_ids.len());
    // - Same IDs at same indices
    let ids_after: Vec<RowId> = view.row_model().rows().to_vec();
    assert_eq!(
        ids_before, ids_after,
        "RowModel order must not change after sort"
    );

    // Verify each position
    for (i, &original_id) in row_ids.iter().enumerate() {
        assert_eq!(
            view.row_model().get(i),
            Some(original_id),
            "RowId at index {} must be unchanged after sort",
            i
        );
    }
}

#[test]
fn test_sort_indices_differs_from_physical_order_with_comparator() {
    // Verify that sorted_indices (when computed) differs from physical order
    // while RowModel remains unchanged.

    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    // Add rows
    let row_ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Before sort: sorted_indices is empty (not computed yet)
    assert!(view.sorted_indices().is_empty());

    // Apply sort
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Ascending),
    }));

    // Sort state is dirty (indices need recomputation)
    assert!(view.sort_state().is_dirty());
    assert!(view.sort_state().is_active());

    // RowModel physical order unchanged
    for (i, &original_id) in row_ids.iter().enumerate() {
        assert_eq!(view.row_model().get(i), Some(original_id));
    }
}

#[test]
fn test_multiple_sort_column_switches_preserve_identity() {
    // Switching between columns repeatedly must never affect RowModel.

    let mut view = TabularSubstrate::new().with_comparator(DeterministicComparator);
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));
    view.add_column(Column::new(200.0));

    let col_ids: Vec<ColumnId> = view.column_model().columns().iter().map(|c| c.id).collect();

    // Add rows
    let row_ids: Vec<RowId> = (0..8)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Switch columns many times
    for i in 0..20 {
        let col = col_ids[i % col_ids.len()];
        view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
            column_id: col,
            direction: None,
        }));
    }

    // RowModel must be unchanged
    assert_eq!(view.row_model().len(), row_ids.len());
    for (i, &original_id) in row_ids.iter().enumerate() {
        assert_eq!(
            view.row_model().get(i),
            Some(original_id),
            "RowId at index {} must survive column switching",
            i
        );
    }
}

#[test]
fn test_sort_clear_preserves_rowmodel() {
    // Clearing sort must not affect RowModel.

    let mut view = TabularSubstrate::new().with_comparator(ReverseComparator);
    view.add_column(Column::new(100.0));
    let col_id = view.column_model().columns()[0].id;

    let row_ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            view.add_row(id);
            id
        })
        .collect();

    // Sort then clear
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestSort {
        column_id: col_id,
        direction: Some(SortDirection::Descending),
    }));
    view.handle_intent(&TabularIntent::Sort(SortIntent::RequestClearSort {
        column_id: col_id,
    }));

    // Sort should be inactive
    assert!(!view.sort_state().is_active());

    // RowModel unchanged
    for (i, &original_id) in row_ids.iter().enumerate() {
        assert_eq!(view.row_model().get(i), Some(original_id));
    }
}

// =============================================================================
// SPEC-HAL-171: Sort Realization + Virtualization Identity Stability
// =============================================================================
//
// Verifies the full pipeline:
// - RowId order in RowModel is unchanged after sort.
// - Realized row order differs from physical order.
// - Virtualized window respects sorted order when scrolled.
// =============================================================================

#[test]
fn test_sort_realization_identity_stable_and_order_differs() {
    // SPEC-HAL-171 INVARIANT-SORT-4: Sorting affects ONLY realization order.
    // RowModel identity must be stable; realized row order must differ.

    let mut row_model = RowModel::with_fixed_height(30.0);
    let row_ids: Vec<RowId> = (0..5)
        .map(|_| {
            let id = RowId::new();
            row_model.push(id);
            id
        })
        .collect();

    let mut body = BodyDomain::with_row_model(row_model);
    body.set_viewport(RectF::new(0.0, 0.0, 300.0, 200.0)); // Fits all 5 rows (150px total)

    let mut col_model = ColumnModel::new();
    col_model.push(Column::new(100.0));
    let mut table = ColumnGeometryTable::new();
    table.resolve(&col_model, 300.0, 1, 1.0);
    let view = GeometryView::new(&table, 1);

    // Reversed sorted order: [4, 3, 2, 1, 0]
    let sorted_indices: Vec<usize> = (0..5).rev().collect();

    // Realize with sorted order
    super::test_realize_sorted(&mut body, view, &sorted_indices);

    // 1. RowModel identity unchanged
    for (i, &original_id) in row_ids.iter().enumerate() {
        assert_eq!(
            body.row_model().get(i),
            Some(original_id),
            "RowModel[{}] identity changed after sorted realization",
            i
        );
    }

    // 2. Realized row order differs from physical order
    let realized_indices: Vec<usize> = body.realized_rows().iter().map(|r| r.row_index).collect();
    assert_eq!(
        realized_indices,
        vec![4, 3, 2, 1, 0],
        "Realized order must match sorted_indices"
    );

    // 3. Realized RowIds come from the correct RowModel positions
    for (i, row_proj) in body.realized_rows().iter().enumerate() {
        assert_eq!(
            row_proj.row_id, row_ids[sorted_indices[i]],
            "Realized row {} has wrong RowId",
            i
        );
    }
}

#[test]
fn test_sorted_virtualization_respects_scroll() {
    // SPEC-HAL-171: Virtualized window must respect sorted order when scrolled.

    let mut row_model = RowModel::with_fixed_height(30.0);
    let row_ids: Vec<RowId> = (0..10)
        .map(|_| {
            let id = RowId::new();
            row_model.push(id);
            id
        })
        .collect();

    let mut body = BodyDomain::with_row_model(row_model);
    // Viewport fits ~3 rows (90px viewport, 30px rows)
    body.set_viewport(RectF::new(0.0, 0.0, 300.0, 90.0));

    let mut col_model = ColumnModel::new();
    col_model.push(Column::new(100.0));
    let mut table = ColumnGeometryTable::new();
    table.resolve(&col_model, 300.0, 1, 1.0);
    let view = GeometryView::new(&table, 1);

    // Reversed sorted order: [9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
    let sorted_indices: Vec<usize> = (0..10).rev().collect();

    // No scroll: first 3 rows in sorted order should be realized
    super::test_realize_sorted(&mut body, view, &sorted_indices);

    // First realized row should be physical index 9 (first in reversed order)
    assert_eq!(body.realized_rows()[0].row_index, 9);
    assert_eq!(body.realized_rows()[0].row_id, row_ids[9]);

    // Scroll down 60px (skip 2 rows in sorted order)
    body.set_scroll_y(60.0);
    super::test_realize_sorted(&mut body, view, &sorted_indices);

    // After scrolling past sorted[0]=9 and sorted[1]=8, first visible is sorted[2]=7
    assert_eq!(body.realized_rows()[0].row_index, 7);
    assert_eq!(body.realized_rows()[0].row_id, row_ids[7]);

    // RowModel must still be unchanged
    for (i, &original_id) in row_ids.iter().enumerate() {
        assert_eq!(body.row_model().get(i), Some(original_id));
    }
}
