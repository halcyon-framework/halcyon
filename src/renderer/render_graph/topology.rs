//! SPEC-HAL-084: RenderGraph Topology
//!
//! The Build phase output. Contains sorted passes and resource descriptors.
//! Immutable after construction.

use super::pass::{RenderPassNode, RenderPassNodeId};
use super::resources::ResourceDescriptor;

/// The output of RenderGraphBuilder::build().
///
/// Contains:
/// - All registered passes
/// - Topologically sorted execution order
/// - All declared resource descriptors
///
/// This is the Build phase output, consumed by Compile phase.
pub struct RenderGraphTopology {
    /// All passes in the graph (unsorted, indexed by RenderPassNodeId).
    pub(crate) passes: Vec<RenderPassNode>,
    /// Topologically sorted pass execution order.
    pub(crate) sorted_order: Vec<RenderPassNodeId>,
    /// All declared resource descriptors.
    #[allow(
        dead_code,
        reason = "HAL-084 D.1 requires complete Build resource declarations"
    )]
    pub(crate) resources: Vec<ResourceDescriptor>,
}

impl RenderGraphTopology {
    /// Get the number of passes in the graph.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Get the sorted execution order.
    pub fn execution_order(&self) -> &[RenderPassNodeId] {
        &self.sorted_order
    }

    /// Get a pass by ID.
    pub fn get_pass(&self, id: RenderPassNodeId) -> Option<&RenderPassNode> {
        self.passes.get(id.0 as usize)
    }

    /// Iterate over passes in execution order.
    pub fn passes_in_order(&self) -> impl Iterator<Item = &RenderPassNode> {
        self.sorted_order
            .iter()
            .filter_map(|id| self.passes.get(id.0 as usize))
    }
}
