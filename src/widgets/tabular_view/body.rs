// =============================================================================
// SPEC-HAL-167: TabularSubstrate Body Domain
// =============================================================================
//
// INVARIANT-1: Body Domain is a sibling of Header Domain under TabularSubstrate.
// INVARIANT-2: Body Domain consumes resolved column geometry unchanged.
// INVARIANT-3: Row and cell identity are stable across scrolling, virtualization,
//              and column visibility changes.
// INVARIANT-4: Only the virtualization layer may decide which rows are realized.
// INVARIANT-5: Geometry for unrealized rows must never be queried.
// INVARIANT-6: Body interactions may emit intent directives but must not mutate
//              column geometry, scroll state, or selection state.
//
// =============================================================================
// SPEC-HAL-172: Tree Indentation Model (structural constraint)
// =============================================================================
//
// INVARIANT-7: Tree indentation is NOT a column.
//              It is a row-local horizontal offset applied BEFORE column slots.
//
// This means:
// - Column geometry is invariant; tree depth does not affect column widths.
// - Header alignment remains intact regardless of tree depth.
// - Tree indentation is purely vertical (depth) + local horizontal (offset).
// - Indentation is computed per-row and applied as a transform, not structure.
//
// This must be locked in before any tree projection implementation.
// =============================================================================

use std::collections::HashMap;

use super::cell_content::{CellContentProjection, project_from_cell};
use super::column::ColumnId;
use super::data_adapter::{CellValue, ValueState};
use super::geometry::GeometryView;
use super::intents::{EditIntent, EditPayload, FocusIntent, NavigationDirection, TabularIntent};
use super::row::{CellId, CellProjection, RowId, RowModel, RowProjection};
use super::tree_paint::TreeRenderData;
use crate::core::math::RectF;

/// Result of body hit-testing with tree awareness.
///
/// SPEC-HAL-172: Distinguishes between regular cell hits and tree control zone hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyHitResult {
    /// Hit a regular cell.
    Cell(CellId),
    /// Hit the tree expand/collapse control zone.
    ///
    /// SPEC-HAL-172: row_index is RowModel index (not RowId).
    TreeControl { row_index: usize },
}

/// Body domain state.
///
/// SPEC-HAL-167: The Body Domain is the scrollable structural region
/// containing row projections.
#[derive(Debug)]
pub struct BodyDomain {
    /// Row model (owns row data).
    row_model: RowModel,
    /// Currently realized rows (subset of all rows, determined by virtualization).
    realized_rows: Vec<RowProjection>,
    /// Currently realized cells.
    realized_cells: Vec<CellProjection>,
    /// SPEC-HAL-173: Content projections — 1:1 with realized_cells, same ordering.
    content_projections: Vec<CellContentProjection>,
    /// SPEC-HAL-167: Cached cell values for realized cells, populated at layout
    /// via `populate_cell_values`, read at paint via `cell_value`. Cleared by
    /// `realize_with_range` at the start of every realization. Aligns ListView
    /// with TreeView's "values resolved at layout, not paint" pattern.
    realized_cell_values: HashMap<(RowId, ColumnId), ValueState<CellValue>>,
    /// Viewport rect (relative to TabularSubstrate origin).
    viewport: RectF,
    /// Vertical scroll offset (received from ScrollArea, not owned).
    scroll_y: f32,
    /// Horizontal scroll offset.
    scroll_x: f32,
    /// Cached visible range (first, last) indices for change detection.
    /// Used to determine if layout invalidation is needed on scroll.
    cached_visible_range: (usize, usize),
}

impl BodyDomain {
    /// Create a new body domain.
    pub fn new() -> Self {
        Self {
            row_model: RowModel::default(),
            realized_rows: Vec::new(),
            realized_cells: Vec::new(),
            content_projections: Vec::new(),
            realized_cell_values: HashMap::new(),
            viewport: RectF::zero(),
            scroll_y: 0.0,
            scroll_x: 0.0,
            cached_visible_range: (0, 0),
        }
    }

    /// Create with explicit row model.
    pub fn with_row_model(row_model: RowModel) -> Self {
        Self {
            row_model,
            realized_rows: Vec::new(),
            realized_cells: Vec::new(),
            content_projections: Vec::new(),
            realized_cell_values: HashMap::new(),
            viewport: RectF::zero(),
            scroll_y: 0.0,
            scroll_x: 0.0,
            cached_visible_range: (0, 0),
        }
    }

    /// Get row model reference.
    pub fn row_model(&self) -> &RowModel {
        &self.row_model
    }

    /// Get mutable row model reference.
    pub fn row_model_mut(&mut self) -> &mut RowModel {
        &mut self.row_model
    }

    /// Get the viewport rect.
    pub fn viewport(&self) -> RectF {
        self.viewport
    }

    /// Set the viewport rect (called during layout).
    pub fn set_viewport(&mut self, viewport: RectF) {
        self.viewport = viewport;
    }

    /// Get vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// Set vertical scroll offset.
    pub fn set_scroll_y(&mut self, offset: f32) {
        self.scroll_y = offset;
    }

    /// Get horizontal scroll offset.
    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    /// Set horizontal scroll offset.
    ///
    /// SPEC-HAL-168 INVARIANT-7/8: Scroll offset applies after geometry resolution
    /// and must not modify column widths or offsets.
    pub fn set_scroll_x(&mut self, offset: f32) {
        self.scroll_x = offset;
    }

    /// Get total content height.
    pub fn content_height(&self) -> f32 {
        self.row_model.total_height()
    }

    /// Get realized rows.
    pub fn realized_rows(&self) -> &[RowProjection] {
        &self.realized_rows
    }

    /// Get realized cells.
    pub fn realized_cells(&self) -> &[CellProjection] {
        &self.realized_cells
    }

    /// Get content projections.
    ///
    /// SPEC-HAL-173: 1:1 with realized_cells, same ordering.
    /// Exists IFF row is realized AND column is visible.
    pub fn content_projections(&self) -> &[CellContentProjection] {
        &self.content_projections
    }

    /// Realize rows for a pre-computed visible range with sorted order.
    ///
    /// O(visible) only — no search phase. The range [first..last_exclusive)
    /// and y_offset_at_first are provided by VirtualizationEngine.
    ///
    /// INV-VZ-3: Touches only the visible slice of sorted_indices.
    pub fn realize_with_range(
        &mut self,
        geometry: GeometryView<'_>,
        sorted_indices: &[usize],
        first: usize,
        last_exclusive: usize,
        y_offset_at_first: f32,
    ) {
        self.realized_rows.clear();
        self.realized_cells.clear();
        self.content_projections.clear();
        self.realized_cell_values.clear();

        if self.row_model.is_empty() || geometry.is_empty() || sorted_indices.is_empty() {
            return;
        }

        if first >= last_exclusive || first >= sorted_indices.len() {
            return;
        }

        let end = last_exclusive.min(sorted_indices.len());
        let row_count = self.row_model.len();

        debug_assert!(
            sorted_indices[first..end]
                .iter()
                .all(|&idx| idx < row_count),
            "realize_with_range: sorted_indices contains out-of-range index (row_count={})",
            row_count
        );

        let slots: Vec<_> = geometry.body_slots().collect();

        let mut y_offset = y_offset_at_first;
        for &row_index in &sorted_indices[first..end] {
            let row_id = self.row_model.get(row_index).unwrap();
            let row_height = self.row_model.height_at(row_index);

            self.realized_rows
                .push(RowProjection::new(row_id, row_index, y_offset, row_height));

            for slot in &slots {
                let cell_rect =
                    RectF::new(slot.x, y_offset + self.viewport.y, slot.width, row_height);
                self.realized_cells
                    .push(CellProjection::new(row_id, slot.column_id, cell_rect));
            }

            y_offset += row_height;
        }

        self.content_projections
            .extend(self.realized_cells.iter().map(project_from_cell));
    }

    /// SPEC-HAL-167: Resolve cell values for every realized cell.
    ///
    /// Called once per layout, after `realize_with_range`. The closure is
    /// invoked once per `(row_id, column_id)` pair in `realized_cells` —
    /// O(realized_cells), independent of total row count. Subsequent paints
    /// read from the cache via `cell_value` instead of calling the data
    /// source per cell.
    ///
    /// Aligns ListView with TreeView's "values at layout" model
    /// (TreeView stores labels in `VisibleRow.label` at layout-time).
    pub fn populate_cell_values<F>(&mut self, mut resolve: F)
    where
        F: FnMut(RowId, ColumnId) -> ValueState<CellValue>,
    {
        for cell in &self.realized_cells {
            let value = resolve(cell.row_id(), cell.column_id());
            self.realized_cell_values
                .insert((cell.row_id(), cell.column_id()), value);
        }
    }

    /// SPEC-HAL-167: Read a cached cell value resolved during the most recent
    /// `populate_cell_values` call. Returns `None` if the cell is not realized
    /// or `populate_cell_values` was not invoked this frame.
    pub fn cell_value(&self, row_id: RowId, column_id: ColumnId) -> Option<&ValueState<CellValue>> {
        self.realized_cell_values.get(&(row_id, column_id))
    }

    /// Update cached visible range from a pre-computed value.
    /// Returns true if the range changed.
    pub fn update_cached_visible_range(&mut self, new_range: (usize, usize)) -> bool {
        let changed = new_range != self.cached_visible_range;
        self.cached_visible_range = new_range;
        changed
    }

    /// Compute total content height for sorted order.
    ///
    /// For fixed-height rows this is the same as natural order.
    /// For variable-height rows this sums heights in sorted order.
    pub fn content_height_sorted(&self, sorted_indices: &[usize]) -> f32 {
        sorted_indices
            .iter()
            .map(|&idx| self.row_model.height_at(idx))
            .sum()
    }

    /// Check if a row is currently realized.
    pub fn is_row_realized(&self, row_id: RowId) -> bool {
        self.realized_rows.iter().any(|r| r.row_id == row_id)
    }

    /// Get cell projection by cell ID.
    ///
    /// SPEC-HAL-167 INVARIANT-5: Returns None for unrealized cells.
    pub fn get_cell(&self, row_id: RowId, column_id: ColumnId) -> Option<&CellProjection> {
        self.realized_cells
            .iter()
            .find(|c| c.row_id() == row_id && c.column_id() == column_id)
    }

    /// Hit-test a point against body cells.
    ///
    /// Point should be in TabularSubstrate coordinates.
    /// Returns cell ID if hit, None otherwise.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<CellId> {
        // TRACE-LISTVIEW: Log hit_test entry point
        #[cfg(feature = "trace-listview")]
        {
            let contains = self.viewport.contains_point(x, y);
            tracing::info!(
                "[BODY.hit_test] input=({:.1},{:.1}) viewport={:?} scroll_y={:.1} contains={}",
                x,
                y,
                self.viewport,
                self.scroll_y,
                contains
            );
        }

        // Check if in body bounds
        if !self.viewport.contains_point(x, y) {
            return None;
        }

        // Apply scroll offsets for hit testing
        let local_x = x + self.scroll_x;
        let local_y = y - self.viewport.y + self.scroll_y;

        #[cfg(feature = "trace-listview")]
        tracing::info!(
            "[BODY.hit_test] local_y={:.1} (y={:.1} - viewport.y={:.1} + scroll_y={:.1})",
            local_y,
            y,
            self.viewport.y,
            self.scroll_y
        );

        // Find row at y position
        let row_index = self.row_model.row_at_y(local_y);
        #[cfg(feature = "trace-listview")]
        tracing::info!(
            "[BODY.hit_test] row_at_y({:.1}) -> {:?}",
            local_y,
            row_index
        );
        let row_index = row_index?;
        let row_id = self.row_model.get(row_index)?;

        // Find column at x position
        for cell in &self.realized_cells {
            if cell.row_id() == row_id && local_x >= cell.rect.left() && local_x < cell.rect.right()
            {
                return Some(cell.cell_id);
            }
        }

        // Row-level hit zone fallback: if we found a row but no cell matched,
        // return the first cell of that row. This ensures clicks anywhere
        // within the row's vertical bounds register as hits.
        if let Some(first_cell) = self.realized_cells.iter().find(|c| c.row_id() == row_id) {
            return Some(first_cell.cell_id);
        }

        None
    }

    /// Hit-test a point with tree-aware control zone detection.
    ///
    /// If tree data is provided and the point hits the tree control zone
    /// (left side of first column, `control_width` pixels wide),
    /// returns `TreeControl` instead of `Cell`.
    ///
    /// The tree control zone is active only when:
    /// - `tree_data` is provided (Some), AND
    /// - `tree_data.has_children(row_index)` is true for the hit row.
    pub fn hit_test_with_tree(
        &self,
        x: f32,
        y: f32,
        tree_data: &dyn TreeRenderData,
        first_column_id: ColumnId,
        control_width: f32,
    ) -> Option<BodyHitResult> {
        if !self.viewport.contains_point(x, y) {
            return None;
        }

        // SPEC-HAL-024: Transform viewport coords → content coords.
        // MUST match hit_test() transformation exactly.
        let local_x = x + self.scroll_x;
        let local_y = y - self.viewport.y + self.scroll_y;

        // Find the realized row at this y position (content space comparison)
        let hit_row = self
            .realized_rows
            .iter()
            .find(|r| local_y >= r.y_offset && local_y < r.y_offset + r.height);

        if let Some(row_proj) = hit_row {
            let row_index = row_proj.row_index;
            let row_id = row_proj.row_id;

            // Check tree control zone first (higher priority than cell)
            if tree_data.has_children(row_index) {
                if let Some(cell) = self
                    .realized_cells
                    .iter()
                    .find(|c| c.row_id() == row_id && c.column_id() == first_column_id)
                {
                    // Tree control zone: left side of the (indented) cell rect
                    if local_x >= cell.rect.left() && local_x < cell.rect.left() + control_width {
                        return Some(BodyHitResult::TreeControl { row_index });
                    }
                }
            }

            // Regular cell hit
            for cell in &self.realized_cells {
                if cell.row_id() == row_id
                    && local_x >= cell.rect.left()
                    && local_x < cell.rect.right()
                {
                    return Some(BodyHitResult::Cell(cell.cell_id));
                }
            }

            // Row-level hit zone fallback
            if let Some(first_cell) = self.realized_cells.iter().find(|c| c.row_id() == row_id) {
                return Some(BodyHitResult::Cell(first_cell.cell_id));
            }
        }

        None
    }

    /// Apply tree indentation to realized cells.
    ///
    /// INVARIANT-TREE-3: Indentation does not modify column geometry.
    /// It only offsets the cell content rect for the first visible column.
    ///
    /// For column `first_column_id`, each cell's x is shifted right by
    /// `depth * indent_step` and its width is reduced accordingly.
    pub fn apply_tree_indentation(
        &mut self,
        tree_data: &dyn TreeRenderData,
        first_column_id: ColumnId,
        indent_step: f32,
    ) {
        // SPEC-HAL-173 INV-173-6: Content projections mutated lockstep with realized_cells.
        for (cell, proj) in self
            .realized_cells
            .iter_mut()
            .zip(self.content_projections.iter_mut())
        {
            if cell.column_id() == first_column_id {
                if let Some(row_index) = self.row_model.index_of(cell.row_id()) {
                    let depth = tree_data.depth(row_index);
                    let indent = depth as f32 * indent_step;
                    cell.rect.x += indent;
                    cell.rect.width = (cell.rect.width - indent).max(0.0);
                    proj.content_rect = cell.rect;
                }
            }
        }
    }

    /// Request to begin editing a cell.
    ///
    /// SPEC-HAL-174 INVARIANT-4: Does not mutate data, selection, geometry, or focus.
    pub fn request_begin_edit(&self, row_id: RowId, column_id: ColumnId) -> TabularIntent {
        EditIntent::RequestBeginEdit { row_id, column_id }.into()
    }

    /// Request to commit an edit.
    ///
    /// SPEC-HAL-174: TabularSubstrate does not interpret payloads.
    pub fn request_commit_edit(
        &self,
        row_id: RowId,
        column_id: ColumnId,
        payload: EditPayload,
    ) -> TabularIntent {
        EditIntent::RequestCommitEdit {
            row_id,
            column_id,
            payload,
        }
        .into()
    }

    /// Request to cancel an edit.
    pub fn request_cancel_edit(&self, row_id: RowId, column_id: ColumnId) -> TabularIntent {
        EditIntent::RequestCancelEdit { row_id, column_id }.into()
    }

    /// Request to focus a cell.
    ///
    /// SPEC-HAL-175 INVARIANT-2: Projects focus into exactly one cell.
    pub fn request_focus(&self, row_id: RowId, column_id: ColumnId) -> TabularIntent {
        FocusIntent::RequestFocus { row_id, column_id }.into()
    }

    /// Request navigation in a direction.
    ///
    /// SPEC-HAL-175: Emits intent with target coordinates, not widgets.
    pub fn request_navigate(
        &self,
        from: Option<(RowId, ColumnId)>,
        direction: NavigationDirection,
    ) -> TabularIntent {
        FocusIntent::RequestNavigate { from, direction }.into()
    }

    /// Request to clear focus.
    pub fn request_clear_focus(&self) -> TabularIntent {
        FocusIntent::RequestClearFocus.into()
    }
}

impl Default for BodyDomain {
    fn default() -> Self {
        Self::new()
    }
}
