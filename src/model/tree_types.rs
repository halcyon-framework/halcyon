// =============================================================================
// SPEC-HAL-251: TreeSubstrate State Model & Projection Law
// SPEC-HAL-252: TreeSubstrate Adapter Contract
// SPEC-HAL-254: TreeSubstrate Internal State Structure & Storage Layout
// =============================================================================
//
// Tree substrate types: identity, adapter contract, projection record,
// expansion operations.
//
// No UI. No rendering. No geometry. Only substrate types.
// =============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

/// Stable, opaque tree node identifier.
///
/// SPEC-HAL-251 §3: Nodes are uniquely identified and stable across frames
/// unless explicitly removed. Identity preserved across expand, collapse,
/// reorder, and data refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeNodeId(u64);

impl TreeNodeId {
    /// Create a new unique TreeNodeId.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        TreeNodeId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Create a TreeNodeId from a raw value.
    pub const fn from_raw(id: u64) -> Self {
        TreeNodeId(id)
    }

    /// Get the raw u64 value.
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for TreeNodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// A node in the linear visible projection.
///
/// SPEC-HAL-254 §4: Minimal by design. No parent pointers, no sibling
/// pointers, no subtree sizes. All topology queries go through adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedNode {
    pub node_id: TreeNodeId,
    pub depth: u16,
}

/// Adapter trait for tree topology supply.
///
/// SPEC-HAL-252 §6: The adapter is authority for node existence, stable
/// identity, parent/child ordering, and expandability hints.
///
/// Adapter MUST NOT encode expansion state, projection state, or UI
/// semantics. Adapter methods are infallible by contract.
pub trait TreeAdapter {
    /// Returns root nodes in stable display order.
    fn roots(&self) -> &[TreeNodeId];

    /// Returns children of `node` in stable display order.
    /// Must return empty slice if the node has no children.
    fn children(&self, node: TreeNodeId) -> &[TreeNodeId];

    /// Fast hint: does node have children?
    /// Default derives from children().is_empty().
    fn has_children(&self, node: TreeNodeId) -> bool {
        !self.children(node).is_empty()
    }

    /// Monotonically increasing revision token.
    /// Must increment when roots/children ordering or membership changes.
    /// SPEC-HAL-252 §7: INV-ADP-005 Revision Monotonicity.
    fn revision(&self) -> u64;
}

/// Expansion operations for TreeSubstrate.
///
/// SPEC-HAL-253 §3.1: Routed via action system.
/// TreeSubstrate is sole authority over expansion state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeExpansionOp {
    Expand(TreeNodeId),
    Collapse(TreeNodeId),
    Toggle(TreeNodeId),
    ExpandAll(TreeNodeId),
    CollapseAll(TreeNodeId),
}
