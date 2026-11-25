// =============================================================================
// TreeSubstrate Unit Tests (White-Box)
// =============================================================================
//
// SPEC-HAL-251 / SPEC-HAL-254 substrate verification.
//
// Invariants verified:
// - Projection order strictly depth-first pre-order
// - Collapse hides subtree, re-expand restores it
// - Expanded set survives ancestor collapse
// - Adapter reconciliation removes stale entries
// - row_index correctness
// - Deterministic rebuild
// - Revision increments on every rebuild
// =============================================================================

use std::collections::HashMap;

use crate::model::tree_substrate::TreeSubstrate;
use crate::model::tree_types::{ProjectedNode, TreeAdapter, TreeExpansionOp, TreeNodeId};

// =============================================================================
// Test Adapter
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

    /// Remove child from parent's children list but leave child's own
    /// children_map entry intact (creates orphan subtree).
    fn remove_child(&mut self, parent: TreeNodeId, child: TreeNodeId) {
        if let Some(children) = self.children_map.get_mut(&parent) {
            children.retain(|&c| c != child);
        }
        self.rev += 1;
    }

    fn remove_node(&mut self, id: TreeNodeId) {
        self.roots.retain(|&r| r != id);
        self.children_map.remove(&id);
        for children in self.children_map.values_mut() {
            children.retain(|&c| c != id);
        }
        self.rev += 1;
    }

    /// Detach node from its current parent (or roots) and reattach under
    /// new_parent. Preserves node's own subtree. Caller must ensure no cycle.
    fn reattach(&mut self, node: TreeNodeId, new_parent: TreeNodeId) {
        self.roots.retain(|&r| r != node);
        for children in self.children_map.values_mut() {
            children.retain(|&c| c != node);
        }
        self.children_map.entry(new_parent).or_default().push(node);
        self.rev += 1;
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

fn pn(id: u64, depth: u16) -> ProjectedNode {
    ProjectedNode {
        node_id: TreeNodeId::from_raw(id),
        depth,
    }
}

// =============================================================================
// 1. Empty tree
// =============================================================================

#[test]
fn empty_tree() {
    let adapter = TestAdapter::new();
    let sub = TreeSubstrate::new(adapter);

    assert!(sub.projection().is_empty());
    assert_eq!(sub.revision(), 1);
}

// =============================================================================
// 2. Single level
// =============================================================================

#[test]
fn single_level() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_root(n(2));
    adapter.add_root(n(3));

    let sub = TreeSubstrate::new(adapter);

    assert_eq!(sub.projection().len(), 3);
    assert_eq!(sub.projection()[0], pn(1, 0));
    assert_eq!(sub.projection()[1], pn(2, 0));
    assert_eq!(sub.projection()[2], pn(3, 0));
}

// =============================================================================
// 3. Multi-level expansion
// =============================================================================

#[test]
fn multi_level_expansion() {
    //  A
    //  └ B
    //    └ C
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(2), n(3));

    let mut sub = TreeSubstrate::new(adapter);

    // Initially only A visible (collapsed).
    assert_eq!(sub.projection().len(), 1);
    assert_eq!(sub.projection()[0], pn(1, 0));

    // Expand A → B becomes visible.
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    assert_eq!(sub.projection().len(), 2);
    assert_eq!(sub.projection()[1], pn(2, 1));

    // Expand B → C becomes visible.
    sub.apply_expansion(TreeExpansionOp::Expand(n(2)));
    assert_eq!(sub.projection().len(), 3);
    assert_eq!(sub.projection()[2], pn(3, 2));
}

// =============================================================================
// 4. Collapse hides subtree
// =============================================================================

#[test]
fn collapse_hides_subtree() {
    //  A
    //  └ B
    //    └ C
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(2), n(3));

    let mut sub = TreeSubstrate::new(adapter);
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    sub.apply_expansion(TreeExpansionOp::Expand(n(2)));
    assert_eq!(sub.projection().len(), 3);

    // Collapse A → B and C disappear.
    sub.apply_expansion(TreeExpansionOp::Collapse(n(1)));
    assert_eq!(sub.projection().len(), 1);
    assert_eq!(sub.projection()[0], pn(1, 0));
}

// =============================================================================
// 5. Re-expansion restores subtree
// =============================================================================

#[test]
fn re_expansion_restores_subtree() {
    //  A
    //  └ B
    //    └ C
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(2), n(3));

    let mut sub = TreeSubstrate::new(adapter);
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    sub.apply_expansion(TreeExpansionOp::Expand(n(2)));
    assert_eq!(sub.projection().len(), 3);

    // Collapse A.
    sub.apply_expansion(TreeExpansionOp::Collapse(n(1)));
    assert_eq!(sub.projection().len(), 1);

    // Re-expand A → B still in expanded set → C reappears.
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    assert_eq!(sub.projection().len(), 3);
    assert_eq!(sub.projection()[2], pn(3, 2));
}

// =============================================================================
// 6. Expanded set survives ancestor collapse
// =============================================================================

#[test]
fn expanded_set_survives_ancestor_collapse() {
    //  A
    //  └ B
    //    └ C
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(2), n(3));

    let mut sub = TreeSubstrate::new(adapter);
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    sub.apply_expansion(TreeExpansionOp::Expand(n(2)));

    // Collapse A. B is hidden but still in expanded set.
    sub.apply_expansion(TreeExpansionOp::Collapse(n(1)));

    assert!(!sub.is_expanded(n(1)));
    assert!(sub.is_expanded(n(2))); // Preserved despite ancestor collapse.
}

// =============================================================================
// 7. Adapter node removal reconciliation
// =============================================================================

#[test]
fn adapter_node_removal_reconciliation() {
    //  A
    //  ├ B
    //  │ └ C
    //  └ D
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(2), n(3));
    adapter.add_child(n(1), n(4));

    let mut sub = TreeSubstrate::new(adapter);
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    sub.apply_expansion(TreeExpansionOp::Expand(n(2)));
    // Projection: A(0), B(1), C(2), D(3)
    assert_eq!(sub.projection().len(), 4);

    // Remove B from adapter.
    sub.adapter_mut().remove_node(n(2));
    sub.reconcile_adapter_revision();

    // B and C gone. A still has D, so A stays expanded.
    assert_eq!(sub.projection().len(), 2);
    assert_eq!(sub.projection()[0], pn(1, 0));
    assert_eq!(sub.projection()[1], pn(4, 1));
    assert!(sub.is_expanded(n(1)));
    assert!(!sub.is_expanded(n(2)));
}

// =============================================================================
// 8. row_index correctness
// =============================================================================

#[test]
fn row_index_correctness() {
    //  A
    //  ├ B
    //  └ C
    //    └ D
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(1), n(3));
    adapter.add_child(n(3), n(4));

    let mut sub = TreeSubstrate::new(adapter);
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    sub.apply_expansion(TreeExpansionOp::Expand(n(3)));

    // Projection: A(0), B(1), C(2), D(3)
    assert_eq!(sub.row_index(n(1)), Some(0));
    assert_eq!(sub.row_index(n(2)), Some(1));
    assert_eq!(sub.row_index(n(3)), Some(2));
    assert_eq!(sub.row_index(n(4)), Some(3));

    // Verify invariant: projection[row_index[node]].node_id == node
    for p in sub.projection() {
        let idx = sub.row_index(p.node_id).unwrap();
        assert_eq!(sub.projection()[idx].node_id, p.node_id);
    }

    // Non-existent node returns None.
    assert_eq!(sub.row_index(n(999)), None);
}

// =============================================================================
// 9. Deterministic rebuild
// =============================================================================

#[test]
fn deterministic_rebuild() {
    let build = || {
        let mut adapter = TestAdapter::new();
        adapter.add_root(n(1));
        adapter.add_child(n(1), n(2));
        adapter.add_child(n(1), n(3));
        adapter.add_child(n(2), n(4));

        let mut sub = TreeSubstrate::new(adapter);
        sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
        sub.apply_expansion(TreeExpansionOp::Expand(n(2)));
        sub
    };

    let sub1 = build();
    let sub2 = build();

    // Same adapter state + same expanded set → identical projection.
    assert_eq!(sub1.projection(), sub2.projection());
}

// =============================================================================
// 10. Revision increments correctly
// =============================================================================

#[test]
fn revision_increments() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));

    let mut sub = TreeSubstrate::new(adapter);
    let r0 = sub.revision();
    assert_eq!(r0, 1); // Initial rebuild.

    // Expansion → rebuild → revision increments.
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    let r1 = sub.revision();
    assert!(r1 > r0);

    // Collapse → rebuild → revision increments.
    sub.apply_expansion(TreeExpansionOp::Collapse(n(1)));
    let r2 = sub.revision();
    assert!(r2 > r1);

    // Reconciliation with changed adapter revision → rebuild → increments.
    sub.adapter_mut().remove_node(n(2)); // Bumps adapter revision.
    sub.reconcile_adapter_revision();
    let r3 = sub.revision();
    assert!(r3 > r2);
}

// =============================================================================
// Helpers: Reachability (test-side verification, independent of substrate)
// =============================================================================

fn collect_reachable<A: TreeAdapter>(adapter: &A) -> std::collections::HashSet<TreeNodeId> {
    let mut reachable = std::collections::HashSet::new();
    let mut stack: Vec<TreeNodeId> = adapter.roots().to_vec();

    while let Some(node) = stack.pop() {
        if !reachable.insert(node) {
            // Already visited via another path — cycle or multi-parent.
            panic!(
                "collect_reachable: node {:?} reached twice (cycle or multi-parent)",
                node
            );
        }
        for &child in adapter.children(node) {
            stack.push(child);
        }
    }

    reachable
}

// =============================================================================
// 11. Orphan pruning
// =============================================================================

#[test]
fn orphan_pruning() {
    //  A
    //  └ B
    //    └ C
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(2), n(3));

    let mut sub = TreeSubstrate::new(adapter);
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    sub.apply_expansion(TreeExpansionOp::Expand(n(2)));
    // Projection: A(0), B(1), C(2)
    assert_eq!(sub.projection().len(), 3);

    // Remove B as child of A, but leave B→C in children_map (orphan).
    sub.adapter_mut().remove_child(n(1), n(2));
    sub.reconcile_adapter_revision();

    // B and C are orphans — unreachable from root A.
    // A has no children now, so it's a leaf.
    assert_eq!(sub.projection().len(), 1);
    assert_eq!(sub.projection()[0], pn(1, 0));

    // Expanded must not contain orphans.
    assert!(!sub.is_expanded(n(1))); // A has no children → pruned.
    assert!(!sub.is_expanded(n(2))); // Unreachable → pruned.
}

// =============================================================================
// 12. Root replacement
// =============================================================================

#[test]
fn root_replacement() {
    //  A
    //  ├ B
    //  └ C
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_child(n(1), n(2));
    adapter.add_child(n(1), n(3));

    let mut sub = TreeSubstrate::new(adapter);
    sub.apply_expansion(TreeExpansionOp::Expand(n(1)));
    // Projection: A(0), B(1), C(2)
    assert_eq!(sub.projection().len(), 3);

    // Replace root A with new root D→E.
    {
        let adapter = sub.adapter_mut();
        adapter.roots.clear();
        adapter.roots.push(n(10));
        adapter.children_map.clear();
        adapter.children_map.insert(n(10), vec![n(11)]);
        adapter.rev += 1;
    }
    sub.reconcile_adapter_revision();

    // Projection is now just D (collapsed).
    assert_eq!(sub.projection().len(), 1);
    assert_eq!(sub.projection()[0], pn(10, 0));

    // Old expanded entries are gone.
    assert!(!sub.is_expanded(n(1)));

    // Expand D → E visible.
    sub.apply_expansion(TreeExpansionOp::Expand(n(10)));
    assert_eq!(sub.projection().len(), 2);
    assert_eq!(sub.projection()[1], pn(11, 1));
}

// =============================================================================
// 13. Deep tree safety
// =============================================================================

#[test]
fn deep_tree_safety() {
    // Build a chain of depth 1000: n(0) → n(1) → ... → n(999)
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(0));
    for i in 0..999u64 {
        adapter.add_child(n(i), n(i + 1));
    }

    let mut sub = TreeSubstrate::new(adapter);

    // Expand all nodes.
    sub.apply_expansion(TreeExpansionOp::ExpandAll(n(0)));

    // All 1000 nodes visible.
    assert_eq!(sub.projection().len(), 1000);

    // Deepest node at depth 999.
    assert_eq!(sub.projection()[999], pn(999, 999));

    // No panic — iterative traversal handles deep trees.
}

// =============================================================================
// 14. Randomized invariant: Expanded ⊆ Reachable (hardened)
// =============================================================================

/// Check if `node` is a descendant of `potential_ancestor` via adapter topology.
fn is_descendant_of<A: TreeAdapter>(
    adapter: &A,
    node: TreeNodeId,
    potential_ancestor: TreeNodeId,
) -> bool {
    let mut stack = vec![potential_ancestor];
    while let Some(current) = stack.pop() {
        for &child in adapter.children(current) {
            if child == node {
                return true;
            }
            stack.push(child);
        }
    }
    false
}

/// Build parent map from adapter topology (test-side, independent of substrate).
/// Panics on multi-parent (DAG) or cycle corruption.
fn build_parent_map<A: TreeAdapter>(adapter: &A) -> HashMap<TreeNodeId, TreeNodeId> {
    let mut parent_map = HashMap::new();
    let mut stack: Vec<TreeNodeId> = adapter.roots().to_vec();
    let mut visited = std::collections::HashSet::new();
    while let Some(node) = stack.pop() {
        if !visited.insert(node) {
            panic!(
                "build_parent_map: cycle detected — node {:?} visited twice",
                node
            );
        }
        for &child in adapter.children(node) {
            if let Some(old_parent) = parent_map.insert(child, node) {
                panic!(
                    "build_parent_map: node {:?} has multiple parents: {:?} and {:?}",
                    child, old_parent, node
                );
            }
            stack.push(child);
        }
    }
    parent_map
}

/// Assert all structural invariants (A–G) on the substrate.
fn assert_all_invariants(sub: &TreeSubstrate<TestAdapter>, last_revision: u64, step: usize) {
    let reachable = collect_reachable(sub.adapter());

    // A. Projection ⊆ Reachable
    for p in sub.projection() {
        assert!(
            reachable.contains(&p.node_id),
            "step {step}: projected node {:?} not reachable",
            p.node_id
        );
    }

    // B. Expanded ⊆ Reachable ∩ Expandable (direct check, not via projection)
    let expanded = sub.expanded_clone();
    for id in &expanded {
        assert!(
            reachable.contains(id),
            "step {step}: expanded node {:?} not reachable",
            id
        );
        assert!(
            sub.adapter().has_children(*id),
            "step {step}: expanded node {:?} is a leaf (has_children = false)",
            id
        );
    }

    // C. No duplicate projection entries
    let mut seen = std::collections::HashSet::with_capacity(sub.projection().len());
    for p in sub.projection() {
        assert!(
            seen.insert(p.node_id),
            "step {step}: duplicate projection entry {:?}",
            p.node_id
        );
    }

    // D. row_index correctness
    for p in sub.projection() {
        let idx = sub
            .row_index(p.node_id)
            .unwrap_or_else(|| panic!("step {step}: row_index missing for {:?}", p.node_id));
        assert_eq!(
            sub.projection()[idx].node_id,
            p.node_id,
            "step {step}: row_index mismatch for {:?}",
            p.node_id
        );
    }

    // E. Depth correctness
    let parent_map = build_parent_map(sub.adapter());
    let proj_positions: HashMap<TreeNodeId, usize> = sub
        .projection()
        .iter()
        .enumerate()
        .map(|(i, p)| (p.node_id, i))
        .collect();
    for (i, p) in sub.projection().iter().enumerate() {
        if p.depth == 0 {
            assert!(
                sub.adapter().roots().contains(&p.node_id),
                "step {step}: depth-0 node {:?} not in adapter roots",
                p.node_id
            );
            continue;
        }
        let parent_id = parent_map.get(&p.node_id).unwrap_or_else(|| {
            panic!(
                "step {step}: non-root node {:?} at depth {} has no parent",
                p.node_id, p.depth
            )
        });
        let parent_pos = proj_positions.get(parent_id).unwrap_or_else(|| {
            panic!(
                "step {step}: parent {:?} of {:?} not in projection",
                parent_id, p.node_id
            )
        });
        assert!(
            *parent_pos < i,
            "step {step}: parent {:?} at idx {} not before child {:?} at idx {}",
            parent_id,
            parent_pos,
            p.node_id,
            i
        );
        assert_eq!(
            p.depth,
            sub.projection()[*parent_pos].depth + 1,
            "step {step}: depth mismatch: child {:?} depth {} != parent {:?} depth {} + 1",
            p.node_id,
            p.depth,
            parent_id,
            sub.projection()[*parent_pos].depth
        );
    }

    // F. Revision monotonicity
    assert!(
        sub.revision() >= last_revision,
        "step {step}: revision went backwards: {} < {}",
        sub.revision(),
        last_revision
    );
}

#[test]
fn expanded_subset_of_reachable_randomized() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(0xDEADBEEF);

    const NODE_COUNT: u64 = 50;

    // Build initial hierarchical tree: node 0 is root, each node i>0 gets
    // a random parent in [0, i-1]. Deterministic via seeded RNG.
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(0));
    for i in 1..NODE_COUNT {
        let parent = rng.gen_range(0..i);
        adapter.add_child(n(parent), n(i));
    }

    let mut sub = TreeSubstrate::new(adapter);

    // Expand all to start with full visibility.
    sub.apply_expansion(TreeExpansionOp::ExpandAll(n(0)));

    let mut last_revision = sub.revision();

    for step in 0..500 {
        let op = rng.gen_range(0..7u32);

        match op {
            0 => {
                // Expand random node
                let id = n(rng.gen_range(0..NODE_COUNT));
                sub.apply_expansion(TreeExpansionOp::Expand(id));
            }
            1 => {
                // Collapse random node
                let id = n(rng.gen_range(0..NODE_COUNT));
                sub.apply_expansion(TreeExpansionOp::Collapse(id));
            }
            2 => {
                // Toggle random node
                let id = n(rng.gen_range(0..NODE_COUNT));
                sub.apply_expansion(TreeExpansionOp::Toggle(id));
            }
            3 => {
                // Remove random child link (orphan subtree)
                let parent = n(rng.gen_range(0..NODE_COUNT));
                let child = n(rng.gen_range(0..NODE_COUNT));
                sub.adapter_mut().remove_child(parent, child);
                sub.reconcile_adapter_revision();
            }
            4 => {
                // Remove random subtree
                let id = n(rng.gen_range(0..NODE_COUNT));
                sub.adapter_mut().remove_node(id);
                sub.reconcile_adapter_revision();
            }
            5 => {
                // Reattach subtree under random valid parent (cycle-safe)
                let node_id = rng.gen_range(0..NODE_COUNT);
                let new_parent_id = rng.gen_range(0..NODE_COUNT);
                if node_id != new_parent_id
                    && !is_descendant_of(sub.adapter(), n(new_parent_id), n(node_id))
                {
                    sub.adapter_mut().reattach(n(node_id), n(new_parent_id));
                    sub.reconcile_adapter_revision();
                }
            }
            6 => {
                // Replace root set entirely
                let new_root = n(rng.gen_range(0..NODE_COUNT));
                {
                    let adapter = sub.adapter_mut();
                    adapter.roots.clear();
                    adapter.roots.push(new_root);
                    adapter.rev += 1;
                }
                sub.reconcile_adapter_revision();
            }
            _ => unreachable!(),
        }

        assert_all_invariants(&sub, last_revision, step);
        last_revision = sub.revision();
    }
}
