//! SPEC-HAL-066: Glyph Quad Builder
//!
//! Converts shaped text runs into positioned glyph quads for GPU rendering.
//! This module is the bridge between shaping (SPEC-065) and the renderer.
//!
//! Pipeline: ShapedCluster → GlyphAtlas lookup → GlyphQuad → GPU vertices

use crate::core::scene::Color;

// =============================================================================
// SPEC-HAL-092A: OPTICAL Y BIAS (Paint-Only)
// =============================================================================
//
// Platform rasterizers produce visually different vertical positioning from
// identical font metrics. This bias is applied ONLY to final glyph quad Y,
// affecting pixels but NOT measurement, layout, or auto-size.
//
// INVARIANTS:
// - Does NOT affect baseline_phys, ascent_phys, or any metric values
// - Does NOT affect glyph advances, line breaking, or shaping
// - Applied AFTER ideal_y_phys computation, BEFORE vertical snap
// - Value is in PHYSICAL PIXELS (not logical)
//
// =============================================================================

/// Platform-scoped optical Y bias for glyph placement (SPEC-HAL-092A).
///
/// Returns a vertical offset in **physical pixels** to compensate for
/// platform-specific rasterization differences.
///
/// Negative values shift glyphs upward (toward top of screen).
/// This is paint-only and does not affect layout or measurement.
///
/// NOTE: After fixing the bearing_y double-division bug in CoreText backend,
/// this bias should be 0.0 for all platforms. If visual centering is still off
/// after testing, this can be tuned per-platform. The bearing_y fix is the
/// primary correction; this is a secondary optical adjustment only.
#[inline]
fn optical_y_bias_phys() -> f32 {
    // All platforms: 0.0 after bearing_y fix
    // Tune if needed after visual verification
    0.0
}
use crate::core::text_engine::{Glyph, TextMetrics};
use crate::core::text_shaper::ShapedCluster;
use crate::renderer::glyph_atlas::AtlasEntry;

/// A positioned glyph quad ready for GPU rendering.
///
/// Per SPEC-HAL-066, this is the output of the run→quad builder.
/// Positions are in absolute logical space per SPEC-HAL-024.
#[derive(Debug, Clone, Copy)]
pub struct GlyphQuad {
    /// Top-left X position in logical pixels
    pub x: f32,
    /// Top-left Y position in logical pixels
    pub y: f32,
    /// Width in logical pixels
    pub width: f32,
    /// Height in logical pixels
    pub height: f32,
    /// UV min coordinates in atlas space (0..1)
    pub uv_min: [f32; 2],
    /// UV max coordinates in atlas space (0..1)
    pub uv_max: [f32; 2],
    /// Text color (RGBA, 0.0-1.0)
    pub color: [f32; 4],
    /// R-010B FIX: Per-glyph downsample offset in UV space
    pub downsample_u: f32,
}

impl GlyphQuad {
    /// Create a new GlyphQuad from position, atlas entry, and color.
    /// R-010B: Accounts for oversample factor when computing display size.
    pub fn new(x: f32, y: f32, entry: &AtlasEntry, color: Color) -> Self {
        // R-010B FIX: Compute per-glyph UV texel width for downsample kernel
        let glyph_texel_u = entry.uv_rect.width / entry.size.width;
        let downsample_offset_u = glyph_texel_u * 0.25; // 0.5 subpixel / oversample(=2)

        Self {
            x,
            y,
            width: entry.size.width / entry.oversample,
            height: entry.size.height / entry.oversample,
            uv_min: [entry.uv_rect.x, entry.uv_rect.y],
            uv_max: [
                entry.uv_rect.x + entry.uv_rect.width,
                entry.uv_rect.y + entry.uv_rect.height,
            ],
            color: [color.r, color.g, color.b, color.a],
            downsample_u: downsample_offset_u,
        }
    }
}

/// Parameters for building glyph quads from a shaped run.
#[derive(Debug, Clone)]
pub struct RunToQuadParams {
    /// Origin X position for the text run (logical pixels)
    pub origin_x: f32,
    /// Origin Y position for the text run (logical pixels, top of text box)
    pub origin_y: f32,
    /// Font ascent in logical pixels (baseline offset from top)
    pub ascent: f32,
    /// Device scale factor (logical → physical conversion)
    pub scale_factor: f32,
    /// Text color
    pub color: Color,
}

/// Result of processing a single shaped cluster.
///
/// Contains the positioned quad (if any) and the glyph key for atlas lookup.
#[derive(Debug)]
pub struct ProcessedCluster {
    /// The atlas key to look up or insert this glyph
    pub glyph_key: crate::renderer::glyph_atlas::GlyphKey,
    /// Pen X after this cluster (for accumulation)
    pub next_pen_x: f32,
}

/// Build a glyph quad from a shaped cluster and atlas entry.
///
/// SPEC-HAL-104: Pixel-stable text rendering using integer positions.
///
/// # Arguments
/// * `cluster` - Shaped cluster from TextShaper (rustybuzz) with f32 offsets
/// * `entry` - Atlas entry with UVs and metrics
/// * `params` - Run parameters (origin, ascent, scale, color)
/// * `pen_x_px` - Current pen X position in physical pixels (f32 for subpixel precision)
///
/// # Returns
/// A positioned GlyphQuad in logical coordinates, or None for empty glyphs.
pub fn build_quad_from_cluster(
    cluster: &ShapedCluster,
    entry: &AtlasEntry,
    params: &RunToQuadParams,
    pen_x_px: f32,
) -> Option<GlyphQuad> {
    // Skip empty glyphs (spaces)
    if entry.size.width == 0.0 || entry.size.height == 0.0 {
        return None;
    }

    let scale = params.scale_factor;
    // R-010B: Account for oversample factor - size is at oversample resolution
    // Note: bearing_x/y are now normalized to physical pixels (not oversample)
    let oversample = entry.oversample;

    // Text origin in physical pixels (snapped for crispness)
    let text_origin_x_phys = (params.origin_x * scale).round();
    let text_origin_y_phys = (params.origin_y * scale).round();

    // Baseline in physical pixels (snapped)
    // params.ascent is already in physical pixels (font_metrics now uses font_size * scale)
    let ascent_phys = params.ascent.round();
    let baseline_y_phys = text_origin_y_phys + ascent_phys;

    // Use f32 offsets from shaping - no integer truncation
    let glyph_x_px = pen_x_px + cluster.x_offset_px;
    let glyph_y_px = cluster.y_offset_px;

    // Ideal position including bearing
    // NOTE: build_quad_from_cluster uses bearing_x/y directly (test-only path).
    // entry.bearing_x/y are at OVERSAMPLE resolution in both backends.
    // For this test-only path, we use them directly (may need scaling for accuracy).
    let ideal_x = text_origin_x_phys + glyph_x_px + entry.bearing_x;
    let ideal_y = baseline_y_phys + glyph_y_px - entry.bearing_y;

    // SPEC-HAL-092A: Apply platform optical bias (paint-only, does not affect layout)
    let biased_y = ideal_y + optical_y_bias_phys();

    // SPEC-HAL-104: Only snap baseline vertically.
    // Horizontal snapping per glyph causes total line length jitter across sizes.
    let snapped_x = ideal_x;
    let snapped_y = biased_y.round();

    // Convert to logical pixels
    // R-010B: entry.size is at oversample resolution, divide by oversample for display size
    let x_logical = snapped_x / scale;
    let y_logical = snapped_y / scale;
    let w_logical = entry.size.width / oversample / scale;
    let h_logical = entry.size.height / oversample / scale;

    // R-010B FIX: Compute per-glyph UV texel width for downsample kernel
    let glyph_texel_u = entry.uv_rect.width / entry.size.width;
    let downsample_offset_u = glyph_texel_u * 0.25; // 0.5 subpixel / oversample(=2)

    Some(GlyphQuad {
        x: x_logical,
        y: y_logical,
        width: w_logical,
        height: h_logical,
        uv_min: [entry.uv_rect.x, entry.uv_rect.y],
        uv_max: [
            entry.uv_rect.x + entry.uv_rect.width,
            entry.uv_rect.y + entry.uv_rect.height,
        ],
        color: [
            params.color.r,
            params.color.g,
            params.color.b,
            params.color.a,
        ],
        downsample_u: downsample_offset_u,
    })
}

/// Parameters for building glyph quads from a Glyph.
///
/// Used by the renderer when working with Glyph from BackendTextEngine.
/// The origin coordinates are PRE-SNAPPED physical pixels (single-snap model).
#[derive(Debug, Clone)]
pub struct GlyphToQuadParams {
    /// Text origin X in physical pixels (already snapped by renderer)
    pub origin_phys_x: f32,
    /// Text origin Y in physical pixels (already snapped by renderer)
    pub origin_phys_y: f32,
    /// Font metrics for baseline calculation
    pub metrics: TextMetrics,
    /// Device scale factor (logical → physical)
    pub scale_factor: f32,
    /// Text color
    pub color: Color,
}

/// Build a glyph quad from a Glyph and atlas entry.
///
/// SPEC-HAL-104: Pixel-stable text rendering.
///
/// The text origin is PRE-SNAPPED by the renderer. This function:
/// 1. Computes ideal physical position from origin + glyph offset + bearing
/// 2. SNAPS the final position to integer pixels
/// 3. Uses integer bitmap dimensions from atlas
///
/// This ensures every glyph lands on pixel boundaries, eliminating
/// the "weight wobble" effect where glyphs appear thicker/thinner
/// based on their fractional position.
///
/// # Arguments
/// * `glyph` - Glyph with logical-pixel position (from shape_with_keys)
/// * `entry` - Atlas entry with UVs and metrics
/// * `params` - Parameters with pre-snapped origin and scale
///
/// # Returns
/// A positioned GlyphQuad in logical coordinates, or None for empty glyphs.
pub fn build_quad_from_glyph(
    glyph: &Glyph,
    entry: &AtlasEntry,
    params: &GlyphToQuadParams,
) -> Option<GlyphQuad> {
    // Skip empty glyphs (spaces)
    if entry.size.width == 0.0 || entry.size.height == 0.0 {
        return None;
    }

    let scale = params.scale_factor;
    // R-010B: Account for oversample factor - size is at oversample resolution
    let oversample = entry.oversample;

    // =======================================================================
    // UNIT CONTRACT: All intermediate values in this function are PHYSICAL PIXELS
    // (display resolution, NOT oversample resolution).
    //
    // Inputs that need scaling:
    //   - metrics.ascent/descent: LOGICAL → multiply by scale
    //   - glyph.x/y: LOGICAL → multiply by scale
    //   - entry.bearing_x/y: OVERSAMPLE → divide by oversample
    //   - entry.size: OVERSAMPLE → divide by oversample
    //
    // Inputs already in physical:
    //   - origin_phys_x/y: already scaled by renderer
    // =======================================================================

    // Convert metrics from LOGICAL to PHYSICAL
    let ascent_phys = params.metrics.ascent * scale;

    // Baseline = top of text box + ascent (all in physical pixels now)
    let baseline_phys = params.origin_phys_y + ascent_phys;

    // Glyph origin in physical pixels
    // glyph.x/y are LOGICAL → multiply by scale
    let gx_phys = params.origin_phys_x + glyph.x * scale;
    let gy_phys = baseline_phys + glyph.y * scale;

    // entry.bearing_x/y are at OVERSAMPLE resolution → divide by oversample
    let bearing_x_phys = entry.bearing_x / oversample;
    let bearing_y_phys = entry.bearing_y / oversample;

    // Ideal bitmap position in physical pixels (before snapping)
    let ideal_x_phys = gx_phys + bearing_x_phys;
    let ideal_y_phys = gy_phys - bearing_y_phys;

    // SPEC-HAL-092A: Apply platform optical bias (paint-only, does not affect layout)
    let biased_y_phys = ideal_y_phys + optical_y_bias_phys();

    // SPEC-HAL-104: Only snap vertically; leave X in subpixel space.
    // Per-glyph X snapping introduces 1px quantization noise in total line length.
    let snapped_x_phys = ideal_x_phys;
    let snapped_y_phys = biased_y_phys.round();

    // SPEC-HAL-104: Use integer bitmap dimensions from atlas
    // R-010B: entry.size is at oversample resolution, divide by oversample for display size
    let w_phys = entry.size.width / oversample;
    let h_phys = entry.size.height / oversample;

    // Convert to logical pixels for GlyphQuad (renderer expects logical coords)
    let x_logical = snapped_x_phys / scale;
    let y_logical = snapped_y_phys / scale;
    let w_logical = w_phys / scale;
    let h_logical = h_phys / scale;

    // R-010B FIX: Compute per-glyph UV texel width for downsample kernel
    let glyph_texel_u = entry.uv_rect.width / entry.size.width;
    let downsample_offset_u = glyph_texel_u * 0.25; // 0.5 subpixel / oversample(=2)

    // Debug assertions for sanity checking
    debug_assert!(w_logical >= 0.0, "Quad width must be non-negative");
    debug_assert!(h_logical >= 0.0, "Quad height must be non-negative");

    Some(GlyphQuad {
        x: x_logical,
        y: y_logical,
        width: w_logical,
        height: h_logical,
        uv_min: [entry.uv_rect.x, entry.uv_rect.y],
        uv_max: [
            entry.uv_rect.x + entry.uv_rect.width,
            entry.uv_rect.y + entry.uv_rect.height,
        ],
        color: [
            params.color.r,
            params.color.g,
            params.color.b,
            params.color.a,
        ],
        downsample_u: downsample_offset_u,
    })
}

/// Calculate the total advance width of a run in physical pixels.
pub fn run_advance_phys(clusters: &[ShapedCluster]) -> f32 {
    clusters.iter().map(|c| c.advance_phys).sum()
}

/// Calculate the total advance width of a run in logical pixels.
pub fn run_advance_logical(clusters: &[ShapedCluster], scale_factor: f32) -> f32 {
    run_advance_phys(clusters) / scale_factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::math::{RectF, Size};

    fn make_entry(advance: f32, bearing_x: f32, bearing_y: f32, w: f32, h: f32) -> AtlasEntry {
        AtlasEntry {
            uv_rect: RectF::new(0.0, 0.0, 0.1, 0.1),
            advance,
            bearing_x,
            bearing_y,
            size: Size::new(w, h),
            ascender: 12.0,
            descender: 4.0,
            line_height: 16.0,
            oversample: 1.0, // Tests use 1.0 for simplicity
        }
    }

    fn make_cluster(glyph_id: u32, x_off: f32, y_off: f32, advance: f32) -> ShapedCluster {
        ShapedCluster {
            glyph_id,
            x_offset_phys: x_off,
            y_offset_phys: y_off,
            advance_phys: advance,
            cluster: 0,
            // f32 values - no integer truncation
            x_offset_px: x_off,
            y_offset_px: y_off,
            advance_px: advance,
        }
    }

    #[test]
    fn test_build_quad_basic() {
        let cluster = make_cluster(65, 0.0, 0.0, 10.0);
        let entry = make_entry(10.0, 1.0, 12.0, 8.0, 14.0);
        let params = RunToQuadParams {
            origin_x: 100.0,
            origin_y: 50.0,
            ascent: 12.0,
            scale_factor: 1.0,
            color: Color::WHITE,
        };

        // pen_x_px is f32 for subpixel precision
        let quad = build_quad_from_cluster(&cluster, &entry, &params, 0.0);
        assert!(quad.is_some());

        let q = quad.unwrap();
        assert!(q.width > 0.0);
        assert!(q.height > 0.0);
    }

    #[test]
    fn test_skip_empty_glyph() {
        let cluster = make_cluster(32, 0.0, 0.0, 5.0); // space
        let entry = make_entry(5.0, 0.0, 0.0, 0.0, 0.0); // empty size
        let params = RunToQuadParams {
            origin_x: 0.0,
            origin_y: 0.0,
            ascent: 12.0,
            scale_factor: 1.0,
            color: Color::WHITE,
        };

        // pen_x_px is f32 for subpixel precision
        let quad = build_quad_from_cluster(&cluster, &entry, &params, 0.0);
        assert!(quad.is_none());
    }

    #[test]
    fn test_run_advance() {
        let clusters = vec![
            make_cluster(65, 0.0, 0.0, 10.0),
            make_cluster(66, 0.0, 0.0, 10.0),
            make_cluster(67, 0.0, 0.0, 8.0),
        ];

        assert_eq!(run_advance_phys(&clusters), 28.0);
        assert_eq!(run_advance_logical(&clusters, 2.0), 14.0);
    }

    fn make_glyph(id: u32, x: f32, y: f32, advance: f32) -> Glyph {
        Glyph { id, x, y, advance }
    }

    fn make_metrics() -> TextMetrics {
        TextMetrics {
            ascent: 12.0,
            descent: 4.0,
            line_gap: 2.0,
            line_height: 18.0,
        }
    }

    #[test]
    fn test_build_quad_from_glyph_basic() {
        let glyph = make_glyph(65, 0.0, 0.0, 10.0);
        let entry = make_entry(10.0, 1.0, 12.0, 8.0, 14.0);
        // Pre-snapped physical coords (100.0 * 1.0 = 100.0, already integral)
        let params = GlyphToQuadParams {
            origin_phys_x: 100.0,
            origin_phys_y: 50.0,
            metrics: make_metrics(),
            scale_factor: 1.0,
            color: Color::WHITE,
        };

        let quad = build_quad_from_glyph(&glyph, &entry, &params);
        assert!(quad.is_some());

        let q = quad.unwrap();
        assert!(q.width > 0.0, "Quad width should be positive");
        assert!(q.height > 0.0, "Quad height should be positive");
    }

    #[test]
    fn test_build_quad_from_glyph_skip_empty() {
        let glyph = make_glyph(32, 0.0, 0.0, 5.0); // space
        let entry = make_entry(5.0, 0.0, 0.0, 0.0, 0.0); // empty size
        let params = GlyphToQuadParams {
            origin_phys_x: 0.0,
            origin_phys_y: 0.0,
            metrics: make_metrics(),
            scale_factor: 1.0,
            color: Color::WHITE,
        };

        let quad = build_quad_from_glyph(&glyph, &entry, &params);
        assert!(quad.is_none(), "Empty glyphs should produce no quad");
    }

    #[test]
    fn test_build_quad_from_glyph_with_scale() {
        let glyph = make_glyph(65, 10.0, 0.0, 10.0); // at x=10 logical
        let entry = make_entry(10.0, 1.0, 12.0, 16.0, 28.0); // 2x size at scale 2.0
        // Pre-snapped physical coords (100.0 * 2.0 = 200.0, 50.0 * 2.0 = 100.0)
        let params = GlyphToQuadParams {
            origin_phys_x: 200.0,
            origin_phys_y: 100.0,
            metrics: make_metrics(),
            scale_factor: 2.0,
            color: Color::WHITE,
        };

        let quad = build_quad_from_glyph(&glyph, &entry, &params);
        assert!(quad.is_some());

        let q = quad.unwrap();
        // Width/height should be in logical pixels (divided by scale)
        assert_eq!(q.width, 8.0); // 16.0 / 2.0
        assert_eq!(q.height, 14.0); // 28.0 / 2.0
    }

    #[test]
    fn test_pixel_snap_no_drift() {
        // SPEC-HAL-104: Test that X is NOT snapped (only Y is snapped)
        // Per-glyph X snapping introduces quantization noise in total line length
        let glyph = make_glyph(65, 5.5, 0.0, 10.0); // fractional x position
        let entry = make_entry(10.0, 1.0, 12.0, 8.0, 14.0);
        let params = GlyphToQuadParams {
            origin_phys_x: 100.0, // pre-snapped
            origin_phys_y: 50.0,  // pre-snapped
            metrics: make_metrics(),
            scale_factor: 1.0,
            color: Color::WHITE,
        };

        let quad = build_quad_from_glyph(&glyph, &entry, &params);
        assert!(quad.is_some());

        let q = quad.unwrap();
        // SPEC-HAL-104: X is NOT snapped (preserves subpixel precision)
        // ideal_x = origin_phys_x + glyph.x * scale + bearing_x = 100 + 5.5 + 1.0 = 106.5
        // snapped_x = ideal_x (no rounding for X)
        // x_logical = 106.5 / 1.0 = 106.5
        assert_eq!(q.x, 106.5);
    }
}
