// SPEC-HAL-310 §7: NavigatorDataSource — hierarchical extension
// of the FROZEN DataSource trait.

use crate::model::navigation_host::NavigationNodeId;
use crate::widgets::tabular_view::data_adapter::DataSource;
use crate::widgets::tabular_view::row::RowId;

/// Extension of DataSource for hierarchical navigation.
///
/// Two methods with flat defaults — existing DataSource impls
/// work unchanged by adding an empty impl block.
///
/// INV-310-0: DataSource is FROZEN. This trait extends it
/// without modification.
pub trait NavigatorDataSource: DataSource {
    /// If this item is a navigable container, return its
    /// NavigationNodeId. Double-click or Enter drills into it.
    ///
    /// Default: None (flat collection, no hierarchy).
    fn navigation_node(&self, _row: RowId) -> Option<NavigationNodeId> {
        None
    }

    /// Return items at the given navigation location.
    /// Called when NavigationHost's current node changes.
    ///
    /// Default: delegates to row_ids() (flat, ignores location).
    fn items_at(&self, _location: NavigationNodeId) -> Vec<RowId> {
        self.row_ids().to_vec()
    }
}
