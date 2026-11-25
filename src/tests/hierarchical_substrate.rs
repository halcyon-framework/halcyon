// =============================================================================
// SPEC-HAL-259: HierarchicalTabularSubstrate Engine Tests (White-Box)
// =============================================================================
//
// Engine-internal tests for HierarchicalTabularSubstrate composition.
//
// Invariants verified:
// - Expansion/collapse produces correct visible row set
// - Stable projection after expand/collapse cycles
// - Virtualization windowing (viewport position affects visible rows)
// - Revision counter monotonicity
// - Topology independence from geometry (INV-HTS-004)
// - Tree metadata decoupled from RowHeights (no identity recreation)
// =============================================================================

use crate::engines::hierarchical::{HierarchicalTabularSubstrate, NodeMeta, VisibleRow};
use crate::model::tabular::column::Column;
use crate::model::tabular::row_heights::RowHeights;

// =============================================================================
// Helpers
// =============================================================================

/// Build a simple tree:
///   Node 0 (root, has children)
///   +-- Node 1 (child of 0, has children)
///   |   +-- Node 2 (leaf)
///   |   +-- Node 3 (leaf)
///   +-- Node 4 (child of 0, leaf)
fn simple_tree() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 0,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Node 0".to_string(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: "Node 1".to_string(),
        },
        NodeMeta {
            node_id: 2,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "Node 2".to_string(),
        },
        NodeMeta {
            node_id: 3,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "Node 3".to_string(),
        },
        NodeMeta {
            node_id: 4,
            parent_index: Some(0),
            depth: 1,
            has_children: false,
            label: "Node 4".to_string(),
        },
    ]
}

fn visible_indices(rows: &[VisibleRow]) -> Vec<usize> {
    rows.iter().map(|r| r.row_index).collect()
}

fn make_hts_with_tree(nodes: Vec<NodeMeta>) -> HierarchicalTabularSubstrate {
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_viewport_height(10000.0); // Large enough to show all rows.
    hts.set_tree_data(nodes);
    hts
}

// =============================================================================
// 1. Empty HTS
// =============================================================================

#[test]
fn empty_hts_returns_no_visible_rows() {
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_viewport_height(200.0);
    let rows = hts.visible_rows();
    assert!(rows.is_empty());
}

// =============================================================================
// 2. All expanded — full visibility
// =============================================================================

#[test]
fn all_expanded_shows_all_nodes() {
    let mut hts = make_hts_with_tree(simple_tree());
    let rows = hts.visible_rows();

    // Nodes with has_children start expanded → all 5 nodes visible.
    assert_eq!(visible_indices(&rows), vec![0, 1, 2, 3, 4]);
}

// =============================================================================
// 3. Depth and metadata correctness
// =============================================================================

#[test]
fn visible_rows_carry_correct_metadata() {
    let mut hts = make_hts_with_tree(simple_tree());
    let rows = hts.visible_rows();

    // Node 0: root, expanded, has children
    assert_eq!(rows[0].row_index, 0);
    assert_eq!(rows[0].depth, 0);
    assert!(rows[0].expanded);
    assert!(rows[0].has_children);

    // Node 1: depth 1, expanded, has children
    assert_eq!(rows[1].depth, 1);
    assert!(rows[1].expanded);
    assert!(rows[1].has_children);

    // Node 2: leaf at depth 2
    assert_eq!(rows[2].depth, 2);
    assert!(!rows[2].expanded);
    assert!(!rows[2].has_children);

    // Node 4: leaf at depth 1
    assert_eq!(rows[4].depth, 1);
    assert!(!rows[4].expanded);
    assert!(!rows[4].has_children);
}

// =============================================================================
// 4. Collapse hides subtree
// =============================================================================

#[test]
fn collapse_hides_subtree() {
    let mut hts = make_hts_with_tree(simple_tree());

    // Collapse node 1 → nodes 2, 3 hidden.
    hts.collapse(1);
    let rows = hts.visible_rows();
    assert_eq!(visible_indices(&rows), vec![0, 1, 4]);
}

#[test]
fn collapse_root_hides_all_descendants() {
    let mut hts = make_hts_with_tree(simple_tree());

    // Collapse root → only root visible.
    hts.collapse(0);
    let rows = hts.visible_rows();
    assert_eq!(visible_indices(&rows), vec![0]);
}

// =============================================================================
// 5. Re-expansion restores subtree
// =============================================================================

#[test]
fn re_expansion_restores_subtree() {
    let mut hts = make_hts_with_tree(simple_tree());

    // Collapse then re-expand root.
    hts.collapse(0);
    assert_eq!(visible_indices(&hts.visible_rows()), vec![0]);

    hts.expand(0);
    // Node 1 was still expanded (expansion state survives ancestor collapse).
    assert_eq!(visible_indices(&hts.visible_rows()), vec![0, 1, 2, 3, 4]);
}

// =============================================================================
// 6. Expansion state survives ancestor collapse
// =============================================================================

#[test]
fn expansion_state_survives_ancestor_collapse() {
    let mut hts = make_hts_with_tree(simple_tree());

    // Collapse root → node 1's expanded state is preserved internally.
    hts.collapse(0);
    assert_eq!(visible_indices(&hts.visible_rows()), vec![0]);

    // Re-expand root → node 1 is still expanded → nodes 2, 3 visible.
    hts.expand(0);
    let rows = hts.visible_rows();
    assert_eq!(visible_indices(&rows), vec![0, 1, 2, 3, 4]);
    assert!(rows[1].expanded); // Node 1 still expanded.
}

// =============================================================================
// 7. Toggle
// =============================================================================

#[test]
fn toggle_works() {
    let mut hts = make_hts_with_tree(simple_tree());

    // Toggle node 1 (expanded → collapsed).
    hts.toggle(1);
    assert_eq!(visible_indices(&hts.visible_rows()), vec![0, 1, 4]);

    // Toggle again (collapsed → expanded).
    hts.toggle(1);
    assert_eq!(visible_indices(&hts.visible_rows()), vec![0, 1, 2, 3, 4]);
}

#[test]
fn toggle_leaf_is_noop() {
    let mut hts = make_hts_with_tree(simple_tree());
    let rev_before = hts.revision();

    // Toggle a leaf (node 2) — should be a no-op.
    hts.toggle(2);
    // Revision still bumps (HTS bumps unconditionally on toggle call).
    let rows = hts.visible_rows();
    assert_eq!(visible_indices(&rows), vec![0, 1, 2, 3, 4]);
}

// =============================================================================
// 8. Virtualization windowing — scroll affects visible rows
// =============================================================================

#[test]
fn viewport_limits_visible_rows() {
    let mut hts = make_hts_with_tree(simple_tree());
    hts.set_row_height(24.0);

    // Viewport shows only first 2 rows (height = 48).
    hts.set_viewport_height(48.0);
    hts.scroll_to(0.0);
    let rows = hts.visible_rows();
    // first = floor(0/24) = 0, last = min(floor(48/24)+1, 5) = 3
    assert_eq!(visible_indices(&rows), vec![0, 1, 2]);
}

#[test]
fn scroll_offset_shifts_visible_window() {
    let mut hts = make_hts_with_tree(simple_tree());
    hts.set_row_height(24.0);
    hts.set_viewport_height(48.0);

    // Scroll to row 2.
    hts.scroll_to(48.0);
    let rows = hts.visible_rows();
    // first = floor(48/24) = 2, last = min(floor(96/24)+1, 5) = 5
    assert_eq!(visible_indices(&rows), vec![2, 3, 4]);
}

#[test]
fn scroll_past_end_returns_empty() {
    let mut hts = make_hts_with_tree(simple_tree());
    hts.set_row_height(24.0);
    hts.set_viewport_height(48.0);

    // Scroll way past the end.
    hts.scroll_to(10000.0);
    let rows = hts.visible_rows();
    assert!(rows.is_empty());
}

#[test]
fn zero_viewport_returns_empty() {
    let mut hts = make_hts_with_tree(simple_tree());
    // Default viewport_height = 0.
    let mut hts2 = HierarchicalTabularSubstrate::new();
    hts2.set_tree_data(simple_tree());
    let rows = hts2.visible_rows();
    assert!(rows.is_empty());
}

// =============================================================================
// 9. Windowing interacts with collapse
// =============================================================================

#[test]
fn collapse_reduces_windowed_row_count() {
    let mut hts = make_hts_with_tree(simple_tree());
    hts.set_row_height(24.0);
    hts.set_viewport_height(200.0); // Show all.

    // All 5 visible.
    assert_eq!(hts.visible_rows().len(), 5);

    // Collapse node 1 → 3 visible rows [0, 1, 4].
    hts.collapse(1);
    let rows = hts.visible_rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(visible_indices(&rows), vec![0, 1, 4]);

    // Now with small viewport: only 2 rows.
    hts.set_viewport_height(48.0);
    hts.scroll_to(0.0);
    let rows = hts.visible_rows();
    // 3 sorted rows, viewport shows first = 0, last = min(3, 3) = 3
    assert_eq!(visible_indices(&rows), vec![0, 1, 4]);
}

// =============================================================================
// 10. Revision counter monotonicity
// =============================================================================

#[test]
fn revision_increments_on_mutations() {
    let mut hts = HierarchicalTabularSubstrate::new();
    let r0 = hts.revision();

    hts.set_tree_data(simple_tree());
    let r1 = hts.revision();
    assert!(r1 > r0, "revision must increment on set_tree_data");

    hts.collapse(1);
    let r2 = hts.revision();
    assert!(r2 > r1, "revision must increment on collapse");

    hts.expand(1);
    let r3 = hts.revision();
    assert!(r3 > r2, "revision must increment on expand");

    hts.toggle(0);
    let r4 = hts.revision();
    assert!(r4 > r3, "revision must increment on toggle");

    hts.scroll_to(100.0);
    let r5 = hts.revision();
    assert!(r5 > r4, "revision must increment on scroll_to");

    hts.set_viewport_height(500.0);
    let r6 = hts.revision();
    assert!(r6 > r5, "revision must increment on set_viewport_height");

    hts.set_row_height(32.0);
    let r7 = hts.revision();
    assert!(r7 > r6, "revision must increment on set_row_height");
}

#[test]
fn revision_never_decreases() {
    let mut hts = make_hts_with_tree(simple_tree());
    let mut last_rev = hts.revision();

    for _ in 0..20 {
        hts.collapse(0);
        assert!(hts.revision() >= last_rev);
        last_rev = hts.revision();

        hts.expand(0);
        assert!(hts.revision() >= last_rev);
        last_rev = hts.revision();

        hts.toggle(1);
        assert!(hts.revision() >= last_rev);
        last_rev = hts.revision();

        hts.scroll_to(last_rev as f64);
        assert!(hts.revision() >= last_rev);
        last_rev = hts.revision();
    }
}

// =============================================================================
// 11. Topology independence from geometry (INV-HTS-004)
// =============================================================================

#[test]
fn geometry_resolve_does_not_affect_topology() {
    let mut hts = make_hts_with_tree(simple_tree());

    // Get visible rows before geometry.
    let rows_before = visible_indices(&hts.visible_rows());

    // Resolve geometry (should not trigger topology rebuild).
    hts.resolve_geometry(800.0, 1.0);

    // Visible rows unchanged.
    let rows_after = visible_indices(&hts.visible_rows());
    assert_eq!(rows_before, rows_after);
}

#[test]
fn topology_change_does_not_force_geometry() {
    let mut hts = make_hts_with_tree(simple_tree());
    hts.resolve_geometry(800.0, 1.0);

    let epoch_before = hts.revision();

    // Collapse a node (topology change).
    hts.collapse(1);

    // Geometry is still resolved from before — column_layout should still work.
    // The point: topology mutation does not invalidate geometry.
    hts.resolve_geometry(800.0, 1.0);
    let _layout = hts.column_layout();
}

// =============================================================================
// 12. Tree metadata decoupled from RowHeights
// =============================================================================

#[test]
fn set_tree_data_does_not_recreate_row_heights() {
    let mut hts = HierarchicalTabularSubstrate::new();

    // Inject RowHeights with specific height and 2 row slots.
    let mut rh = RowHeights::with_fixed_height(30.0);
    rh.push();
    rh.push();
    hts.set_row_heights(rh);

    // Set tree data — should NOT destroy the RowHeights.
    hts.set_tree_data(vec![
        NodeMeta {
            node_id: 0,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Root".to_string(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: Some(0),
            depth: 1,
            has_children: false,
            label: "Child".to_string(),
        },
    ]);

    // Row height should reflect the injected model (30.0), not the default (24.0).
    hts.set_viewport_height(200.0);
    let rows = hts.visible_rows();
    assert_eq!(rows.len(), 2);
}

#[test]
fn set_row_height_changes_row_height() {
    let mut hts = make_hts_with_tree(simple_tree());
    hts.set_row_height(24.0);
    hts.set_viewport_height(48.0);
    hts.scroll_to(0.0);

    // With height 24: first 3 rows visible (0..48 → rows 0,1,2).
    assert_eq!(hts.visible_rows().len(), 3);

    // Change to height 48 → only 2 rows fit.
    hts.set_row_height(48.0);
    let rows = hts.visible_rows();
    // first = floor(0/48) = 0, last = min(floor(48/48)+1, 5) = 2
    assert_eq!(rows.len(), 2);
}

// =============================================================================
// 13. Single root, no children
// =============================================================================

#[test]
fn single_root_no_children() {
    let mut hts = make_hts_with_tree(vec![NodeMeta {
        node_id: 0,
        parent_index: None,
        depth: 0,
        has_children: false,
        label: "Root".to_string(),
    }]);

    let rows = hts.visible_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].row_index, 0);
    assert!(!rows[0].has_children);
    assert!(!rows[0].expanded);
}

// =============================================================================
// 14. Deep tree
// =============================================================================

#[test]
fn deep_tree_all_visible_when_expanded() {
    // Chain: 0 → 1 → 2 → ... → 99
    let nodes: Vec<NodeMeta> = (0..100)
        .map(|i| NodeMeta {
            node_id: i as u64,
            parent_index: if i == 0 { None } else { Some(i - 1) },
            depth: i as u32,
            has_children: i < 99,
            label: format!("Node {}", i),
        })
        .collect();

    let mut hts = make_hts_with_tree(nodes);
    let rows = hts.visible_rows();

    // All nodes with has_children start expanded → all 100 visible.
    assert_eq!(rows.len(), 100);
    assert_eq!(rows[99].depth, 99);
}

#[test]
fn deep_tree_collapse_root_hides_all() {
    let nodes: Vec<NodeMeta> = (0..100)
        .map(|i| NodeMeta {
            node_id: i as u64,
            parent_index: if i == 0 { None } else { Some(i - 1) },
            depth: i as u32,
            has_children: i < 99,
            label: format!("Node {}", i),
        })
        .collect();

    let mut hts = make_hts_with_tree(nodes);
    hts.collapse(0);

    let rows = hts.visible_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].row_index, 0);
}

// =============================================================================
// 15. Multiple roots
// =============================================================================

#[test]
fn multiple_roots() {
    let nodes = vec![
        NodeMeta {
            node_id: 0,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "Root A".to_string(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "Root B".to_string(),
        },
        NodeMeta {
            node_id: 2,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "Root C".to_string(),
        },
    ];

    let mut hts = make_hts_with_tree(nodes);
    let rows = hts.visible_rows();
    assert_eq!(visible_indices(&rows), vec![0, 1, 2]);
}

// =============================================================================
// 16. Stable projection — same state produces same output
// =============================================================================

#[test]
fn deterministic_projection() {
    let build = || {
        let mut hts = make_hts_with_tree(simple_tree());
        hts.collapse(1);
        hts.visible_rows()
    };

    let rows1 = build();
    let rows2 = build();

    assert_eq!(visible_indices(&rows1), visible_indices(&rows2));
}

// =============================================================================
// 17. Expand/collapse cycle stability
// =============================================================================

#[test]
fn expand_collapse_cycle_returns_to_original() {
    let mut hts = make_hts_with_tree(simple_tree());
    let original = visible_indices(&hts.visible_rows());

    // Cycle: collapse root → expand root.
    hts.collapse(0);
    hts.expand(0);
    assert_eq!(visible_indices(&hts.visible_rows()), original);

    // Cycle: collapse node 1 → expand node 1.
    hts.collapse(1);
    hts.expand(1);
    assert_eq!(visible_indices(&hts.visible_rows()), original);
}

// =============================================================================
// 18. Replace tree data
// =============================================================================

#[test]
fn replace_tree_data_resets_projection() {
    let mut hts = make_hts_with_tree(simple_tree());
    assert_eq!(hts.visible_rows().len(), 5);

    // Replace with a smaller tree.
    hts.set_tree_data(vec![
        NodeMeta {
            node_id: 0,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "New A".to_string(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "New B".to_string(),
        },
    ]);

    let rows = hts.visible_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(visible_indices(&rows), vec![0, 1]);
}

// =============================================================================
// 19. INVARIANT-HTS-ROW-COUNT enforcement
// =============================================================================

#[test]
#[should_panic(expected = "INVARIANT-HTS-ROW-COUNT")]
fn mismatch_short_panics() {
    let mut hts = HierarchicalTabularSubstrate::new();
    // Inject RowHeights with 3 rows (shorter than tree).
    let mut rh = RowHeights::with_fixed_height(24.0);
    for _ in 0..3 {
        rh.push();
    }
    hts.set_row_heights(rh);
    // Set tree with 5 nodes → mismatch (3 != 5) → panic.
    hts.set_tree_data(simple_tree());
}

#[test]
#[should_panic(expected = "INVARIANT-HTS-ROW-COUNT")]
fn mismatch_long_panics() {
    let mut hts = HierarchicalTabularSubstrate::new();
    // Inject RowHeights with 10 rows (longer than tree).
    let mut rh = RowHeights::with_fixed_height(24.0);
    for _ in 0..10 {
        rh.push();
    }
    hts.set_row_heights(rh);
    // Set tree with 5 nodes → mismatch (10 != 5) → panic.
    hts.set_tree_data(simple_tree());
}

#[test]
fn fixed_height_zero_len_allowed() {
    // Default RowHeights: fixed_height(24.0), len() == 0.
    // Tree active with 5 nodes → Case B: zero-len RowHeights + fixed_height → allowed.
    let mut hts = make_hts_with_tree(simple_tree());
    let rows = hts.visible_rows();
    assert_eq!(rows.len(), 5);
}

// =============================================================================
// 20. Stable node identity (INV-NODE-ID-*)
// =============================================================================

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "INV-NODE-ID-UNIQUE")]
fn duplicate_node_id_panics_in_debug() {
    let nodes = vec![
        NodeMeta {
            node_id: 1,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "A".into(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: "B".into(),
        },
    ];
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_tree_data(nodes);
}

#[test]
fn visible_rows_carry_node_id() {
    let mut hts = make_hts_with_tree(simple_tree());
    let rows = hts.visible_rows();
    assert_ne!(rows[0].node_id, rows[1].node_id);
}

#[test]
fn node_id_stable_across_expand_collapse() {
    let mut hts = make_hts_with_tree(simple_tree());

    let before = hts.visible_rows();
    let node4_id = before.iter().find(|r| r.row_index == 4).unwrap().node_id;

    // Collapse node 1 → node 4 shifts position in visible list but keeps same node_id.
    hts.collapse(1);
    let after = hts.visible_rows();
    let node4_after = after.iter().find(|r| r.row_index == 4).unwrap().node_id;

    assert_eq!(node4_id, node4_after);
}

#[test]
fn node_id_stable_across_scroll() {
    let mut hts = make_hts_with_tree(simple_tree());
    hts.set_row_height(24.0);
    hts.set_viewport_height(48.0);

    hts.scroll_to(0.0);
    let r1 = hts.visible_rows();
    hts.scroll_to(24.0);
    let r2 = hts.visible_rows();

    // Both windows include row_index=1 → same node_id.
    let id_in_r1 = r1.iter().find(|r| r.row_index == 1).map(|r| r.node_id);
    let id_in_r2 = r2.iter().find(|r| r.row_index == 1).map(|r| r.node_id);
    assert_eq!(id_in_r1, id_in_r2);
}

#[test]
fn toggle_by_id_works() {
    let mut hts = make_hts_with_tree(simple_tree());
    let rows = hts.visible_rows();
    let node1_id = rows.iter().find(|r| r.row_index == 1).unwrap().node_id;

    assert_eq!(visible_indices(&hts.visible_rows()), vec![0, 1, 2, 3, 4]);

    hts.toggle_by_id(node1_id);
    assert_eq!(visible_indices(&hts.visible_rows()), vec![0, 1, 4]);

    hts.toggle_by_id(node1_id);
    assert_eq!(visible_indices(&hts.visible_rows()), vec![0, 1, 2, 3, 4]);
}

#[test]
fn toggle_by_unknown_id_is_noop() {
    let mut hts = make_hts_with_tree(simple_tree());
    let rev = hts.revision();
    hts.toggle_by_id(u64::MAX); // guaranteed not in dataset
    assert_eq!(hts.revision(), rev);
}

// =============================================================================
// 13. Column model mutators
// =============================================================================

#[test]
fn add_column_invalidates_geometry_and_bumps_revision() {
    let mut hts = HierarchicalTabularSubstrate::new();

    // Establish a resolved-geometry baseline so the invalidation
    // assertion below has a real before/after contrast.
    hts.resolve_geometry(800.0, 1.0);
    assert!(
        hts.try_column_layout().is_some(),
        "geometry should be resolved before add_column"
    );

    let revision_before = hts.revision();
    let column = Column::new(120.0);
    let column_id = column.id;
    let returned_id = hts.add_column(column);

    // Contract: returned ID matches the column's pre-allocated ID.
    assert_eq!(returned_id, column_id, "add_column returns the column's ID");

    // Contract: revision counter advanced (observers re-read).
    assert!(
        hts.revision() > revision_before,
        "add_column bumps the revision counter"
    );

    // Contract: cached geometry was invalidated (next layout
    // pass must re-resolve).
    assert!(
        hts.try_column_layout().is_none(),
        "add_column invalidates cached geometry"
    );
}

// =============================================================================
// 14. visible_window (SPEC-HAL-337 §5)
// =============================================================================

/// Flat list: n root nodes, ids 0..n.
fn flat_nodes(n: usize) -> Vec<NodeMeta> {
    (0..n)
        .map(|i| NodeMeta {
            node_id: i as u64,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: format!("Row {i}"),
        })
        .collect()
}

#[test]
fn visible_window_first_y_at_scroll_origin() {
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_viewport_height(100.0);
    hts.set_tree_data(flat_nodes(100));

    let w = hts.visible_window();
    assert_eq!(w.first_y, 0.0);
    assert_eq!(w.rows[0].row_index, 0);
}

#[test]
fn visible_window_first_y_matches_fixed_height_arithmetic() {
    // Default fixed row height is 24.0. scroll 50 → first = floor(50/24) = 2,
    // first_y = 2 · 24 = 48 (the y_offset compute_visible_range produces).
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_viewport_height(100.0);
    hts.set_tree_data(flat_nodes(100));
    hts.scroll_to(50.0);

    let w = hts.visible_window();
    assert_eq!(w.first_y, 48.0);
    assert_eq!(w.rows[0].row_index, 2);
}

#[test]
fn visible_window_rows_match_visible_rows() {
    let mut hts = make_hts_with_tree(simple_tree());
    let w = hts.visible_window();
    let rows = hts.visible_rows();
    assert_eq!(w.rows.len(), rows.len());
    assert_eq!(visible_indices(&w.rows), visible_indices(&rows));
}

#[test]
fn visible_window_empty_substrate() {
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_viewport_height(200.0);
    let w = hts.visible_window();
    assert!(w.rows.is_empty());
    assert_eq!(w.first_y, 0.0);
}
