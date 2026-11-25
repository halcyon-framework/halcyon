// =============================================================================
// TabularSubstrate Header Projection Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-166: Header domain projection and interactions.
// All tests use only public APIs.
// =============================================================================

use crate::widgets::tabular_view::{
    Column, ColumnGeometryTable, ColumnModel, GeometryView, HeaderDomain, HeaderHitResult,
    ResizeIntent, TabularIntent,
};

fn setup_geometry() -> ColumnGeometryTable {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 450.0, 1, 1.0);
    table
}

#[test]
fn test_header_projection() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    assert_eq!(header.cells().len(), 3);
    assert_eq!(header.cells()[0].rect.width, 100.0);
    assert_eq!(header.cells()[1].rect.x, 100.0);
    assert_eq!(header.cells()[1].rect.width, 150.0);
}

#[test]
fn test_header_height() {
    let header = HeaderDomain::new(30.0);
    assert_eq!(header.height(), 30.0);
}

#[test]
fn test_header_default_height() {
    let header = HeaderDomain::default();
    assert_eq!(header.height(), 24.0);
}

#[test]
fn test_header_hit_test_cell() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    // Hit first cell
    let result = header.hit_test(50.0, 15.0);
    assert!(matches!(
        result,
        Some(HeaderHitResult::Cell {
            column_index: 0,
            ..
        })
    ));

    // Hit second cell
    let result = header.hit_test(150.0, 15.0);
    assert!(matches!(
        result,
        Some(HeaderHitResult::Cell {
            column_index: 1,
            ..
        })
    ));
}

#[test]
fn test_header_hit_test_resize_zone() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    // Hit resize zone at edge of first column (x=100)
    let result = header.hit_test(99.0, 15.0);
    assert!(matches!(
        result,
        Some(HeaderHitResult::ResizeZone {
            column_index: 0,
            ..
        })
    ));
}

#[test]
fn test_header_hit_test_outside_bounds() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    // Below header
    let result = header.hit_test(50.0, 35.0);
    assert!(result.is_none());

    // Above header
    let result = header.hit_test(50.0, -5.0);
    assert!(result.is_none());
}

#[test]
fn test_resize_lifecycle() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    let col_id = header.cells()[0].column_id;

    // Begin resize
    let intent = header.begin_resize(col_id, 100.0);
    assert!(matches!(
        intent,
        TabularIntent::Resize(ResizeIntent::Begin { .. })
    ));
    assert!(header.is_resizing());
    assert_eq!(header.resizing_column(), Some(col_id));

    // Update resize
    let intent = header.update_resize(20.0).unwrap();
    assert!(matches!(
        intent,
        TabularIntent::Resize(ResizeIntent::Update { proposed_width, .. }) if proposed_width == 120.0
    ));

    // Commit resize
    let intent = header.commit_resize(120.0).unwrap();
    assert!(matches!(
        intent,
        TabularIntent::Resize(ResizeIntent::Commit { .. })
    ));
    assert!(!header.is_resizing());
}

#[test]
fn test_resize_cancel() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    let col_id = header.cells()[0].column_id;

    // Begin resize
    header.begin_resize(col_id, 100.0);
    assert!(header.is_resizing());

    // Cancel resize
    let intent = header.cancel_resize().unwrap();
    assert!(matches!(
        intent,
        TabularIntent::Resize(ResizeIntent::Cancel { .. })
    ));
    assert!(!header.is_resizing());
}

#[test]
fn test_header_scroll_offset() {
    let mut header = HeaderDomain::new(30.0);
    assert_eq!(header.scroll_x(), 0.0);

    header.set_scroll_x(50.0);
    assert_eq!(header.scroll_x(), 50.0);
}

#[test]
fn test_header_sort_request() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    let col_id = header.cells()[0].column_id;

    let intent = header.request_sort(col_id);
    assert!(matches!(intent, TabularIntent::Sort(_)));
}
