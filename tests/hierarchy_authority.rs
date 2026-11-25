use halcyon::semantics::hierarchy::{
    DeletionStrategy, HierarchyAuthority, HierarchyError, HierarchyNodeId,
};
use std::collections::HashSet;

fn n(id: u64) -> HierarchyNodeId {
    HierarchyNodeId(id)
}

fn id_set(ids: &[u64]) -> HashSet<HierarchyNodeId> {
    ids.iter().map(|&id| n(id)).collect()
}

// 1. Cycle Prevention
#[test]
fn add_edge_rejects_cycle() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(2), n(3)).unwrap();

    // C → A would create cycle
    let err = h.add_edge(n(3), n(1)).unwrap_err();
    assert_eq!(
        err,
        HierarchyError::CycleDetected {
            parent: n(3),
            child: n(1)
        }
    );

    // Hierarchy unchanged
    let snap = h.snapshot();
    assert_eq!(snap.node_count(), 3);
    assert_eq!(snap.children(n(1)), &[n(2)]);
    assert_eq!(snap.children(n(2)), &[n(3)]);
    assert_eq!(snap.children(n(3)), &[] as &[HierarchyNodeId]);

    let desc: HashSet<_> = snap.descendants(n(1)).into_iter().collect();
    assert_eq!(desc, id_set(&[2, 3]));
    let anc: HashSet<_> = snap.ancestors(n(3)).into_iter().collect();
    assert_eq!(anc, id_set(&[1, 2]));

    // Self-edge rejected as cycle
    let err = h.add_edge(n(1), n(1)).unwrap_err();
    assert_eq!(
        err,
        HierarchyError::CycleDetected {
            parent: n(1),
            child: n(1)
        }
    );
}

// 2. DeleteLeaf Enforcement
#[test]
fn delete_leaf_fails_if_children_exist() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();

    let err = h
        .remove_node(n(1), DeletionStrategy::DeleteLeaf)
        .unwrap_err();
    assert_eq!(err, HierarchyError::NodeHasChildren(n(1)));

    let snap = h.snapshot();
    assert!(snap.contains(n(1)));
    assert!(snap.contains(n(2)));
    assert_eq!(snap.children(n(1)), &[n(2)]);
    let p: HashSet<_> = snap.parents(n(2)).into_iter().collect();
    assert_eq!(p, id_set(&[1]));
}

// 3. DeleteCascade
#[test]
fn delete_cascade_removes_entire_subtree() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_node(n(4)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(2), n(3)).unwrap();
    h.add_edge(n(1), n(4)).unwrap();

    h.remove_node(n(1), DeletionStrategy::DeleteCascade)
        .unwrap();

    let snap = h.snapshot();
    assert_eq!(snap.node_count(), 0);
    assert!(!snap.contains(n(1)));
    assert!(!snap.contains(n(2)));
    assert!(!snap.contains(n(3)));
    assert!(!snap.contains(n(4)));
    assert!(snap.roots().is_empty());
}

// 4. DeleteReparent (Valid Case)
#[test]
fn delete_reparent_moves_children() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(2), n(3)).unwrap();

    h.remove_node(n(2), DeletionStrategy::DeleteReparent { new_parent: n(1) })
        .unwrap();

    let snap = h.snapshot();
    assert!(!snap.contains(n(2)));
    assert!(snap.contains(n(3)));
    let p: HashSet<_> = snap.parents(n(3)).into_iter().collect();
    assert_eq!(p, id_set(&[1]));

    let children_of_a: HashSet<_> = snap.children(n(1)).iter().copied().collect();
    assert!(children_of_a.contains(&n(3)));
    // C appears exactly once
    assert_eq!(
        snap.children(n(1)).iter().filter(|&&c| c == n(3)).count(),
        1
    );

    // No cycles: A's descendants should include C only
    let desc: HashSet<_> = snap.descendants(n(1)).into_iter().collect();
    assert_eq!(desc, id_set(&[3]));
}

// 5. DeleteReparent (Cycle Protection)
#[test]
fn delete_reparent_rejects_descendant_parent() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(2), n(3)).unwrap();

    let err = h
        .remove_node(n(1), DeletionStrategy::DeleteReparent { new_parent: n(3) })
        .unwrap_err();
    assert_eq!(
        err,
        HierarchyError::ReparentCycleDetected {
            node: n(1),
            new_parent: n(3)
        }
    );

    // Hierarchy unchanged
    let snap = h.snapshot();
    assert_eq!(snap.node_count(), 3);
    assert_eq!(snap.children(n(1)), &[n(2)]);
    assert_eq!(snap.children(n(2)), &[n(3)]);
}

// 6. DeleteDetach Orphaning
#[test]
fn delete_detach_orphans_children() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(2), n(3)).unwrap();

    h.remove_node(n(2), DeletionStrategy::DeleteDetach).unwrap();

    let snap = h.snapshot();
    assert!(!snap.contains(n(2)));
    assert!(snap.contains(n(3)));
    assert!(snap.parents(n(3)).is_empty());

    let roots: HashSet<_> = snap.roots().into_iter().collect();
    assert!(roots.contains(&n(3)));

    assert!(snap.contains(n(1)));
    assert_eq!(snap.children(n(1)), &[] as &[HierarchyNodeId]);
}

// 7. Reorder Children Validation
#[test]
fn reorder_children_requires_exact_match() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_node(n(4)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(1), n(3)).unwrap();

    // Valid reorder
    h.reorder_children(n(1), &[n(3), n(2)]).unwrap();
    assert_eq!(h.snapshot().children(n(1)), &[n(3), n(2)]);

    // Too few
    let err = h.reorder_children(n(1), &[n(2)]).unwrap_err();
    assert_eq!(err, HierarchyError::ReorderMismatch { parent: n(1) });

    // Wrong elements
    let err = h.reorder_children(n(1), &[n(2), n(4)]).unwrap_err();
    assert_eq!(err, HierarchyError::ReorderMismatch { parent: n(1) });
}

// 8. Snapshot Stability / Read-Only Guarantee
#[test]
fn snapshot_is_side_effect_free() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(2), n(3)).unwrap();

    // Take snapshot, exercise all read methods
    let snap = h.snapshot();
    let _ = snap.nodes().collect::<Vec<_>>();
    let _ = snap.node_count();
    let _ = snap.contains(n(1));
    let _ = snap.children(n(1));
    let _ = snap.parents(n(3));
    let _ = snap.roots();
    let _ = snap.ancestors(n(3));
    let _ = snap.descendants(n(1));
    drop(snap);

    // State identical after snapshot usage
    let snap2 = h.snapshot();
    assert_eq!(snap2.node_count(), 3);
    assert_eq!(snap2.children(n(1)), &[n(2)]);
    assert_eq!(snap2.children(n(2)), &[n(3)]);
    let anc: HashSet<_> = snap2.ancestors(n(3)).into_iter().collect();
    assert_eq!(anc, id_set(&[1, 2]));
    let desc: HashSet<_> = snap2.descendants(n(1)).into_iter().collect();
    assert_eq!(desc, id_set(&[2, 3]));
}

// 9. Ancestors / Descendants Correctness (DAG)
#[test]
fn ancestors_and_descendants_are_correct() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_node(n(4)).unwrap();
    h.add_edge(n(1), n(2)).unwrap();
    h.add_edge(n(1), n(3)).unwrap();
    h.add_edge(n(2), n(4)).unwrap();

    let snap = h.snapshot();

    let anc_d: HashSet<_> = snap.ancestors(n(4)).into_iter().collect();
    assert_eq!(anc_d, id_set(&[2, 1]));

    let desc_a: HashSet<_> = snap.descendants(n(1)).into_iter().collect();
    assert_eq!(desc_a, id_set(&[2, 3, 4]));
}

// 10.5. Taxonomy: Multiple Parents (DAG)
#[test]
fn taxonomy_multiple_parents_supported() {
    let mut h = HierarchyAuthority::new();
    h.add_node(n(1)).unwrap();
    h.add_node(n(2)).unwrap();
    h.add_node(n(3)).unwrap();
    h.add_edge(n(1), n(3)).unwrap();
    h.add_edge(n(2), n(3)).unwrap();

    let snap = h.snapshot();

    let p: HashSet<_> = snap.parents(n(3)).into_iter().collect();
    assert_eq!(p, id_set(&[1, 2]));

    let anc: HashSet<_> = snap.ancestors(n(3)).into_iter().collect();
    assert_eq!(anc, id_set(&[1, 2]));

    let desc_a: HashSet<_> = snap.descendants(n(1)).into_iter().collect();
    assert!(desc_a.contains(&n(3)));

    let desc_b: HashSet<_> = snap.descendants(n(2)).into_iter().collect();
    assert!(desc_b.contains(&n(3)));
}

// 11. Deep Structure / Recursion Protection
#[test]
fn deep_chain_does_not_overflow() {
    let mut h = HierarchyAuthority::new();
    let count = 10_000u64;

    for i in 0..count {
        h.add_node(n(i)).unwrap();
    }
    for i in 0..count - 1 {
        h.add_edge(n(i), n(i + 1)).unwrap();
    }

    let snap = h.snapshot();
    let desc = snap.descendants(n(0));
    assert_eq!(desc.len(), (count - 1) as usize);
    drop(snap);

    h.remove_node(n(0), DeletionStrategy::DeleteCascade)
        .unwrap();

    let snap = h.snapshot();
    assert_eq!(snap.node_count(), 0);
    assert!(snap.roots().is_empty());
}
