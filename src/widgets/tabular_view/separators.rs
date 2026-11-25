// =============================================================================
// SPEC-HAL-170: Column Separators and Visual Guides
// =============================================================================
//
// Pure computation helper that derives separator geometry from resolved column
// geometry. No state, no identity, no storage.
//
// INV-170-1: Separator geometry derived ONLY from resolved ColumnGeometryTable.
// INV-170-2: No persistent state. Computed fresh each paint frame.
// INV-170-4: Exactly visible_columns - 1 separators emitted.
// INV-170-6: No hit-testing. Header Domain owns resize zones.
//
// This module is PURE computation: it reads resolved geometry and emits rects.
// It does NOT access PaintSink, ColumnModel, or any mutable state.
// =============================================================================

use super::column::ColumnId;
use super::geometry::ResolvedColumnGeometry;
use crate::core::math::RectF;
use crate::core::paint::Color;

/// Separator line width in pixels.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// Default separator color (subtle gray, semi-transparent).
pub const SEPARATOR_COLOR: Color = Color::from_linear(0.3, 0.3, 0.3, 0.5);

/// Separator color when associated column is actively being resized.
pub const SEPARATOR_HOVER_COLOR: Color = Color::from_linear(0.5, 0.5, 0.5, 0.9);

/// A separator segment to be painted.
///
/// SPEC-HAL-170: Separators are visual guides with no identity or state.
/// Each segment is a vertical rect at a column boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorSegment {
    /// Bounding rect for this separator (in TabularSubstrate content coordinates).
    pub rect: RectF,
}

/// Emit column separator segments for all visible column boundaries.
///
/// SPEC-HAL-170: Derives separator positions from resolved column geometry.
/// Calls `emit` for each separator with its rect and color.
///
/// # Arguments
/// * `columns` - Resolved visible column geometries (from `ColumnGeometryTable::columns()`).
/// * `total_height` - Full height of separator (header + body).
/// * `active_resize_column` - Column currently being resized (for hover highlight).
/// * `emit` - Callback receiving each computed segment and its color.
///
/// # Invariants
/// * INV-170-1: Input is resolved geometry only.
/// * INV-170-4: Emits exactly `columns.len() - 1` separators (0 for empty or single column).
pub fn emit_column_separators<F>(
    columns: &[ResolvedColumnGeometry],
    total_height: f32,
    active_resize_column: Option<ColumnId>,
    mut emit: F,
) where
    F: FnMut(SeparatorSegment, Color),
{
    if columns.len() <= 1 {
        return;
    }

    let expected_count = columns.len() - 1;
    let mut emitted = 0;

    // Emit separator at the right edge of each column except the last.
    for col in &columns[..expected_count] {
        let separator_x = col.x_offset + col.width;
        let rect = RectF::new(
            separator_x - SEPARATOR_WIDTH * 0.5,
            0.0,
            SEPARATOR_WIDTH,
            total_height,
        );
        let color = if active_resize_column == Some(col.column_id) {
            SEPARATOR_HOVER_COLOR
        } else {
            SEPARATOR_COLOR
        };
        emit(SeparatorSegment { rect }, color);
        emitted += 1;
    }

    debug_assert_eq!(
        emitted, expected_count,
        "SPEC-HAL-170 INV-170-4 VIOLATION: emitted {} separators, expected {}",
        emitted, expected_count
    );
}
