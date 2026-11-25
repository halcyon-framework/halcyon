// =============================================================================
// SPEC-HAL-171: TabularSubstrate Sort State
// =============================================================================
//
// INVARIANT-SORT-1 (Authority): Sort state is owned exclusively by TabularSubstrate.
//                               Header and Body may only emit intents.
// INVARIANT-SORT-2 (Intent-only): A sort change happens only via
//                                 TabularIntent::Sort(SortIntent).
// INVARIANT-SORT-3 (Determinism): Given same (columns, data, prior sort state,
//                                 intents), the resulting sort state and
//                                 realized order is deterministic.
// INVARIANT-SORT-4 (Stability): Sorting never changes RowId identity mapping
//                               for a given underlying data row; it only
//                               changes realization order.
// INVARIANT-SORT-5 (No mutation outside layout/update): Sort state may not be
//                   mutated during paint/input phases; only in TabularSubstrate's
//                   update/handle_intent path.
// INVARIANT-SORT-6 (Fallback): If comparator/data access is unavailable,
//                              sorting is a no-op while still updating sort
//                              indicator state.
// =============================================================================

use std::cmp::Ordering;

use super::column::ColumnId;
use super::intents::{SortDirection, SortIntent};

/// Sort state owned exclusively by TabularSubstrate.
///
/// SPEC-HAL-171: Single-column sort only. Multi-sort is explicitly excluded.
#[derive(Debug, Clone)]
pub struct SortState {
    /// Currently sorted column (None = unsorted).
    active_column: Option<ColumnId>,
    /// Sort direction (only meaningful when active_column is Some).
    direction: SortDirection,
    /// Version counter for change detection.
    version: u64,
    /// Whether sort order needs recomputation.
    dirty: bool,
}

impl SortState {
    /// Create a new unsorted state.
    pub fn new() -> Self {
        Self {
            active_column: None,
            direction: SortDirection::Ascending,
            version: 0,
            dirty: false,
        }
    }

    /// Get the currently sorted column.
    pub fn active_column(&self) -> Option<ColumnId> {
        self.active_column
    }

    /// Get the current sort direction.
    ///
    /// Returns None if no column is actively sorted.
    pub fn direction(&self) -> Option<SortDirection> {
        self.active_column.map(|_| self.direction)
    }

    /// Get the version counter.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Check if sort order needs recomputation.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag after recomputation.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Check if sorting is active.
    pub fn is_active(&self) -> bool {
        self.active_column.is_some()
    }

    /// Apply a sort intent.
    ///
    /// SPEC-HAL-171: This is the ONLY mutation path for sort state.
    /// INVARIANT-SORT-2: All changes happen via SortIntent.
    pub fn apply_intent(&mut self, intent: &SortIntent) {
        match intent {
            SortIntent::RequestSort {
                column_id,
                direction,
            } => {
                if self.active_column == Some(*column_id) {
                    // Same column: toggle direction or use explicit
                    self.direction = direction.unwrap_or_else(|| self.direction.toggle());
                } else {
                    // New column: set as active with default or explicit direction
                    self.active_column = Some(*column_id);
                    self.direction = direction.unwrap_or(SortDirection::Ascending);
                }
                self.version += 1;
                self.dirty = true;
            }
            SortIntent::RequestClearSort { column_id } => {
                if self.active_column == Some(*column_id) {
                    self.active_column = None;
                    self.version += 1;
                    self.dirty = true;
                }
            }
        }
    }

    /// Clear all sort state.
    pub fn clear(&mut self) {
        if self.active_column.is_some() {
            self.active_column = None;
            self.version += 1;
            self.dirty = true;
        }
    }
}

impl Default for SortState {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for providing row comparison during sort.
///
/// SPEC-HAL-171: TabularSubstrate does not own data. The comparator is provided
/// externally and called during sort order computation.
///
/// INVARIANT-SORT-6: If compare_rows returns None, sorting falls back to
/// natural order (no reordering) while still updating sort indicator state.
pub trait RowComparator {
    /// Compare two rows by their indices for a given column.
    ///
    /// Returns Some(Ordering) if comparison is possible, None if data is
    /// unavailable or comparison is not supported for this column.
    ///
    /// The indices are into the RowModel (stable row positions).
    fn compare_rows(&self, a_index: usize, b_index: usize, column_id: ColumnId)
    -> Option<Ordering>;
}

/// Null comparator that always returns natural order.
///
/// Used when no comparator is provided. Sorting becomes a no-op
/// but sort indicator state is still tracked.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullComparator;

impl RowComparator for NullComparator {
    fn compare_rows(
        &self,
        _a_index: usize,
        _b_index: usize,
        _column_id: ColumnId,
    ) -> Option<Ordering> {
        None
    }
}

/// Compute sorted realization order from sort state and comparator.
///
/// SPEC-HAL-171 INVARIANT-SORT-3: Given same inputs, output is deterministic.
/// SPEC-HAL-171 INVARIANT-SORT-4: RowIds are not changed, only order.
///
/// Returns a Vec<usize> of row indices in sorted order.
pub fn compute_sorted_order<C: RowComparator + ?Sized>(
    row_count: usize,
    sort_state: &SortState,
    comparator: &C,
) -> Vec<usize> {
    // Start with natural order
    let mut indices: Vec<usize> = (0..row_count).collect();

    // If no active sort, return natural order
    let (column_id, direction) = match (sort_state.active_column(), sort_state.direction()) {
        (Some(col), Some(dir)) => (col, dir),
        _ => return indices,
    };

    // Sort with stable sort to preserve order for equal elements
    indices.sort_by(|&a, &b| {
        match comparator.compare_rows(a, b, column_id) {
            Some(ord) => match direction {
                SortDirection::Ascending => ord,
                SortDirection::Descending => ord.reverse(),
            },
            // No comparison available: preserve natural order
            None => a.cmp(&b),
        }
    });

    indices
}
