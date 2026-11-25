//! ListView smoke tests
//!
//! Verifies that ListView skeleton:
//! - Configures substrate row model from data source RowIds
//! - Survives BaseOrderReplaced invalidation without index leakage
//! - Uses only RowId-based identity (no index-based APIs)
//! - RowHost lifecycle: creates entries for realized rows
//! - Invalidation → rebind wiring: marks rows for rebind
//! - Selection authority: TabularSubstrate is sole owner; ListView is dumb mirror
//!
//! These are smoke tests only. Behavioral tests come later.

#![cfg(test)]

use crate::widgets::tabular_view::column::ColumnId;
use crate::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, ColumnInvalidationScope, DataInvalidationEvent, DataSource,
    Extent, PolicyInvalidationScope, UniverseMode, ValueState,
};
use crate::widgets::tabular_view::intents::{SelectionIntent, TabularIntent};
use crate::widgets::tabular_view::row::RowId;

// =============================================================================
// Test Data Source (Simple In-Memory Implementation)
// =============================================================================

/// Simple test data source implementing SPEC-HAL-188 DataSource trait.
///
/// This is a minimal implementation for smoke testing.
/// For comprehensive testing, use the baseline oracle.
struct TestDataSource {
    /// RowId sequence (base row order).
    rows: Vec<RowId>,
    /// Cell data (keyed by row index, single column).
    data: Vec<String>,
    /// Pending invalidation events.
    pending_events: Vec<DataInvalidationEvent>,
}

impl TestDataSource {
    /// Create with N rows.
    fn with_rows(n: usize) -> Self {
        let rows: Vec<RowId> = (0..n).map(|_| RowId::new()).collect();
        let data: Vec<String> = (0..n).map(|i| format!("Row {}", i)).collect();
        Self {
            rows,
            data,
            pending_events: Vec::new(),
        }
    }

    /// Replace base row order (triggers BaseOrderReplaced).
    ///
    /// INV-ROW-BASE-REPLACE: This is the ONLY way to change base order.
    fn replace_rows(&mut self, new_rows: Vec<RowId>, new_data: Vec<String>) {
        self.rows = new_rows;
        self.data = new_data;
        self.pending_events
            .push(DataInvalidationEvent::BaseOrderReplaced);
    }

    /// Get row index by RowId.
    fn row_index(&self, row_id: RowId) -> Option<usize> {
        self.rows.iter().position(|&r| r == row_id)
    }

    /// Emit CellsInvalidated for specific rows.
    fn invalidate_cells(&mut self, rows: Vec<RowId>) {
        self.pending_events
            .push(DataInvalidationEvent::CellsInvalidated {
                rows,
                columns: ColumnInvalidationScope::All,
            });
    }

    /// Emit PolicyInvalidated for all rows.
    fn invalidate_policy_all(&mut self) {
        self.pending_events
            .push(DataInvalidationEvent::PolicyInvalidated {
                scope: PolicyInvalidationScope::All,
            });
    }
}

impl DataSource for TestDataSource {
    fn row_ids(&self) -> &[RowId] {
        &self.rows
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, _column: ColumnId) -> ValueState<CellValue> {
        match self.row_index(row) {
            Some(idx) => ValueState::Ready(CellValue::String(self.data[idx].clone())),
            None => ValueState::Missing,
        }
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

// =============================================================================
// Smoke Tests
// =============================================================================

/// Test: ListView can be created with a data source.
#[test]
fn test_listview_creation() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(10);
    let list_view = ListView::new(source);

    // Verify row count from adapter.
    assert_eq!(list_view.row_count(), 10);

    // Verify substrate has rows.
    assert_eq!(list_view.substrate().row_model().len(), 10);
}

/// Test: ListView renders N rows from data source.
#[test]
fn test_listview_renders_rows() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(100);
    let list_view = ListView::new(source);

    // Verify substrate configuration.
    assert_eq!(list_view.substrate().row_model().len(), 100);

    // Verify adapter has correct extent.
    assert_eq!(list_view.adapter().extent(), Extent::Exact(100));
}

/// Test: BaseOrderReplaced invalidation triggers row model rebuild.
///
/// INV-ROW-BASE-REPLACE: No index recovery logic. Full replace-only.
#[test]
fn test_base_order_replaced_invalidation() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    // Verify initial state.
    assert_eq!(list_view.row_count(), 5);
    let original_row_ids: Vec<RowId> = list_view.adapter().cached_row_ids().to_vec();

    // Trigger BaseOrderReplaced via adapter.
    let new_rows: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    let new_data: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
    list_view
        .adapter_mut()
        .source_mut()
        .replace_rows(new_rows.clone(), new_data);

    // Process invalidation.
    let result = list_view.process_invalidation();

    // Verify invalidation was processed.
    assert!(
        result.base_order_replaced,
        "BaseOrderReplaced should be true"
    );

    // Verify row model was rebuilt.
    assert_eq!(list_view.row_count(), 3);
    assert_eq!(list_view.substrate().row_model().len(), 3);

    // Verify new RowIds are different from original.
    let new_row_ids: Vec<RowId> = list_view.adapter().cached_row_ids().to_vec();
    assert_ne!(original_row_ids, new_row_ids);
    assert_eq!(new_row_ids.len(), 3);
}

/// Test: No index-based APIs exposed.
///
/// INV-ROW-INDEX-EPHEMERAL: Verifies no index-based helpers exist.
#[test]
fn test_no_index_based_apis() {
    use crate::widgets::list_view::ListView;

    // This test documents that ListView has NO:
    // - selected_index()
    // - row_at_index()
    // - scroll_to_index()
    // - get_index(row_id)
    //
    // If any such methods are added, this test should be updated
    // with a clear SPEC justification.

    let source = TestDataSource::with_rows(10);
    let list_view = ListView::new(source);

    // The only way to access rows is via RowId through substrate.
    // We can get row_ids from the adapter, but there's no index-based getter.
    let row_ids = list_view.adapter().cached_row_ids();
    assert_eq!(row_ids.len(), 10);

    // Substrate row_model() provides index_of(RowId), but this is
    // internal to substrate and ephemeral (not cached across frames).
    let first_row_id = row_ids[0];
    let substrate_index = list_view.substrate().row_model().index_of(first_row_id);
    assert_eq!(substrate_index, Some(0));

    // There is NO list_view.get_row(0) method.
    // There is NO list_view.selected_index() method.
    // There is NO list_view.scroll_to_index(5) method.
}

/// Test: RowHost uses only RowId as key, not index.
#[test]
fn test_row_host_rowid_keyed() {
    use crate::widgets::list_view::RowHost;

    let mut host = RowHost::new();

    // Create rows by RowId.
    let row_id1 = RowId::new();
    let row_id2 = RowId::new();

    host.get_or_create(row_id1);
    host.get_or_create(row_id2);

    // Verify access is by RowId only.
    assert!(host.contains(row_id1));
    assert!(host.contains(row_id2));

    // There is NO host.get_by_index(0) method.
    // The only API is RowId-based.
    assert_eq!(host.widget_count(), 2);
}

/// Test: ListView survives multiple BaseOrderReplaced events.
#[test]
fn test_multiple_base_order_replacements() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    // First replacement.
    let rows1: Vec<RowId> = (0..10).map(|_| RowId::new()).collect();
    let data1: Vec<String> = (0..10).map(|i| format!("R1-{}", i)).collect();
    list_view
        .adapter_mut()
        .source_mut()
        .replace_rows(rows1, data1);
    list_view.process_invalidation();
    assert_eq!(list_view.row_count(), 10);

    // Second replacement.
    let rows2: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    let data2: Vec<String> = vec!["X".into(), "Y".into(), "Z".into()];
    list_view
        .adapter_mut()
        .source_mut()
        .replace_rows(rows2, data2);
    list_view.process_invalidation();
    assert_eq!(list_view.row_count(), 3);

    // Third replacement (back to larger).
    let rows3: Vec<RowId> = (0..20).map(|_| RowId::new()).collect();
    let data3: Vec<String> = (0..20).map(|i| format!("Final-{}", i)).collect();
    list_view
        .adapter_mut()
        .source_mut()
        .replace_rows(rows3, data3);
    list_view.process_invalidation();
    assert_eq!(list_view.row_count(), 20);

    // Substrate should have 20 rows.
    assert_eq!(list_view.substrate().row_model().len(), 20);
}

/// Test: Empty data source creates empty ListView.
#[test]
fn test_empty_data_source() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(0);
    let list_view = ListView::new(source);

    assert_eq!(list_view.row_count(), 0);
    assert_eq!(list_view.substrate().row_model().len(), 0);
    assert!(list_view.adapter().cached_row_ids().is_empty());
}

// =============================================================================
// RowHost Lifecycle Tests
// =============================================================================

/// Test: RowHost creates entries when realize_row_host() is called.
///
/// This verifies that the RowHost lifecycle is working: calling realize_row_host()
/// populates the RowHost with RowWidgetState entries for realized rows.
#[test]
fn test_row_host_lifecycle_creates_entries() {
    use crate::core::layout::{Constraints, LayoutResult};
    use crate::core::math::RectF;
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(10);
    let mut list_view = ListView::new(source);

    // Before realize: row_host should be empty.
    assert_eq!(list_view.debug_row_host_count(), 0);

    // Configure substrate viewport to realize rows.
    // We need to set viewport and prepare_evaluation to realize rows.
    let viewport = RectF::new(0.0, 0.0, 200.0, 300.0);
    list_view.substrate_mut().set_body_viewport(viewport);
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);

    // Now call realize_row_host to populate RowHost.
    list_view.realize_row_host();

    // After realize: row_host should have entries for realized rows.
    // With 10 rows at 24px height, 300px viewport should realize all 10.
    let host_count = list_view.debug_row_host_count();
    assert!(host_count > 0, "RowHost should have entries after realize");
    assert_eq!(host_count, 10, "All 10 rows should be realized in viewport");
}

/// Test: RowHost is cleared on BaseOrderReplaced.
#[test]
fn test_row_host_cleared_on_base_order_replaced() {
    use crate::core::math::RectF;
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    // Realize rows.
    let viewport = RectF::new(0.0, 0.0, 200.0, 200.0);
    list_view.substrate_mut().set_body_viewport(viewport);
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);
    list_view.realize_row_host();

    assert_eq!(list_view.debug_row_host_count(), 5);

    // Trigger BaseOrderReplaced.
    let new_rows: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    let new_data: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
    list_view
        .adapter_mut()
        .source_mut()
        .replace_rows(new_rows, new_data);
    list_view.process_invalidation();

    // RowHost should be cleared.
    assert_eq!(
        list_view.debug_row_host_count(),
        0,
        "RowHost should be cleared on BaseOrderReplaced"
    );

    // Re-realize to get new entries.
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);
    list_view.realize_row_host();
    assert_eq!(
        list_view.debug_row_host_count(),
        3,
        "RowHost should have 3 entries after re-realize"
    );
}

// =============================================================================
// Invalidation → Rebind Tests
// =============================================================================

/// Test: CellsInvalidated marks affected rows for rebind.
#[test]
fn test_cells_invalidated_marks_rows_for_rebind() {
    use crate::core::math::RectF;
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    // Realize rows.
    let viewport = RectF::new(0.0, 0.0, 200.0, 200.0);
    list_view.substrate_mut().set_body_viewport(viewport);
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);
    list_view.realize_row_host();

    // Get some row IDs to invalidate.
    let row_ids = list_view.adapter().cached_row_ids().to_vec();
    let rows_to_invalidate = vec![row_ids[1], row_ids[3]];

    // Emit CellsInvalidated.
    list_view
        .adapter_mut()
        .source_mut()
        .invalidate_cells(rows_to_invalidate);

    // Process invalidation.
    let result = list_view.process_invalidation();
    assert!(result.cells_invalidated, "cells_invalidated should be true");
    assert!(
        !result.base_order_replaced,
        "base_order_replaced should be false"
    );

    // The rows should be marked for rebind internally.
    // We can verify this indirectly: realize_row_host should clear rebind flags.
    // (In a real implementation, we'd check needs_rebind before calling rebind_complete.)
    // For this skeleton test, we just verify the invalidation was processed without error.
}

/// Test: PolicyInvalidated marks rows for rebind.
#[test]
fn test_policy_invalidated_marks_rows_for_rebind() {
    use crate::core::math::RectF;
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    // Realize rows.
    let viewport = RectF::new(0.0, 0.0, 200.0, 200.0);
    list_view.substrate_mut().set_body_viewport(viewport);
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);
    list_view.realize_row_host();

    // Emit PolicyInvalidated (all).
    list_view.adapter_mut().source_mut().invalidate_policy_all();

    // Process invalidation.
    let result = list_view.process_invalidation();
    assert!(
        result.policy_invalidated,
        "policy_invalidated should be true"
    );
    assert!(
        !result.base_order_replaced,
        "base_order_replaced should be false"
    );

    // The invalidation was processed; rows should be marked for rebind.
    // Calling realize_row_host again should clear rebind flags.
    list_view.realize_row_host();
}

// =============================================================================
// Selection Authority Tests
// =============================================================================
//
// These tests prove that TabularSubstrate is the SOLE semantic authority for
// selection. ListView is a dumb mirror: it forwards intents and reads state
// from substrate, but owns NO selection state of its own.
//
// Key invariants:
// - Selection state lives in TabularSubstrate.selected_rows()
// - ListView has NO selected_rows field, NO selection cache
// - All selection changes flow through TabularIntent::Selection → substrate.handle_intent()
// - ListView reflects selection only via substrate queries
// =============================================================================

/// Test: SelectRow intent changes selection in substrate.
///
/// Proves: Selection changes via intent → substrate, not via ListView.
#[test]
fn test_selection_intent_select_row() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    let row_ids = list_view.adapter().cached_row_ids().to_vec();
    let target_row = row_ids[2];

    // Initial state: no selection.
    assert!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows()
            .is_empty(),
        "Initially no rows selected"
    );

    // Send SelectRow intent through substrate (as ListView would).
    let intent = TabularIntent::Selection(SelectionIntent::SelectRow { row_id: target_row });
    list_view.substrate_mut().handle_intent(&intent);

    // Selection state is in substrate.
    let selected = list_view
        .substrate()
        .legacy_selection_state()
        .selected_rows();
    assert_eq!(selected.len(), 1, "One row should be selected");
    assert_eq!(selected[0], target_row, "Correct row should be selected");

    // ListView has NO selection state of its own.
    // There is NO list_view.selected_rows() method.
    // There is NO list_view.selection field.
    // The ONLY way to query selection is via substrate.
}

/// Test: Multiple SelectRow intents replace selection (single-select behavior).
#[test]
fn test_selection_intent_select_replaces_previous() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    let row_ids = list_view.adapter().cached_row_ids().to_vec();

    // Select first row.
    let intent1 = TabularIntent::Selection(SelectionIntent::SelectRow { row_id: row_ids[0] });
    list_view.substrate_mut().handle_intent(&intent1);
    assert_eq!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows(),
        &[row_ids[0]]
    );

    // Select third row - should replace, not add.
    let intent2 = TabularIntent::Selection(SelectionIntent::SelectRow { row_id: row_ids[2] });
    list_view.substrate_mut().handle_intent(&intent2);

    let selected = list_view
        .substrate()
        .legacy_selection_state()
        .selected_rows();
    assert_eq!(selected.len(), 1, "Selection should be replaced, not added");
    assert_eq!(selected[0], row_ids[2], "Third row should now be selected");
}

/// Test: ToggleRow intent toggles selection state.
#[test]
fn test_selection_intent_toggle_row() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    let row_ids = list_view.adapter().cached_row_ids().to_vec();

    // Toggle first row (add to selection).
    let toggle1 = TabularIntent::Selection(SelectionIntent::ToggleRow { row_id: row_ids[0] });
    list_view.substrate_mut().handle_intent(&toggle1);
    assert!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows()
            .contains(&row_ids[0]),
        "Row should be selected after toggle"
    );

    // Toggle second row (add to selection).
    let toggle2 = TabularIntent::Selection(SelectionIntent::ToggleRow { row_id: row_ids[1] });
    list_view.substrate_mut().handle_intent(&toggle2);
    assert_eq!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows()
            .len(),
        2,
        "Two rows should be selected"
    );

    // Toggle first row again (remove from selection).
    list_view.substrate_mut().handle_intent(&toggle1);
    let selected = list_view
        .substrate()
        .legacy_selection_state()
        .selected_rows();
    assert_eq!(selected.len(), 1, "One row should remain selected");
    assert!(
        !selected.contains(&row_ids[0]),
        "First row should be deselected"
    );
    assert!(
        selected.contains(&row_ids[1]),
        "Second row should still be selected"
    );
}

/// Test: ClearSelection intent clears all selection.
#[test]
fn test_selection_intent_clear_selection() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    let row_ids = list_view.adapter().cached_row_ids().to_vec();

    // Select multiple rows.
    for row_id in &row_ids[0..3] {
        let toggle = TabularIntent::Selection(SelectionIntent::ToggleRow { row_id: *row_id });
        list_view.substrate_mut().handle_intent(&toggle);
    }
    assert_eq!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows()
            .len(),
        3
    );

    // Clear selection.
    let clear = TabularIntent::Selection(SelectionIntent::ClearSelection);
    list_view.substrate_mut().handle_intent(&clear);

    assert!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows()
            .is_empty(),
        "Selection should be empty after clear"
    );
}

/// Test: ExtendToRow intent for range selection.
#[test]
fn test_selection_intent_extend_to_row() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    let row_ids = list_view.adapter().cached_row_ids().to_vec();

    // Select first row (establishes anchor).
    let select = TabularIntent::Selection(SelectionIntent::SelectRow { row_id: row_ids[0] });
    list_view.substrate_mut().handle_intent(&select);

    // Extend to third row.
    let extend = TabularIntent::Selection(SelectionIntent::ExtendToRow { row_id: row_ids[2] });
    list_view.substrate_mut().handle_intent(&extend);

    // Should select range from anchor to target.
    let selected = list_view
        .substrate()
        .legacy_selection_state()
        .selected_rows();
    assert!(
        selected.len() >= 2,
        "Range selection should include multiple rows"
    );
    assert!(
        selected.contains(&row_ids[0]),
        "Anchor should be in selection"
    );
    assert!(
        selected.contains(&row_ids[2]),
        "Target should be in selection"
    );
}

/// Test: ListView has NO selection state - it's a dumb mirror.
///
/// This test documents that ListView does NOT have:
/// - selected_rows field
/// - selection_anchor field
/// - any selection cache
/// - any selection logic
///
/// All selection state is in TabularSubstrate.
#[test]
fn test_listview_has_no_selection_state() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(5);
    let mut list_view = ListView::new(source);

    let row_ids = list_view.adapter().cached_row_ids().to_vec();

    // Mutate selection via substrate.
    let intent = TabularIntent::Selection(SelectionIntent::SelectRow { row_id: row_ids[1] });
    list_view.substrate_mut().handle_intent(&intent);

    // The ONLY way to observe selection is via substrate.
    // ListView is a pass-through; it has NO selection field.
    //
    // If someone adds list_view.selected_rows(), this test should fail compilation
    // or require explicit SPEC justification.
    let substrate_selection = list_view
        .substrate()
        .legacy_selection_state()
        .selected_rows();
    assert_eq!(substrate_selection, &[row_ids[1]]);

    // There is NO:
    // - list_view.selected_rows()
    // - list_view.selection_anchor()
    // - list_view.is_selected(row_id)
    // - list_view.selection_count()
    //
    // All such queries go through substrate().
}

/// Test: Selection survives across multiple operations (substrate is stateful).
#[test]
fn test_selection_state_persists_in_substrate() {
    use crate::widgets::list_view::ListView;

    let source = TestDataSource::with_rows(10);
    let mut list_view = ListView::new(source);

    let row_ids = list_view.adapter().cached_row_ids().to_vec();

    // Build up selection through multiple operations.
    list_view
        .substrate_mut()
        .handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
            row_id: row_ids[0],
        }));
    list_view
        .substrate_mut()
        .handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
            row_id: row_ids[3],
        }));
    list_view
        .substrate_mut()
        .handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
            row_id: row_ids[7],
        }));

    // Query multiple times - state should be consistent.
    let sel1 = list_view
        .substrate()
        .legacy_selection_state()
        .selected_rows()
        .to_vec();
    let sel2 = list_view
        .substrate()
        .legacy_selection_state()
        .selected_rows()
        .to_vec();
    assert_eq!(sel1, sel2, "Selection should be consistent across queries");

    // State persists (substrate is the authority).
    assert!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows()
            .contains(&row_ids[3])
    );
    assert!(
        list_view
            .substrate()
            .legacy_selection_state()
            .selected_rows()
            .contains(&row_ids[7])
    );
}

// =============================================================================
// Scroll + BaseOrderReplaced Edge Case Tests
// =============================================================================

/// Test: ListView realizes rows after BaseOrderReplaced with smaller dataset.
///
/// BUG REPRO: When scroll_y exceeds new content height, ListView fails to
/// realize any rows, resulting in empty paint despite data existing.
///
/// This test must FAIL on current code until the bug is fixed.
#[test]
fn test_scroll_position_after_base_order_replaced_with_smaller_dataset() {
    use crate::core::math::RectF;
    use crate::widgets::list_view::ListView;

    // Create ListView with many rows.
    let source = TestDataSource::with_rows(1000);
    let mut list_view = ListView::new(source);

    // Configure viewport.
    let viewport = RectF::new(0.0, 0.0, 200.0, 300.0);
    list_view.substrate_mut().set_body_viewport(viewport);
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);

    // Scroll deep into the list (row height = 24px, so 1000 rows = 24000px content).
    // Scroll to position 20000px (near the end).
    list_view.set_scroll_y(20000.0);
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);
    list_view.realize_row_host();

    // Verify we have realized rows at deep scroll position.
    let deep_scroll_count = list_view.debug_row_host_count();
    assert!(
        deep_scroll_count > 0,
        "Should have realized rows at deep scroll"
    );

    // Now trigger BaseOrderReplaced with a MUCH smaller dataset (only 5 rows).
    // Content height becomes 5 * 24px = 120px.
    // But scroll_y is still 20000px.
    let new_rows: Vec<RowId> = (0..5).map(|_| RowId::new()).collect();
    let new_data: Vec<String> = vec!["A".into(), "B".into(), "C".into(), "D".into(), "E".into()];
    list_view
        .adapter_mut()
        .source_mut()
        .replace_rows(new_rows, new_data);

    // Process the invalidation.
    let result = list_view.process_invalidation();
    assert!(result.base_order_replaced, "Should be BaseOrderReplaced");

    // Re-prepare substrate evaluation after rebuild.
    list_view.substrate_mut().prepare_evaluation(200.0, 1.0);

    // Realize rows after BaseOrderReplaced.
    list_view.realize_row_host();

    // BUG: Current code fails here because scroll_y (20000) >> content_height (120).
    // Realization finds no visible rows since all content is above viewport.
    //
    // EXPECTED: At least one row should be realized after identity death + rebuild.
    // The scroll position should be clamped or reset so data is visible.
    let row_count_after = list_view.debug_row_host_count();
    assert!(
        row_count_after > 0,
        "BUG: After BaseOrderReplaced with smaller dataset, scroll_y ({}) exceeds \
         new content height. ListView realizes 0 rows despite 5 rows existing. \
         Scroll must be clamped or reset on identity death.",
        list_view.scroll_y()
    );

    // Additional verification: all 5 new rows should be realizable in the viewport.
    // (5 rows * 24px = 120px fits in 300px viewport)
    assert_eq!(
        row_count_after, 5,
        "All 5 rows should be realized after scroll correction"
    );
}
