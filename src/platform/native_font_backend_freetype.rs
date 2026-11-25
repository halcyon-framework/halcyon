//! SPEC-HAL-047 + SPEC-HAL-065: Linux/Unix FreeType implementation of `NativeFontBackend`.
//!
//! Per SPEC-HAL-065, this backend performs RASTERIZATION ONLY.
//! All text shaping is handled by TextShaper (rustybuzz).
//!
//! # CRITICAL INVARIANT
//!
//! Glyph IDs are only valid for the font that produced them.
//!
//! The rasterizer MUST use the exact same font bytes as the shaper (rustybuzz).
//! Using a different font (e.g., system DejaVu vs embedded Inter) will corrupt
//! all text because glyph ID mappings differ between fonts.

use crate::core::text_engine::TextMetrics;
use crate::platform::native_font_backend::{GlyphBitmap, GlyphFormat, NativeFontBackend};
use crate::renderer::glyph_atlas::{GlyphKey, GlyphMetrics};

use freetype::bitmap::PixelMode;
use freetype::face::LoadFlag;
use freetype::{Face, Library};
use std::sync::Arc;

/// Linux/Unix FreeType implementation of `NativeFontBackend`.
pub struct FreeTypeBackend {
    _lib: Library,
    face: Face,
    font_id: u64,
    /// Owned font data - must outlive the Face.
    /// FreeType Face references this memory.
    _font_data: Option<Arc<Vec<u8>>>,
}

impl std::fmt::Debug for FreeTypeBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreeTypeBackend")
            .field("font_id", &self.font_id)
            .finish()
    }
}

impl FreeTypeBackend {
    /// Create a FreeTypeBackend from font data bytes.
    ///
    /// The `font_data` must be the same bytes used by the shaper.
    pub fn new(font_data: Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        let lib = Library::init().ok()?;
        let face = lib.new_memory_face(font_data.to_vec(), 0).ok()?;

        Some(Self {
            _lib: lib,
            face,
            font_id,
            _font_data: Some(font_data),
        })
    }
}

impl NativeFontBackend for FreeTypeBackend {
    fn from_font_data(data: Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        Self::new(data, font_id)
    }

    fn font_metrics(&mut self, font_size: f32, scale: f32) -> TextMetrics {
        // Return LOGICAL pixels: compute at physical size, then divide by scale.
        // Physical scaling is renderer's job, not layout's.
        let physical_size = font_size * scale;
        let char_height_26_6 = (physical_size * 64.0).round() as isize;

        self.face
            .set_char_size(0, char_height_26_6, 72, 72)
            .expect("FreeType: set_char_size failed");

        let m = self
            .face
            .size_metrics()
            .expect("FreeType: missing size metrics");

        let ascent_phys = m.ascender as f32 / 64.0;
        let descent_phys = -m.descender as f32 / 64.0; // descender is negative
        let line_height_phys = m.height as f32 / 64.0;
        let line_gap_phys = (line_height_phys - ascent_phys - descent_phys).max(0.0);

        // Convert back to logical pixels
        TextMetrics {
            ascent: ascent_phys / scale,
            descent: descent_phys / scale,
            line_gap: line_gap_phys / scale,
            line_height: line_height_phys / scale,
        }
    }

    fn rasterize_glyph(&mut self, key: &GlyphKey, scale: f32) -> Option<GlyphBitmap> {
        if key.glyph_id == 0 {
            return None;
        }
        let glyph_index = key.glyph_id;
        let font_size = key.font_size_scaled as f32 / 100.0;
        // R-010B: 2× oversample for sharper LCD text
        let oversample = 2.0_f32;
        let physical_size = font_size * scale * oversample;

        let char_height_26_6 = (physical_size * 64.0).round() as isize;
        self.face.set_char_size(0, char_height_26_6, 72, 72).ok()?;

        self.face.load_glyph(glyph_index, LoadFlag::RENDER).ok()?;

        let glyph = self.face.glyph();
        let bm = glyph.bitmap();

        let width = bm.width() as u32;
        let height = bm.rows() as u32;

        if width == 0 || height == 0 {
            // SPEC-HAL-360: Explicit no-ink metrics, before pixel-format validation.
            let gm = glyph.metrics();
            let advance = gm.horiAdvance as f32 / 64.0;

            return Some(GlyphBitmap {
                key: *key,
                pixels: Vec::new(),
                metrics: GlyphMetrics {
                    advance,
                    bearing_x: 0.0,
                    bearing_y: 0.0,
                    width: 0,
                    height: 0,
                    ascender: 0.0,
                    descender: 0.0,
                    line_height: 0.0,
                },
                format: GlyphFormat::Grayscale,
            });
        }

        if bm.pixel_mode().ok()? != PixelMode::Gray {
            return None;
        }
        let pitch = bm.pitch();
        let buffer = bm.buffer();
        let stride = pitch.unsigned_abs() as usize;

        let mut pixels = vec![0u8; (width * height) as usize];

        // Handle positive pitch (top-down rows) and negative pitch (bottom-up rows)
        if pitch >= 0 {
            for y in 0..height as usize {
                let src_offset = y * stride;
                let dst_offset = y * width as usize;
                let src_slice = &buffer[src_offset..src_offset + width as usize];
                pixels[dst_offset..dst_offset + width as usize].copy_from_slice(src_slice);
            }
        } else {
            // Negative pitch: rows are stored bottom-to-top
            for y in 0..height as usize {
                let src_row = (height as usize - 1) - y;
                let src_offset = src_row * stride;
                let dst_offset = y * width as usize;
                let src_slice = &buffer[src_offset..src_offset + width as usize];
                pixels[dst_offset..dst_offset + width as usize].copy_from_slice(src_slice);
            }
        }

        let gm = glyph.metrics();
        let size = self
            .face
            .size_metrics()
            .expect("FreeType: missing size metrics");

        let advance = gm.horiAdvance as f32 / 64.0;
        let bearing_x = gm.horiBearingX as f32 / 64.0;
        let bearing_y = gm.horiBearingY as f32 / 64.0;

        let ascender = size.ascender as f32 / 64.0;
        let descender = -size.descender as f32 / 64.0;
        let line_height = size.height as f32 / 64.0;

        let metrics = GlyphMetrics {
            advance,
            bearing_x,
            bearing_y,
            width,
            height,
            ascender,
            descender,
            line_height,
        };

        Some(GlyphBitmap {
            key: *key,
            pixels,
            metrics,
            format: GlyphFormat::Grayscale,
        })
    }
}
