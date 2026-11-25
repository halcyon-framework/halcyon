use crate::core::event::Modifiers;
use crate::widgets::tabular_view::column::{ColumnId, ColumnScrollDomain};
use crate::widgets::tabular_view::data_adapter::DataSource;

use super::core::ListViewCore;

/// Actions emitted by ListView in response to user gestures.
pub enum ListViewAction {
    /// Toggle a column between PinnedLeft and Scrollable.
    ToggleColumnPin { column_id: ColumnId },
    /// Begin column drag-reorder from a header click.
    BeginColumnReorder { column_id: ColumnId },
}

/// Map a header pointer-down event to a column action.
///
/// This is the input mapping layer for header cell interactions.
/// Called AFTER config button, separator, and pin-toggle checks have
/// already been evaluated and returned None. Any remaining primary
/// click in a header cell arms column drag-reorder.
///
/// Returns None only if no action applies (currently always returns
/// BeginColumnReorder for the remaining header click).
pub fn map_header_click(_modifiers: &Modifiers, column_id: ColumnId) -> Option<ListViewAction> {
    Some(ListViewAction::BeginColumnReorder { column_id })
}

/// Resolve a ListView action against model state. Returns true if applied.
///
/// Only handles actions that mutate model state (column model, substrate).
/// Widget-local state transitions (e.g., BeginColumnReorder arming drag_state)
/// are handled by ListView directly after action dispatch.
pub fn resolve<S: DataSource>(action: &ListViewAction, core: &mut ListViewCore<S>) -> bool {
    match action {
        ListViewAction::ToggleColumnPin { column_id } => {
            let current = core
                .cms
                .hts()
                .column_model()
                .get(*column_id)
                .map(|c| c.scroll_domain);
            let Some(current_domain) = current else {
                return false;
            };

            let new_domain = match current_domain {
                ColumnScrollDomain::PinnedLeft => ColumnScrollDomain::Scrollable,
                ColumnScrollDomain::Scrollable => ColumnScrollDomain::PinnedLeft,
                ColumnScrollDomain::PinnedRight => ColumnScrollDomain::Scrollable,
            };

            core.set_column_scroll_domain(*column_id, new_domain)
        }
        ListViewAction::BeginColumnReorder { .. } => false, // Widget-local state, not model mutation
    }
}
