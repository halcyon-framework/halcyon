// =============================================================================
// SPEC-HAL-313 §3.4: ExpandableRowHost
//
// Composable component managing per-row widget subtrees for expanded rows.
// Embedded in ListViewBody and TreeViewBody (not a standalone widget).
//
// CONTRACT:
//   - Owns the widget factory (Fn(RowId) -> Box<dyn Widget>).
//   - Owns the mounted-widget map (RowId -> NodeId).
//   - MUST NOT own or reference RowExpansion (that's substrate-layer).
//   - sync() is called during body layout to reconcile mounted state
//     against the substrate's RowExpansion.
//   - For Cap policy, factory widgets are wrapped in ScrollArea at mount time.
//   - For Fixed/Natural policy, factory widgets are mounted directly.
//
// INV-313-FACTORY-PURE-1: Factory called exactly once per Expand per row.
// INV-313-FACTORY-FN-1: Factory is Fn, not FnMut.
// INV-313-VIRT-BOUND-1: Memory = O(|expanded set|).
// =============================================================================

use std::collections::HashMap;

use crate::core::widget::Widget;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, RowExpansion};
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::tabular_view::row::RowId;

/// SPEC-HAL-313 §3.4: Manages per-row widget subtrees for expanded rows.
///
/// Embedded (not via trait impl, just as a field) in ListViewBody and
/// TreeViewBody. The body calls `sync()` during layout to reconcile
/// mounted widgets against the substrate's RowExpansion state.
pub struct ExpandableRowHost {
    /// Widget factory. Called once per row on expand.
    /// INV-313-FACTORY-FN-1: Fn, not FnMut.
    factory: Option<Box<dyn Fn(RowId) -> Box<dyn Widget>>>,

    /// Mounted expanded-widget NodeIds, keyed by RowId.
    /// These are children of the body's tree node.
    mounted: HashMap<RowId, NodeId>,

    /// Last observed RowExpansion version. Used for change detection
    /// in sync() to skip work when nothing changed.
    last_version: u64,
}

impl ExpandableRowHost {
    /// Create a new host with no factory.
    pub fn new() -> Self {
        Self {
            factory: None,
            mounted: HashMap::new(),
            last_version: 0,
        }
    }

    /// Set the widget factory.
    ///
    /// INV-313-FACTORY-FN-1: Must be Fn (not FnMut).
    /// INV-313-FACTORY-PURE-1: Called exactly once per Expand per row.
    pub fn set_factory(&mut self, f: impl Fn(RowId) -> Box<dyn Widget> + 'static) {
        self.factory = Some(Box::new(f));
    }

    /// Whether a factory has been set.
    pub fn has_factory(&self) -> bool {
        self.factory.is_some()
    }

    // =========================================================================
    // Sync — called from body layout
    // =========================================================================

    /// Reconcile mounted widgets against the substrate's RowExpansion state.
    ///
    /// Called during body layout. Mounts widgets for newly expanded rows,
    /// unmounts widgets for newly collapsed rows.
    ///
    /// For `ExpandedHeightPolicy::Cap`, factory widgets are wrapped in a
    /// ScrollArea at mount time. The ScrollArea provides vertical overflow
    /// scrolling when the child's natural height exceeds the cap. When the
    /// child fits, no scrollbar is shown (zero visual overhead).
    ///
    /// # Arguments
    /// - `expansion`: the substrate's current RowExpansion state
    /// - `tree`: mutable widget tree for add_child / remove_node
    /// - `parent_node`: the body widget's NodeId (mounted children become its tree children)
    ///
    /// # Returns
    /// Vec of (RowId, is_mounted) changes. `true` = newly mounted, `false` = newly unmounted.
    /// Empty Vec = nothing changed.
    pub fn sync(
        &mut self,
        expansion: &RowExpansion,
        tree: &mut WidgetTree,
        parent_node: NodeId,
    ) -> Vec<(RowId, bool)> {
        // Fast path: no factory → nothing to do.
        let Some(ref factory) = self.factory else {
            return Vec::new();
        };

        // Fast path: version unchanged → nothing to do.
        if expansion.version() == self.last_version {
            return Vec::new();
        }
        self.last_version = expansion.version();

        let mut changes = Vec::new();

        // Unmount rows that are no longer expanded.
        let to_unmount: Vec<RowId> = self
            .mounted
            .keys()
            .copied()
            .filter(|id| !expansion.is_expanded(*id))
            .collect();

        for row_id in to_unmount {
            if let Some(node_id) = self.mounted.remove(&row_id) {
                tree.remove_node(node_id);
                changes.push((row_id, false));
            }
        }

        // Mount rows that are newly expanded.
        let to_mount: Vec<RowId> = expansion
            .expanded_rows()
            .filter(|id| !self.mounted.contains_key(id))
            .collect();

        let height_policy = expansion.height_policy();

        for row_id in to_mount {
            let widget = factory(row_id);
            let node_id = self.mount_widget(widget, height_policy, tree, parent_node);
            self.mounted.insert(row_id, node_id);
            changes.push((row_id, true));
        }

        changes
    }

    /// Mount a single widget, optionally wrapping in ScrollArea for Cap policy.
    ///
    /// Returns the NodeId of the top-level mounted node (ScrollArea for Cap,
    /// raw widget for Fixed/Natural).
    fn mount_widget(
        &self,
        widget: Box<dyn Widget>,
        height_policy: ExpandedHeightPolicy,
        tree: &mut WidgetTree,
        parent_node: NodeId,
    ) -> NodeId {
        match height_policy {
            ExpandedHeightPolicy::Cap(_) => {
                // Wrap in ScrollArea for overflow scrolling.
                // ScrollArea defaults: vertical enabled, horizontal disabled.
                let scroll_area = ScrollArea::new_unwired();
                let sa_node = tree.add_child(parent_node, Box::new(scroll_area), None, 0);

                // Add factory widget as child of ScrollArea.
                let content_node = tree.add_child(sa_node, widget, None, 0);

                // Wire ScrollArea to its content.
                if let Some(node) = tree.node_mut(sa_node) {
                    if let Some(sa) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                        sa.wire_content(content_node);
                    }
                }

                sa_node
            }
            ExpandedHeightPolicy::Fixed(_) | ExpandedHeightPolicy::Natural => {
                // Mount directly — no scroll wrapper.
                tree.add_child(parent_node, widget, None, 0)
            }
        }
    }

    // =========================================================================
    // Query
    // =========================================================================

    /// Get the mounted NodeId for a row (if expanded and mounted).
    pub fn mounted_node(&self, row_id: RowId) -> Option<NodeId> {
        self.mounted.get(&row_id).copied()
    }

    /// Whether a row has a mounted widget subtree.
    pub fn is_mounted(&self, row_id: RowId) -> bool {
        self.mounted.contains_key(&row_id)
    }

    /// Number of currently mounted widget subtrees.
    pub fn mounted_count(&self) -> usize {
        self.mounted.len()
    }

    /// Iterate over all mounted (RowId, NodeId) pairs.
    pub fn mounted_entries(&self) -> impl Iterator<Item = (RowId, NodeId)> + '_ {
        self.mounted
            .iter()
            .map(|(&row_id, &node_id)| (row_id, node_id))
    }

    // =========================================================================
    // Cleanup
    // =========================================================================

    /// Unmount all expanded widgets. Called on widget teardown or
    /// when expansion is disabled.
    ///
    /// INV-313-FILTER-UNMOUNT-1: Used when filter/sort removes expanded rows.
    pub fn unmount_all(&mut self, tree: &mut WidgetTree) {
        for (_, node_id) in self.mounted.drain() {
            tree.remove_node(node_id);
        }
        self.last_version = 0;
    }
}

impl Default for ExpandableRowHost {
    fn default() -> Self {
        Self::new()
    }
}
