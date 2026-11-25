// =============================================================================
// Scroll Domain Partition Tests
// =============================================================================
//
// Tests for column scroll domain partitioning (PinnedLeft vs Scrollable).
// Validates geometry resolution, invariant enforcement, and slot domain tagging.
// =============================================================================

use crate::widgets::tabular_view::{
    Column, ColumnGeometryTable, ColumnModel, ColumnScrollDomain, GeometryView,
};

// =============================================================================
// Geometry Partitioning
// =============================================================================

#[test]
fn test_all_scrollable_default() {
    // No pinned columns — identical to pre-change behavior.
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 450.0, 1, 1.0);

    assert_eq!(table.pinned_count(1), 0);
    assert_eq!(table.pinned_width(1), 0.0);
    assert_eq!(table.total_width(1), 450.0);
    assert_eq!(table.pinned_columns(1).len(), 0);
    assert_eq!(table.scrollable_columns(1).len(), 3);

    let cols = table.columns(1);
    assert_eq!(cols[0].x_offset, 0.0);
    assert_eq!(cols[0].width, 100.0);
    assert_eq!(cols[0].scroll_domain, ColumnScrollDomain::Scrollable);
    assert_eq!(cols[1].x_offset, 100.0);
    assert_eq!(cols[2].x_offset, 250.0);
}

#[test]
fn test_pinned_left_partition() {
    // 1 pinned + 2 scrollable.
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(200.0));
    model.push(Column::new(150.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    assert_eq!(table.pinned_count(1), 1);
    assert_eq!(table.pinned_width(1), 80.0);

    // Pinned column at x=0.
    let pinned = table.pinned_columns(1);
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].x_offset, 0.0);
    assert_eq!(pinned[0].width, 80.0);
    assert_eq!(pinned[0].scroll_domain, ColumnScrollDomain::PinnedLeft);

    // Scrollable columns start at pinned_width.
    let scrollable = table.scrollable_columns(1);
    assert_eq!(scrollable.len(), 2);
    assert_eq!(scrollable[0].x_offset, 80.0);
    assert_eq!(scrollable[0].width, 200.0);
    assert_eq!(scrollable[0].scroll_domain, ColumnScrollDomain::Scrollable);
    assert_eq!(scrollable[1].x_offset, 280.0);
    assert_eq!(scrollable[1].width, 150.0);

    assert_eq!(table.total_width(1), 430.0);
}

#[test]
fn test_all_pinned() {
    // All pinned — pinned_width == total_width, scrollable empty.
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(150.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    assert_eq!(table.pinned_count(1), 2);
    assert_eq!(table.pinned_width(1), 250.0);
    assert_eq!(table.total_width(1), 250.0);
    assert_eq!(table.pinned_columns(1).len(), 2);
    assert_eq!(table.scrollable_columns(1).len(), 0);
}

#[test]
fn test_zero_columns() {
    let model = ColumnModel::new();
    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    assert_eq!(table.pinned_count(1), 0);
    assert_eq!(table.pinned_width(1), 0.0);
    assert_eq!(table.total_width(1), 0.0);
}

#[test]
fn test_hidden_pinned_excluded() {
    // Hidden pinned column not counted in pinned_width.
    let mut model = ColumnModel::new();
    model.push(
        Column::new(100.0)
            .with_scroll_domain(ColumnScrollDomain::PinnedLeft)
            .with_visible(false),
    );
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    assert_eq!(table.pinned_count(1), 0);
    assert_eq!(table.pinned_width(1), 0.0);
    assert_eq!(table.scrollable_columns(1).len(), 1);
    assert_eq!(table.scrollable_columns(1)[0].width, 200.0);
}

// =============================================================================
// Invariant Enforcement
// =============================================================================

#[test]
#[should_panic(expected = "INV-SCROLL-DOMAIN-ELASTIC")]
fn test_pinned_elastic_panics() {
    let mut model = ColumnModel::new();
    model.push(
        Column::new(0.0)
            .with_scroll_domain(ColumnScrollDomain::PinnedLeft)
            .with_flex(1.0),
    );
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);
}

#[test]
fn test_scrollable_elastic_allowed() {
    // 1 pinned + 1 fixed scrollable + 1 flex scrollable.
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(100.0));
    model.push(Column::new(0.0).with_flex(1.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    // Pinned: 80px. Scrollable available: 500 - 80 = 420. Fixed: 100. Flex: 320.
    assert_eq!(table.pinned_width(1), 80.0);
    let scrollable = table.scrollable_columns(1);
    assert_eq!(scrollable[0].width, 100.0);
    assert_eq!(scrollable[1].width, 320.0);
    assert_eq!(table.total_width(1), 500.0);
}

#[test]
#[should_panic(expected = "INV-COL-ELASTIC-1")]
fn test_multiple_scrollable_elastic_panics() {
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(0.0).with_flex(1.0));
    model.push(Column::new(0.0).with_flex(2.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);
}

// =============================================================================
// Slot Domain Tagging
// =============================================================================

#[test]
fn test_slot_carries_scroll_domain() {
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    let view = GeometryView::new(&table, 1);

    let header_slots: Vec<_> = view.header_slots().collect();
    assert_eq!(
        header_slots[0].scroll_domain,
        ColumnScrollDomain::PinnedLeft
    );
    assert_eq!(
        header_slots[1].scroll_domain,
        ColumnScrollDomain::Scrollable
    );

    let body_slots: Vec<_> = view.body_slots().collect();
    assert_eq!(body_slots[0].scroll_domain, ColumnScrollDomain::PinnedLeft);
    assert_eq!(body_slots[1].scroll_domain, ColumnScrollDomain::Scrollable);
}

#[test]
fn test_pinned_slots_iterator() {
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(60.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(200.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    let view = GeometryView::new(&table, 1);
    let pinned: Vec<_> = view.pinned_slots().collect();
    assert_eq!(pinned.len(), 2);
    assert_eq!(pinned[0].scroll_domain, ColumnScrollDomain::PinnedLeft);
    assert_eq!(pinned[1].scroll_domain, ColumnScrollDomain::PinnedLeft);
    assert_eq!(pinned[0].x, 0.0);
    assert_eq!(pinned[1].x, 80.0);
}

#[test]
fn test_scrollable_slots_iterator() {
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(200.0));
    model.push(Column::new(150.0));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    let view = GeometryView::new(&table, 1);
    let scrollable: Vec<_> = view.scrollable_slots().collect();
    assert_eq!(scrollable.len(), 2);
    assert_eq!(scrollable[0].scroll_domain, ColumnScrollDomain::Scrollable);
    assert_eq!(scrollable[1].scroll_domain, ColumnScrollDomain::Scrollable);
    assert_eq!(scrollable[0].x, 80.0);
    assert_eq!(scrollable[1].x, 280.0);
}

// =============================================================================
// Scroll-Independence
// =============================================================================

#[test]
fn test_geometry_independent_of_scroll() {
    // resolve() produces identical geometry regardless of scroll state
    // (geometry doesn't see scroll state — scroll_x is not a parameter).
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(200.0));

    let mut table1 = ColumnGeometryTable::new();
    table1.resolve(&model, 500.0, 1, 1.0);

    let mut table2 = ColumnGeometryTable::new();
    table2.resolve(&model, 500.0, 1, 1.0);

    // Both tables produce identical geometry.
    assert_eq!(table1.total_width(1), table2.total_width(1));
    assert_eq!(table1.pinned_width(1), table2.pinned_width(1));
    assert_eq!(table1.pinned_count(1), table2.pinned_count(1));

    let cols1 = table1.columns(1);
    let cols2 = table2.columns(1);
    assert_eq!(cols1.len(), cols2.len());
    for (a, b) in cols1.iter().zip(cols2.iter()) {
        assert_eq!(a.x_offset, b.x_offset);
        assert_eq!(a.width, b.width);
        assert_eq!(a.scroll_domain, b.scroll_domain);
    }
}
