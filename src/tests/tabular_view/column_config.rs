// =============================================================================
// Column Configuration Integration Tests
// =============================================================================
//
// Tests for column configuration state, persistence infrastructure, reorder,
// PinnedRight geometry, structure hash, reconciliation, and normalization.
// All tests use only public APIs.
// =============================================================================

use std::collections::{HashMap, HashSet};

use crate::widgets::tabular_view::{
    Column, ColumnConfigurationState, ColumnGeometryTable, ColumnId, ColumnModel,
    ColumnScrollDomain, GeometryView, TabularSubstrate, ViewConfigKey, compute_structure_hash,
    normalize_json, reconcile_json,
};

// =============================================================================
// Test 1: Apply state roundtrip
// =============================================================================

#[test]
fn apply_state_roundtrip() {
    let mut substrate = TabularSubstrate::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let col_c = Column::new(200.0);
    let col_d = Column::new(120.0);
    let a_id = col_a.id;
    let b_id = col_b.id;
    let c_id = col_c.id;
    let d_id = col_d.id;
    substrate.add_column(col_a);
    substrate.add_column(col_b);
    substrate.add_column(col_c);
    substrate.add_column(col_d);

    // Create custom config: reorder (C, A, D, B), hide B, pin C left, pin D right.
    let config = ColumnConfigurationState {
        column_order: vec![c_id, a_id, d_id, b_id],
        preferred_widths: HashMap::from([(a_id, 175.0)]),
        hidden: {
            let mut h = HashSet::new();
            h.insert(b_id);
            h
        },
        pinned_left: {
            let mut p = HashSet::new();
            p.insert(c_id);
            p
        },
        pinned_right: {
            let mut p = HashSet::new();
            p.insert(d_id);
            p
        },
    };

    let changed = substrate.apply_column_configuration(&config);
    assert!(changed);

    // Extract via from_model and verify.
    let extracted = ColumnConfigurationState::from_model(substrate.column_model());

    assert_eq!(extracted.column_order, vec![c_id, a_id, d_id, b_id]);
    assert_eq!(extracted.preferred_width_for(a_id), Some(175.0));
    assert!(extracted.hidden.contains(&b_id));
    assert_eq!(extracted.hidden.len(), 1);
    assert!(extracted.pinned_left.contains(&c_id));
    assert_eq!(extracted.pinned_left.len(), 1);
    assert!(extracted.pinned_right.contains(&d_id));
    assert_eq!(extracted.pinned_right.len(), 1);
}

// =============================================================================
// Test 2: Reset restores baseline
// =============================================================================

#[test]
fn reset_restores_baseline() {
    let mut substrate = TabularSubstrate::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let col_c = Column::new(200.0);
    let a_id = col_a.id;
    let b_id = col_b.id;
    let c_id = col_c.id;
    substrate.add_column(col_a);
    substrate.add_column(col_b);
    substrate.add_column(col_c);

    // Capture default config.
    let default_config = ColumnConfigurationState::from_model(substrate.column_model());

    // Modify: reorder, hide one, pin one.
    let modified = ColumnConfigurationState {
        column_order: vec![c_id, a_id, b_id],
        preferred_widths: HashMap::new(),
        hidden: {
            let mut h = HashSet::new();
            h.insert(b_id);
            h
        },
        pinned_left: {
            let mut p = HashSet::new();
            p.insert(c_id);
            p
        },
        pinned_right: HashSet::new(),
    };
    substrate.apply_column_configuration(&modified);

    // Verify modified state.
    assert!(!substrate.is_column_visible(b_id));
    assert_eq!(substrate.column_model().columns()[0].id, c_id);

    // Reset by applying default config.
    substrate.apply_column_configuration(&default_config);

    // Verify original order and visibility restored.
    let cols = substrate.column_model().columns();
    assert_eq!(cols[0].id, a_id);
    assert_eq!(cols[1].id, b_id);
    assert_eq!(cols[2].id, c_id);
    assert!(substrate.is_column_visible(a_id));
    assert!(substrate.is_column_visible(b_id));
    assert!(substrate.is_column_visible(c_id));
    assert_eq!(cols[0].scroll_domain, ColumnScrollDomain::Scrollable);
    assert_eq!(cols[1].scroll_domain, ColumnScrollDomain::Scrollable);
    assert_eq!(cols[2].scroll_domain, ColumnScrollDomain::Scrollable);
}

// =============================================================================
// Test 3: Reorder correctness
// =============================================================================

#[test]
fn reorder_valid_permutation() {
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

    let changed = model.reorder(&[c_id, a_id, b_id]);
    assert!(changed);
    assert_eq!(model.columns()[0].id, c_id);
    assert_eq!(model.columns()[1].id, a_id);
    assert_eq!(model.columns()[2].id, b_id);
}

#[test]
fn reorder_already_ordered_returns_false() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let a_id = col_a.id;
    let b_id = col_b.id;
    model.push(col_a);
    model.push(col_b);

    let changed = model.reorder(&[a_id, b_id]);
    assert!(!changed);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "reorder: new_order length mismatch")]
fn reorder_wrong_length_panics() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let a_id = col_a.id;
    model.push(col_a);
    model.push(Column::new(150.0));

    model.reorder(&[a_id]);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "reorder:")]
fn reorder_missing_id_panics() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let a_id = col_a.id;
    model.push(col_a);
    model.push(col_b);

    let bogus_id = ColumnId::new();
    model.reorder(&[a_id, bogus_id]);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "reorder: duplicate")]
fn reorder_duplicate_id_panics() {
    let mut model = ColumnModel::new();
    let col_a = Column::new(100.0);
    let a_id = col_a.id;
    model.push(col_a);
    model.push(Column::new(150.0));

    model.reorder(&[a_id, a_id]);
}

// =============================================================================
// Test 4: PinnedRight geometry (3-way partition)
// =============================================================================

#[test]
fn pinned_right_geometry_partition() {
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(200.0));
    model.push(Column::new(150.0));
    model.push(Column::new(70.0).with_scroll_domain(ColumnScrollDomain::PinnedRight));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 600.0, 1, 1.0);

    // 3-way partition counts.
    assert_eq!(table.pinned_count(1), 1);
    assert_eq!(table.pinned_right_count(1), 1);
    assert_eq!(table.scrollable_columns(1).len(), 2);

    // Widths.
    assert_eq!(table.pinned_width(1), 80.0);
    assert_eq!(table.pinned_right_width(1), 70.0);

    // Positions.
    let pinned_left = table.pinned_columns(1);
    assert_eq!(pinned_left[0].x_offset, 0.0);
    assert_eq!(pinned_left[0].width, 80.0);

    let scrollable = table.scrollable_columns(1);
    assert_eq!(scrollable[0].x_offset, 80.0);
    assert_eq!(scrollable[0].width, 200.0);
    assert_eq!(scrollable[1].x_offset, 280.0);
    assert_eq!(scrollable[1].width, 150.0);

    let pinned_right = table.pinned_right_columns(1);
    assert_eq!(pinned_right[0].x_offset, 600.0 - 70.0);
    assert_eq!(pinned_right[0].width, 70.0);
}

#[test]
fn pinned_right_geometry_view_slots() {
    let mut model = ColumnModel::new();
    model.push(Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft));
    model.push(Column::new(200.0));
    model.push(Column::new(70.0).with_scroll_domain(ColumnScrollDomain::PinnedRight));

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);

    let view = GeometryView::new(&table, 1);

    let pinned_right: Vec<_> = view.pinned_right_slots().collect();
    assert_eq!(pinned_right.len(), 1);
    assert_eq!(
        pinned_right[0].scroll_domain,
        ColumnScrollDomain::PinnedRight
    );

    let scrollable: Vec<_> = view.scrollable_slots().collect();
    assert_eq!(scrollable.len(), 1);
    assert_eq!(scrollable[0].scroll_domain, ColumnScrollDomain::Scrollable);
}

#[test]
#[should_panic(expected = "INV-SCROLL-DOMAIN-ELASTIC")]
fn pinned_right_elastic_panics() {
    let mut model = ColumnModel::new();
    model.push(Column::new(200.0));
    model.push(
        Column::new(0.0)
            .with_scroll_domain(ColumnScrollDomain::PinnedRight)
            .with_flex(1.0),
    );

    let mut table = ColumnGeometryTable::new();
    table.resolve(&model, 500.0, 1, 1.0);
}

// =============================================================================
// Test 5: Structure hash stability
// =============================================================================

#[test]
fn structure_hash_same_columns_same_hash() {
    let mut model = ColumnModel::new();
    model.push(Column::new(100.0));
    model.push(Column::new(200.0));

    let hash1 = compute_structure_hash(&model);
    let hash2 = compute_structure_hash(&model);
    assert_eq!(hash1, hash2);
    assert!(hash1.starts_with("sha256:"));
}

#[test]
fn structure_hash_different_order_different_hash() {
    let col_a = Column::new(100.0);
    let col_b = Column::new(200.0);
    let a_id = col_a.id;
    let b_id = col_b.id;

    let mut model1 = ColumnModel::new();
    model1.push(col_a);
    model1.push(col_b);

    let mut model2 = ColumnModel::new();
    model2.push(Column::with_id(b_id, 200.0));
    model2.push(Column::with_id(a_id, 100.0));

    let hash1 = compute_structure_hash(&model1);
    let hash2 = compute_structure_hash(&model2);
    assert_ne!(hash1, hash2);
}

#[test]
fn structure_hash_added_column_different() {
    let col_a = Column::new(100.0);
    let a_id = col_a.id;

    let mut model1 = ColumnModel::new();
    model1.push(col_a);

    let mut model2 = ColumnModel::new();
    model2.push(Column::with_id(a_id, 100.0));
    model2.push(Column::new(200.0));

    let hash1 = compute_structure_hash(&model1);
    let hash2 = compute_structure_hash(&model2);
    assert_ne!(hash1, hash2);
}

// =============================================================================
// Test 6: Reconciliation on JSON
// =============================================================================

#[test]
fn reconciliation_drops_unknown_appends_new() {
    // Saved config has columns A, B, C. Live model has A, B, D.
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let col_c = Column::new(200.0);
    let col_d = Column::new(120.0);
    let a_raw = col_a.id.raw();
    let b_raw = col_b.id.raw();
    let c_raw = col_c.id.raw();
    let d_raw = col_d.id.raw();

    let saved_json = serde_json::json!({
        "column_order": [a_raw, b_raw, c_raw],
        "hidden": [c_raw],
        "pinned_left": [a_raw],
        "pinned_right": [c_raw],
    });

    // Live model: A, B, D (no C).
    let mut live_model = ColumnModel::new();
    live_model.push(col_a);
    live_model.push(col_b);
    live_model.push(col_d);

    let reconciled = reconcile_json(&saved_json, &live_model);

    // Order: A, B preserved from saved; C dropped; D appended.
    let order: Vec<u64> = serde_json::from_value(reconciled["column_order"].clone()).unwrap();
    assert_eq!(order, vec![a_raw, b_raw, d_raw]);

    // Hidden: C was hidden but dropped (unknown ID).
    let hidden: Vec<u64> = serde_json::from_value(reconciled["hidden"].clone()).unwrap();
    assert!(hidden.is_empty());

    // Pinned left: A preserved.
    let pinned_left: Vec<u64> = serde_json::from_value(reconciled["pinned_left"].clone()).unwrap();
    assert_eq!(pinned_left, vec![a_raw]);

    // Pinned right: C was pinned right but dropped.
    let pinned_right: Vec<u64> =
        serde_json::from_value(reconciled["pinned_right"].clone()).unwrap();
    assert!(pinned_right.is_empty());
}

// =============================================================================
// Test 7: Normalization with extra unknown fields
// =============================================================================

#[test]
fn normalization_tolerates_extra_fields() {
    let raw = serde_json::json!({
        "column_order": [1, 2, 3],
        "hidden": [2],
        "pinned_left": [1],
        "pinned_right": [],
        "some_future_field": "hello",
        "another_unknown": 42,
    });

    let normalized = normalize_json(&raw);

    // Known fields preserved.
    let order: Vec<u64> = serde_json::from_value(normalized["column_order"].clone()).unwrap();
    assert_eq!(order, vec![1, 2, 3]);

    let hidden: Vec<u64> = serde_json::from_value(normalized["hidden"].clone()).unwrap();
    assert_eq!(hidden, vec![2]);

    let pinned_left: Vec<u64> = serde_json::from_value(normalized["pinned_left"].clone()).unwrap();
    assert_eq!(pinned_left, vec![1]);

    // Extra fields stripped.
    assert!(normalized.get("some_future_field").is_none());
    assert!(normalized.get("another_unknown").is_none());
}

// =============================================================================
// Test 8: Normalization with missing fields
// =============================================================================

#[test]
fn normalization_injects_missing_defaults() {
    let raw = serde_json::json!({
        "column_order": [1, 2],
        "hidden": [2],
    });

    let normalized = normalize_json(&raw);

    // Provided fields preserved.
    let order: Vec<u64> = serde_json::from_value(normalized["column_order"].clone()).unwrap();
    assert_eq!(order, vec![1, 2]);

    let hidden: Vec<u64> = serde_json::from_value(normalized["hidden"].clone()).unwrap();
    assert_eq!(hidden, vec![2]);

    // Missing fields injected as empty arrays.
    let pinned_left: Vec<u64> = serde_json::from_value(normalized["pinned_left"].clone()).unwrap();
    assert!(pinned_left.is_empty());

    let pinned_right: Vec<u64> =
        serde_json::from_value(normalized["pinned_right"].clone()).unwrap();
    assert!(pinned_right.is_empty());
}

// =============================================================================
// Test 9: Structure hash mismatch triggers reconciliation (not failure)
// =============================================================================

#[test]
fn structure_hash_mismatch_triggers_reconciliation() {
    use crate::widgets::tabular_view::ViewConfigStore;

    // In-memory store.
    struct MemStore {
        data: std::cell::RefCell<Option<serde_json::Value>>,
    }
    impl ViewConfigStore for MemStore {
        fn load(&self, _key: &ViewConfigKey) -> Option<serde_json::Value> {
            self.data.borrow().clone()
        }
        fn save(&self, _key: &ViewConfigKey, value: &serde_json::Value) {
            *self.data.borrow_mut() = Some(value.clone());
        }
        fn clear(&self, _key: &ViewConfigKey) {
            *self.data.borrow_mut() = None;
        }
    }

    // Create substrate with columns A, B.
    let mut substrate = TabularSubstrate::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0);
    let a_id = col_a.id;
    let b_id = col_b.id;
    substrate.add_column(col_a);
    substrate.add_column(col_b);

    let key = ViewConfigKey {
        app_namespace: "test".to_string(),
        widget_type: "list_view".to_string(),
        view_id: "test_view".to_string(),
    };

    // Save current config.
    let store = MemStore {
        data: std::cell::RefCell::new(None),
    };
    crate::widgets::tabular_view::save_column_config(substrate.column_model(), &key, &store);

    // Verify save worked.
    assert!(store.data.borrow().is_some());

    // Now create a NEW substrate with A, B, C (structure changed).
    let mut substrate2 = TabularSubstrate::new();
    substrate2.add_column(Column::with_id(a_id, 100.0));
    substrate2.add_column(Column::with_id(b_id, 150.0));
    substrate2.add_column(Column::new(200.0)); // New column C.

    // Load with hash mismatch: reconciliation produces a config but
    // apply_column_configuration returns false when the reconciled state
    // matches the live model (no actual mutation needed).
    let raw = store.load(&key).unwrap();
    let applied = crate::widgets::tabular_view::apply_raw_config_json(&mut substrate2, &key, raw);
    // Reconciliation runs but no visible mutation occurs (columns already match).
    assert!(!applied);

    // All 3 columns should still be present and visible.
    assert_eq!(substrate2.column_model().len(), 3);
    for col in substrate2.column_model().columns() {
        assert!(col.visible);
    }
}

// =============================================================================
// Test 10: Canonical re-emit is stable (idempotent)
// =============================================================================

#[test]
fn canonical_reemit_is_stable() {
    use crate::widgets::tabular_view::ViewConfigStore;

    struct MemStore {
        data: std::cell::RefCell<Option<serde_json::Value>>,
    }
    impl ViewConfigStore for MemStore {
        fn load(&self, _key: &ViewConfigKey) -> Option<serde_json::Value> {
            self.data.borrow().clone()
        }
        fn save(&self, _key: &ViewConfigKey, value: &serde_json::Value) {
            *self.data.borrow_mut() = Some(value.clone());
        }
        fn clear(&self, _key: &ViewConfigKey) {
            *self.data.borrow_mut() = None;
        }
    }

    let mut substrate = TabularSubstrate::new();
    let col_a = Column::new(100.0);
    let col_b = Column::new(150.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft);
    let col_c = Column::new(200.0);
    substrate.add_column(col_a);
    substrate.add_column(col_b);
    substrate.add_column(col_c);

    // Hide column C.
    let c_id = substrate.column_model().columns()[2].id;
    substrate.set_column_visible(c_id, false);

    let key = ViewConfigKey {
        app_namespace: "test".to_string(),
        widget_type: "list_view".to_string(),
        view_id: "idempotent_test".to_string(),
    };

    let store = MemStore {
        data: std::cell::RefCell::new(None),
    };

    // Save once.
    crate::widgets::tabular_view::save_column_config(substrate.column_model(), &key, &store);
    let json1 = store.data.borrow().clone().unwrap();

    // Load, apply, save again.
    let raw = store.load(&key).unwrap();
    crate::widgets::tabular_view::apply_raw_config_json(&mut substrate, &key, raw);
    crate::widgets::tabular_view::save_column_config(substrate.column_model(), &key, &store);
    let json2 = store.data.borrow().clone().unwrap();

    // The two JSON outputs should be identical.
    assert_eq!(json1, json2);
}
