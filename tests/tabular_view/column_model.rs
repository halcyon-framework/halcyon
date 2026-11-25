// =============================================================================
// TabularSubstrate Column Model Integration Tests
// =============================================================================
//
// Tests for SPEC-HAL-165: Column model ownership and versioning.
// All tests use only public APIs.
// =============================================================================

use halcyon::widgets::tabular_view::{Column, ColumnId, ColumnModel, TabularSubstrate};

#[test]
fn test_column_id_uniqueness() {
    let id1 = ColumnId::new();
    let id2 = ColumnId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_column_model_versioning() {
    let mut model = ColumnModel::new();
    assert_eq!(model.version(), 0);

    model.push(Column::new(100.0));
    assert_eq!(model.version(), 1);

    let id = model.columns()[0].id;
    model.set_visible(id, false);
    assert_eq!(model.version(), 2);
}

#[test]
fn test_visible_columns() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0).with_visible(true));
    model.push(Column::new(100.0).with_visible(false));
    model.push(Column::new(100.0).with_visible(true));

    let visible: Vec<_> = model.visible_columns().collect();
    assert_eq!(visible.len(), 2);
}

#[test]
fn test_column_creation() {
    let col = Column::new(100.0);
    assert_eq!(col.preferred_width, 100.0);
    assert_eq!(col.min_width, 0.0);
    assert_eq!(col.max_width, f32::INFINITY);
    assert!(col.visible);
    assert!(col.flex.is_none());
}

#[test]
fn test_column_builder_methods() {
    let col = Column::new(100.0)
        .with_min_width(50.0)
        .with_max_width(200.0)
        .with_flex(1.0)
        .with_visible(false);

    assert_eq!(col.preferred_width, 100.0);
    assert_eq!(col.min_width, 50.0);
    assert_eq!(col.max_width, 200.0);
    assert_eq!(col.flex, Some(1.0));
    assert!(!col.visible);
}

#[test]
fn test_column_model_push_and_len() {
    let mut model = ColumnModel::new();
    assert!(model.is_empty());
    assert_eq!(model.len(), 0);

    model.push(Column::new(100.0));
    model.push(Column::new(150.0));

    assert!(!model.is_empty());
    assert_eq!(model.len(), 2);
}

#[test]
fn test_column_model_insert() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(200.0));

    let version_before = model.version();
    model.insert(1, Column::new(150.0));

    assert_eq!(model.len(), 3);
    assert_eq!(model.version(), version_before + 1);
    assert_eq!(model.columns()[1].preferred_width, 150.0);
}

#[test]
fn test_column_model_remove() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));
    model.push(Column::new(200.0));

    let col_id = model.columns()[1].id;
    let version_before = model.version();

    let removed = model.remove(col_id);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().preferred_width, 150.0);
    assert_eq!(model.len(), 2);
    assert_eq!(model.version(), version_before + 1);
}

#[test]
fn test_column_model_get_and_index_of() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));

    let col_id = model.columns()[0].id;
    assert!(model.get(col_id).is_some());
    assert_eq!(model.index_of(col_id), Some(0));

    let fake_id = ColumnId::new();
    assert!(model.get(fake_id).is_none());
    assert_eq!(model.index_of(fake_id), None);
}

#[test]
fn test_column_model_set_preferred_width() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));

    let col_id = model.columns()[0].id;
    let version_before = model.version();

    assert!(model.set_preferred_width(col_id, 150.0));
    assert_eq!(model.get(col_id).unwrap().preferred_width, 150.0);
    assert_eq!(model.version(), version_before + 1);
}

#[test]
fn test_column_model_set_visible() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));

    let col_id = model.columns()[0].id;
    assert!(model.get(col_id).unwrap().visible);

    let version_before = model.version();
    assert!(model.set_visible(col_id, false));
    assert!(!model.get(col_id).unwrap().visible);
    assert_eq!(model.version(), version_before + 1);

    // Setting to same value should return false and not increment version
    let version_before = model.version();
    assert!(!model.set_visible(col_id, false));
    assert_eq!(model.version(), version_before);
}

#[test]
fn test_column_model_clear() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(150.0));

    let version_before = model.version();
    model.clear();

    assert!(model.is_empty());
    assert_eq!(model.version(), version_before + 1);
}

#[test]
fn test_column_model_with_columns() {
    let columns = vec![Column::new(100.0), Column::new(150.0), Column::new(200.0)];
    let model = ColumnModel::with_columns(columns);

    assert_eq!(model.len(), 3);
    assert_eq!(model.columns()[0].preferred_width, 100.0);
    assert_eq!(model.columns()[1].preferred_width, 150.0);
    assert_eq!(model.columns()[2].preferred_width, 200.0);
}

// =============================================================================
// TabularSubstrate Column Operations Integration Tests
// =============================================================================

#[test]
fn test_tabular_view_creation() {
    let view = TabularSubstrate::new();
    assert!(view.column_model().is_empty());
    assert_eq!(view.header().height(), 24.0); // Default
}

#[test]
fn test_tabular_view_with_columns() {
    let columns = vec![Column::new(100.0), Column::new(150.0), Column::new(200.0)];
    let view = TabularSubstrate::with_columns(columns);
    assert_eq!(view.column_model().len(), 3);
}

#[test]
fn test_tabular_view_column_operations() {
    let mut view = TabularSubstrate::new();

    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));

    assert_eq!(view.column_model().len(), 2);

    let col_id = view.column_model().columns()[0].id;
    assert!(view.set_column_width(col_id, 120.0));
    assert_eq!(
        view.column_model().get(col_id).unwrap().preferred_width,
        120.0
    );
}

#[test]
fn test_tabular_view_column_visibility() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));

    let col_id = view.column_model().columns()[0].id;
    assert!(view.column_model().get(col_id).unwrap().visible);

    assert!(view.set_column_visible(col_id, false));
    assert!(!view.column_model().get(col_id).unwrap().visible);

    // Should have 1 visible column now
    assert_eq!(view.column_model().visible_columns().count(), 1);
}
