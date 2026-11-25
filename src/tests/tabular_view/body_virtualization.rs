// =============================================================================
// TabularSubstrate Body Virtualization Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-167: Body domain and row virtualization.
// All tests use only public APIs.
// =============================================================================

use crate::core::math::RectF;
use crate::widgets::tabular_view::{
    BodyDomain, Column, ColumnGeometryTable, ColumnModel, EditIntent, FocusIntent, GeometryView,
    RowId, RowModel, TabularIntent,
};

fn setup_geometry() -> ColumnGeometryTable {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 250.0, 1, 1.0);
    table
}

fn setup_body() -> BodyDomain {
    let mut row_model = RowModel::with_fixed_height(30.0);
    for _ in 0..10 {
        row_model.push(RowId::new());
    }

    let mut body = BodyDomain::with_row_model(row_model);
    body.set_viewport(RectF::new(0.0, 24.0, 300.0, 100.0)); // 24px header
    body
}

#[test]
fn test_body_realize_viewport() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut body = setup_body();

    super::test_realize(&mut body, view);

    // With 100px viewport and 30px rows, should realize ~4 rows
    assert!(body.realized_rows().len() >= 3);
    assert!(body.realized_rows().len() <= 5);

    // Each row should have 2 cells (2 columns)
    assert_eq!(body.realized_cells().len(), body.realized_rows().len() * 2);
}

#[test]
fn test_body_realize_with_scroll() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut body = setup_body();

    // Scroll down 60px (2 rows)
    body.set_scroll_y(60.0);
    super::test_realize(&mut body, view);

    // First realized row should be at index 2
    assert_eq!(body.realized_rows()[0].row_index, 2);
}

#[test]
fn test_body_content_height() {
    let body = setup_body();
    // 10 rows * 30px = 300px
    assert_eq!(body.content_height(), 300.0);
}

#[test]
fn test_body_viewport() {
    let mut body = BodyDomain::new();
    body.set_viewport(RectF::new(0.0, 24.0, 300.0, 100.0));

    let viewport = body.viewport();
    assert_eq!(viewport.x, 0.0);
    assert_eq!(viewport.y, 24.0);
    assert_eq!(viewport.width, 300.0);
    assert_eq!(viewport.height, 100.0);
}

#[test]
fn test_body_scroll_offsets() {
    let mut body = BodyDomain::new();

    body.set_scroll_x(50.0);
    body.set_scroll_y(100.0);

    assert_eq!(body.scroll_x(), 50.0);
    assert_eq!(body.scroll_y(), 100.0);
}

#[test]
fn test_body_hit_test() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut body = setup_body();
    super::test_realize(&mut body, view);

    // Hit first cell of first row
    let result = body.hit_test(50.0, 34.0); // 24px header + 10px into first row
    assert!(result.is_some());

    // Hit outside body
    let result = body.hit_test(50.0, 10.0); // In header area
    assert!(result.is_none());
}

#[test]
fn test_body_row_realization_check() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut body = setup_body();
    super::test_realize(&mut body, view);

    // First realized row should be realized
    let first_row_id = body.realized_rows()[0].row_id;
    assert!(body.is_row_realized(first_row_id));

    // Create a random row ID - should not be realized
    let random_id = RowId::new();
    assert!(!body.is_row_realized(random_id));
}

#[test]
fn test_intent_emission_edit() {
    let body = BodyDomain::new();
    let row_id = RowId::new();
    let col_id = crate::widgets::tabular_view::ColumnId::new();

    let intent = body.request_begin_edit(row_id, col_id);
    assert!(matches!(
        intent,
        TabularIntent::Edit(EditIntent::RequestBeginEdit { .. })
    ));
}

#[test]
fn test_intent_emission_focus() {
    let body = BodyDomain::new();
    let row_id = RowId::new();
    let col_id = crate::widgets::tabular_view::ColumnId::new();

    let intent = body.request_focus(row_id, col_id);
    assert!(matches!(
        intent,
        TabularIntent::Focus(FocusIntent::RequestFocus { .. })
    ));
}

#[test]
fn test_intent_emission_navigate() {
    let body = BodyDomain::new();

    let intent = body.request_navigate(
        None,
        crate::widgets::tabular_view::NavigationDirection::Down,
    );
    assert!(matches!(
        intent,
        TabularIntent::Focus(FocusIntent::RequestNavigate { .. })
    ));
}

#[test]
fn test_intent_emission_clear_focus() {
    let body = BodyDomain::new();

    let intent = body.request_clear_focus();
    assert!(matches!(
        intent,
        TabularIntent::Focus(FocusIntent::RequestClearFocus)
    ));
}

#[test]
fn test_body_empty_realization() {
    let table = setup_geometry();
    let view = GeometryView::new(&table, 1);
    let mut body = BodyDomain::new(); // Empty row model
    body.set_viewport(RectF::new(0.0, 24.0, 300.0, 100.0));

    super::test_realize(&mut body, view);

    assert!(body.realized_rows().is_empty());
    assert!(body.realized_cells().is_empty());
}
