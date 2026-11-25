// =============================================================================
// SPEC-HAL-313 §3.1: Row Expansion State Model
//
// CONTRACT:
//   - RowExpansion is a pure expansion-state oracle for substrate consumption.
//   - It stores set membership (which rows are expanded), policy, height
//     policy, and a version counter.
//   - It MUST NOT contain: widget instances, NodeIds, widget factories,
//     height values, hit-testing logic, scroll helpers, layout data,
//     or any higher-layer concern.
//   - It is NOT a widget factory holder. Factory ownership lives on the
//     body layer (ExpandableRowHost).
//   - Substrates (TabularSubstrate, HierarchicalTabularSubstrate) depend
//     on this type. RowExpansion MUST NOT depend on engine or widget types
//     beyond the RowId value type.
//
// INV-313-SUBSTRATE-OWNS-1: Expansion state is owned exclusively by the
// substrate's RowExpansion. No widget, consumer, or body holds its own copy.
// =============================================================================

use std::collections::HashSet;

use crate::widgets::tabular_view::row::RowId;

/// SPEC-HAL-313 §3.1: Expansion policy governing toggle behavior.
///
/// Determines how many rows can be expanded simultaneously and how
/// toggle operations interact with existing expansions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionPolicy {
    /// Expansion is unavailable. All toggle calls are no-ops.
    Disabled,
    /// At most one row expanded at a time. Expanding a row collapses
    /// the previously expanded row (if any).
    SingleExpand,
    /// Any number of rows can be expanded simultaneously. Each toggle
    /// is independent.
    MultiExpand,
}

impl Default for ExpansionPolicy {
    fn default() -> Self {
        ExpansionPolicy::Disabled
    }
}

/// SPEC-HAL-313 §3.1: Height policy for the expanded region.
///
/// Controls how the expanded row's additional height is determined
/// from the child widget's natural size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpandedHeightPolicy {
    /// Added height is exactly this constant, regardless of child size.
    Fixed(f32),
    /// Added height is `min(child.natural_height, cap)`. If the child
    /// exceeds the cap, the body wraps it in a ScrollArea.
    /// INV-313-CAP-WRAP-1: Wrapping occurs only when natural > cap.
    Cap(f32),
    /// Added height equals the child's natural height. No bound.
    Natural,
}

impl Default for ExpandedHeightPolicy {
    fn default() -> Self {
        ExpandedHeightPolicy::Fixed(72.0)
    }
}

/// SPEC-HAL-313 §3.1: Row expansion state.
///
/// Pure data model tracking which rows are expanded. Owned exclusively
/// by the substrate. Consumers configure policy and height policy;
/// the substrate processes ExpansionIntent to mutate this state.
///
/// INV-313-SUBSTRATE-OWNS-1: Single owner (substrate).
/// INV-313-PHASE-SEP-1: Mutations only during intent processing.
/// FORBID-313-WIDGET-FACTORY-IN-MODEL-1: No factory, NodeIds, or widget refs.
#[derive(Debug)]
pub struct RowExpansion {
    /// Set of currently expanded row IDs.
    expanded: HashSet<RowId>,
    /// Expansion policy.
    policy: ExpansionPolicy,
    /// Height policy for the expanded region.
    height_policy: ExpandedHeightPolicy,
    /// Monotonic version counter. Bumped on any mutation to the expanded set.
    version: u64,
}

impl RowExpansion {
    /// Create a new RowExpansion with the given policy.
    pub fn new(policy: ExpansionPolicy) -> Self {
        Self {
            expanded: HashSet::new(),
            policy,
            height_policy: ExpandedHeightPolicy::default(),
            version: 0,
        }
    }

    /// Create a new RowExpansion with both policy and height policy.
    pub fn with_height_policy(
        policy: ExpansionPolicy,
        height_policy: ExpandedHeightPolicy,
    ) -> Self {
        Self {
            expanded: HashSet::new(),
            policy,
            height_policy,
            version: 0,
        }
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Current expansion policy.
    pub fn policy(&self) -> ExpansionPolicy {
        self.policy
    }

    /// Current height policy for the expanded region.
    pub fn height_policy(&self) -> ExpandedHeightPolicy {
        self.height_policy
    }

    /// Whether a specific row is expanded.
    pub fn is_expanded(&self, row_id: RowId) -> bool {
        self.expanded.contains(&row_id)
    }

    /// Number of currently expanded rows.
    pub fn expanded_count(&self) -> usize {
        self.expanded.len()
    }

    /// Iterate over all currently expanded row IDs.
    pub fn expanded_rows(&self) -> impl Iterator<Item = RowId> + '_ {
        self.expanded.iter().copied()
    }

    /// Monotonic version counter. Bumped on any expansion state mutation.
    pub fn version(&self) -> u64 {
        self.version
    }

    // =========================================================================
    // Configuration
    // =========================================================================

    /// Set the expansion policy. Does NOT clear existing expansions —
    /// the caller (substrate) is responsible for collapsing rows if the
    /// new policy requires it (e.g., switching from Multi to Single).
    pub fn set_policy(&mut self, policy: ExpansionPolicy) {
        self.policy = policy;
    }

    /// Set the height policy for the expanded region.
    pub fn set_height_policy(&mut self, policy: ExpandedHeightPolicy) {
        self.height_policy = policy;
    }

    // =========================================================================
    // Mutations
    // =========================================================================

    /// Toggle expansion for a row. Returns a list of (RowId, is_now_expanded)
    /// pairs describing what changed. Empty if nothing changed (e.g., policy
    /// is Disabled).
    ///
    /// INV-313-PHASE-SEP-1: Call only during intent processing.
    pub fn toggle(&mut self, row_id: RowId) -> Vec<(RowId, bool)> {
        match self.policy {
            ExpansionPolicy::Disabled => Vec::new(),

            ExpansionPolicy::SingleExpand => {
                if self.expanded.contains(&row_id) {
                    // Collapse the currently expanded row.
                    self.expanded.remove(&row_id);
                    self.version += 1;
                    vec![(row_id, false)]
                } else {
                    // Collapse previous (if any), expand new.
                    let mut changes = Vec::new();
                    let prev_ids: Vec<RowId> = self.expanded.iter().copied().collect();
                    for prev in prev_ids {
                        changes.push((prev, false));
                    }
                    self.expanded.clear();
                    self.expanded.insert(row_id);
                    changes.push((row_id, true));
                    self.version += 1;
                    changes
                }
            }

            ExpansionPolicy::MultiExpand => {
                if self.expanded.remove(&row_id) {
                    self.version += 1;
                    vec![(row_id, false)]
                } else {
                    self.expanded.insert(row_id);
                    self.version += 1;
                    vec![(row_id, true)]
                }
            }
        }
    }

    /// Expand a specific row. Returns changes (empty if already expanded
    /// or policy is Disabled).
    ///
    /// INV-313-PHASE-SEP-1: Call only during intent processing.
    pub fn expand(&mut self, row_id: RowId) -> Vec<(RowId, bool)> {
        if self.policy == ExpansionPolicy::Disabled {
            return Vec::new();
        }
        if self.expanded.contains(&row_id) {
            return Vec::new();
        }

        let mut changes = Vec::new();

        // SingleExpand: collapse previous first.
        if self.policy == ExpansionPolicy::SingleExpand {
            for &prev in self.expanded.iter() {
                changes.push((prev, false));
            }
            self.expanded.clear();
        }

        self.expanded.insert(row_id);
        changes.push((row_id, true));
        self.version += 1;
        changes
    }

    /// Collapse a specific row. Returns changes (empty if not expanded
    /// or policy is Disabled).
    ///
    /// INV-313-PHASE-SEP-1: Call only during intent processing.
    pub fn collapse(&mut self, row_id: RowId) -> Vec<(RowId, bool)> {
        if self.policy == ExpansionPolicy::Disabled {
            return Vec::new();
        }
        if self.expanded.remove(&row_id) {
            self.version += 1;
            vec![(row_id, false)]
        } else {
            Vec::new()
        }
    }

    /// Collapse all expanded rows. Returns all collapsed RowIds.
    ///
    /// INV-313-PHASE-SEP-1: Call only during intent processing.
    /// INV-313-FILTER-UNMOUNT-1: Used when filter/sort removes expanded rows.
    pub fn collapse_all(&mut self) -> Vec<(RowId, bool)> {
        if self.expanded.is_empty() {
            return Vec::new();
        }
        let changes: Vec<_> = self.expanded.iter().map(|&id| (id, false)).collect();
        self.expanded.clear();
        self.version += 1;
        changes
    }

    /// Remove a row from the expanded set without policy enforcement.
    /// Used when a row is deleted from the data source — the row no
    /// longer exists, so its expansion state is simply discarded.
    ///
    /// Returns true if the row was expanded (and is now removed).
    pub fn remove_row(&mut self, row_id: RowId) -> bool {
        if self.expanded.remove(&row_id) {
            self.version += 1;
            true
        } else {
            false
        }
    }
}

impl Default for RowExpansion {
    fn default() -> Self {
        Self::new(ExpansionPolicy::default())
    }
}
