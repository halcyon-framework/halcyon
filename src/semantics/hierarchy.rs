// SPEC-HAL-230: Hierarchy / Taxonomy Structural Authority
// Logical structural model — non-interactive, policy-free, atemporal.
//
// Binding invariants (mechanically true):
// INV-HIER-OWNERSHIP-1: Sole ownership of all structural relationships
// INV-HIER-NO-INTERACTION: No interaction semantics
// INV-HIER-NO-NAV: No navigation
// INV-HIER-NO-HISTORY: No history
// INV-HIER-NO-SELECTION: No selection or focus
// INV-HIER-NO-INFERENCE: No implicit meaning
// INV-HIER-DELETE-EXPLICIT: Deletion strategy required
// INV-HIER-VALIDATION: Structural integrity
// INV-HIER-NAV-ORTHOGONAL: No coupling to navigation
// INV-HIER-NO-POLICY: No access control
//
// All hierarchy traversals are iterative by design.
// Recursion is forbidden to protect against deep or malformed graphs.

use std::collections::{HashMap, HashSet};

/// Identifies a hierarchy node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HierarchyNodeId(pub u64);

/// Deletion strategy for node removal (INV-HIER-DELETE-EXPLICIT).
/// All node deletion MUST specify a strategy. No implicit defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionStrategy {
    /// Fails if node has children.
    DeleteLeaf,
    /// Deletes node and entire descendant subtree.
    DeleteCascade,
    /// Deletes node and reattaches direct children to an explicitly provided parent.
    DeleteReparent { new_parent: HierarchyNodeId },
    /// Deletes node and leaves children orphaned but valid.
    DeleteDetach,
}

/// Error type for hierarchy operations (INV-HIER-VALIDATION).
/// Failures are immediate and atomic. No silent repair. No fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HierarchyError {
    /// Node already exists.
    NodeAlreadyExists(HierarchyNodeId),
    /// Node does not exist.
    NodeNotFound(HierarchyNodeId),
    /// Edge already exists.
    EdgeAlreadyExists {
        parent: HierarchyNodeId,
        child: HierarchyNodeId,
    },
    /// Edge does not exist.
    EdgeNotFound {
        parent: HierarchyNodeId,
        child: HierarchyNodeId,
    },
    /// Adding this edge would create a cycle.
    CycleDetected {
        parent: HierarchyNodeId,
        child: HierarchyNodeId,
    },
    /// DeleteLeaf requires no children.
    NodeHasChildren(HierarchyNodeId),
    /// Reparent target does not exist or is the node being deleted.
    ReparentTargetNotFound(HierarchyNodeId),
    /// Reparent target is a descendant of the node being deleted.
    ReparentCycleDetected {
        node: HierarchyNodeId,
        new_parent: HierarchyNodeId,
    },
    /// Reorder children list does not match existing children.
    ReorderMismatch { parent: HierarchyNodeId },
}

/// Hierarchy / Taxonomy Structural Authority (SPEC-HAL-230).
///
/// Owns all nodes and edges. Validates all mutations.
/// Exposes read-only snapshots. No UI, no navigation, no permissions.
pub struct HierarchyAuthority {
    nodes: HashSet<HierarchyNodeId>,
    children: HashMap<HierarchyNodeId, Vec<HierarchyNodeId>>,
    parents: HashMap<HierarchyNodeId, HashSet<HierarchyNodeId>>,
}

impl HierarchyAuthority {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            children: HashMap::new(),
            parents: HashMap::new(),
        }
    }

    /// Add a node. Fails if node already exists.
    pub fn add_node(&mut self, node_id: HierarchyNodeId) -> Result<(), HierarchyError> {
        if !self.nodes.insert(node_id) {
            return Err(HierarchyError::NodeAlreadyExists(node_id));
        }
        Ok(())
    }

    /// Add a directed edge from parent to child.
    /// Both nodes must exist. No duplicate edges. No cycles.
    pub fn add_edge(
        &mut self,
        parent: HierarchyNodeId,
        child: HierarchyNodeId,
    ) -> Result<(), HierarchyError> {
        if !self.nodes.contains(&parent) {
            return Err(HierarchyError::NodeNotFound(parent));
        }
        if !self.nodes.contains(&child) {
            return Err(HierarchyError::NodeNotFound(child));
        }
        if parent == child {
            return Err(HierarchyError::CycleDetected { parent, child });
        }

        // Duplicate check
        if let Some(existing) = self.children.get(&parent) {
            if existing.contains(&child) {
                return Err(HierarchyError::EdgeAlreadyExists { parent, child });
            }
        }

        // Cycle detection: iterative DFS from child following child edges.
        // If parent is reachable from child, adding parent→child creates a cycle.
        if self.is_reachable_iterative(child, parent) {
            return Err(HierarchyError::CycleDetected { parent, child });
        }

        // Apply
        self.children.entry(parent).or_default().push(child);
        self.parents.entry(child).or_default().insert(parent);
        Ok(())
    }

    /// Remove a directed edge from parent to child.
    pub fn remove_edge(
        &mut self,
        parent: HierarchyNodeId,
        child: HierarchyNodeId,
    ) -> Result<(), HierarchyError> {
        if !self.nodes.contains(&parent) {
            return Err(HierarchyError::NodeNotFound(parent));
        }
        if !self.nodes.contains(&child) {
            return Err(HierarchyError::NodeNotFound(child));
        }

        let children_list = self
            .children
            .get_mut(&parent)
            .ok_or(HierarchyError::EdgeNotFound { parent, child })?;
        let pos = children_list
            .iter()
            .position(|c| *c == child)
            .ok_or(HierarchyError::EdgeNotFound { parent, child })?;
        children_list.remove(pos);
        if children_list.is_empty() {
            self.children.remove(&parent);
        }

        if let Some(parent_set) = self.parents.get_mut(&child) {
            parent_set.remove(&parent);
            if parent_set.is_empty() {
                self.parents.remove(&child);
            }
        }

        Ok(())
    }

    /// Remove a node with explicit deletion strategy (INV-HIER-DELETE-EXPLICIT).
    pub fn remove_node(
        &mut self,
        node_id: HierarchyNodeId,
        strategy: DeletionStrategy,
    ) -> Result<(), HierarchyError> {
        if !self.nodes.contains(&node_id) {
            return Err(HierarchyError::NodeNotFound(node_id));
        }
        match strategy {
            DeletionStrategy::DeleteLeaf => self.delete_leaf(node_id),
            DeletionStrategy::DeleteCascade => self.delete_cascade(node_id),
            DeletionStrategy::DeleteReparent { new_parent } => {
                self.delete_reparent(node_id, new_parent)
            }
            DeletionStrategy::DeleteDetach => self.delete_detach(node_id),
        }
    }

    /// Reorder children of a parent node.
    /// new_order must contain exactly the same children as currently exist.
    pub fn reorder_children(
        &mut self,
        parent: HierarchyNodeId,
        new_order: &[HierarchyNodeId],
    ) -> Result<(), HierarchyError> {
        if !self.nodes.contains(&parent) {
            return Err(HierarchyError::NodeNotFound(parent));
        }

        let current = self
            .children
            .get(&parent)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        if current.len() != new_order.len() {
            return Err(HierarchyError::ReorderMismatch { parent });
        }

        let current_set: HashSet<HierarchyNodeId> = current.iter().copied().collect();
        let new_set: HashSet<HierarchyNodeId> = new_order.iter().copied().collect();
        if current_set != new_set || new_set.len() != new_order.len() {
            return Err(HierarchyError::ReorderMismatch { parent });
        }

        self.children.insert(parent, new_order.to_vec());
        Ok(())
    }

    /// Create a read-only snapshot (side-effect free, no mutation handles).
    pub fn snapshot(&self) -> HierarchySnapshot<'_> {
        HierarchySnapshot { authority: self }
    }

    // --- Internal helpers (all iterative, no recursion) ---

    /// Iterative reachability: can `target` be reached from `start` via child edges?
    /// Uses explicit stack + visited set (INV-HIER-RECURSION-1).
    fn is_reachable_iterative(&self, start: HierarchyNodeId, target: HierarchyNodeId) -> bool {
        let mut stack = vec![start];
        let mut visited = HashSet::new();
        while let Some(node) = stack.pop() {
            if node == target {
                return true;
            }
            if !visited.insert(node) {
                continue;
            }
            if let Some(children) = self.children.get(&node) {
                for &child in children {
                    if !visited.contains(&child) {
                        stack.push(child);
                    }
                }
            }
        }
        false
    }

    /// Iterative descendant collection (INV-HIER-RECURSION-2: analyze phase only).
    fn collect_descendants_iterative(&self, start: HierarchyNodeId) -> Vec<HierarchyNodeId> {
        let mut result = Vec::new();
        let mut stack = vec![start];
        let mut visited = HashSet::new();
        visited.insert(start);
        while let Some(node) = stack.pop() {
            if let Some(children) = self.children.get(&node) {
                for &child in children {
                    if visited.insert(child) {
                        result.push(child);
                        stack.push(child);
                    }
                }
            }
        }
        result
    }

    /// DeleteLeaf: fails if node has children.
    fn delete_leaf(&mut self, node_id: HierarchyNodeId) -> Result<(), HierarchyError> {
        if let Some(children) = self.children.get(&node_id) {
            if !children.is_empty() {
                return Err(HierarchyError::NodeHasChildren(node_id));
            }
        }
        self.purge_node(node_id);
        Ok(())
    }

    /// DeleteCascade: two-phase (INV-HIER-RECURSION-2).
    /// Phase 1: collect all descendants iteratively.
    /// Phase 2: purge all (no traversal).
    fn delete_cascade(&mut self, node_id: HierarchyNodeId) -> Result<(), HierarchyError> {
        let descendants = self.collect_descendants_iterative(node_id);
        for &desc in &descendants {
            self.purge_node(desc);
        }
        self.purge_node(node_id);
        Ok(())
    }

    /// DeleteReparent: reattach direct children to new_parent, then delete node.
    fn delete_reparent(
        &mut self,
        node_id: HierarchyNodeId,
        new_parent: HierarchyNodeId,
    ) -> Result<(), HierarchyError> {
        if !self.nodes.contains(&new_parent) {
            return Err(HierarchyError::ReparentTargetNotFound(new_parent));
        }
        if node_id == new_parent {
            return Err(HierarchyError::ReparentTargetNotFound(new_parent));
        }

        // new_parent must not be a descendant of node_id (would create cycle after reparent)
        let descendants = self.collect_descendants_iterative(node_id);
        if descendants.contains(&new_parent) {
            return Err(HierarchyError::ReparentCycleDetected {
                node: node_id,
                new_parent,
            });
        }

        // Phase 1: collect direct children
        let direct_children: Vec<HierarchyNodeId> =
            self.children.get(&node_id).cloned().unwrap_or_default();

        // Phase 2: reparent children, then purge
        for &child in &direct_children {
            if let Some(parent_set) = self.parents.get_mut(&child) {
                parent_set.remove(&node_id);
                parent_set.insert(new_parent);
            }
            let new_children = self.children.entry(new_parent).or_default();
            if !new_children.contains(&child) {
                new_children.push(child);
            }
        }
        self.purge_node(node_id);
        Ok(())
    }

    /// DeleteDetach: remove node, leave children orphaned but valid.
    fn delete_detach(&mut self, node_id: HierarchyNodeId) -> Result<(), HierarchyError> {
        let direct_children: Vec<HierarchyNodeId> =
            self.children.get(&node_id).cloned().unwrap_or_default();
        for &child in &direct_children {
            if let Some(parent_set) = self.parents.get_mut(&child) {
                parent_set.remove(&node_id);
                if parent_set.is_empty() {
                    self.parents.remove(&child);
                }
            }
        }
        self.purge_node(node_id);
        Ok(())
    }

    /// Purge a node from all data structures (apply-phase only, no traversal).
    fn purge_node(&mut self, node_id: HierarchyNodeId) {
        self.nodes.remove(&node_id);

        // Remove from all parents' children lists
        if let Some(parent_set) = self.parents.remove(&node_id) {
            for parent in parent_set {
                if let Some(children) = self.children.get_mut(&parent) {
                    children.retain(|c| *c != node_id);
                    if children.is_empty() {
                        self.children.remove(&parent);
                    }
                }
            }
        }

        // Remove own children list and clean up children's parent sets
        if let Some(children_list) = self.children.remove(&node_id) {
            for child in children_list {
                if let Some(parent_set) = self.parents.get_mut(&child) {
                    parent_set.remove(&node_id);
                    if parent_set.is_empty() {
                        self.parents.remove(&child);
                    }
                }
            }
        }
    }
}

/// Read-only structural snapshot (SPEC-HAL-230 §8).
/// Side-effect free. No mutation handles. No "current" or "active" node.
pub struct HierarchySnapshot<'a> {
    authority: &'a HierarchyAuthority,
}

impl<'a> HierarchySnapshot<'a> {
    /// All nodes in the hierarchy.
    pub fn nodes(&self) -> impl Iterator<Item = HierarchyNodeId> + '_ {
        self.authority.nodes.iter().copied()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.authority.nodes.len()
    }

    /// Whether a node exists.
    pub fn contains(&self, node: HierarchyNodeId) -> bool {
        self.authority.nodes.contains(&node)
    }

    /// Ordered children of a node (semantic ordering).
    pub fn children(&self, node: HierarchyNodeId) -> &[HierarchyNodeId] {
        self.authority
            .children
            .get(&node)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Parents of a node (unordered; DAG allows multiple parents).
    pub fn parents(&self, node: HierarchyNodeId) -> Vec<HierarchyNodeId> {
        self.authority
            .parents
            .get(&node)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Nodes with no parents (roots or orphans).
    pub fn roots(&self) -> Vec<HierarchyNodeId> {
        self.authority
            .nodes
            .iter()
            .filter(|n| !self.authority.parents.contains_key(n))
            .copied()
            .collect()
    }

    /// All ancestors reachable from a node (iterative upward traversal).
    /// Returns empty if node has no parents or does not exist.
    pub fn ancestors(&self, node: HierarchyNodeId) -> Vec<HierarchyNodeId> {
        let mut result = Vec::new();
        let mut stack = vec![node];
        let mut visited = HashSet::new();
        visited.insert(node);
        while let Some(current) = stack.pop() {
            if let Some(parent_set) = self.authority.parents.get(&current) {
                for &parent in parent_set {
                    if visited.insert(parent) {
                        result.push(parent);
                        stack.push(parent);
                    }
                }
            }
        }
        result
    }

    /// All descendants reachable from a node (iterative downward traversal).
    /// Returns empty if node has no children or does not exist.
    pub fn descendants(&self, node: HierarchyNodeId) -> Vec<HierarchyNodeId> {
        let mut result = Vec::new();
        let mut stack = vec![node];
        let mut visited = HashSet::new();
        visited.insert(node);
        while let Some(current) = stack.pop() {
            if let Some(children) = self.authority.children.get(&current) {
                for &child in children {
                    if visited.insert(child) {
                        result.push(child);
                        stack.push(child);
                    }
                }
            }
        }
        result
    }
}
