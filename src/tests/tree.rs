// =============================================================================
// Tree Model Unit Tests (White-Box)
// =============================================================================
//
// Tests for Tree = Substrate + Interaction Semantics.
//
// Categories:
// 1. Focus initialization
// 2. Up/down navigation boundaries
// 3. Right expands then moves to child
// 4. Left collapses then moves to parent
// 5. Collapse hides focused child → focus revalidates to ancestor
// 6. Adapter reconciliation removes subtree → focus revalidates
// 7. ExpandAll/CollapseAll preserve deterministic navigation
// =============================================================================

use std::collections::HashMap;

use crate::model::tree::Tree;
use crate::model::tree_types::{TreeAdapter, TreeNodeId};

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

    fn remove_node(&mut self, id: TreeNodeId) {
        self.roots.retain(|&r| r != id);
        self.children_map.remove(&id);
        for children in self.children_map.values_mut() {
            children.retain(|&c| c != id);
        }
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

/// Build a standard 3-level test tree:
///
///   1
///   ├── 2
///   │   ├── 4
///   │   └── 5
///   └── 3
///       ├── 6
///       └── 7
fn build_standard_tree() -> TestAdapter {
    let mut a = TestAdapter::new();
    a.add_root(n(1));
    a.add_child(n(1), n(2));
    a.add_child(n(1), n(3));
    a.add_child(n(2), n(4));
    a.add_child(n(2), n(5));
    a.add_child(n(3), n(6));
    a.add_child(n(3), n(7));
    a
}

// =============================================================================
// 1. Focus Initialization
// =============================================================================

#[test]
fn focus_init_empty_tree() {
    let adapter = TestAdapter::new();
    let tree = Tree::new(adapter);

    assert!(tree.projection().is_empty());
    assert_eq!(tree.focused(), None);
    assert_eq!(tree.focused_index(), None);
}

#[test]
fn focus_init_nonempty_tree() {
    let adapter = build_standard_tree();
    let tree = Tree::new(adapter);

    // Only root visible initially (nothing expanded).
    assert_eq!(tree.projection().len(), 1);
    assert_eq!(tree.focused(), Some(n(1)));
    assert_eq!(tree.focused_index(), Some(0));
}

#[test]
fn focus_init_multiple_roots() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(10));
    adapter.add_root(n(20));
    adapter.add_root(n(30));
    let tree = Tree::new(adapter);

    assert_eq!(tree.projection().len(), 3);
    // Focus should be first row.
    assert_eq!(tree.focused(), Some(n(10)));
    assert_eq!(tree.focused_index(), Some(0));
}

// =============================================================================
// 2. Up/Down Navigation Boundaries
// =============================================================================

#[test]
fn move_down_basic() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_root(n(2));
    adapter.add_root(n(3));
    let mut tree = Tree::new(adapter);

    assert_eq!(tree.focused(), Some(n(1)));

    tree.move_down();
    assert_eq!(tree.focused(), Some(n(2)));

    tree.move_down();
    assert_eq!(tree.focused(), Some(n(3)));
}

#[test]
fn move_down_at_bottom_boundary() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_root(n(2));
    let mut tree = Tree::new(adapter);

    tree.move_down(); // → n(2)
    tree.move_down(); // Already at bottom, should stay.
    assert_eq!(tree.focused(), Some(n(2)));
}

#[test]
fn move_up_basic() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_root(n(2));
    adapter.add_root(n(3));
    let mut tree = Tree::new(adapter);

    tree.move_down(); // → n(2)
    tree.move_down(); // → n(3)
    tree.move_up(); // → n(2)
    assert_eq!(tree.focused(), Some(n(2)));
}

#[test]
fn move_up_at_top_boundary() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    adapter.add_root(n(2));
    let mut tree = Tree::new(adapter);

    tree.move_up(); // Already at top, should stay.
    assert_eq!(tree.focused(), Some(n(1)));
}

#[test]
fn move_up_down_on_empty_tree() {
    let adapter = TestAdapter::new();
    let mut tree = Tree::new(adapter);

    tree.move_up();
    assert_eq!(tree.focused(), None);

    tree.move_down();
    assert_eq!(tree.focused(), None);
}

// =============================================================================
// 3. Right Expands Then Moves to Child
// =============================================================================

#[test]
fn right_expands_collapsed_node() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    // Root is collapsed, has children.
    assert!(!tree.is_expanded(n(1)));
    assert_eq!(tree.focused(), Some(n(1)));

    // Right → should expand, NOT move.
    tree.move_right();
    assert!(tree.is_expanded(n(1)));
    assert_eq!(tree.focused(), Some(n(1)));

    // Projection now: 1, 2, 3
    assert_eq!(tree.projection().len(), 3);
}

#[test]
fn right_moves_to_child_when_expanded() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    // Expand root first.
    tree.move_right(); // Expands n(1).
    assert!(tree.is_expanded(n(1)));
    assert_eq!(tree.focused(), Some(n(1)));

    // Right again → move to first child.
    tree.move_right();
    assert_eq!(tree.focused(), Some(n(2)));
}

#[test]
fn right_on_leaf_does_nothing() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    // Expand root, then expand n(2), focus on n(4) (leaf).
    tree.expand(n(1));
    tree.expand(n(2));
    // Projection: 1, 2, 4, 5, 3
    // Navigate to n(4).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    assert_eq!(tree.focused(), Some(n(4)));

    // Right on leaf → nothing.
    tree.move_right();
    assert_eq!(tree.focused(), Some(n(4)));
}

// =============================================================================
// 4. Left Collapses Then Moves to Parent
// =============================================================================

#[test]
fn left_collapses_expanded_node() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand(n(1));
    assert!(tree.is_expanded(n(1)));
    assert_eq!(tree.focused(), Some(n(1)));

    // Left on expanded → collapse.
    tree.move_left();
    assert!(!tree.is_expanded(n(1)));
    assert_eq!(tree.focused(), Some(n(1)));

    // Back to just root visible.
    assert_eq!(tree.projection().len(), 1);
}

#[test]
fn left_moves_to_parent_when_collapsed() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand(n(1));
    // Projection: 1, 2, 3. Focus on n(2).
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(2)));

    // n(2) is collapsed (has children but not expanded).
    // Left → move to parent (n(1)).
    tree.move_left();
    assert_eq!(tree.focused(), Some(n(1)));
}

#[test]
fn left_on_root_does_nothing() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    assert_eq!(tree.focused(), Some(n(1)));

    // Left on root (collapsed, depth 0) → nothing.
    tree.move_left();
    assert_eq!(tree.focused(), Some(n(1)));
}

#[test]
fn left_deep_navigation() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    // Expand all the way down.
    tree.expand(n(1));
    tree.expand(n(2));
    // Projection: 1, 2, 4, 5, 3

    // Navigate to n(4) (leaf, depth 2).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    assert_eq!(tree.focused(), Some(n(4)));

    // Left → n(4) is leaf (collapsed), move to parent n(2).
    tree.move_left();
    assert_eq!(tree.focused(), Some(n(2)));

    // Left → n(2) is expanded, collapse it.
    tree.move_left();
    assert!(!tree.is_expanded(n(2)));
    assert_eq!(tree.focused(), Some(n(2)));

    // Left → n(2) now collapsed, move to parent n(1).
    tree.move_left();
    assert_eq!(tree.focused(), Some(n(1)));
}

// =============================================================================
// 5. Collapse Hides Focused Child → Focus Revalidates to Ancestor
// =============================================================================

#[test]
fn collapse_parent_revalidates_focus_to_ancestor() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand(n(1));
    tree.expand(n(2));
    // Projection: 1, 2, 4, 5, 3

    // Focus on n(4).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    assert_eq!(tree.focused(), Some(n(4)));

    // Collapse n(1) → hides everything under it.
    tree.collapse(n(1));
    // Projection: 1 only.
    assert_eq!(tree.projection().len(), 1);
    // Focus should revalidate to n(1) (nearest visible ancestor).
    assert_eq!(tree.focused(), Some(n(1)));
}

#[test]
fn collapse_grandparent_revalidates_deep_focus() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand(n(1));
    tree.expand(n(2));
    // Projection: 1, 2, 4, 5, 3

    // Focus on n(5).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    tree.move_down(); // → n(5)
    assert_eq!(tree.focused(), Some(n(5)));

    // Collapse n(2) → hides n(4) and n(5).
    tree.collapse(n(2));
    // Projection: 1, 2, 3
    assert_eq!(tree.projection().len(), 3);
    // Focus should revalidate to n(2) (direct parent, still visible).
    assert_eq!(tree.focused(), Some(n(2)));
}

// =============================================================================
// 6. Adapter Reconciliation Removes Subtree → Focus Revalidates
// =============================================================================

#[test]
fn reconcile_removes_focused_node() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand(n(1));
    // Projection: 1, 2, 3

    // Focus on n(2).
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(2)));

    // Remove n(2) from adapter.
    tree.adapter_mut().remove_node(n(2));
    tree.reconcile();

    // n(2) gone. Projection: 1, 3
    assert_eq!(tree.projection().len(), 2);
    // Focus should revalidate to n(1) (parent of removed node).
    assert_eq!(tree.focused(), Some(n(1)));
}

#[test]
fn reconcile_removes_entire_subtree_with_deep_focus() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand(n(1));
    tree.expand(n(2));
    // Projection: 1, 2, 4, 5, 3

    // Focus on n(5).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    tree.move_down(); // → n(5)
    assert_eq!(tree.focused(), Some(n(5)));

    // Remove n(2) and all its children from adapter.
    tree.adapter_mut().remove_node(n(4));
    tree.adapter_mut().remove_node(n(5));
    tree.adapter_mut().remove_node(n(2));
    tree.reconcile();

    // Projection: 1, 3 (n(1) still expanded but n(2) subtree gone).
    assert_eq!(tree.projection().len(), 2);
    // Focus should revalidate to n(1) (ancestor).
    assert_eq!(tree.focused(), Some(n(1)));
}

#[test]
fn reconcile_empties_tree() {
    let mut adapter = TestAdapter::new();
    adapter.add_root(n(1));
    let mut tree = Tree::new(adapter);

    assert_eq!(tree.focused(), Some(n(1)));

    tree.adapter_mut().remove_node(n(1));
    tree.reconcile();

    assert!(tree.projection().is_empty());
    assert_eq!(tree.focused(), None);
}

// =============================================================================
// 6b. Focus Authority: empty → non-empty transitions
// =============================================================================

#[test]
fn reconcile_empty_to_nonempty_sets_focus() {
    // T1: Start empty, mutate adapter to add root, reconcile → focus set.
    let adapter = TestAdapter::new();
    let mut tree = Tree::new(adapter);

    assert!(tree.projection().is_empty());
    assert_eq!(tree.focused(), None);

    // Add a root and bump revision.
    tree.adapter_mut().add_root(n(100));
    tree.adapter_mut().rev += 1;
    tree.reconcile();

    assert_eq!(tree.projection().len(), 1);
    assert_eq!(tree.focused(), Some(n(100)));
    assert_eq!(tree.focused_index(), Some(0));
}

#[test]
fn reconcile_empty_to_nonempty_then_root_replacement_keeps_focus_valid() {
    // T2: empty → non-empty → root replacement → reconcile → focus valid and non-None.
    let adapter = TestAdapter::new();
    let mut tree = Tree::new(adapter);

    // Phase 1: empty → non-empty.
    tree.adapter_mut().add_root(n(50));
    tree.adapter_mut().rev += 1;
    tree.reconcile();
    assert_eq!(tree.focused(), Some(n(50)));

    // Phase 2: replace root n(50) with n(60).
    tree.adapter_mut().roots.retain(|&r| r != n(50));
    tree.adapter_mut().children_map.remove(&n(50));
    tree.adapter_mut().roots.push(n(60));
    tree.adapter_mut().rev += 1;
    tree.reconcile();

    // n(50) gone, projection has only n(60).
    assert_eq!(tree.projection().len(), 1);
    assert!(tree.focused().is_some());
    assert_eq!(tree.focused(), Some(n(60)));
    assert_eq!(tree.focused_index(), Some(0));
}

#[test]
fn collapse_all_deep_focus_guarantees_non_none() {
    // T3: collapse_all from deep focused node, projection non-empty ⇒ focused non-None == root.
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand_all(n(1));
    // Full tree: 1, 2, 4, 5, 3, 6, 7

    // Focus on n(7) (deepest leaf).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    tree.move_down(); // → n(5)
    tree.move_down(); // → n(3)
    tree.move_down(); // → n(6)
    tree.move_down(); // → n(7)
    assert_eq!(tree.focused(), Some(n(7)));

    // CollapseAll from root.
    tree.collapse_all(n(1));
    assert_eq!(tree.projection().len(), 1);
    assert!(
        tree.focused().is_some(),
        "focused must be non-None when projection non-empty"
    );
    assert_eq!(tree.focused(), Some(n(1)));
}

// =============================================================================
// 7. ExpandAll/CollapseAll Preserve Deterministic Navigation
// =============================================================================

#[test]
fn expand_all_reveals_full_subtree() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand_all(n(1));
    // Full tree visible: 1, 2, 4, 5, 3, 6, 7
    assert_eq!(tree.projection().len(), 7);
    assert_eq!(tree.focused(), Some(n(1)));

    // Navigate through all nodes.
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(2)));
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(4)));
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(5)));
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(3)));
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(6)));
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(7)));
    // Bottom boundary.
    tree.move_down();
    assert_eq!(tree.focused(), Some(n(7)));
}

#[test]
fn collapse_all_hides_subtree_revalidates_focus() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand_all(n(1));
    // Full tree: 1, 2, 4, 5, 3, 6, 7

    // Focus on n(6).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    tree.move_down(); // → n(5)
    tree.move_down(); // → n(3)
    tree.move_down(); // → n(6)
    assert_eq!(tree.focused(), Some(n(6)));

    // CollapseAll from root.
    tree.collapse_all(n(1));
    // Only root visible.
    assert_eq!(tree.projection().len(), 1);
    // Focus revalidates to n(1).
    assert_eq!(tree.focused(), Some(n(1)));
}

#[test]
fn collapse_all_subtree_preserves_sibling_focus() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand_all(n(1));
    // Full tree: 1, 2, 4, 5, 3, 6, 7

    // Focus on n(3) (sibling of n(2)).
    tree.move_down(); // → n(2)
    tree.move_down(); // → n(4)
    tree.move_down(); // → n(5)
    tree.move_down(); // → n(3)
    assert_eq!(tree.focused(), Some(n(3)));

    // CollapseAll n(2) subtree only.
    tree.collapse_all(n(2));
    // Projection: 1, 2, 3, 6, 7 (n(3) still expanded).
    assert_eq!(tree.projection().len(), 5);
    // Focus on n(3) is still valid.
    assert_eq!(tree.focused(), Some(n(3)));
}

#[test]
fn expand_all_then_navigate_up_down() {
    let adapter = build_standard_tree();
    let mut tree = Tree::new(adapter);

    tree.expand_all(n(1));
    // 1, 2, 4, 5, 3, 6, 7

    // Go to bottom.
    for _ in 0..6 {
        tree.move_down();
    }
    assert_eq!(tree.focused(), Some(n(7)));

    // Go back to top.
    for _ in 0..6 {
        tree.move_up();
    }
    assert_eq!(tree.focused(), Some(n(1)));
}
