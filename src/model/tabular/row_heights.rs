// =============================================================================
// Engine-grade row height oracle.
//
// CONTRACT:
//   - RowHeights is a pure height oracle for engine consumption.
//   - It stores row count, per-row heights, and a version counter.
//   - It must NOT contain: identity (RowId), selection state, projection
//     data, hit-testing logic, scroll helpers, or any widget-layer concern.
//   - It is NOT a general-purpose row container.
//   - Callers must guarantee index validity for all indexed operations.
//     Out-of-bounds access is a caller bug, not an expected error path.
//   - Engines (VirtualizationEngine, HTS, ColumnGeometry) depend on this
//     type. Widgets depend on it via RowModel composition. Neither may
//     be inverted: RowHeights must never depend on engine or widget types.
//
// Extracted from RowModel (widgets/tabular_view/row.rs) per engine boundary
// refactor. Widget-layer RowModel wraps this type and adds identity registry
// (Vec<RowId>) with INV-ROWMODEL-SYNC enforcing count agreement.
// =============================================================================

/// Engine-grade row height oracle.
///
/// Provides height queries, version tracking, and row count without any
/// widget-layer concerns (identity, hit-testing, scroll helpers).
///
/// Used by VirtualizationEngine, HierarchicalBlockVirtualization, and
/// HierarchicalTabularSubstrate.
#[derive(Debug, Clone)]
pub struct RowHeights {
    /// Fixed row height (if uniform).
    fixed_height: Option<f32>,
    /// Variable row heights (if non-uniform).
    variable_heights: Vec<f32>,
    /// Number of rows tracked.
    row_count: usize,
    /// Monotonic version counter for height mutations.
    /// Bumped on any operation that changes row heights or row count.
    height_version: u64,
}

impl RowHeights {
    /// Create a row heights oracle with fixed (uniform) row height.
    pub fn with_fixed_height(height: f32) -> Self {
        Self {
            fixed_height: Some(height),
            variable_heights: Vec::new(),
            row_count: 0,
            height_version: 0,
        }
    }

    /// Create a fixed-height oracle with an explicit row count.
    pub fn with_fixed_height_count(height: f32, row_count: usize) -> Self {
        Self {
            fixed_height: Some(height),
            variable_heights: Vec::new(),
            row_count,
            height_version: 0,
        }
    }

    /// Create a row heights oracle with variable (per-row) heights.
    pub fn with_variable_heights() -> Self {
        Self {
            fixed_height: None,
            variable_heights: Vec::new(),
            row_count: 0,
            height_version: 0,
        }
    }

    /// Get the number of rows.
    pub fn len(&self) -> usize {
        self.row_count
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    /// Get the fixed row height (if using fixed-height mode).
    pub fn fixed_height(&self) -> Option<f32> {
        self.fixed_height
    }

    /// Get row height at index.
    pub fn height_at(&self, index: usize) -> f32 {
        if let Some(h) = self.fixed_height {
            h
        } else {
            self.variable_heights.get(index).copied().unwrap_or(0.0)
        }
    }

    /// Monotonic version counter for height/count mutations.
    /// Used by VirtualizationEngine to detect prefix-sum staleness.
    pub fn height_version(&self) -> u64 {
        self.height_version
    }

    /// Set the fixed row height.
    ///
    /// State contract: fixed mode ⟹ variable_heights is empty.
    /// Switching to fixed mode clears any stale variable-height data.
    pub fn set_fixed_height(&mut self, height: f32) {
        self.fixed_height = Some(height);
        self.variable_heights.clear();
        self.height_version += 1;
    }

    /// Set height for a row (variable height mode only).
    pub fn set_height(&mut self, index: usize, height: f32) {
        if self.fixed_height.is_none() {
            if let Some(h) = self.variable_heights.get_mut(index) {
                *h = height;
                self.height_version += 1;
            }
        }
    }

    /// Switch from fixed-height to variable-height mode.
    ///
    /// Initializes `variable_heights` with the current fixed height for all
    /// existing rows, then clears `fixed_height`. After this call,
    /// `set_height(index, h)` will work per-row.
    ///
    /// No-op if already in variable-height mode.
    ///
    /// SPEC-HAL-312A INV-312A-VARIABLE-SWITCH.
    pub fn promote_to_variable(&mut self) {
        if let Some(h) = self.fixed_height.take() {
            self.variable_heights = vec![h; self.row_count];
            self.height_version += 1;
        }
    }

    /// Add a row slot with default height (0.0 for variable mode).
    pub fn push(&mut self) {
        self.row_count += 1;
        if self.fixed_height.is_none() {
            self.variable_heights.push(0.0);
        }
        self.height_version += 1;
    }

    /// Add a row slot with explicit height (for variable height mode).
    pub fn push_with_height(&mut self, height: f32) {
        self.row_count += 1;
        if self.fixed_height.is_none() {
            self.variable_heights.push(height);
        }
        self.height_version += 1;
    }

    /// Remove a row slot at index.
    ///
    /// Panics (debug) if index is out of bounds — callers must guarantee validity.
    pub fn remove(&mut self, index: usize) {
        debug_assert!(index < self.row_count, "RowHeights::remove out of bounds");
        if self.fixed_height.is_none() {
            self.variable_heights.remove(index);
        }
        self.row_count -= 1;
        self.height_version += 1;
    }

    /// Clear all row slots.
    pub fn clear(&mut self) {
        self.row_count = 0;
        self.variable_heights.clear();
        self.height_version += 1;
    }
}

impl Default for RowHeights {
    fn default() -> Self {
        Self::with_fixed_height(24.0)
    }
}
