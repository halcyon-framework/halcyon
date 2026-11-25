// =============================================================================
// SPEC-HAL-173: Cell Content Projection Contract
// =============================================================================
//
// INVARIANT-1: CellContentProjection is pure data — no behavior, no ownership,
//              no rendering.
// INVARIANT-2: content_rect is copied directly from the corresponding
//              CellProjection.rect — no independent derivation.
// INVARIANT-3: Content projections exist IFF row is realized AND column is visible.
// INVARIANT-4: Projection is idempotent — same inputs always produce same output.
// INVARIANT-5: No geometry mutation — projection does not modify column geometry
//              or row model.
// INVARIANT-6: Tree indentation stays lockstep with realized_cells — mutated
//              together, never recomputed independently.
//
// =============================================================================

use super::row::{CellId, CellProjection};
use crate::core::math::RectF;

/// Pure data descriptor for cell content projection.
///
/// SPEC-HAL-173: Defines the usable content rect within a cell.
/// This is the contract given to embedded widgets (checkboxes, editors,
/// pickers, custom widgets) — they own layout, state, and focus inside
/// this rect.
///
/// Lifecycle: exists IFF row is realized AND column is visible.
/// Destroyed deterministically when either condition becomes false.
///
/// Content projections are 1:1 with realized_cells in both count and ordering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellContentProjection {
    /// Cell identity (same as corresponding CellProjection).
    pub cell_id: CellId,
    /// Content rect — copied directly from CellProjection.rect.
    pub content_rect: RectF,
}

/// Project cell content from a realized cell.
///
/// SPEC-HAL-173 INV-173-2: content_rect is copied directly from cell.rect.
/// No independent geometry derivation. No additional inputs.
pub fn project_from_cell(cell: &CellProjection) -> CellContentProjection {
    CellContentProjection {
        cell_id: cell.cell_id,
        content_rect: cell.rect,
    }
}
