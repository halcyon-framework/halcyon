// =============================================================================
// Tree Model: Substrate + Interaction Semantics
// =============================================================================
//
// Minimal Tree model layer. Owns TreeSubstrate, focus state, and navigation
// semantics (up/down/left/right). Exposes a stable projection interface.
//
// No UI. No rendering. No geometry. Only semantic engine.
// =============================================================================

use super::tree_substrate::TreeSubstrate;
use super::tree_types::{ProjectedNode, TreeAdapter, TreeExpansionOp, TreeNodeId};

/// Tree model: substrate + focus + navigation.
///
/// Single-focus only. No selection model.
pub struct Tree<A: TreeAdapter> {
    substrate: TreeSubstrate<A>,
    focused: Option<TreeNodeId>,
}

impl<A: TreeAdapter> Tree<A> {
    /// Create a new Tree with the given adapter.
    /// If projection non-empty, focus first row. Else focus = None.
    pub fn new(adapter: A) -> Self {
        let substrate = TreeSubstrate::new(adapter);
        let focused = substrate.projection().first().map(|p| p.node_id);
        Self { substrate, focused }
    }

    // =========================================================================
    // Projection Access
    // =========================================================================

    /// Current linear projection of visible nodes.
    pub fn projection(&self) -> &[ProjectedNode] {
        self.substrate.projection()
    }

    /// Current substrate revision.
    pub fn revision(&self) -> u64 {
        self.substrate.revision()
    }

    /// Currently focused node, if any.
    pub fn focused(&self) -> Option<TreeNodeId> {
        self.focused
    }

    /// Row index of the currently focused node, if any.
    pub fn focused_index(&self) -> Option<usize> {
        self.focused.and_then(|id| self.substrate.row_index(id))
    }

    // =========================================================================
    // Navigation API
    // =========================================================================

    /// Move focus to previous visible row.
    pub fn move_up(&mut self) {
        if let Some(idx) = self.focused_index() {
            if idx > 0 {
                self.focused = Some(self.substrate.projection()[idx - 1].node_id);
            }
        }
        debug_assert!(self.verify_focus_invariant());
    }

    /// Move focus to next visible row.
    pub fn move_down(&mut self) {
        if let Some(idx) = self.focused_index() {
            let proj = self.substrate.projection();
            if idx + 1 < proj.len() {
                self.focused = Some(proj[idx + 1].node_id);
            }
        }
        debug_assert!(self.verify_focus_invariant());
    }

    /// Right: if focused has children and is collapsed → expand.
    /// Else if expanded → move to first child (index + 1).
    pub fn move_right(&mut self) {
        if let Some(id) = self.focused {
            if self.substrate.adapter().has_children(id) {
                if !self.substrate.is_expanded(id) {
                    // Collapsed with children → expand.
                    self.substrate.apply_expansion(TreeExpansionOp::Expand(id));
                    self.revalidate_focus();
                } else {
                    // Expanded → move to first child (next row in projection).
                    if let Some(idx) = self.substrate.row_index(id) {
                        let proj = self.substrate.projection();
                        if idx + 1 < proj.len() {
                            self.focused = Some(proj[idx + 1].node_id);
                        }
                    }
                }
            }
        }
        debug_assert!(self.verify_focus_invariant());
    }

    /// Left: if focused is expanded → collapse.
    /// Else → move focus to parent (walk backward by depth).
    pub fn move_left(&mut self) {
        if let Some(id) = self.focused {
            if self.substrate.is_expanded(id) {
                // Expanded → collapse.
                self.substrate
                    .apply_expansion(TreeExpansionOp::Collapse(id));
                self.revalidate_focus();
            } else {
                // Not expanded → move to parent.
                if let Some(parent) = self.find_parent(id) {
                    self.focused = Some(parent);
                }
            }
        }
        debug_assert!(self.verify_focus_invariant());
    }

    /// Expand a specific node.
    pub fn expand(&mut self, id: TreeNodeId) {
        self.substrate.apply_expansion(TreeExpansionOp::Expand(id));
        self.revalidate_focus();
    }

    /// Collapse a specific node.
    pub fn collapse(&mut self, id: TreeNodeId) {
        self.substrate
            .apply_expansion(TreeExpansionOp::Collapse(id));
        self.revalidate_focus();
    }

    /// Toggle expansion of a specific node.
    pub fn toggle(&mut self, id: TreeNodeId) {
        self.substrate.apply_expansion(TreeExpansionOp::Toggle(id));
        self.revalidate_focus();
    }

    /// Expand node and all descendants.
    pub fn expand_all(&mut self, id: TreeNodeId) {
        self.substrate
            .apply_expansion(TreeExpansionOp::ExpandAll(id));
        self.revalidate_focus();
    }

    /// Collapse node and all descendants.
    pub fn collapse_all(&mut self, id: TreeNodeId) {
        self.substrate
            .apply_expansion(TreeExpansionOp::CollapseAll(id));
        self.revalidate_focus();
    }

    // =========================================================================
    // Adapter Reconciliation
    // =========================================================================

    /// Reconcile with adapter revision changes, then revalidate focus.
    pub fn reconcile(&mut self) {
        self.substrate.reconcile_adapter_revision();
        self.revalidate_focus();
    }

    // =========================================================================
    // Adapter Access
    // =========================================================================

    /// Immutable access to the adapter.
    pub fn adapter(&self) -> &A {
        self.substrate.adapter()
    }

    /// Mutable access to the adapter.
    pub fn adapter_mut(&mut self) -> &mut A {
        self.substrate.adapter_mut()
    }

    /// O(1) row index lookup for a node. None if not in projection.
    pub fn row_index(&self, node: TreeNodeId) -> Option<usize> {
        self.substrate.row_index(node)
    }

    /// Check if a node is expanded.
    pub fn is_expanded(&self, id: TreeNodeId) -> bool {
        self.substrate.is_expanded(id)
    }

    // =========================================================================
    // Private: Focus Revalidation
    // =========================================================================

    /// Revalidate focus after any mutation.
    ///
    /// If focused node no longer in projection:
    /// - Move focus to nearest visible ancestor.
    /// - If none → clamp to first visible.
    /// - If projection empty → focus = None.
    fn revalidate_focus(&mut self) {
        let proj = self.substrate.projection();
        if proj.is_empty() {
            self.focused = None;
            return;
        }

        match self.focused {
            None => {
                // Projection non-empty but no focus → force to first visible.
                self.focused = Some(proj[0].node_id);
            }
            Some(id) => {
                // Still in projection → nothing to do.
                if self.substrate.row_index(id).is_some() {
                    return;
                }

                // Not in projection → find nearest visible ancestor.
                if let Some(ancestor) = self.find_nearest_visible_ancestor(id) {
                    self.focused = Some(ancestor);
                    return;
                }

                // No ancestor found → clamp to first visible.
                self.focused = Some(proj[0].node_id);
            }
        }

        debug_assert!(self.verify_focus_invariant());
    }

    /// Find parent of a node using projection depth logic.
    /// Walk backward from node's row until depth == current_depth - 1.
    fn find_parent(&self, node: TreeNodeId) -> Option<TreeNodeId> {
        let idx = self.substrate.row_index(node)?;
        let proj = self.substrate.projection();
        let current_depth = proj[idx].depth;
        if current_depth == 0 {
            return None; // Root node has no parent.
        }
        let target_depth = current_depth - 1;
        // Walk backward.
        for i in (0..idx).rev() {
            if proj[i].depth == target_depth {
                return Some(proj[i].node_id);
            }
        }
        None
    }

    /// Find the nearest visible ancestor of a node that's no longer in projection.
    /// Uses the adapter to walk the parent chain.
    fn find_nearest_visible_ancestor(&self, node: TreeNodeId) -> Option<TreeNodeId> {
        // Walk adapter parent chain by checking all projected nodes.
        // Since we don't store parent pointers, use brute force:
        // find which projected node is the ancestor of `node` at the
        // deepest level.
        //
        // Strategy: check if any node in projection is a parent of `node`
        // in the adapter topology. Walk from roots down and find the deepest
        // ancestor that contains `node` in its subtree.
        let proj = self.substrate.projection();

        // Build set of projected node IDs for fast lookup.
        let mut best: Option<(usize, TreeNodeId)> = None; // (depth, node_id)

        for p in proj {
            if self.is_ancestor_of(p.node_id, node) {
                match best {
                    None => best = Some((p.depth as usize, p.node_id)),
                    Some((d, _)) if (p.depth as usize) > d => {
                        best = Some((p.depth as usize, p.node_id))
                    }
                    _ => {}
                }
            }
        }

        best.map(|(_, id)| id)
    }

    /// Check if `ancestor` is an ancestor of `descendant` in the adapter topology.
    /// Iterative DFS from ancestor looking for descendant.
    fn is_ancestor_of(&self, ancestor: TreeNodeId, descendant: TreeNodeId) -> bool {
        let adapter = self.substrate.adapter();
        let mut stack = vec![ancestor];
        while let Some(node) = stack.pop() {
            for &child in adapter.children(node) {
                if child == descendant {
                    return true;
                }
                stack.push(child);
            }
        }
        false
    }

    /// Debug-only focus invariant check.
    /// R5: If projection non-empty, focused must be Some and in projection.
    ///     If projection empty, focused must be None.
    fn verify_focus_invariant(&self) -> bool {
        let proj = self.substrate.projection();
        if proj.is_empty() {
            return self.focused.is_none();
        }
        match self.focused {
            None => false, // Forbidden: projection non-empty requires focus.
            Some(id) => self.substrate.row_index(id).is_some(),
        }
    }
}
