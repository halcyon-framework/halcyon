// =============================================================================
// TabularSubstrate Row Identity Stability Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-167: Row and cell identity stability.
// All tests use only public APIs.
// =============================================================================

use halcyon::widgets::tabular_view::{CellId, ColumnId, RowId, RowModel};

#[test]
fn test_row_id_uniqueness() {
    let id1 = RowId::new();
    let id2 = RowId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_row_id_from_raw() {
    let id = RowId::from_raw(42);
    assert_eq!(id.raw(), 42);
}

#[test]
fn test_cell_id_equality() {
    let row = RowId::new();
    let col = ColumnId::new();
    let cell1 = CellId::new(row, col);
    let cell2 = CellId::new(row, col);
    assert_eq!(cell1, cell2);
}

#[test]
fn test_cell_id_inequality() {
    let row1 = RowId::new();
    let row2 = RowId::new();
    let col = ColumnId::new();
    let cell1 = CellId::new(row1, col);
    let cell2 = CellId::new(row2, col);
    assert_ne!(cell1, cell2);
}

#[test]
fn test_fixed_height_model() {
    let mut model = RowModel::with_fixed_height(30.0);
    model.push(RowId::new());
    model.push(RowId::new());
    model.push(RowId::new());

    assert_eq!(model.len(), 3);
    assert_eq!(model.total_height(), 90.0);
    assert_eq!(model.y_offset_at(2), 60.0);
    assert_eq!(model.row_at_y(45.0), Some(1));
}

#[test]
fn test_variable_height_model() {
    let mut model = RowModel::with_variable_heights();
    model.push_with_height(RowId::new(), 20.0);
    model.push_with_height(RowId::new(), 30.0);
    model.push_with_height(RowId::new(), 40.0);

    assert_eq!(model.len(), 3);
    assert_eq!(model.total_height(), 90.0);
    assert_eq!(model.y_offset_at(2), 50.0);
    assert_eq!(model.height_at(1), 30.0);
}

#[test]
fn test_row_model_get_and_index_of() {
    let mut model = RowModel::with_fixed_height(30.0);
    let id1 = RowId::new();
    let id2 = RowId::new();
    let id3 = RowId::new();

    model.push(id1);
    model.push(id2);
    model.push(id3);

    assert_eq!(model.get(0), Some(id1));
    assert_eq!(model.get(1), Some(id2));
    assert_eq!(model.get(2), Some(id3));
    assert_eq!(model.get(3), None);

    assert_eq!(model.index_of(id1), Some(0));
    assert_eq!(model.index_of(id2), Some(1));
    assert_eq!(model.index_of(id3), Some(2));
    assert_eq!(model.index_of(RowId::new()), None);
}

#[test]
fn test_row_model_rows() {
    let mut model = RowModel::with_fixed_height(30.0);
    let id1 = RowId::new();
    let id2 = RowId::new();

    model.push(id1);
    model.push(id2);

    let rows = model.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], id1);
    assert_eq!(rows[1], id2);
}

#[test]
fn test_row_model_remove() {
    let mut model = RowModel::with_fixed_height(30.0);
    let id1 = RowId::new();
    let id2 = RowId::new();
    let id3 = RowId::new();

    model.push(id1);
    model.push(id2);
    model.push(id3);

    let removed = model.remove(1);
    assert_eq!(removed, Some(id2));
    assert_eq!(model.len(), 2);
    assert_eq!(model.get(0), Some(id1));
    assert_eq!(model.get(1), Some(id3));
}

#[test]
fn test_row_model_clear() {
    let mut model = RowModel::with_fixed_height(30.0);
    model.push(RowId::new());
    model.push(RowId::new());

    assert_eq!(model.len(), 2);

    model.clear();
    assert!(model.is_empty());
    assert_eq!(model.len(), 0);
}

#[test]
fn test_row_at_y_boundary_conditions() {
    let mut model = RowModel::with_fixed_height(30.0);
    model.push(RowId::new());
    model.push(RowId::new());
    model.push(RowId::new());

    // Negative y
    assert_eq!(model.row_at_y(-1.0), None);

    // At row boundaries
    assert_eq!(model.row_at_y(0.0), Some(0));
    assert_eq!(model.row_at_y(29.9), Some(0));
    assert_eq!(model.row_at_y(30.0), Some(1));
    assert_eq!(model.row_at_y(59.9), Some(1));
    assert_eq!(model.row_at_y(60.0), Some(2));

    // Past content
    assert_eq!(model.row_at_y(90.0), None);
    assert_eq!(model.row_at_y(100.0), None);
}

#[test]
fn test_variable_height_row_at_y() {
    let mut model = RowModel::with_variable_heights();
    model.push_with_height(RowId::new(), 20.0);
    model.push_with_height(RowId::new(), 30.0);
    model.push_with_height(RowId::new(), 40.0);

    assert_eq!(model.row_at_y(0.0), Some(0));
    assert_eq!(model.row_at_y(19.9), Some(0));
    assert_eq!(model.row_at_y(20.0), Some(1));
    assert_eq!(model.row_at_y(49.9), Some(1));
    assert_eq!(model.row_at_y(50.0), Some(2));
    assert_eq!(model.row_at_y(89.9), Some(2));
    assert_eq!(model.row_at_y(90.0), None);
}

#[test]
fn test_row_model_set_height() {
    let mut model = RowModel::with_variable_heights();
    model.push_with_height(RowId::new(), 20.0);
    model.push_with_height(RowId::new(), 30.0);

    assert_eq!(model.height_at(1), 30.0);

    model.set_height(1, 50.0);
    assert_eq!(model.height_at(1), 50.0);
    assert_eq!(model.total_height(), 70.0);
}

#[test]
fn test_row_model_default() {
    let model = RowModel::default();
    // Default has fixed height of 24.0
    assert!(model.is_empty());
}
