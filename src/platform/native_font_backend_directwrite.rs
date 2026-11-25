//! SPEC-HAL-047 + SPEC-HAL-065: Windows DirectWrite implementation of `NativeFontBackend`.
//!
//! Per SPEC-HAL-065, this backend performs RASTERIZATION ONLY.
//! All text shaping is handled by TextShaper (rustybuzz).
//!
//! Uses dwrote crate for safe DirectWrite bindings.

use crate::core::text_engine::TextMetrics;
use crate::platform::native_font_backend::{GlyphBitmap, GlyphFormat, NativeFontBackend};
use crate::renderer::glyph_atlas::{GlyphKey, GlyphMetrics};

use dwrote::{
    DWRITE_MEASURING_MODE_NATURAL, Font, FontCollection, FontFace, FontMetrics, FontStretch,
    FontStyle, FontWeight, GdiInterop, GlyphOffset, RenderingParams,
};

/// Windows DirectWrite implementation of `NativeFontBackend`.
pub struct DirectWriteBackend {
    font_face: Option<FontFace>,
    gdi_interop: GdiInterop,
    rendering_params: RenderingParams,
    font_id: u64,
}

impl std::fmt::Debug for DirectWriteBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectWriteBackend")
            .field("font_id", &self.font_id)
            .field("has_font", &self.font_face.is_some())
            .finish()
    }
}

// Helper functions to extract fields from FontMetrics enum (dwrote 0.11.5)
fn get_design_units_per_em(m: &FontMetrics) -> u16 {
    match m {
        FontMetrics::Metrics0(m) => m.designUnitsPerEm,
        FontMetrics::Metrics1(m) => m.designUnitsPerEm,
    }
}

fn get_ascent(m: &FontMetrics) -> u16 {
    match m {
        FontMetrics::Metrics0(m) => m.ascent,
        FontMetrics::Metrics1(m) => m.ascent,
    }
}

fn get_descent(m: &FontMetrics) -> u16 {
    match m {
        FontMetrics::Metrics0(m) => m.descent,
        FontMetrics::Metrics1(m) => m.descent,
    }
}

fn get_line_gap(m: &FontMetrics) -> i16 {
    match m {
        FontMetrics::Metrics0(m) => m.lineGap,
        FontMetrics::Metrics1(m) => m.lineGap,
    }
}

impl DirectWriteBackend {
    pub fn new() -> Self {
        let font_collection = FontCollection::system();
        let gdi_interop = GdiInterop::create();
        let rendering_params = RenderingParams::create_for_primary_monitor();

        // Try Inter first, fall back to Segoe UI
        let (font, font_id) = Self::load_font(&font_collection, "Inter")
            .or_else(|| Self::load_font(&font_collection, "Segoe UI"))
            .unwrap_or((None, 0));

        let font_face = font.map(|f| f.create_font_face());

        Self {
            font_face,
            gdi_interop,
            rendering_params,
            font_id,
        }
    }

    /// Create a DirectWriteBackend from raw font data.
    ///
    /// This is the preferred constructor for SPEC-HAL-065 integration, as it ensures
    /// the backend rasterizes using the same font data that rustybuzz uses for shaping.
    pub fn new_from_data(font_data: std::sync::Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        use dwrote::FontFile;

        let gdi_interop = GdiInterop::create();
        let rendering_params = RenderingParams::create_for_primary_monitor();

        let font_file = FontFile::new_from_buffer(font_data)?;
        let font_face = font_file
            .create_face(0, dwrote::DWRITE_FONT_SIMULATIONS_NONE)
            .ok()?;

        Some(Self {
            font_face: Some(font_face),
            gdi_interop,
            rendering_params,
            font_id,
        })
    }

    fn load_font(collection: &FontCollection, name: &str) -> Option<(Option<Font>, u64)> {
        let family = collection.font_family_by_name(name).ok()??;
        let font = family
            .first_matching_font(FontWeight::Regular, FontStretch::Normal, FontStyle::Normal)
            .ok()?;
        Some((Some(font), Self::hash_font_name(name)))
    }

    fn hash_font_name(name: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        name.hash(&mut h);
        h.finish()
    }
}

impl Default for DirectWriteBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeFontBackend for DirectWriteBackend {
    fn from_font_data(data: std::sync::Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        Self::new_from_data(data, font_id)
    }

    fn font_metrics(&mut self, font_size: f32, _scale: f32) -> TextMetrics {
        let Some(face) = self.font_face.as_ref() else {
            return TextMetrics {
                ascent: 0.0,
                descent: 0.0,
                line_gap: 0.0,
                line_height: 0.0,
            };
        };

        // Return LOGICAL pixels: font_size is logical, metrics scale with it.
        // Physical scaling is renderer's job, not layout's.
        let fm = face.metrics();
        let design_units_per_em = get_design_units_per_em(&fm) as f32;
        let unit_scale = font_size / design_units_per_em;

        let ascent = get_ascent(&fm) as f32 * unit_scale;
        let descent = get_descent(&fm) as f32 * unit_scale;
        let line_gap = get_line_gap(&fm) as f32 * unit_scale;
        let line_height = ascent + descent + line_gap;

        TextMetrics {
            ascent,
            descent,
            line_gap,
            line_height,
        }
    }

    fn rasterize_glyph(&mut self, key: &GlyphKey, scale: f32) -> Option<GlyphBitmap> {
        if key.glyph_id == 0 {
            return None;
        }
        let face = self.font_face.as_ref()?;
        let glyph_index = key.glyph_id as u16;
        let font_size = key.font_size_scaled as f32 / 100.0;
        // R-010B: 2× oversample for sharper LCD text
        let oversample = 2.0_f32;
        let physical_size = font_size * scale * oversample;

        let fm = face.metrics();
        let design_units_per_em = get_design_units_per_em(&fm) as f32;
        let unit_scale = physical_size / design_units_per_em;

        // Get glyph metrics via design_glyph_metrics
        let glyph_metrics = face.design_glyph_metrics(&[glyph_index], false).ok()?;
        let gm = glyph_metrics.first()?;

        // Black-box size calculation:
        // - Width: advanceWidth - leftSideBearing - rightSideBearing
        // - Height: use font ascent + descent (glyph vertical metrics are unreliable)
        let bb_width =
            (gm.advanceWidth as i32 - gm.leftSideBearing - gm.rightSideBearing) as f32 * unit_scale;
        let bb_height = (get_ascent(&fm) + get_descent(&fm)) as f32 * unit_scale;

        // Empty glyph (space, etc.)
        if bb_width <= 0.0 || bb_height <= 0.0 {
            let advance = gm.advanceWidth as f32 * unit_scale;
            return Some(GlyphBitmap {
                key: *key,
                pixels: Vec::new(),
                metrics: GlyphMetrics {
                    advance,
                    bearing_x: 0.0,
                    bearing_y: 0.0,
                    width: 0,
                    height: 0,
                    ascender: get_ascent(&fm) as f32 * unit_scale,
                    descender: get_descent(&fm) as f32 * unit_scale,
                    line_height: (get_ascent(&fm) as f32
                        + get_descent(&fm) as f32
                        + get_line_gap(&fm) as f32)
                        * unit_scale,
                },
                format: GlyphFormat::Grayscale,
            });
        }

        // Bitmap size with 1px padding
        let width = bb_width.ceil() as u32 + 2;
        let height = bb_height.ceil() as u32 + 2;

        // Bearings for the renderer (positioning when drawing the glyph to screen)
        let bearing_x = gm.leftSideBearing as f32 * unit_scale;
        let ascent_phys = get_ascent(&fm) as f32 * unit_scale;

        // Rasterize via GdiInterop + BitmapRenderTarget
        let rt = self.gdi_interop.create_bitmap_render_target(width, height);
        rt.set_pixels_per_dip(1.0);

        // Position glyph within bitmap:
        // - draw_x: offset by left bearing (glyph origin to black box start) + 1px padding
        // - draw_y: baseline position = ascent from top + 1px padding
        let draw_x = -bearing_x + 1.0;
        let draw_y = ascent_phys + 1.0;

        let advances = [gm.advanceWidth as f32 * unit_scale];
        let offsets = [GlyphOffset {
            advanceOffset: 0.0,
            ascenderOffset: 0.0,
        }];

        rt.draw_glyph_run(
            draw_x,
            draw_y,
            DWRITE_MEASURING_MODE_NATURAL,
            face,
            physical_size,
            &[glyph_index],
            &advances,
            &offsets,
            &self.rendering_params,
            &(1.0, 1.0, 1.0),
        );

        // get_opaque_values_as_mask() returns RGBA (4 bytes per pixel):
        // - Bytes 0-2: 0xff (white)
        // - Byte 3: coverage value (from original R channel)
        let rgba = rt.get_opaque_values_as_mask();

        // Extract alpha channel (byte 3) which contains coverage
        let pixels: Vec<u8> = rgba
            .chunks(4)
            .map(|c| c.get(3).copied().unwrap_or(0))
            .collect();

        let metrics = GlyphMetrics {
            advance: gm.advanceWidth as f32 * unit_scale,
            bearing_x,
            bearing_y: ascent_phys, // Distance from baseline to top of glyph bitmap
            width,
            height,
            ascender: ascent_phys,
            descender: get_descent(&fm) as f32 * unit_scale,
            line_height: (get_ascent(&fm) as f32
                + get_descent(&fm) as f32
                + get_line_gap(&fm) as f32)
                * unit_scale,
        };

        Some(GlyphBitmap {
            key: *key,
            metrics,
            pixels,
            format: GlyphFormat::Grayscale,
        })
    }
}
