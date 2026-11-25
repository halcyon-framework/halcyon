// =============================================================================
// TabularSubstrate Geometry Resolution Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-168: Horizontal geometry resolution.
// All tests use only public APIs via TabularSubstrate.
// =============================================================================

use crate::widgets::tabular_view::{Column, ColumnGeometryTable, ColumnModel, GeometryView};

#[test]
fn test_basic_resolution() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 450.0, 1, 1.0);

    assert!(table.is_resolved());
    assert_eq!(table.len(1), 3);
    assert_eq!(table.total_width(1), 450.0);

    let columns = table.columns(1);
    assert_eq!(columns[0].x_offset, 0.0);
    assert_eq!(columns[0].width, 100.0);
    assert_eq!(columns[1].x_offset, 100.0);
    assert_eq!(columns[1].width, 150.0);
    assert_eq!(columns[2].x_offset, 250.0);
    assert_eq!(columns[2].width, 200.0);
}

#[test]
fn test_flex_distribution() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0)); // Fixed
    model.push(Column::new(0.0).with_flex(1.0)); // Single flex absorbs remainder

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 400.0, 1, 1.0);

    let columns = table.columns(1);
    assert_eq!(columns[0].width, 100.0);
    // Remaining 300 goes to single flex column
    assert_eq!(columns[1].width, 300.0);
    assert_eq!(table.total_width(1), 400.0);
}

#[test]
#[should_panic(expected = "INVARIANT-6")]
fn test_read_before_resolution_panics() {
    let table = ColumnGeometryTable::new();
    let _ = table.columns(0); // Should panic
}

#[test]
#[should_panic(expected = "INVARIANT-5")]
fn test_double_resolution_panics() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 100.0, 1, 1.0);
    table.resolve(&model, 100.0, 1, 1.0); // Should panic - same frame
}

#[test]
fn test_resolution_different_frames_allowed() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 100.0, 1, 1.0);
    table.invalidate();
    table.resolve(&model, 100.0, 2, 1.0); // Different frame - OK
    assert!(table.is_resolved());
}

#[test]
fn test_geometry_view_access() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 250.0, 1, 1.0);

    let view = GeometryView::new(&table, 1);
    assert_eq!(view.len(), 2);
    assert_eq!(view.total_width(), 250.0);

    // Check header slots
    let header_slots: Vec<_> = view.header_slots().collect();
    assert_eq!(header_slots.len(), 2);
    assert_eq!(header_slots[0].x, 0.0);
    assert_eq!(header_slots[0].width, 100.0);

    // Check body slots
    let body_slots: Vec<_> = view.body_slots().collect();
    assert_eq!(body_slots.len(), 2);
    assert_eq!(body_slots[1].x, 100.0);
    assert_eq!(body_slots[1].width, 150.0);
}

// =============================================================================
// SPEC-HAL-180 Law Enforcement Tests
// =============================================================================

#[test]
#[should_panic(expected = "INV-COL-ELASTIC-1")]
fn test_elastic_cap_violation_panics() {
    // Two flex columns → must panic per INV-COL-ELASTIC-1.
    let mut model = ColumnModel::new();
    model.push(Column::new(0.0).with_flex(1.0));
    model.push(Column::new(0.0).with_flex(2.0));
    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 300.0, 1, 1.0);
}

#[test]
fn test_single_elastic_absorbs_remainder() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(0.0).with_flex(1.0));
    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 400.0, 1, 1.0);
    let cols = table.columns(1);
    assert_eq!(cols[0].width, 100.0);
    assert_eq!(cols[1].width, 300.0);
    assert_eq!(table.total_width(1), 400.0);
}
