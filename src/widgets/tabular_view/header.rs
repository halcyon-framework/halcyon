// =============================================================================
// SPEC-HAL-166: TabularSubstrate Header Domain
// =============================================================================
//
// INVARIANT-1: Header domain is a read-only consumer of column geometry.
// INVARIANT-2: Header contains exactly one cell per visible column.
// INVARIANT-3: Header cells consume resolved geometry without adjustment.
// INVARIANT-4: Header cell identity is stable across scrolling, virtualization,
//              and column visibility toggling.
// INVARIANT-5: Pointer events in header bounds route exclusively to Header Domain.
// INVARIANT-6: Header interactions may emit intent directives but must not
//              mutate state directly.
// INVARIANT-7: Headers may not read body geometry or row state.
// =============================================================================

use super::column::ColumnId;
use super::geometry::{ColumnSlot, GeometryView};
use super::intents::{ResizeIntent, SortIntent, TabularIntent};
use crate::core::math::RectF;

/// Header cell projection.
///
/// SPEC-HAL-166: Header cells consume resolved geometry without adjustment.
#[derive(Debug, Clone, Copy)]
pub struct HeaderCell {
    /// Column this header cell represents.
    pub column_id: ColumnId,
    /// Cell rect (derived from column geometry + header height).
    pub rect: RectF,
}

impl HeaderCell {
    /// Create a new header cell from a column slot.
    ///
    /// SPEC-HAL-168: Consumes ColumnSlot, not raw geometry fields.
    pub fn from_slot(slot: ColumnSlot, header_height: f32) -> Self {
        Self {
            column_id: slot.column_id,
            rect: RectF::new(slot.x, 0.0, slot.width, header_height),
        }
    }
}

/// Header domain state.
///
/// SPEC-HAL-166: The Header Domain is a structural sibling of the Body Domain.
/// It contains exactly one cell per visible column.
#[derive(Debug)]
pub struct HeaderDomain {
    /// Header height.
    height: f32,
    /// Projected header cells (populated during layout).
    cells: Vec<HeaderCell>,
    /// Active resize state (if any).
    resize_state: Option<HeaderResizeState>,
    /// Horizontal scroll offset (for rendering).
    scroll_x: f32,
}

/// Internal resize tracking state.
#[derive(Debug, Clone)]
struct HeaderResizeState {
    column_id: ColumnId,
    initial_width: f32,
}

impl HeaderDomain {
    /// Create a new header domain.
    pub fn new(height: f32) -> Self {
        Self {
            height,
            cells: Vec::new(),
            resize_state: None,
            scroll_x: 0.0,
        }
    }

    /// Get header height.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Set header height.
    pub fn set_height(&mut self, height: f32) {
        self.height = height;
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

    /// Get the header bounds (relative to TabularSubstrate origin).
    pub fn bounds(&self, total_width: f32) -> RectF {
        RectF::new(0.0, 0.0, total_width, self.height)
    }

    /// Get projected header cells.
    pub fn cells(&self) -> &[HeaderCell] {
        &self.cells
    }

    /// Project header cells from resolved column geometry.
    ///
    /// SPEC-HAL-166 INVARIANT-1: Read-only consumer of column geometry.
    /// SPEC-HAL-166 INVARIANT-2: Exactly one cell per visible column.
    /// SPEC-HAL-166 INVARIANT-3: Consumes resolved geometry without adjustment.
    /// SPEC-HAL-168: Uses GeometryView for epoch-safe access via ColumnSlot.
    pub fn project(&mut self, geometry: GeometryView<'_>) {
        self.cells.clear();
        for slot in geometry.header_slots() {
            self.cells.push(HeaderCell::from_slot(slot, self.height));
        }
    }

    /// Hit-test a point against header cells.
    ///
    /// SPEC-HAL-166: Hit-testing is performed against header-local geometry only.
    /// Point should be in TabularSubstrate coordinates.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<HeaderHitResult> {
        // Check if in header bounds (y)
        if y < 0.0 || y > self.height {
            return None;
        }

        // Apply scroll offset for hit testing
        let local_x = x + self.scroll_x;

        // Check for resize zones first (edges between columns)
        for (i, cell) in self.cells.iter().enumerate() {
            let right_edge = cell.rect.right();
            // Resize hit zone: 4px on either side of the edge
            if (local_x - right_edge).abs() <= 4.0 {
                return Some(HeaderHitResult::ResizeZone {
                    column_id: cell.column_id,
                    column_index: i,
                });
            }
        }

        // Check for cell hits
        for (i, cell) in self.cells.iter().enumerate() {
            if local_x >= cell.rect.left() && local_x < cell.rect.right() {
                return Some(HeaderHitResult::Cell {
                    column_id: cell.column_id,
                    column_index: i,
                });
            }
        }

        None
    }

    /// Begin a resize operation.
    ///
    /// SPEC-HAL-169: Returns BeginResize intent. Does not mutate geometry.
    pub fn begin_resize(&mut self, column_id: ColumnId, current_width: f32) -> TabularIntent {
        self.resize_state = Some(HeaderResizeState {
            column_id,
            initial_width: current_width,
        });

        ResizeIntent::Begin {
            column_id,
            initial_width: current_width,
        }
        .into()
    }

    /// Update resize with delta.
    ///
    /// SPEC-HAL-169 INVARIANT-4: Does not read or write column geometry.
    pub fn update_resize(&self, delta_x: f32) -> Option<TabularIntent> {
        let state = self.resize_state.as_ref()?;
        let proposed_width = (state.initial_width + delta_x).max(0.0);

        Some(
            ResizeIntent::Update {
                column_id: state.column_id,
                proposed_width,
            }
            .into(),
        )
    }

    /// Commit resize operation.
    ///
    /// SPEC-HAL-169: Emits structural mutation request.
    /// SPEC-HAL-169 INVARIANT-5: Emits proposed_width only — substrate decides final width.
    pub fn commit_resize(&mut self, proposed_width: f32) -> Option<TabularIntent> {
        let state = self.resize_state.take()?;

        Some(
            ResizeIntent::Commit {
                column_id: state.column_id,
                proposed_width,
            }
            .into(),
        )
    }

    /// Cancel resize operation.
    pub fn cancel_resize(&mut self) -> Option<TabularIntent> {
        let state = self.resize_state.take()?;

        Some(
            ResizeIntent::Cancel {
                column_id: state.column_id,
            }
            .into(),
        )
    }

    /// Check if resize is active.
    pub fn is_resizing(&self) -> bool {
        self.resize_state.is_some()
    }

    /// Get the column being resized (if any).
    pub fn resizing_column(&self) -> Option<ColumnId> {
        self.resize_state.as_ref().map(|s| s.column_id)
    }

    /// Request sort on a column.
    ///
    /// SPEC-HAL-171 INVARIANT-1: Must reference exactly one column_id.
    /// SPEC-HAL-171: Emits intent only, no mutation.
    pub fn request_sort(&self, column_id: ColumnId) -> TabularIntent {
        SortIntent::RequestSort {
            column_id,
            direction: None, // Toggle
        }
        .into()
    }

    /// Request sort with explicit direction.
    pub fn request_sort_direction(
        &self,
        column_id: ColumnId,
        direction: super::intents::SortDirection,
    ) -> TabularIntent {
        SortIntent::RequestSort {
            column_id,
            direction: Some(direction),
        }
        .into()
    }

    /// Request to clear sort on a column.
    pub fn request_clear_sort(&self, column_id: ColumnId) -> TabularIntent {
        SortIntent::RequestClearSort { column_id }.into()
    }
}

impl Default for HeaderDomain {
    fn default() -> Self {
        Self::new(24.0)
    }
}

/// Result of header hit-testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderHitResult {
    /// Hit a header cell.
    Cell {
        column_id: ColumnId,
        column_index: usize,
    },
    /// Hit a resize zone between columns.
    ResizeZone {
        column_id: ColumnId,
        column_index: usize,
    },
}
