// =============================================================================
// SPEC-HAL-167: TabularSubstrate Row Projection
// =============================================================================
//
// INVARIANT-3: Row and cell identity are stable across scrolling, virtualization,
//              and column visibility changes.
// INVARIANT-4: Only the virtualization layer may decide which rows are realized.
// INVARIANT-5: Geometry for unrealized rows must never be queried.
//
// INVARIANT-6: Cell identity lifecycle constraint:
//              A cell's identity is stable ONLY while:
//              (a) its column is visible, AND
//              (b) its row is realized.
//              Editors and other cell-attached state MUST be torn down
//              deterministically when either condition becomes false.
//
// Index-based identity is forbidden.
// =============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

use super::column::ColumnId;
use crate::core::math::RectF;
use crate::core::ordered_selection::OrderedIndex;
use crate::model::tabular::row_heights::RowHeights;

/// Stable, opaque row identifier.
///
/// SPEC-HAL-167: Row identity derives from (TabularSubstrateId, RowId).
/// Identity is stable across scrolling, virtualization, and column visibility changes.
/// Index-based identity is forbidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowId(u64);

impl RowId {
    /// Create a new unique RowId.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        RowId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Create a RowId from a raw value (for deserialization/external identity).
    pub const fn from_raw(id: u64) -> Self {
        RowId(id)
    }

    /// Get the raw u64 value (for debugging/serialization).
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for RowId {
    fn default() -> Self {
        Self::new()
    }
}

/// Row projection data.
///
/// SPEC-HAL-167: A Row is a projection unit, not a widget container.
/// Row height resolution may be fixed or variable but is vertically scoped only.
#[derive(Debug, Clone, Copy)]
pub struct RowProjection {
    /// Stable row identifier.
    pub row_id: RowId,
    /// Frame-local row index (for rendering order only, NOT identity).
    pub row_index: usize,
    /// Vertical offset from body origin.
    pub y_offset: f32,
    /// Row height.
    pub height: f32,
}

impl RowProjection {
    /// Create a new row projection.
    pub fn new(row_id: RowId, row_index: usize, y_offset: f32, height: f32) -> Self {
        Self {
            row_id,
            row_index,
            y_offset,
            height,
        }
    }

    /// Get the row's bounding rect (relative to body origin).
    pub fn bounds(&self, total_width: f32) -> RectF {
        RectF::new(0.0, self.y_offset, total_width, self.height)
    }
}

/// Cell identity.
///
/// SPEC-HAL-167: Cell identity derives from (TabularSubstrateId, RowId, ColumnId).
///
/// INVARIANT-6: This identity is stable ONLY while the column is visible AND
/// the row is realized. Any state attached to a cell (editors, focus, selection)
/// MUST be torn down deterministically when either condition becomes false.
/// This is critical for inline editing correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellId {
    pub row_id: RowId,
    pub column_id: ColumnId,
}

impl CellId {
    pub const fn new(row_id: RowId, column_id: ColumnId) -> Self {
        Self { row_id, column_id }
    }
}

/// Cell projection data.
///
/// SPEC-HAL-167: For each realized row, exactly one cell is projected per visible column.
/// Cells may not span columns or rows.
#[derive(Debug, Clone, Copy)]
pub struct CellProjection {
    /// Cell identity.
    pub cell_id: CellId,
    /// Cell rect (in TabularSubstrate coordinates).
    pub rect: RectF,
}

impl CellProjection {
    /// Create a new cell projection.
    pub fn new(row_id: RowId, column_id: ColumnId, rect: RectF) -> Self {
        Self {
            cell_id: CellId::new(row_id, column_id),
            rect,
        }
    }

    /// Get the row ID.
    pub fn row_id(&self) -> RowId {
        self.cell_id.row_id
    }

    /// Get the column ID.
    pub fn column_id(&self) -> ColumnId {
        self.cell_id.column_id
    }
}

/// Row realization state.
///
/// SPEC-HAL-167 INVARIANT-4: Only the virtualization layer decides which rows are realized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowRealizationState {
    /// Row is not realized (virtualized out).
    Unrealized,
    /// Row is realized and visible.
    Realized,
}

/// Row model for tracking row data and realization.
///
/// SPEC-HAL-167: The Body Domain owns row realization lifecycle.
///
/// Composes RowHeights (engine-grade height oracle) with widget-only
/// identity registry (rows: Vec<RowId>).
///
/// INV-ROWMODEL-SYNC: `self.rows.len() == self.heights.len()` must hold
/// at all times. Enforced by debug_assert in every public method.
#[derive(Debug)]
pub struct RowModel {
    /// All row IDs in logical order.
    rows: Vec<RowId>,
    /// Engine-grade height oracle.
    heights: RowHeights,
}

impl RowModel {
    /// INV-ROWMODEL-SYNC: rows and heights must always agree on count.
    #[inline]
    fn assert_sync(&self) {
        debug_assert_eq!(
            self.rows.len(),
            self.heights.len(),
            "RowModel/RowHeights desync"
        );
    }

    /// Create a row model with fixed row height.
    pub fn with_fixed_height(height: f32) -> Self {
        Self {
            rows: Vec::new(),
            heights: RowHeights::with_fixed_height(height),
        }
    }

    /// Create a row model with variable row heights.
    pub fn with_variable_heights() -> Self {
        Self {
            rows: Vec::new(),
            heights: RowHeights::with_variable_heights(),
        }
    }

    // =========================================================================
    // Height methods — forwarded to RowHeights
    // =========================================================================

    /// Get the number of rows.
    pub fn len(&self) -> usize {
        self.assert_sync();
        self.rows.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.assert_sync();
        self.rows.is_empty()
    }

    /// Get row height at index.
    pub fn height_at(&self, index: usize) -> f32 {
        self.assert_sync();
        self.heights.height_at(index)
    }

    /// Get the fixed row height (if using fixed-height mode).
    pub fn fixed_height(&self) -> Option<f32> {
        self.assert_sync();
        self.heights.fixed_height()
    }

    /// Monotonic version counter for height/count mutations.
    /// Used by VirtualizationEngine to detect prefix-sum staleness.
    pub fn height_version(&self) -> u64 {
        self.assert_sync();
        self.heights.height_version()
    }

    /// Set the fixed row height.
    ///
    /// **INV-TV-ROW-HEIGHT-CONSISTENCY**: This sets the uniform height for all rows
    /// when using fixed-height mode. Call this before adding rows to ensure all
    /// rows use the correct height.
    pub fn set_fixed_height(&mut self, height: f32) {
        self.assert_sync();
        self.heights.set_fixed_height(height);
        self.assert_sync();
    }

    /// Set height for a row (variable height mode only).
    pub fn set_height(&mut self, index: usize, height: f32) {
        self.assert_sync();
        self.heights.set_height(index, height);
        self.assert_sync();
    }

    /// Access the inner RowHeights (engine-grade height oracle).
    pub fn heights(&self) -> &RowHeights {
        self.assert_sync();
        &self.heights
    }

    /// Mutable access to the inner RowHeights.
    pub fn heights_mut(&mut self) -> &mut RowHeights {
        self.assert_sync();
        &mut self.heights
    }

    // =========================================================================
    // Identity methods — widget-only
    // =========================================================================

    /// Get all row IDs.
    pub fn rows(&self) -> &[RowId] {
        self.assert_sync();
        &self.rows
    }

    /// Get row ID at index.
    pub fn get(&self, index: usize) -> Option<RowId> {
        self.assert_sync();
        self.rows.get(index).copied()
    }

    /// Get row index by ID.
    pub fn index_of(&self, id: RowId) -> Option<usize> {
        self.assert_sync();
        self.rows.iter().position(|&r| r == id)
    }

    // =========================================================================
    // Mutation — maintains rows + heights in sync
    // =========================================================================

    /// Add a row.
    pub fn push(&mut self, id: RowId) {
        self.assert_sync();
        self.rows.push(id);
        self.heights.push();
        self.assert_sync();
    }

    /// Add a row with specific height (for variable height mode).
    pub fn push_with_height(&mut self, id: RowId, height: f32) {
        self.assert_sync();
        self.rows.push(id);
        self.heights.push_with_height(height);
        self.assert_sync();
    }

    /// Remove a row by index.
    pub fn remove(&mut self, index: usize) -> Option<RowId> {
        self.assert_sync();
        if index < self.rows.len() {
            self.heights.remove(index);
            let id = self.rows.remove(index);
            self.assert_sync();
            Some(id)
        } else {
            None
        }
    }

    /// Clear all rows.
    pub fn clear(&mut self) {
        self.assert_sync();
        self.rows.clear();
        self.heights.clear();
        self.assert_sync();
    }

    // =========================================================================
    // Scroll/hit-testing helpers — widget-only convenience
    // =========================================================================

    /// Get total content height.
    pub fn total_height(&self) -> f32 {
        self.assert_sync();
        if let Some(h) = self.heights.fixed_height() {
            h * self.rows.len() as f32
        } else {
            (0..self.heights.len())
                .map(|i| self.heights.height_at(i))
                .sum()
        }
    }

    /// Calculate y_offset for a row at given index.
    pub fn y_offset_at(&self, index: usize) -> f32 {
        self.assert_sync();
        if let Some(h) = self.heights.fixed_height() {
            h * index as f32
        } else {
            (0..index).map(|i| self.heights.height_at(i)).sum()
        }
    }

    /// Find row at y position.
    pub fn row_at_y(&self, y: f32) -> Option<usize> {
        self.assert_sync();
        if y < 0.0 {
            return None;
        }

        if let Some(h) = self.heights.fixed_height() {
            let index = (y / h) as usize;
            if index < self.rows.len() {
                Some(index)
            } else {
                None
            }
        } else {
            let mut offset = 0.0;
            for i in 0..self.heights.len() {
                let height = self.heights.height_at(i);
                if y >= offset && y < offset + height {
                    return Some(i);
                }
                offset += height;
            }
            None
        }
    }
}

impl Default for RowModel {
    fn default() -> Self {
        Self::with_fixed_height(24.0)
    }
}

// =============================================================================
// OrderedIndex Implementation for ordered_selection Integration
// =============================================================================

impl OrderedIndex<RowId> for RowModel {
    fn len(&self) -> usize {
        self.assert_sync();
        self.rows.len()
    }

    fn key_at(&self, index: usize) -> RowId {
        self.assert_sync();
        self.rows[index]
    }

    fn index_of(&self, key: RowId) -> Option<usize> {
        self.assert_sync();
        self.rows.iter().position(|&r| r == key)
    }
}
