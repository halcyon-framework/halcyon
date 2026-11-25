// =============================================================================
// SPEC-HAL-337 §5 / FORBID-337-2: HTS → RowGeometry projection adapter
// =============================================================================
//
// Timeline widgets never hold `SharedHts`. This adapter is the sanctioned
// boundary: it derives per-frame `RowGeometry` from the shared topology
// substrate's published read surface (`visible_window`, `row_height`,
// `scroll_offset` — INV-337-R1) and feeds the SPEC-HAL-335 §7 seam
// (`TimelineCanvas::set_row_geometry`). Composition code calls it once per
// frame; no consumer re-implements the virtualization arithmetic
// (SPEC-PM-009).
// =============================================================================

use crate::engines::hierarchical::SharedHts;

/// Frame-resolved row geometry from the row authority (INV-335-R2).
/// `y` is relative to the canvas top (viewport space).
///
/// Relocated here from `widgets::timeline_canvas`: SPEC-HAL-335 §7 is
/// substrate law and the FORBID-337-2 adapter (engine layer) produces
/// this type — engines do not depend on widgets. The widget re-exports
/// it, so the old import path remains valid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowGeometry {
    pub id: u64,
    pub y: f32,
    pub height: f32,
}

/// Derive the per-frame `RowGeometry` set from the shared topology
/// substrate (SPEC-HAL-337 §5 / FORBID-337-2).
///
/// **INV-HTSROWS-1 (fixed-height contract):** defined for fixed-row-height
/// substrates — `row_height()` is the uniform height of every visible row.
/// Variable-height bound mode is unsupported: `visible_window` does not
/// publish per-row heights.
///
/// **Coordinate contract (SPEC-PM-002 discipline):**
/// `visible_window().first_y` is content-space; `RowGeometry.y` is
/// viewport-space (canvas-top relative). The mapping is
/// `content_y - scroll_offset()`, so the first visible row's `y` lies in
/// `(-height, 0]` while partially scrolled off the top.
///
/// An empty topology yields an empty `Vec`, which the §7 seam defines as
/// free mode.
pub fn hts_rows(hts: &SharedHts) -> Vec<RowGeometry> {
    let mut guard = hts.borrow_mut();
    let height = guard.row_height();
    // SPEC-PM-017: f32 shims — rung 4 converts this adapter to the
    // single-f64-subtraction form (INV-PM-017-3).
    let scroll = guard.scroll_offset() as f32;
    let window = guard.visible_window();
    let first_y = window.first_y as f32;
    window
        .rows
        .iter()
        .enumerate()
        .map(|(i, vr)| RowGeometry {
            id: vr.node_id,
            y: (first_y + i as f32 * height) - scroll,
            height,
        })
        .collect()
}
