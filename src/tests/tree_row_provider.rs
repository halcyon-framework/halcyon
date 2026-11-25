// =============================================================================
// Tree ↔ ListView Projection Contract Tests (White-Box)
// =============================================================================
//
// Verifies TreeRowProvider trait and TreeListProjection adapter.
//
// Categories:
// 1. TreeRowProvider accessor correctness
// 2. Smoke: Tree projection → RowId identity mapping
//
// NOTE: TabularSubstrate no longer owns tree projection (SPEC-HAL-259).
// Tree-aware substrate tests live in hierarchical_substrate.rs.
// =============================================================================

use std::collections::HashMap;

use crate::model::tree::Tree;
use crate::model::tree_row_provider::{TreeListProjection, TreeRowProvider};
use crate::model::tree_types::{TreeAdapter, TreeNodeId};
use crate::widgets::tabular_view::row::RowId;

// =============================================================================
// Test Adapter (same shape as tree.rs tests)
// =============================================================================

struct TestAdapter {
    roots: Vec<TreeNodeId>,
    children_map: HashMap<TreeNodeId, Vec<TreeNodeId>>,
    rev: u64,
}

impl TestAdapter {
    fn new() -> Self {
        Self {
            roots: Vec::new(),
            children_map: HashMap::new(),
            rev: 1,
        }
    }

    fn add_root(&mut self, id: TreeNodeId) {
        self.roots.push(id);
    }

    fn add_child(&mut self, parent: TreeNodeId, child: TreeNodeId) {
        self.children_map.entry(parent).or_default().push(child);
    }
}

impl TreeAdapter for TestAdapter {
    fn roots(&self) -> &[TreeNodeId] {
        &self.roots
    }

    fn children(&self, node: TreeNodeId) -> &[TreeNodeId] {
        self.children_map.get(&node).map_or(&[], |v| v.as_slice())
    }

    fn revision(&self) -> u64 {
        self.rev
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn n(id: u64) -> TreeNodeId {
    TreeNodeId::from_raw(id)
}

/// Build a standard test tree:
///   root(1)
///     ├─ child_a(2)
///     │   ├─ grandchild_1(4)
///     │   └─ grandchild_2(5)
///     └─ child_b(3)
///         └─ grandchild_3(6)
fn build_tree() -> TestAdapter {
    let mut a = TestAdapter::new();
    a.add_root(n(1));
    a.add_child(n(1), n(2));
    a.add_child(n(1), n(3));
    a.add_child(n(2), n(4));
    a.add_child(n(2), n(5));
    a.add_child(n(3), n(6));
    a
}

/// Materialize RowIds from a TreeRowProvider (for row identity verification).
fn materialize_row_ids(provider: &dyn TreeRowProvider) -> Vec<RowId> {
    (0..provider.row_count())
        .map(|i| RowId::from_raw(provider.node_id_at(i).raw()))
        .collect()
}

// =============================================================================
// 1. TreeRowProvider accessor correctness
// =============================================================================

#[test]
fn provider_initial_collapsed_tree() {
    // Tree starts with only roots visible (nothing expanded).
    let tree = Tree::new(build_tree());
    let proj = TreeListProjection::new(&tree);

    assert_eq!(proj.row_count(), 1);
    assert_eq!(proj.node_id_at(0), n(1));
    assert_eq!(proj.depth_at(0), 0);
    assert!(proj.is_expandable(0)); // root has children
    assert!(!proj.is_expanded(0)); // starts collapsed
    assert_eq!(proj.row_index_of(n(1)), Some(0));
    assert_eq!(proj.row_index_of(n(2)), None); // not visible
}

#[test]
fn provider_after_expand_root() {
    let mut tree = Tree::new(build_tree());
    tree.expand(n(1));
    let proj = TreeListProjection::new(&tree);

    assert_eq!(proj.row_count(), 3); // root + child_a + child_b
    assert_eq!(proj.node_id_at(0), n(1));
    assert_eq!(proj.node_id_at(1), n(2));
    assert_eq!(proj.node_id_at(2), n(3));

    assert_eq!(proj.depth_at(0), 0);
    assert_eq!(proj.depth_at(1), 1);
    assert_eq!(proj.depth_at(2), 1);

    assert!(proj.is_expanded(0)); // root expanded
    assert!(!proj.is_expanded(1)); // child_a collapsed
    assert!(!proj.is_expanded(2)); // child_b collapsed

    assert!(proj.is_expandable(1)); // child_a has children
    assert!(proj.is_expandable(2)); // child_b has children

    assert_eq!(proj.row_index_of(n(1)), Some(0));
    assert_eq!(proj.row_index_of(n(2)), Some(1));
    assert_eq!(proj.row_index_of(n(3)), Some(2));
    assert_eq!(proj.row_index_of(n(4)), None); // not visible
}

#[test]
fn provider_fully_expanded() {
    let mut tree = Tree::new(build_tree());
    tree.expand_all(n(1));
    let proj = TreeListProjection::new(&tree);

    assert_eq!(proj.row_count(), 6);
    // DFS pre-order: 1, 2, 4, 5, 3, 6
    assert_eq!(proj.node_id_at(0), n(1));
    assert_eq!(proj.node_id_at(1), n(2));
    assert_eq!(proj.node_id_at(2), n(4));
    assert_eq!(proj.node_id_at(3), n(5));
    assert_eq!(proj.node_id_at(4), n(3));
    assert_eq!(proj.node_id_at(5), n(6));

    // Depths
    assert_eq!(proj.depth_at(2), 2); // grandchild_1
    assert_eq!(proj.depth_at(5), 2); // grandchild_3

    // Leaves are not expandable
    assert!(!proj.is_expandable(2)); // grandchild_1
    assert!(!proj.is_expandable(3)); // grandchild_2

    // Leaves not expanded
    assert!(!proj.is_expanded(2));
    assert!(!proj.is_expanded(3));
}

// =============================================================================
// 2. Smoke: Tree projection → RowId materialization
// =============================================================================
//
// NOTE: TabularSubstrate no longer owns tree projection (SPEC-HAL-259).
// Tree-aware substrate tests live in hierarchical_substrate.rs.
// These tests verify TreeRowProvider → RowId identity mapping only.

#[test]
fn smoke_tree_row_ids_match_projection() {
    let mut tree = Tree::new(build_tree());
    tree.expand(n(1));

    let proj = TreeListProjection::new(&tree);
    assert_eq!(proj.row_count(), 3);

    let row_ids = materialize_row_ids(&proj);
    assert_eq!(row_ids.len(), 3);

    // RowId raw values match TreeNodeId raw values.
    assert_eq!(row_ids[0].raw(), n(1).raw());
    assert_eq!(row_ids[1].raw(), n(2).raw());
    assert_eq!(row_ids[2].raw(), n(3).raw());
}

#[test]
fn smoke_fully_expanded_tree_row_ids() {
    let mut tree = Tree::new(build_tree());
    tree.expand_all(n(1));

    let proj = TreeListProjection::new(&tree);
    let row_ids = materialize_row_ids(&proj);

    // DFS pre-order: 1, 2, 4, 5, 3, 6
    assert_eq!(row_ids.len(), 6);
    let expected: Vec<u64> = vec![1, 2, 4, 5, 3, 6];
    for (i, &expected_raw) in expected.iter().enumerate() {
        assert_eq!(row_ids[i].raw(), expected_raw, "Row {i} id mismatch");
    }
}
