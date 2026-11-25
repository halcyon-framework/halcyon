// =============================================================================
// SPEC-HAL-173: Cell Content Projection Integration Tests
// =============================================================================
//
// Tests for cell content projection contract.
// All tests use only public APIs.
//
// INV-173-2: content_rect copied directly from CellProjection.rect.
// INV-173-3: Content projections exist IFF row realized AND column visible.
// INV-173-4: Idempotent — same inputs produce same output.
// INV-173-5: No geometry mutation.
// =============================================================================

use crate::core::math::RectF;
use crate::widgets::tabular_view::{
    BodyDomain, Column, ColumnGeometryTable, ColumnModel, GeometryView, RowId, RowModel,
};

fn setup_geometry() -> (ColumnModel, ColumnGeometryTable) {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 250.0, 1, 1.0);
    (model, table)
}

fn setup_body(row_count: usize) -> BodyDomain {
    let mut row_model = RowModel::with_fixed_height(30.0);
    for _ in 0..row_count {
        row_model.push(RowId::new());
    }

    let mut body = BodyDomain::with_row_model(row_model);
    body.set_viewport(RectF::new(0.0, 24.0, 300.0, 100.0));
    body
}

// =============================================================================
// INV-173-2: content_rect == realized_cells[i].rect
// =============================================================================

#[test]
fn test_content_rect_matches_realized_cells() {
    let (_model, table) = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut body = setup_body(10);

    let indices: Vec<usize> = (0..10).collect();
    super::test_realize_sorted(&mut body, view, &indices);

    let cells = body.realized_cells();
    let projections = body.content_projections();

    assert!(!cells.is_empty());
    for (i, (cell, proj)) in cells.iter().zip(projections.iter()).enumerate() {
        assert_eq!(
            proj.content_rect, cell.rect,
            "content_rect mismatch at index {}",
            i
        );
        assert_eq!(
            proj.cell_id, cell.cell_id,
            "cell_id mismatch at index {}",
            i
        );
    }
}

// =============================================================================
// INV-173-2: Count equality
// =============================================================================

#[test]
fn test_content_projection_count() {
    let (_model, table) = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut body = setup_body(10);

    let indices: Vec<usize> = (0..10).collect();
    super::test_realize_sorted(&mut body, view, &indices);

    assert_eq!(
        body.content_projections().len(),
        body.realized_cells().len(),
        "content_projections count must equal realized_cells count"
    );
}

// =============================================================================
// INV-173-4: Identity stability across scroll
// =============================================================================

#[test]
fn test_identity_stability_across_scroll() {
    let (_model, table) = setup_geometry();
    let mut body = setup_body(20);

    // Realize at scroll=0
    let indices: Vec<usize> = (0..20).collect();
    let view = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view, &indices);

    // Capture cell_ids for first realized row
    let first_row_id = body.realized_rows()[0].row_id;
    let first_projections: Vec<_> = body
        .content_projections()
        .iter()
        .filter(|p| p.cell_id.row_id == first_row_id)
        .copied()
        .collect();

    // Scroll down then back
    body.set_scroll_y(90.0);
    let view2 = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view2, &indices);

    body.set_scroll_y(0.0);
    let view3 = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view3, &indices);

    // Same row should produce same cell_ids
    let restored: Vec<_> = body
        .content_projections()
        .iter()
        .filter(|p| p.cell_id.row_id == first_row_id)
        .copied()
        .collect();

    assert_eq!(first_projections.len(), restored.len());
    for (a, b) in first_projections.iter().zip(restored.iter()) {
        assert_eq!(a.cell_id, b.cell_id);
    }
}

// =============================================================================
// INV-173-3: Teardown on scroll out
// =============================================================================

#[test]
fn test_teardown_on_scroll_out() {
    let (_model, table) = setup_geometry();
    let mut body = setup_body(20);

    let indices: Vec<usize> = (0..20).collect();
    let view = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view, &indices);

    let count_before = body.content_projections().len();
    assert!(count_before > 0);

    // Scroll far past all rows (all rows unrealized)
    body.set_scroll_y(10000.0);
    let view2 = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view2, &indices);

    assert_eq!(
        body.content_projections().len(),
        0,
        "scrolled-out rows must produce no projections"
    );
}

// =============================================================================
// INV-173-3: Teardown on column hide
// =============================================================================

#[test]
fn test_teardown_on_column_hide() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    model.push(col_a);
    let col_b = Column::new(150.0);
    let col_b_id = col_b.id;
    model.push(col_b);

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 250.0, 1, 1.0);

    let mut body = setup_body(5);
    let indices: Vec<usize> = (0..5).collect();

    let view = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view, &indices);

    let count_2col = body.content_projections().len();
    // 2 columns × realized_rows
    assert_eq!(count_2col, body.realized_rows().len() * 2);

    // Hide column B, re-resolve geometry
    model.set_visible(col_b_id, false);
    let mut table2 = ColumnGeometryTable::new();
    table2.resolve(&model, 100.0, 2, 1.0); // visible: col_a only = 100

    let view2 = GeometryView::new(&table2, 2);
    super::test_realize_sorted(&mut body, view2, &indices);

    let count_1col = body.content_projections().len();
    // 1 column × realized_rows
    assert_eq!(count_1col, body.realized_rows().len() * 1);
    assert!(count_1col < count_2col);
}

// =============================================================================
// INV-173-5: No geometry mutation
// =============================================================================

#[test]
fn test_no_geometry_mutation() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 250.0, 1, 1.0);

    // Snapshot geometry before
    let columns_before: Vec<_> = table.columns(1).to_vec();

    let view = GeometryView::new(&table, 1);
    let mut body = setup_body(10);
    let indices: Vec<usize> = (0..10).collect();
    super::test_realize_sorted(&mut body, view, &indices);

    // Verify content projections exist
    assert!(!body.content_projections().is_empty());

    // Geometry unchanged
    let columns_after = table.columns(1);
    assert_eq!(columns_before.len(), columns_after.len());
    for (before, after) in columns_before.iter().zip(columns_after.iter()) {
        assert_eq!(before.column_id, after.column_id);
        assert_eq!(before.x_offset, after.x_offset);
        assert_eq!(before.width, after.width);
    }
}

// =============================================================================
// INV-173-4: Idempotent realization
// =============================================================================

#[test]
fn test_idempotent_realization() {
    let (_model, table) = setup_geometry();
    let mut body = setup_body(10);
    let indices: Vec<usize> = (0..10).collect();

    // Realize once
    let view = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view, &indices);
    let first: Vec<_> = body.content_projections().to_vec();

    // Realize again with same state
    let view2 = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view2, &indices);
    let second: Vec<_> = body.content_projections().to_vec();

    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.cell_id, b.cell_id);
        assert_eq!(a.content_rect, b.content_rect);
    }
}

// =============================================================================
// INV-173-3: Empty model produces no projections
// =============================================================================

#[test]
fn test_empty_model() {
    let (_model, table) = setup_geometry();

    // Empty row model
    let row_model = RowModel::with_fixed_height(30.0);
    let mut body = BodyDomain::with_row_model(row_model);
    body.set_viewport(RectF::new(0.0, 24.0, 300.0, 100.0));

    let view = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view, &[]);

    assert_eq!(body.content_projections().len(), 0);
    assert_eq!(body.realized_cells().len(), 0);
}

#[test]
fn test_empty_column_model() {
    let model = ColumnModel::new();
    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 300.0, 1, 1.0);

    let mut body = setup_body(5);
    let indices: Vec<usize> = (0..5).collect();

    let view = GeometryView::new(&table, 1);
    super::test_realize_sorted(&mut body, view, &indices);

    assert_eq!(body.content_projections().len(), 0);
    assert_eq!(body.realized_cells().len(), 0);
}
