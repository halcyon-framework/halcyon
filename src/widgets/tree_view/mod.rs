// =============================================================================
// SPEC-HAL-259: TreeView — Hierarchical Widget on Engine Composition
// =============================================================================
//
// TreeView renders hierarchical data using HierarchicalTabularSubstrate
// (SPEC-HAL-259) as the single source of truth for topology, geometry, and
// virtualization.
//
// Composition (mirrors ListView):
//   TreeView (composite) → ScrollArea → TreeViewBody (content widget)
//
// INV-TREE-1: Single Topology Authority — only engines/hierarchical for UI.
// INV-TREE-2: No TreeProjection in TabularSubstrate.
// INV-TREE-4: Rendering reads projection metadata only from SPEC-HAL-259 outputs.
// INV-TREE-5: Tree input events route to hierarchical engine.
//
// =============================================================================

mod body;
mod footer;
mod tree_view;

pub use body::TreeViewBody;
#[cfg(test)]
pub(crate) use body::{
    TreeTopologyIntent, TreeViewDisclosureHitZoneProbeEntry, TreeViewIndentationGeometryProbeEntry,
    TreeViewRealizedCellProbeEntry, TreeViewRealizedRowProbeEntry,
    TreeViewVisibleProjectionProbeEntry,
};
pub use footer::TreeViewFooter;
pub use tree_view::TreeView;
#[cfg(test)]
pub(crate) use tree_view::{TreeViewColumnProbeEntry, TreeViewStructuralProbeEntry};
