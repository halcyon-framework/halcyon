// =============================================================================
// HTS Subtree Operations — Integration Test
//
// Exercises expand_subtree / collapse_subtree through the public
// HierarchicalTabularSubstrate API. No internal engine access.
// =============================================================================

use halcyon::engines::hierarchical::{HierarchicalTabularSubstrate, NodeMeta};

/// Build a 3-depth test tree (pre-order layout):
///
///   [0] Root (depth 0, visible root row)
///     [1] A (depth 1, has_children)
///       [2] A.1 (depth 2, has_children)
///         [3] A.1.a (depth 3, leaf)
///         [4] A.1.b (depth 3, leaf)
///       [5] A.2 (depth 2, leaf)
///     [6] B (depth 1, has_children)
///       [7] B.1 (depth 2, leaf)
///       [8] B.2 (depth 2, has_children)
///         [9] B.2.a (depth 3, leaf)
fn build_tree() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 100,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Root".into(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: "A".into(),
        },
        NodeMeta {
            node_id: 2,
            parent_index: Some(1),
            depth: 2,
            has_children: true,
            label: "A.1".into(),
        },
        NodeMeta {
            node_id: 3,
            parent_index: Some(2),
            depth: 3,
            has_children: false,
            label: "A.1.a".into(),
        },
        NodeMeta {
            node_id: 4,
            parent_index: Some(2),
            depth: 3,
            has_children: false,
            label: "A.1.b".into(),
        },
        NodeMeta {
            node_id: 5,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "A.2".into(),
        },
        NodeMeta {
            node_id: 6,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: "B".into(),
        },
        NodeMeta {
            node_id: 7,
            parent_index: Some(6),
            depth: 2,
            has_children: false,
            label: "B.1".into(),
        },
        NodeMeta {
            node_id: 8,
            parent_index: Some(6),
            depth: 2,
            has_children: true,
            label: "B.2".into(),
        },
        NodeMeta {
            node_id: 9,
            parent_index: Some(8),
            depth: 3,
            has_children: false,
            label: "B.2.a".into(),
        },
    ]
}

fn make_hts(nodes: Vec<NodeMeta>) -> HierarchicalTabularSubstrate {
    let mut hts = HierarchicalTabularSubstrate::new();
    hts.set_viewport_height(10000.0); // Large viewport so all rows are visible.
    hts.set_tree_data(nodes);
    hts
}

fn visible_ids(hts: &mut HierarchicalTabularSubstrate) -> Vec<u64> {
    hts.visible_rows().iter().map(|vr| vr.node_id).collect()
}

#[test]
fn expand_subtree_expands_all_descendants() {
    let mut hts = make_hts(build_tree());

    // Collapse everything first.
    hts.collapse_by_id(1); // A
    hts.collapse_by_id(2); // A.1
    hts.collapse_by_id(6); // B
    hts.collapse_by_id(8); // B.2

    // Only root and top-level nodes should be visible (A and B, both collapsed).
    let ids = visible_ids(&mut hts);
    assert_eq!(
        ids,
        vec![100, 1, 6],
        "After collapsing all, only root, A, and B visible"
    );

    // Expand subtree rooted at A — should expand A, A.1 recursively.
    hts.expand_subtree_by_id(1);
    let ids = visible_ids(&mut hts);
    assert!(ids.contains(&1), "A visible");
    assert!(ids.contains(&2), "A.1 visible");
    assert!(ids.contains(&3), "A.1.a visible");
    assert!(ids.contains(&4), "A.1.b visible");
    assert!(ids.contains(&5), "A.2 visible");
    // B subtree should still be collapsed.
    assert!(!ids.contains(&7), "B.1 should NOT be visible");
    assert!(!ids.contains(&9), "B.2.a should NOT be visible");
}

#[test]
fn collapse_subtree_collapses_all_descendants() {
    let mut hts = make_hts(build_tree());

    // Everything starts expanded. Collapse subtree at A.
    hts.collapse_subtree_by_id(1);
    let ids = visible_ids(&mut hts);

    // A should be visible but collapsed; its children hidden.
    assert!(ids.contains(&1), "A visible (collapsed)");
    assert!(!ids.contains(&2), "A.1 hidden");
    assert!(!ids.contains(&3), "A.1.a hidden");
    assert!(!ids.contains(&5), "A.2 hidden");

    // B subtree should be unaffected (still expanded).
    assert!(ids.contains(&6), "B visible");
    assert!(ids.contains(&7), "B.1 visible");
    assert!(ids.contains(&8), "B.2 visible");
    assert!(ids.contains(&9), "B.2.a visible");
}

#[test]
fn expand_subtree_noop_on_leaf() {
    let mut hts = make_hts(build_tree());
    let rev_before = hts.revision();

    // Node 3 (A.1.a) is a leaf — expand_subtree should be a no-op.
    hts.expand_subtree_by_id(3);
    let rev_after = hts.revision();

    // Revision should NOT have bumped (leaf has no children to expand).
    assert_eq!(
        rev_before, rev_after,
        "No revision bump on leaf expand_subtree"
    );
}

#[test]
fn expand_subtree_noop_no_revision_bump() {
    let mut hts = make_hts(build_tree());

    // Everything starts expanded. expand_subtree on A should be a no-op.
    let rev_before = hts.revision();
    hts.expand_subtree_by_id(1);
    let rev_after = hts.revision();

    assert_eq!(
        rev_before, rev_after,
        "No revision bump when already fully expanded"
    );
}

#[test]
fn collapse_subtree_noop_no_revision_bump() {
    let mut hts = make_hts(build_tree());

    // Collapse everything first.
    hts.collapse_subtree_by_id(1);
    hts.collapse_subtree_by_id(6);

    let rev_before = hts.revision();
    // Collapse again — should be no-op.
    hts.collapse_subtree_by_id(1);
    let rev_after = hts.revision();

    assert_eq!(
        rev_before, rev_after,
        "No revision bump when already fully collapsed"
    );
}

#[test]
fn revision_bumps_once_per_subtree_op() {
    let mut hts = make_hts(build_tree());

    // Collapse everything.
    hts.collapse_subtree_by_id(1);
    hts.collapse_subtree_by_id(6);

    let rev_before = hts.revision();
    // Expand subtree at A (multiple nodes expanded in one call).
    hts.expand_subtree_by_id(1);
    let rev_after = hts.revision();

    assert_eq!(
        rev_after,
        rev_before + 1,
        "Revision bumps exactly once per subtree op"
    );
}
