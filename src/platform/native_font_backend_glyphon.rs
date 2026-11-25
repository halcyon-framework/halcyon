//! SPEC-HAL-047 + SPEC-HAL-065: Glyphon compatibility backend.
//!
//! Per SPEC-HAL-065, this backend performs RASTERIZATION ONLY.
//! All text shaping is handled by TextShaper (rustybuzz).
//!
//! Used as fallback for unsupported platforms and during migration.

use crate::core::text_engine::TextMetrics;
use crate::platform::font_loader::load_inter_font;
use crate::platform::native_font_backend::{GlyphBitmap, GlyphFormat, NativeFontBackend};
use crate::renderer::glyph_atlas::{GlyphKey, GlyphMetrics};
use glyphon::fontdb;
use glyphon::{FontSystem, SwashCache};

/// Glyphon-based implementation of `NativeFontBackend`.
///
/// Provides compatibility fallback using glyphon for shaping and swash for rasterization.
pub struct GlyphonBackend {
    font_system: FontSystem,
    swash_cache: SwashCache,
    /// Cached raw font data for metric extraction
    font_data: Option<Vec<u8>>,
    admitted_face: Option<(u64, fontdb::ID)>,
}

impl GlyphonBackend {
    /// Create a new GlyphonBackend with Inter font loaded.
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();

        // Load Inter font into the font database
        let _inter_font_id = load_inter_font(font_system.db_mut());

        let mut backend = Self {
            font_system,
            swash_cache: SwashCache::new(),
            font_data: None,
            admitted_face: None,
        };
        backend.load_font_data();
        backend
    }

    /// Load raw font data for metric extraction.
    fn load_font_data(&mut self) {
        let db = self.font_system.db();
        if let Some(face_info) = db.faces().next() {
            if let Some((source, _index)) = db.face_source(face_info.id) {
                use glyphon::fontdb::Source;
                match source {
                    Source::Binary(arc) => {
                        self.font_data = Some(arc.as_ref().as_ref().to_vec());
                    }
                    Source::File(path) => {
                        if let Ok(data) = std::fs::read(path) {
                            self.font_data = Some(data);
                        }
                    }
                    Source::SharedFile(path, _) => {
                        if let Ok(data) = std::fs::read(path) {
                            self.font_data = Some(data);
                        }
                    }
                }
            }
        }
    }

    /// Get font metrics from TTF data.
    /// Returns LOGICAL pixels: font_size is logical, metrics scale with it.
    fn get_font_metrics(&self, font_size: f32, _scale: f32) -> TextMetrics {
        if let Some(ref font_data) = self.font_data {
            if let Ok(face) = ttf_parser::Face::parse(font_data, 0) {
                let units_per_em = face.units_per_em() as f32;
                let unit_scale = font_size / units_per_em;

                let ascent = face.ascender() as f32 * unit_scale;
                let descent = (-face.descender() as f32) * unit_scale;
                let line_gap = face.line_gap() as f32 * unit_scale;
                let line_height = ascent + descent + line_gap;

                return TextMetrics {
                    ascent,
                    descent,
                    line_gap,
                    line_height,
                };
            }
        }

        // Fallback synthetic metrics (logical units)
        TextMetrics {
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
            line_gap: font_size * 0.05,
            line_height: font_size * 1.25,
        }
    }

    /// Get mutable access to the font system (for Buffer creation).
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Rasterize a glyph using the font_id from shaping.
    ///
    /// This is called internally by `rasterize_glyph` after extracting the font_id.
    fn rasterize_glyph_internal(
        &mut self,
        glyph_id: u32,
        font_id: fontdb::ID,
        physical_size: f32,
    ) -> Option<(Vec<u8>, GlyphMetrics)> {
        // Build cache key for swash
        let weight = ttf_parser::Face::parse(self.font_data.as_deref()?, 0)
            .ok()?
            .weight()
            .to_number();
        let (cache_key, _, _) = glyphon::CacheKey::new(
            font_id,
            glyph_id as u16,
            physical_size,
            (0.0, 0.0),
            glyphon::Weight(weight),
            glyphon::cosmic_text::CacheKeyFlags::empty(),
        );

        // Rasterize using swash cache
        let image = self
            .swash_cache
            .get_image_uncached(&mut self.font_system, cache_key)?;

        let (width, height) = (image.placement.width as u32, image.placement.height as u32);

        let bitmap = match image.content {
            glyphon::SwashContent::Mask => image.data.clone(),
            glyphon::SwashContent::Color => {
                let mut grayscale = Vec::with_capacity((width * height) as usize);
                for chunk in image.data.chunks(4) {
                    grayscale.push(chunk.get(3).copied().unwrap_or(255));
                }
                grayscale
            }
            glyphon::SwashContent::SubpixelMask => {
                let mut grayscale = Vec::with_capacity((width * height) as usize);
                for chunk in image.data.chunks(3) {
                    let avg = (chunk[0] as u32 + chunk[1] as u32 + chunk[2] as u32) / 3;
                    grayscale.push(avg as u8);
                }
                grayscale
            }
        };

        // Get advance from font metrics
        let advance = if let Some(ref font_data) = self.font_data {
            if let Ok(face) = ttf_parser::Face::parse(font_data, 0) {
                let units_per_em = face.units_per_em() as f32;
                let scale = physical_size / units_per_em;
                let glyph = ttf_parser::GlyphId(glyph_id as u16);
                face.glyph_hor_advance(glyph)
                    .map(|a| a as f32 * scale)
                    .unwrap_or(physical_size * 0.5)
            } else {
                physical_size * 0.5
            }
        } else {
            physical_size * 0.5
        };

        // Extract font metrics
        let (ascender, descender, line_height_val) = if let Some(ref font_data) = self.font_data {
            if let Ok(face) = ttf_parser::Face::parse(font_data, 0) {
                let units_per_em = face.units_per_em() as f32;
                let scale = physical_size / units_per_em;
                let asc = face.ascender() as f32 * scale;
                let desc = face.descender() as f32 * scale;
                let gap = face.line_gap() as f32 * scale;
                (asc, desc, asc - desc + gap)
            } else {
                (
                    physical_size * 0.8,
                    physical_size * -0.2,
                    physical_size * 1.25,
                )
            }
        } else {
            (
                physical_size * 0.8,
                physical_size * -0.2,
                physical_size * 1.25,
            )
        };

        let metrics = GlyphMetrics {
            advance,
            bearing_x: image.placement.left as f32,
            bearing_y: image.placement.top as f32,
            width,
            height,
            ascender,
            descender,
            line_height: line_height_val,
        };

        Some((bitmap, metrics))
    }
}

impl Default for GlyphonBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeFontBackend for GlyphonBackend {
    fn from_font_data(data: std::sync::Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        ttf_parser::Face::parse(&data, 0).ok()?;
        let mut font_system = FontSystem::new();
        let native_id = font_system
            .db_mut()
            .load_font_source(fontdb::Source::Binary(data.clone()))
            .into_iter()
            .next()?;
        Some(Self {
            font_system,
            swash_cache: SwashCache::new(),
            font_data: Some(data.as_ref().clone()),
            admitted_face: Some((font_id, native_id)),
        })
    }

    fn font_metrics(&mut self, font_size: f32, scale: f32) -> TextMetrics {
        self.get_font_metrics(font_size, scale)
    }

    fn rasterize_glyph(&mut self, key: &GlyphKey, scale: f32) -> Option<GlyphBitmap> {
        // Reconstruct fontdb::ID from the u64 stored in key
        let font_id = if let Some((admitted_id, native_id)) = self.admitted_face {
            if admitted_id != key.font_id {
                return None;
            }
            native_id
        } else {
            unsafe { std::mem::transmute::<u64, fontdb::ID>(key.font_id) }
        };

        // Compute physical size from key
        let font_size = key.font_size_scaled as f32 / 100.0;
        // R-010B: 2× oversample for sharper LCD text
        let oversample = 2.0_f32;
        let physical_size = font_size * scale * oversample;

        let (pixels, metrics) =
            self.rasterize_glyph_internal(key.glyph_id, font_id, physical_size)?;

        Some(GlyphBitmap {
            key: *key,
            metrics,
            pixels,
            format: GlyphFormat::Grayscale,
        })
    }
}
