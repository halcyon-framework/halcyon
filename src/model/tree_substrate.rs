// =============================================================================
// SPEC-HAL-251: TreeSubstrate State Model & Projection Law
// SPEC-HAL-254: TreeSubstrate Internal State Structure & Storage Layout
// =============================================================================
//
// Hierarchical projection engine. Projects a tree-structured dataset into
// a flat, deterministic, virtualizable row stream.
//
// Full rebuild on every structural mutation. No incremental diffing.
// No UI. No rendering. No geometry. Only substrate state.
// =============================================================================

use std::collections::{HashMap, HashSet};

use super::tree_types::{ProjectedNode, TreeAdapter, TreeExpansionOp, TreeNodeId};

/// Hierarchical projection engine.
///
/// SPEC-HAL-254 §3: Canonical stored state. Stores expansion state and
/// visible linear projection. Does NOT store topology, geometry, UI state,
/// selection, or focus. Separation of concerns is absolute.
pub struct TreeSubstrate<A: TreeAdapter> {
    adapter: A,

    /// Expanded node set.
    /// Node ∈ expanded ⇒ children are visible in projection.
    /// SPEC-HAL-254 §5: If node not in set ⇒ collapsed.
    expanded: HashSet<TreeNodeId>,

    /// Linear visible projection (depth-first pre-order).
    /// SPEC-HAL-254 §6: Index in vector = row index for virtualization.
    projection: Vec<ProjectedNode>,

    /// O(1) row lookup by node ID.
    /// SPEC-HAL-254 §7: projection[row_index[node]].node_id == node
    row_index: HashMap<TreeNodeId, usize>,

    /// Revision counter. Increments on every rebuild.
    /// SPEC-HAL-254 §8: Every structural mutation increments revision.
    revision: u64,

    /// Last seen adapter revision for reconciliation.
    adapter_revision: u64,
}

impl<A: TreeAdapter> TreeSubstrate<A> {
    /// Create a new TreeSubstrate with the given adapter.
    /// Performs initial projection rebuild.
    pub fn new(adapter: A) -> Self {
        let adapter_rev = adapter.revision();
        let mut s = Self {
            adapter,
            expanded: HashSet::new(),
            projection: Vec::new(),
            row_index: HashMap::new(),
            revision: 0,
            adapter_revision: adapter_rev,
        };
        s.rebuild();
        s
    }

    /// Current linear projection of visible nodes.
    pub fn projection(&self) -> &[ProjectedNode] {
        &self.projection
    }

    /// O(1) row index lookup for a node.
    pub fn row_index(&self, node: TreeNodeId) -> Option<usize> {
        self.row_index.get(&node).copied()
    }

    /// Check if a node is in the expanded set.
    pub fn is_expanded(&self, node: TreeNodeId) -> bool {
        self.expanded.contains(&node)
    }

    /// Test-only: clone of expanded set for direct invariant verification.
    #[cfg(test)]
    pub(crate) fn expanded_clone(&self) -> HashSet<TreeNodeId> {
        self.expanded.clone()
    }

    /// Apply an expansion operation.
    ///
    /// SPEC-HAL-254 §11: Mutate expanded, validate, rebuild.
    /// No incremental projection diffing allowed.
    pub fn apply_expansion(&mut self, op: TreeExpansionOp) {
        match op {
            TreeExpansionOp::Expand(id) => {
                // INV-TREE-STATE-001: Only expandable nodes (has_children).
                if self.adapter.has_children(id) {
                    self.expanded.insert(id);
                }
            }
            TreeExpansionOp::Collapse(id) => {
                // SPEC-HAL-254 §12: Remove from expanded, do NOT remove descendants.
                // Preserves subtree expansion state for re-expand.
                self.expanded.remove(&id);
            }
            TreeExpansionOp::Toggle(id) => {
                if self.expanded.contains(&id) {
                    self.expanded.remove(&id);
                } else if self.adapter.has_children(id) {
                    self.expanded.insert(id);
                }
            }
            TreeExpansionOp::ExpandAll(id) => self.expand_all(id),
            TreeExpansionOp::CollapseAll(id) => self.collapse_all(id),
        }
        self.rebuild();
    }

    /// Reconcile with adapter revision changes.
    ///
    /// SPEC-HAL-254 §13: If adapter revision changed, remove stale
    /// expansion entries (non-existent or non-expandable nodes) and
    /// rebuild projection. Must never panic on stale entries.
    pub fn reconcile_adapter_revision(&mut self) {
        let current_rev = self.adapter.revision();
        if current_rev == self.adapter_revision {
            return;
        }
        self.adapter_revision = current_rev;

        // INV-TREE-STATE-001: Every node in expanded must be reachable and expandable.
        // Compute full reachability from roots via adapter topology.
        let adapter = &self.adapter;
        let mut reachable = HashSet::new();
        let mut stack: Vec<TreeNodeId> = adapter.roots().to_vec();
        while let Some(node) = stack.pop() {
            if reachable.insert(node) {
                for &child in adapter.children(node) {
                    stack.push(child);
                }
            }
        }
        // Node must be reachable from roots AND expandable.
        self.expanded
            .retain(|&id| reachable.contains(&id) && adapter.has_children(id));

        self.rebuild();
    }

    /// Current substrate revision.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Immutable access to the adapter.
    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    /// Mutable access to the adapter.
    pub fn adapter_mut(&mut self) -> &mut A {
        &mut self.adapter
    }

    // =========================================================================
    // Private: Rebuild
    // =========================================================================

    /// Full projection rebuild.
    ///
    /// SPEC-HAL-254 §9.2: Clear projection, clear row_index, traverse
    /// depth-first pre-order, rebuild row_index, increment revision.
    ///
    /// Iterative to avoid stack overflow on deep trees (SPEC-HAL-251 Case G).
    fn rebuild(&mut self) {
        self.projection.clear();
        self.row_index.clear();

        // Iterative depth-first pre-order traversal.
        // Stack elements: (node, depth).
        // Push roots in reverse so first root is processed first.
        let roots: Vec<TreeNodeId> = self.adapter.roots().to_vec();
        let mut stack: Vec<(TreeNodeId, u16)> = Vec::with_capacity(roots.len());
        for &root in roots.iter().rev() {
            stack.push((root, 0));
        }

        while let Some((node, depth)) = stack.pop() {
            let idx = self.projection.len();
            self.projection.push(ProjectedNode {
                node_id: node,
                depth,
            });
            self.row_index.insert(node, idx);

            if self.expanded.contains(&node) {
                let children: &[TreeNodeId] = self.adapter.children(node);
                // Push in reverse so first child is processed first.
                for &child in children.iter().rev() {
                    let next_depth = depth.checked_add(1).expect("Tree depth overflow (u16)");
                    stack.push((child, next_depth));
                }
            }
        }

        // Reachability pruning: Expanded ⊆ ReachableNodes.
        // Walk full adapter topology (all children, regardless of expansion)
        // to find every node reachable from roots. Prune expanded set.
        let mut reachable = HashSet::with_capacity(self.projection.len());
        let mut reach_stack: Vec<TreeNodeId> = self.adapter.roots().to_vec();
        while let Some(node) = reach_stack.pop() {
            if reachable.insert(node) {
                for &child in self.adapter.children(node) {
                    reach_stack.push(child);
                }
            }
        }
        self.expanded.retain(|id| reachable.contains(id));

        self.revision += 1;

        debug_assert!(self.verify_invariants());
    }

    /// Iterative expand-all: expand node and all descendants.
    /// SPEC-HAL-253 §5: INV-EXP-003 — O(size_of_subtree).
    fn expand_all(&mut self, root: TreeNodeId) {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if self.adapter.has_children(node) {
                self.expanded.insert(node);
                for &child in self.adapter.children(node) {
                    stack.push(child);
                }
            }
        }
    }

    /// Iterative collapse-all: collapse node and all descendants.
    /// SPEC-HAL-253 §5: INV-EXP-003 — O(size_of_subtree).
    fn collapse_all(&mut self, root: TreeNodeId) {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            self.expanded.remove(&node);
            for &child in self.adapter.children(node) {
                stack.push(child);
            }
        }
    }

    /// Verify structural invariants (debug builds only).
    fn verify_invariants(&self) -> bool {
        // INV-TREE-STATE-002: No duplicate node IDs in projection.
        let mut seen = HashSet::with_capacity(self.projection.len());
        for p in &self.projection {
            if !seen.insert(p.node_id) {
                return false;
            }
        }

        // SPEC-HAL-254 §7: projection[row_index[node]].node_id == node
        for (&node, &idx) in &self.row_index {
            if idx >= self.projection.len() || self.projection[idx].node_id != node {
                return false;
            }
        }

        // row_index must cover all projected nodes.
        if self.row_index.len() != self.projection.len() {
            return false;
        }

        true
    }
}
