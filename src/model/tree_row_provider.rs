// =============================================================================
// Tree ↔ ListView Projection Contract
// =============================================================================
//
// Defines the formal adapter layer between Tree (model) and ListView
// (substrate consumer). Pure read-only projection accessor.
//
// No geometry. No selection. No columns. No virtualization.
// ListView remains the virtualization engine.
// =============================================================================

use super::tree::Tree;
use super::tree_types::{TreeAdapter, TreeNodeId};

/// Contract between Tree model and ListView/TabularSubstrate.
///
/// Read-only projection accessor. Exposes stable row order, identity
/// mapping, depth, and expansion state. No geometry, no selection,
/// no column logic, no virtualization.
pub trait TreeRowProvider {
    /// Number of visible rows in current projection.
    fn row_count(&self) -> usize;

    /// Node ID at projection row index.
    /// Panics if `row >= row_count()`.
    fn node_id_at(&self, row: usize) -> TreeNodeId;

    /// Row index of a node ID. None if not in projection.
    fn row_index_of(&self, node: TreeNodeId) -> Option<usize>;

    /// Depth at row index (for indentation). 0 = root.
    /// Panics if `row >= row_count()`.
    fn depth_at(&self, row: usize) -> u16;

    /// Whether the node at row index has children (expandable).
    /// Panics if `row >= row_count()`.
    fn is_expandable(&self, row: usize) -> bool;

    /// Whether the node at row index is currently expanded.
    /// Panics if `row >= row_count()`.
    fn is_expanded(&self, row: usize) -> bool;
}

/// Lightweight projection view over Tree for ListView consumption.
///
/// Wraps `&Tree<A>` and implements `TreeRowProvider`.
/// Zero allocation, zero copy. Borrows the tree for the duration.
pub struct TreeListProjection<'a, A: TreeAdapter> {
    tree: &'a Tree<A>,
}

impl<'a, A: TreeAdapter> TreeListProjection<'a, A> {
    /// Create a new projection view over the given tree.
    pub fn new(tree: &'a Tree<A>) -> Self {
        Self { tree }
    }
}

impl<'a, A: TreeAdapter> TreeRowProvider for TreeListProjection<'a, A> {
    fn row_count(&self) -> usize {
        self.tree.projection().len()
    }

    fn node_id_at(&self, row: usize) -> TreeNodeId {
        self.tree.projection()[row].node_id
    }

    fn row_index_of(&self, node: TreeNodeId) -> Option<usize> {
        self.tree.row_index(node)
    }

    fn depth_at(&self, row: usize) -> u16 {
        self.tree.projection()[row].depth
    }

    fn is_expandable(&self, row: usize) -> bool {
        let node_id = self.tree.projection()[row].node_id;
        self.tree.adapter().has_children(node_id)
    }

    fn is_expanded(&self, row: usize) -> bool {
        let node_id = self.tree.projection()[row].node_id;
        self.tree.is_expanded(node_id)
    }
}
