// =============================================================================
// SPEC-HAL-170: Column Separator Geometry Integration Tests
// =============================================================================
//
// Tests for column separator derivation from resolved geometry.
// All tests use only public APIs.
//
// INV-170-1: Geometry derived from ColumnGeometryTable only.
// INV-170-3: Hard guard — columns() panics if unresolved.
// INV-170-4: Exactly visible_columns - 1 separators.
// =============================================================================

use crate::widgets::tabular_view::{
    Column, ColumnGeometryTable, ColumnModel, SEPARATOR_COLOR, SEPARATOR_HOVER_COLOR,
    SEPARATOR_WIDTH, SeparatorSegment, emit_column_separators,
};

use crate::core::paint::Color;

#[test]
fn test_separator_positions_basic() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 450.0, 1, 1.0);

    let columns = table.columns(1);
    let total_height = 124.0; // header 24 + body 100

    let mut segments: Vec<(SeparatorSegment, Color)> = Vec::new();
    emit_column_separators(columns, total_height, None, |seg, color| {
        segments.push((seg, color));
    });

    assert_eq!(segments.len(), 2);

    // First separator: after column 0 (x=100)
    let sep0_x = segments[0].0.rect.x;
    assert!((sep0_x - (100.0 - SEPARATOR_WIDTH * 0.5)).abs() < f32::EPSILON);

    // Second separator: after column 1 (x=250)
    let sep1_x = segments[1].0.rect.x;
    assert!((sep1_x - (250.0 - SEPARATOR_WIDTH * 0.5)).abs() < f32::EPSILON);

    // No separator at column 2 right edge (450)
    // (only 2 separators for 3 columns)
}

#[test]
fn test_separator_count() {
    let mut model = ColumnModel::new();
    model.push(Column::new(50.0));
    model.push(Column::new(50.0));
    model.push(Column::new(50.0));
    model.push(Column::new(50.0));
    model.push(Column::new(50.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 250.0, 1, 1.0);

    let mut count = 0;
    emit_column_separators(table.columns(1), 100.0, None, |_, _| {
        count += 1;
    });

    assert_eq!(count, 4); // 5 columns → 4 separators
}

#[test]
fn test_no_separator_single_column() {
    let mut model = ColumnModel::new();
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 200.0, 1, 1.0);

    let mut count = 0;
    emit_column_separators(table.columns(1), 100.0, None, |_, _| {
        count += 1;
    });

    assert_eq!(count, 0);
}

#[test]
fn test_no_separator_empty() {
    let model = ColumnModel::new();

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    let mut count = 0;
    emit_column_separators(table.columns(1), 100.0, None, |_, _| {
        count += 1;
    });

    assert_eq!(count, 0);
}

#[test]
fn test_separator_hidden_columns() {
    // Geometry resolution filters to visible columns only (geometry.rs:187).
    // Separators follow resolved geometry — hidden columns produce no separators.
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    let col_b = Column::new(150.0);
    let col_b_id = col_b.id;
    model.push(col_b);
    model.push(Column::new(200.0));

    // Hide the middle column
    model.set_visible(col_b_id, false);

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 300.0, 1, 1.0); // visible: 100 + 200 = 300

    // Only 2 visible columns resolved
    assert_eq!(table.columns(1).len(), 2);

    let mut count = 0;
    emit_column_separators(table.columns(1), 100.0, None, |_, _| {
        count += 1;
    });

    // 2 visible columns → 1 separator
    assert_eq!(count, 1);
}

#[test]
fn test_separator_height_spans_full() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(100.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 200.0, 1, 1.0);

    let header_height = 24.0;
    let body_height = 100.0;
    let total_height = header_height + body_height;

    let mut segments: Vec<SeparatorSegment> = Vec::new();
    emit_column_separators(table.columns(1), total_height, None, |seg, _| {
        segments.push(seg);
    });

    assert_eq!(segments.len(), 1);
    assert!((segments[0].rect.height - total_height).abs() < f32::EPSILON);
    assert!((segments[0].rect.y).abs() < f32::EPSILON);
}

#[test]
fn test_separator_hover_color() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let col_a_id = col_a.id;
    model.push(col_a);
    model.push(Column::new(100.0));
    model.push(Column::new(100.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 300.0, 1, 1.0);

    let mut colors: Vec<Color> = Vec::new();
    emit_column_separators(table.columns(1), 100.0, Some(col_a_id), |_, color| {
        colors.push(color);
    });

    assert_eq!(colors.len(), 2);
    // First separator is at right edge of col_a → hover color
    assert_eq!(colors[0], SEPARATOR_HOVER_COLOR);
    // Second separator is at right edge of col_b → normal color
    assert_eq!(colors[1], SEPARATOR_COLOR);
}

#[test]
#[should_panic(expected = "INVARIANT-6")]
fn test_separator_unresolved_panics() {
    // INV-170-3: Hard guard — calling columns() before resolution panics.
    // This is the same code path as TabularSubstrate::paint() calling
    // self.geometry_table.columns() on unresolved geometry.
    let table = ColumnGeometryTable::new();
    let _ = table.columns(0); // Panics with INVARIANT-6
}
