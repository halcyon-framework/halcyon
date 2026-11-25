// =============================================================================
// Column Visibility Integration Tests
// =============================================================================
//
// Tests for column visibility: programmatic API on ColumnModel + geometry.
// All tests use only public APIs.
// =============================================================================

use crate::widgets::tabular_view::{
    Column, ColumnGeometryTable, ColumnModel, ColumnScrollDomain, GeometryView, HeaderDomain,
};

// =============================================================================
// Geometry Effects
// =============================================================================

#[test]
fn hide_middle_column_shrinks_geometry() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let col_c = Column::new(200.0);
    let b_id = col_b.id;
    model.push(col_a);
    model.push(col_b);
    model.push(col_c);

    // Hide middle column.
    model.set_visible(b_id, false);

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    assert_eq!(table.len(1), 2);
    assert_eq!(table.total_width(1), 300.0);

    let cols = table.columns(1);
    assert_eq!(cols[0].width, 100.0);
    assert_eq!(cols[0].x_offset, 0.0);
    assert_eq!(cols[1].width, 200.0);
    assert_eq!(cols[1].x_offset, 100.0);
}

#[test]
fn hide_pinned_column_shrinks_pinned_domain() {
    let mut model = ColumnModel::new();
    let col_p1 = Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft);
    let col_p2 = Column::new(60.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft);
    let col_s1 = Column::new(200.0);
    let p1_id = col_p1.id;
    model.push(col_p1);
    model.push(col_p2);
    model.push(col_s1);

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);
    assert_eq!(table.pinned_count(1), 2);
    assert_eq!(table.pinned_width(1), 140.0);

    // Hide one pinned column, re-resolve.
    model.set_visible(p1_id, false);
    table.invalidate();
    table.resolve(&model, 500.0, 2, 1.0);

    assert_eq!(table.pinned_count(2), 1);
    assert_eq!(table.pinned_width(2), 60.0);
    assert_eq!(table.len(2), 2);
}

// =============================================================================
// Toggle / Order Preservation
// =============================================================================

#[test]
fn toggle_preserves_order() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let col_c = Column::new(200.0);
    let a_id = col_a.id;
    let b_id = col_b.id;
    let c_id = col_c.id;
    model.push(col_a);
    model.push(col_b);
    model.push(col_c);

    // Hide B, resolve.
    model.set_visible(b_id, false);
    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);
    assert_eq!(table.len(1), 2);

    // Show B, re-resolve.
    model.set_visible(b_id, true);
    table.invalidate();
    table.resolve(&model, 500.0, 2, 1.0);

    assert_eq!(table.len(2), 3);
    let cols = table.columns(2);
    assert_eq!(cols[0].column_id, a_id);
    assert_eq!(cols[1].column_id, b_id);
    assert_eq!(cols[2].column_id, c_id);
}

#[test]
fn visibility_does_not_change_domain_membership() {
    let mut model = ColumnModel::new();
    let col_p = Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft);
    let col_s = Column::new(200.0);
    let p_id = col_p.id;
    model.push(col_p);
    model.push(col_s);

    // Hide pinned column, then show it.
    model.set_visible(p_id, false);
    model.set_visible(p_id, true);

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    assert_eq!(table.pinned_count(1), 1);
    let pinned = table.pinned_columns(1);
    assert_eq!(pinned[0].column_id, p_id);
    assert_eq!(pinned[0].scroll_domain, ColumnScrollDomain::PinnedLeft);
}

// =============================================================================
// Programmatic API Round-Trip (TabularSubstrate)
// =============================================================================

#[test]
fn programmatic_api_round_trip() {
    use crate::widgets::tabular_view::TabularSubstrate;

    let mut substrate = TabularSubstrate::new();
    let col = Column::new(100.0);
    let col_id = col.id;
    substrate.add_column(col);

    assert!(substrate.is_column_visible(col_id));

    let changed = substrate.set_column_visible(col_id, false);
    assert!(changed);
    assert!(!substrate.is_column_visible(col_id));

    let toggled = substrate.toggle_column_visible(col_id);
    assert!(toggled);
    assert!(substrate.is_column_visible(col_id));
}

// =============================================================================
// Header Slot Exclusion
// =============================================================================

#[test]
fn hidden_column_excluded_from_header_slots() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let col_c = Column::new(200.0);
    let a_id = col_a.id;
    let b_id = col_b.id;
    model.push(col_a);
    model.push(col_b);
    model.push(col_c);

    // Hide column B.
    model.set_visible(b_id, false);

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    let view = GeometryView::new(&table, 1);
    let mut header = HeaderDomain::new(30.0);
    header.project(view);

    let ids: Vec<_> = header.cells().iter().map(|c| c.column_id).collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], a_id);
    assert!(!ids.contains(&b_id));
}
