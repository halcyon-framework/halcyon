// =============================================================================
// Column Configuration State
// =============================================================================
//
// Captures the user-facing column configuration: order, visibility, pin state.
// Used for persistence, reset-to-default, and overlay display.
//
// This is a pure data snapshot — no widget references, no geometry.
// =============================================================================

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::column::{ColumnId, ColumnModel, ColumnScrollDomain};

/// Serializable column configuration state.
///
/// Captures order, preferred widths, visibility, and pin state for all columns in a model.
/// Used for persistence (save/load) and reset-to-default workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnConfigurationState {
    pub column_order: Vec<ColumnId>,
    #[serde(default)]
    pub preferred_widths: HashMap<ColumnId, f32>,
    pub hidden: HashSet<ColumnId>,
    pub pinned_left: HashSet<ColumnId>,
    pub pinned_right: HashSet<ColumnId>,
}

impl ColumnConfigurationState {
    /// Extract current configuration state from a live column model.
    pub fn from_model(model: &ColumnModel) -> Self {
        let mut column_order = Vec::with_capacity(model.len());
        let mut preferred_widths = HashMap::with_capacity(model.len());
        let mut hidden = HashSet::new();
        let mut pinned_left = HashSet::new();
        let mut pinned_right = HashSet::new();

        for col in model.columns() {
            column_order.push(col.id);
            preferred_widths.insert(col.id, col.preferred_width);
            if !col.visible {
                hidden.insert(col.id);
            }
            match col.scroll_domain {
                ColumnScrollDomain::PinnedLeft => {
                    pinned_left.insert(col.id);
                }
                ColumnScrollDomain::PinnedRight => {
                    pinned_right.insert(col.id);
                }
                ColumnScrollDomain::Scrollable => {}
            }
        }

        Self {
            column_order,
            preferred_widths,
            hidden,
            pinned_left,
            pinned_right,
        }
    }

    /// Extract baseline/default configuration from a live column model.
    ///
    /// Identical to `from_model` — captures the initial state as the default.
    pub fn default_from_model(model: &ColumnModel) -> Self {
        Self::from_model(model)
    }

    /// Compute the scroll domain for a column ID based on pin sets.
    pub fn domain_for(&self, id: ColumnId) -> ColumnScrollDomain {
        if self.pinned_left.contains(&id) {
            ColumnScrollDomain::PinnedLeft
        } else if self.pinned_right.contains(&id) {
            ColumnScrollDomain::PinnedRight
        } else {
            ColumnScrollDomain::Scrollable
        }
    }

    /// Check if a column is visible (not in the hidden set).
    pub fn is_visible(&self, id: ColumnId) -> bool {
        !self.hidden.contains(&id)
    }

    /// Return the persisted preferred width for a stable column identity.
    pub fn preferred_width_for(&self, id: ColumnId) -> Option<f32> {
        self.preferred_widths.get(&id).copied()
    }
}

/// Snapshot of a single column's metadata for overlay display.
#[derive(Debug, Clone)]
pub struct ColumnMetaSnapshot {
    pub id: ColumnId,
    pub title: String,
    pub visible: bool,
    pub domain: ColumnScrollDomain,
    pub order_index: usize,
}
