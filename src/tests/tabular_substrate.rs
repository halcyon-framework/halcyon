// =============================================================================
// TabularSubstrate Substrate Invariant Tests (White-Box)
// =============================================================================
//
// SPEC-HAL-164 substrate verification: 10k-row scroll/hit-test/navigation/selection.
//
// These are ENGINE-INTERNAL tests requiring pub(crate) access.
// For black-box API tests, see tests/tabular_substrate.rs
//
// Invariants verified:
// - [A] Coordinate & Scroll: hit-testing consistent across scroll positions
// - [B] Virtualization: selection/focus on RowId, not realized index
// - [C] Focus & Navigation: deterministic, no drift
// - [D] Selection: independent of focus, survives scroll
//
// LEGACY READ — selection assertions use selection_state() (deprecated mirror).
// Will migrate to ordered_selection_state accessors when available.
// =============================================================================

use crate::core::math::RectF;
use crate::widgets::tabular_view::TabularSubstrate;
use crate::widgets::tabular_view::column::Column;
use crate::widgets::tabular_view::intents::{
    FocusIntent, NavigationDirection, ReorderIntent, ResizeIntent, SelectionIntent, TabularIntent,
};
use crate::widgets::tabular_view::row::RowId;

const ROW_COUNT: usize = 10_000;
const ROW_HEIGHT: f32 = 24.0;
const VIEWPORT_HEIGHT: f32 = 480.0; // 20 rows visible
const HEADER_HEIGHT: f32 = 30.0;
const COLUMN_WIDTH: f32 = 200.0;

/// Create a TabularSubstrate with 10,000 rows, single column, fixed height.
fn create_substrate() -> (TabularSubstrate, Vec<RowId>) {
    let mut view = TabularSubstrate::new();

    // Single column
    view.add_column(Column::new(COLUMN_WIDTH));

    // 10,000 rows with fixed height (default 24px)
    let row_ids: Vec<RowId> = (0..ROW_COUNT).map(|_| RowId::new()).collect();
    for &id in &row_ids {
        view.add_row(id);
    }

    // Set viewport (header + body area)
    view.body_mut().set_viewport(RectF::new(
        0.0,
        HEADER_HEIGHT,
        COLUMN_WIDTH,
        VIEWPORT_HEIGHT,
    ));

    // Initialize sorted indices (natural order)
    view.set_sorted_indices((0..ROW_COUNT).collect());

    (view, row_ids)
}

/// Realize the viewport for the current scroll position.
fn realize(view: &mut TabularSubstrate) {
    view.test_begin_evaluation();
    view.test_resolve_geometry(COLUMN_WIDTH);
    view.test_realize_viewport();
}

// =============================================================================
// INV-A: Coordinate & Scroll Invariants
// =============================================================================

#[test]
fn inv_a1_substrate_setup() {
    let (view, row_ids) = create_substrate();

    assert_eq!(view.body_ref().row_model().len(), ROW_COUNT);
    assert_eq!(row_ids.len(), ROW_COUNT);
    assert_eq!(view.column_model().columns().len(), 1);

    let expected_height = ROW_COUNT as f32 * ROW_HEIGHT;
    assert_eq!(view.body_ref().content_height(), expected_height);
}

#[test]
fn inv_a2_scroll_hit_test_consistency() {
    let (mut view, row_ids) = create_substrate();

    let scroll_positions: [f32; 5] = [0.0, 1000.0, 5000.0, 100_000.0, 239_000.0];

    for &scroll_y in &scroll_positions {
        let max_scroll = (ROW_COUNT as f32 * ROW_HEIGHT - VIEWPORT_HEIGHT).max(0.0);
        let scroll_y: f32 = scroll_y.min(max_scroll);

        view.body_mut().set_scroll_y(scroll_y);
        realize(&mut view);

        let expected_first_row = (scroll_y / ROW_HEIGHT).floor() as usize;
        let test_y = HEADER_HEIGHT + 12.0;
        let hit_result = view.body_ref().hit_test(COLUMN_WIDTH / 2.0, test_y);

        assert!(
            hit_result.is_some(),
            "No hit at scroll_y={}, test_y={}",
            scroll_y,
            test_y
        );

        let cell_id = hit_result.unwrap();
        let hit_row_index = row_ids
            .iter()
            .position(|&r| r == cell_id.row_id)
            .expect("Hit row not in row_ids");

        assert!(
            hit_row_index >= expected_first_row && hit_row_index <= expected_first_row + 1,
            "scroll_y={}, expected row ~{}, got row {}",
            scroll_y,
            expected_first_row,
            hit_row_index
        );
    }
}

// inv_a3_hit_test_tree_consistency: REMOVED
// Tree-aware hit-testing is now handled by TreeView (SPEC-HAL-259).
// TabularSubstrate is flat-only; no tree_projection_ref() exists.

// =============================================================================
// INV-C: Focus & Navigation Invariants
// =============================================================================

#[test]
fn inv_c1_navigation_no_drift() {
    let (mut view, row_ids) = create_substrate();

    realize(&mut view);

    // Navigate down 100 times from no focus
    for i in 0..100 {
        let nav_intent = TabularIntent::Focus(FocusIntent::RequestNavigate {
            from: view
                .focus_state()
                .focused()
                .map(|c| (c.row_id, c.column_id)),
            direction: NavigationDirection::Down,
        });
        view.handle_intent(&nav_intent);

        let focused = view
            .focus_state()
            .focused()
            .expect("Focus lost after navigation");
        let focus_row_index = row_ids.iter().position(|&r| r == focused.row_id).unwrap();

        // First Down from None goes to row 1. After (i+1) navigations, focus at row (i+1).
        let expected_row = i + 1;
        assert_eq!(
            focus_row_index,
            expected_row,
            "After {} navigations, at row {} (expected {})",
            i + 1,
            focus_row_index,
            expected_row
        );
    }

    // Navigate up 50 times
    for _ in 0..50 {
        let nav_intent = TabularIntent::Focus(FocusIntent::RequestNavigate {
            from: view
                .focus_state()
                .focused()
                .map(|c| (c.row_id, c.column_id)),
            direction: NavigationDirection::Up,
        });
        view.handle_intent(&nav_intent);
    }

    let after_up = view.focus_state().focused().unwrap();
    let after_up_row = row_ids.iter().position(|&r| r == after_up.row_id).unwrap();
    assert_eq!(
        after_up_row, 50,
        "Expected row 50, got row {}",
        after_up_row
    );
}

#[test]
fn inv_c2_navigation_boundaries() {
    let (mut view, row_ids) = create_substrate();
    let col_id = view.column_model().columns()[0].id;

    realize(&mut view);

    // Focus first row, try Up
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[0],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: view
            .focus_state()
            .focused()
            .map(|c| (c.row_id, c.column_id)),
        direction: NavigationDirection::Up,
    }));
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        row_ids[0],
        "Focus should clamp at first row"
    );

    // Focus last row, try Down
    let last_idx = ROW_COUNT - 1;
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[last_idx],
        column_id: col_id,
    }));
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: view
            .focus_state()
            .focused()
            .map(|c| (c.row_id, c.column_id)),
        direction: NavigationDirection::Down,
    }));
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        row_ids[last_idx],
        "Focus should clamp at last row"
    );
}

#[test]
fn inv_c3_page_navigation() {
    let (mut view, row_ids) = create_substrate();
    let col_id = view.column_model().columns()[0].id;

    realize(&mut view);

    // Focus row 500
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[500],
        column_id: col_id,
    }));

    let page_size = view.page_size();

    // PageDown
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: view
            .focus_state()
            .focused()
            .map(|c| (c.row_id, c.column_id)),
        direction: NavigationDirection::PageDown,
    }));

    let after_pagedown = view.focus_state().focused().unwrap().row_id;
    let after_pagedown_idx = row_ids.iter().position(|&r| r == after_pagedown).unwrap();
    assert_eq!(
        after_pagedown_idx,
        500 + page_size,
        "PageDown should move {} rows",
        page_size
    );

    // PageUp
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: view
            .focus_state()
            .focused()
            .map(|c| (c.row_id, c.column_id)),
        direction: NavigationDirection::PageUp,
    }));

    let after_pageup = view.focus_state().focused().unwrap().row_id;
    let after_pageup_idx = row_ids.iter().position(|&r| r == after_pageup).unwrap();
    assert_eq!(after_pageup_idx, 500, "PageUp should return to row 500");
}

// =============================================================================
// INV-D: Selection Invariants
// =============================================================================

#[test]
fn inv_d1_click_to_select() {
    let (mut view, row_ids) = create_substrate();

    let scroll_y = 5000.0 * ROW_HEIGHT;
    view.body_mut().set_scroll_y(scroll_y);
    realize(&mut view);

    let test_y = HEADER_HEIGHT + 12.0;
    let hit_result = view
        .body_ref()
        .hit_test(COLUMN_WIDTH / 2.0, test_y)
        .expect("No hit");
    let hit_row_id = hit_result.row_id;

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: hit_row_id,
    }));

    assert!(
        view.legacy_selection_state().is_selected(hit_row_id),
        "Row not selected after SelectRow intent"
    );
    assert_eq!(
        view.legacy_selection_state().selected_rows().len(),
        1,
        "Expected 1 selected row"
    );

    // Selection survives scroll
    view.body_mut().set_scroll_y(0.0);
    realize(&mut view);
    assert!(
        view.legacy_selection_state().is_selected(hit_row_id),
        "Selection lost after scroll"
    );
}

#[test]
fn inv_d2_selection_focus_independence() {
    let (mut view, row_ids) = create_substrate();
    let col_id = view.column_model().columns()[0].id;

    realize(&mut view);

    // Select row 100
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[100],
    }));

    // Focus row 200
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[200],
        column_id: col_id,
    }));

    assert!(
        view.legacy_selection_state().is_selected(row_ids[100]),
        "Selection lost"
    );
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        row_ids[200],
        "Focus wrong"
    );

    // Navigate focus 10 times
    for _ in 0..10 {
        view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
            from: view
                .focus_state()
                .focused()
                .map(|c| (c.row_id, c.column_id)),
            direction: NavigationDirection::Down,
        }));
    }

    // Selection still at row 100
    assert!(
        view.legacy_selection_state().is_selected(row_ids[100]),
        "Selection changed after focus navigation"
    );

    let focus_row = view.focus_state().focused().unwrap().row_id;
    let focus_idx = row_ids.iter().position(|&r| r == focus_row).unwrap();
    assert!(
        focus_idx >= 205 && focus_idx <= 215,
        "Focus at wrong row {} (expected ~210)",
        focus_idx
    );
}

#[test]
fn inv_d3_range_selection() {
    let (mut view, row_ids) = create_substrate();

    realize(&mut view);

    // Select row 50 (sets anchor)
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[50],
    }));

    // Extend to row 60
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: row_ids[60],
    }));

    // Should have rows 50-60 selected (11 rows)
    let selected = view.legacy_selection_state().selected_rows();
    assert_eq!(
        selected.len(),
        11,
        "Expected 11 selected rows, got {}",
        selected.len()
    );

    for i in 50..=60 {
        assert!(
            view.legacy_selection_state().is_selected(row_ids[i]),
            "Row {} not selected in range",
            i
        );
    }
}

// =============================================================================
// INV-E: BaseRowOrder Epoch Invariants (INV-ROW-ORDER-EPOCH-1 through EPOCH-4)
// =============================================================================
//
// Tests validate that:
// 1. Epoch increments on structural row mutations (test_only accessor)
// 2. effective_row_order() is self-healing (no panic even without precompute)
// 3. Navigation and selection work correctly after base-order mutations
// =============================================================================

#[test]
fn inv_e1_epoch_increments_on_add_row() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(COLUMN_WIDTH));

    let initial_epoch = view.base_row_order_epoch_for_test();
    assert!(initial_epoch > 0, "Epoch should start at 1, not 0");

    // Add rows - each should increment epoch
    for i in 0..5 {
        let before = view.base_row_order_epoch_for_test();
        view.add_row(RowId::new());
        let after = view.base_row_order_epoch_for_test();
        assert_eq!(
            after,
            before + 1,
            "Epoch should increment by 1 on add_row (iteration {})",
            i
        );
    }
}

#[test]
fn inv_e2_epoch_increments_on_clear_rows() {
    let (mut view, _row_ids) = create_substrate();
    realize(&mut view);

    let before = view.base_row_order_epoch_for_test();
    view.clear_rows();
    let after = view.base_row_order_epoch_for_test();

    assert_eq!(
        after,
        before + 1,
        "Epoch should increment by 1 on clear_rows"
    );
}

#[test]
fn inv_e3_effective_row_order_self_healing() {
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(COLUMN_WIDTH));

    // Add rows
    let row_ids: Vec<RowId> = (0..100).map(|_| RowId::new()).collect();
    for &id in &row_ids {
        view.add_row(id);
    }

    // Set viewport
    view.body_mut().set_viewport(RectF::new(
        0.0,
        HEADER_HEIGHT,
        COLUMN_WIDTH,
        VIEWPORT_HEIGHT,
    ));

    // DO NOT call realize() or set_sorted_indices() - test self-healing
    // effective_row_order() should recompute internally and NOT panic
    let order = view.effective_row_order();
    assert_eq!(
        order.len(),
        100,
        "Self-healing: effective_row_order should return valid order even without precompute"
    );
}

#[test]
fn inv_e4_selection_survives_base_order_mutation() {
    let (mut view, row_ids) = create_substrate();
    realize(&mut view);

    // Select some rows
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[500],
    }));
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ToggleRow {
        row_id: row_ids[1000],
    }));

    assert!(view.legacy_selection_state().is_selected(row_ids[500]));
    assert!(view.legacy_selection_state().is_selected(row_ids[1000]));

    // Add more rows (mutates base order, invalidates epoch)
    for _ in 0..10 {
        view.add_row(RowId::new());
    }

    // DO NOT manually update sorted_indices — test self-healing
    // Navigation should work because effective_row_order() auto-recomputes

    // Selection should survive - it's identity-based (RowId), not index-based
    assert!(
        view.legacy_selection_state().is_selected(row_ids[500]),
        "Selection lost after base order mutation"
    );
    assert!(
        view.legacy_selection_state().is_selected(row_ids[1000]),
        "Selection lost after base order mutation"
    );
}

#[test]
fn inv_e5_focus_survives_base_order_mutation() {
    let (mut view, row_ids) = create_substrate();
    let col_id = view.column_model().columns()[0].id;
    realize(&mut view);

    // Focus a row
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[750],
        column_id: col_id,
    }));

    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        row_ids[750],
        "Focus should be on row 750"
    );

    // Add more rows (mutates base order, invalidates epoch)
    for _ in 0..10 {
        view.add_row(RowId::new());
    }

    // DO NOT manually update sorted_indices — test self-healing
    // Focus should survive - it's identity-based (RowId), not index-based
    assert_eq!(
        view.focus_state().focused().unwrap().row_id,
        row_ids[750],
        "Focus lost after base order mutation"
    );
}

#[test]
fn inv_e6_navigation_uses_effective_order() {
    let (mut view, row_ids) = create_substrate();
    let col_id = view.column_model().columns()[0].id;
    realize(&mut view);

    // Focus row 100
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestFocus {
        row_id: row_ids[100],
        column_id: col_id,
    }));

    // Navigate down
    view.handle_intent(&TabularIntent::Focus(FocusIntent::RequestNavigate {
        from: view
            .focus_state()
            .focused()
            .map(|c| (c.row_id, c.column_id)),
        direction: NavigationDirection::Down,
    }));

    let after_nav = view.focus_state().focused().unwrap().row_id;
    let after_nav_idx = row_ids.iter().position(|&r| r == after_nav).unwrap();

    // In natural sorted order, down from 100 should go to 101
    assert_eq!(
        after_nav_idx, 101,
        "Navigation should use effective_row_order (expected 101, got {})",
        after_nav_idx
    );
}

#[test]
fn inv_e7_range_selection_uses_effective_order() {
    let (mut view, row_ids) = create_substrate();
    realize(&mut view);

    // Select row 200 (sets anchor)
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[200],
    }));

    // Extend to row 205 - should select 200-205 in effective order
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: row_ids[205],
    }));

    // Verify range 200-205 is selected (6 rows)
    let selected = view.legacy_selection_state().selected_rows();
    assert_eq!(
        selected.len(),
        6,
        "Expected 6 rows selected in range, got {}",
        selected.len()
    );

    for i in 200..=205 {
        assert!(
            view.legacy_selection_state().is_selected(row_ids[i]),
            "Row {} should be selected in range",
            i
        );
    }
}

// =============================================================================
// SPEC-HAL-255: Geometry Lifecycle Isolation Proof
// =============================================================================

/// A) Stable frames: full resolve == 1, short-circuits accumulate.
#[test]
#[cfg(debug_assertions)]
fn geometry_isolation_stable_frames() {
    let (mut view, _row_ids) = create_substrate();

    // First realize: full resolve (initial, resolved=false → must compute).
    realize(&mut view);
    let (full, short) = view.debug_geometry_counters();
    assert_eq!(full, 1, "First realize must trigger exactly 1 full resolve");
    assert_eq!(short, 0, "No short-circuits on first resolve");

    // Subsequent realizes with no changes: all short-circuit.
    for i in 1..=10 {
        realize(&mut view);
        let (full, short) = view.debug_geometry_counters();
        assert_eq!(full, 1, "Stable frame {i}: full_resolve_count must stay 1");
        assert_eq!(
            short, i,
            "Stable frame {i}: short_circuit_count must be {i}"
        );
    }
}

/// B) Scroll does NOT trigger geometry recompute.
#[test]
#[cfg(debug_assertions)]
fn geometry_isolation_scroll() {
    let (mut view, _row_ids) = create_substrate();
    realize(&mut view);
    let (baseline_full, _) = view.debug_geometry_counters();

    // Scroll to various positions and re-realize.
    for offset in [100.0, 500.0, 2000.0, 0.0] {
        view.body_mut().set_scroll_y(offset);
        realize(&mut view);
    }

    let (full, _) = view.debug_geometry_counters();
    assert_eq!(
        full, baseline_full,
        "Scroll must not trigger geometry recompute (full_resolve stayed at {baseline_full})"
    );
}

/// C) Selection does NOT trigger geometry recompute.
#[test]
#[cfg(debug_assertions)]
fn geometry_isolation_selection() {
    let (mut view, row_ids) = create_substrate();
    realize(&mut view);
    let (baseline_full, _) = view.debug_geometry_counters();

    // Apply selection intents.
    view.handle_intent(&TabularIntent::Selection(SelectionIntent::SelectRow {
        row_id: row_ids[50],
    }));
    realize(&mut view);

    view.handle_intent(&TabularIntent::Selection(SelectionIntent::ExtendToRow {
        row_id: row_ids[100],
    }));
    realize(&mut view);

    let (full, _) = view.debug_geometry_counters();
    assert_eq!(
        full, baseline_full,
        "Selection must not trigger geometry recompute"
    );
}

/// D) Column resize DOES trigger geometry recompute.
#[test]
#[cfg(debug_assertions)]
fn geometry_isolation_resize_triggers() {
    let (mut view, _row_ids) = create_substrate();
    realize(&mut view);
    let (baseline_full, _) = view.debug_geometry_counters();

    // Get first column ID.
    let col_id = {
        let cols: Vec<_> = view.geometry_view().header_slots().collect();
        cols[0].column_id
    };

    // Resize via intent.
    view.handle_intent(&TabularIntent::Resize(ResizeIntent::Commit {
        column_id: col_id,
        proposed_width: 300.0,
    }));
    realize(&mut view);

    let (full, _) = view.debug_geometry_counters();
    assert_eq!(
        full,
        baseline_full + 1,
        "Resize commit must trigger exactly 1 additional full resolve"
    );
}

/// E) Container width change DOES trigger geometry recompute.
#[test]
#[cfg(debug_assertions)]
fn geometry_isolation_width_change_triggers() {
    let (mut view, _row_ids) = create_substrate();
    realize(&mut view);
    let (baseline_full, _) = view.debug_geometry_counters();

    // Resolve with different available_width.
    view.test_begin_evaluation();
    view.test_resolve_geometry(COLUMN_WIDTH + 100.0);
    view.test_realize_viewport();

    let (full, _) = view.debug_geometry_counters();
    assert_eq!(
        full,
        baseline_full + 1,
        "Container width change must trigger exactly 1 additional full resolve"
    );
}

/// F) Column reorder DOES trigger geometry recompute.
#[test]
#[cfg(debug_assertions)]
fn geometry_isolation_reorder_triggers() {
    // Need 2+ columns for reorder.
    let mut view = TabularSubstrate::new();
    view.add_column(Column::new(100.0));
    view.add_column(Column::new(150.0));

    let row_ids: Vec<RowId> = (0..100).map(|_| RowId::new()).collect();
    for &id in &row_ids {
        view.add_row(id);
    }
    view.body_mut()
        .set_viewport(RectF::new(0.0, HEADER_HEIGHT, 250.0, VIEWPORT_HEIGHT));
    view.set_sorted_indices((0..100).collect());

    // First realize.
    view.test_begin_evaluation();
    view.test_resolve_geometry(250.0);
    view.test_realize_viewport();
    let (baseline_full, _) = view.debug_geometry_counters();
    assert_eq!(baseline_full, 1);

    // Get column IDs.
    let col_ids: Vec<_> = view
        .geometry_view()
        .header_slots()
        .map(|s| s.column_id)
        .collect();

    // Reorder: swap columns.
    view.handle_intent(&TabularIntent::Reorder(ReorderIntent::Apply {
        new_order: vec![col_ids[1], col_ids[0]],
    }));

    view.test_begin_evaluation();
    view.test_resolve_geometry(250.0);
    view.test_realize_viewport();

    let (full, _) = view.debug_geometry_counters();
    assert_eq!(
        full,
        baseline_full + 1,
        "Column reorder must trigger exactly 1 additional full resolve"
    );
}

// =============================================================================
// SPEC-HAL-258: Invalidation Graph Enforcement
// =============================================================================

/// Proves the engine dirty flag mechanism:
/// - Initial frame: geometry_dirty starts true → full recompute
/// - Stable frame without dirty flag → no full recompute (short-circuit)
/// - Model mutation (resize) sets dirty flag → exactly one full recompute
/// - After clearing dirty, stable again
/// - Non-geometry dirty flags (topology, virtualization, selection) do NOT
///   influence geometry recompute
///
/// SPEC-HAL-257 alignment: each frame drives the full evaluation protocol
/// (begin → resolve geometry → produce token → prepare_evaluation) so that
/// EvaluationMode is never None when prepare_evaluation checks it.
#[test]
#[cfg(debug_assertions)]
fn invalidation_graph_geometry_requires_dirty_flag() {
    let (mut view, _row_ids) = create_substrate();

    // Helper: drive one full evaluation frame via the token path.
    fn eval_frame(view: &mut TabularSubstrate, width: f32) {
        view.test_begin_evaluation();
        view.test_resolve_geometry(width);
        let scroll_y = view.body_ref().scroll_y();
        let _ = view.update_visible_range_from_scroll(scroll_y);
        view.prepare_evaluation(width, 1.0);
    }

    // Initial frame: geometry_dirty starts true → full recompute.
    eval_frame(&mut view, COLUMN_WIDTH);
    let (full, short) = view.debug_geometry_counters();
    assert_eq!(
        full, 1,
        "Initial resolve must trigger exactly 1 full recompute"
    );
    assert_eq!(short, 0, "No short-circuits on first resolve");

    // Stable frame: dirty flag cleared, same width → short-circuit.
    eval_frame(&mut view, COLUMN_WIDTH);
    let (full, short) = view.debug_geometry_counters();
    assert_eq!(full, 1, "Stable frame: no additional full recompute");
    assert_eq!(short, 1, "Stable frame: one short-circuit");

    // Trigger dirty via resize intent (mutates column model → sets geometry_dirty).
    let col_id = view
        .geometry_view()
        .header_slots()
        .next()
        .unwrap()
        .column_id;
    view.handle_intent(&TabularIntent::Resize(ResizeIntent::Commit {
        column_id: col_id,
        proposed_width: 300.0,
    }));

    // Dirty flag set → full recompute.
    eval_frame(&mut view, COLUMN_WIDTH);
    let (full, _) = view.debug_geometry_counters();
    assert_eq!(
        full, 2,
        "Dirty flag must trigger exactly 1 additional full recompute"
    );

    // Verify stable again after dirty cleared.
    eval_frame(&mut view, COLUMN_WIDTH);
    let (full, _) = view.debug_geometry_counters();
    assert_eq!(full, 2, "After clearing dirty, no more full recomputes");

    // Non-geometry dirty flags must NOT influence geometry recompute (INV-IG-3).
    view.test_set_topology_dirty(true);
    view.test_set_virtualization_dirty(true);
    view.test_set_selection_dirty(true);

    eval_frame(&mut view, COLUMN_WIDTH);
    let (full, _) = view.debug_geometry_counters();
    assert_eq!(
        full, 2,
        "Non-geometry dirty flags must not trigger geometry recompute"
    );
}

// =============================================================================
// Epoch-Bound Access Barrier Tests (INVARIANT-6)
// =============================================================================

/// Proves that geometry_view() panics if geometry was never resolved.
/// This is the INVARIANT-6 enforcement: geometry cannot be read before
/// resolution. The centralized assert_resolved_for_epoch() fires first.
#[test]
#[should_panic(expected = "accessed before resolve()")]
fn geometry_barrier_panics_before_resolve() {
    let (mut view, _row_ids) = create_substrate();
    // Begin evaluation but do NOT call resolve_geometry.
    view.test_begin_evaluation();

    // This must panic: geometry was never resolved.
    let _ = view.geometry_view();
}

// =============================================================================
// SPEC-HAL-258: Substrate Gate Correctness
// =============================================================================

/// Proves the substrate gate works: clean frames use stamp_epoch() (short-circuit),
/// not resolve() (full recompute). The short_circuit_count increments while
/// full_resolve_count stays at 1.
///
/// SPEC-HAL-257 alignment: each frame drives the full evaluation protocol
/// (begin → resolve geometry → produce token → prepare_evaluation) so that
/// EvaluationMode is never None when prepare_evaluation checks it.
#[test]
#[cfg(debug_assertions)]
fn stable_frame_stamps_epoch_not_full_recompute() {
    let (mut view, _row_ids) = create_substrate();

    // First frame: geometry_dirty starts true → full recompute.
    view.test_begin_evaluation();
    view.test_resolve_geometry(COLUMN_WIDTH);
    let scroll_y = view.body_ref().scroll_y();
    let _ = view.update_visible_range_from_scroll(scroll_y);
    view.prepare_evaluation(COLUMN_WIDTH, 1.0);

    let (full, short) = view.debug_geometry_counters();
    assert_eq!(full, 1, "First frame must trigger exactly one full resolve");
    assert_eq!(
        short, 0,
        "First frame must not trigger any short-circuit stamps"
    );

    // Second frame: no mutations, same width → clean path (stamp_epoch).
    view.test_begin_evaluation();
    view.test_resolve_geometry(COLUMN_WIDTH);
    let scroll_y = view.body_ref().scroll_y();
    let _ = view.update_visible_range_from_scroll(scroll_y);
    view.prepare_evaluation(COLUMN_WIDTH, 1.0);

    let (full, short) = view.debug_geometry_counters();
    assert_eq!(
        full, 1,
        "Second frame must NOT trigger another full resolve"
    );
    assert_eq!(
        short, 1,
        "Second frame must stamp epoch via short-circuit path"
    );
}

// =============================================================================
// SPEC-HAL-257: Token Overwrite + Embedded Scroll Discipline Regression Tests
// =============================================================================

/// Test 1: Embedded ListView scroll does not produce tokens.
///
/// Construct a ListView-embedded substrate, call set_scroll_y_from_scroll_area
/// repeatedly with changing scroll_y. Assert:
/// - no panic
/// - visible_range remains None after each scroll
/// - virtualization_dirty becomes true when scroll changes
#[test]
fn embedded_scroll_does_not_produce_tokens() {
    let (mut view, _row_ids) = create_substrate();
    view.set_listview_embedded();

    // Initial realize to establish baseline.
    realize(&mut view);

    // Scroll to several positions via the non-token setter.
    for scroll_y in [100.0, 250.0, 0.0, 500.0, 100.0] {
        let changed = view.set_scroll_y_from_scroll_area(scroll_y);

        // Token must never be produced by the non-token path.
        assert!(
            !view.test_has_visible_range_token(),
            "set_scroll_y_from_scroll_area must not produce a VisibleRangeToken (scroll_y={scroll_y})"
        );

        // virtualization_dirty must be set when scroll changes.
        if changed {
            assert!(
                view.test_is_virtualization_dirty(),
                "virtualization_dirty must be true after scroll change (scroll_y={scroll_y})"
            );
        }

        // Simulate the layout pass that would follow (resets dirty flags).
        realize(&mut view);
    }

    // Same scroll value should not change anything.
    let changed = view.set_scroll_y_from_scroll_area(100.0);
    assert!(!changed, "Same scroll_y must return false");
    assert!(
        !view.test_has_visible_range_token(),
        "No token produced even on no-change scroll"
    );
}

/// Test 2: resolve_layout_geometry drains any stale token.
///
/// Force a token into visible_range on a non-embedded substrate, then
/// flip to embedded and call resolve_layout_geometry. Assert it drains
/// the token (visible_range is None afterward) without panic.
#[test]
fn resolve_layout_geometry_drains_stale_token() {
    let (mut view, _row_ids) = create_substrate();

    // Phase 1: Produce a token via the non-embedded token path.
    view.test_begin_evaluation();
    view.test_resolve_geometry(COLUMN_WIDTH);
    let _ = view.update_visible_range_from_scroll(0.0);
    assert!(
        view.test_has_visible_range_token(),
        "Token must be present after update_visible_range_from_scroll"
    );

    // Phase 2: Now mark as embedded and call resolve_layout_geometry.
    // This simulates the "stale token from previous frame" scenario.
    view.set_listview_embedded();
    view.resolve_layout_geometry(COLUMN_WIDTH, 1.0);

    // Token must be drained.
    assert!(
        !view.test_has_visible_range_token(),
        "resolve_layout_geometry must drain any pre-existing token"
    );

    // Rows must be realized (not empty).
    assert!(
        !view.body_ref().realized_rows().is_empty(),
        "resolve_layout_geometry must realize rows"
    );
}

/// Test 3: Standalone token path produces, consumes, and realizes.
///
/// Create a standalone TabularSubstrate (not embedded), call
/// update_visible_range_from_scroll once, then prepare_evaluation.
/// Assert token is consumed (visible_range None after) and rows are realized.
#[test]
fn standalone_token_path_produces_consumes_realizes() {
    let (mut view, _row_ids) = create_substrate();
    assert!(!view.is_embedded_in_listview(), "Must not be embedded");

    // Frame 1: Initial realize via direct path.
    realize(&mut view);

    // Frame 2: Token path — scroll sync then prepare_evaluation.
    view.test_begin_evaluation();
    view.test_resolve_geometry(COLUMN_WIDTH);

    // Produce token.
    let changed = view.update_visible_range_from_scroll(200.0);
    assert!(changed, "Scroll to new position must return true");
    assert!(
        view.test_has_visible_range_token(),
        "Token must be present after update_visible_range_from_scroll"
    );

    // Consume token via prepare_evaluation.
    view.prepare_evaluation(COLUMN_WIDTH, 1.0);
    assert!(
        !view.test_has_visible_range_token(),
        "Token must be consumed after prepare_evaluation"
    );

    // Rows must be realized.
    assert!(
        !view.body_ref().realized_rows().is_empty(),
        "prepare_evaluation must realize rows"
    );

    // Frame 3: Same scroll position — no change.
    view.test_begin_evaluation();
    view.test_resolve_geometry(COLUMN_WIDTH);
    let changed2 = view.update_visible_range_from_scroll(200.0);
    assert!(!changed2, "Same scroll position must not report change");
}
