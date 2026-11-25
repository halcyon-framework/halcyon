use crate::core::math::Point;
use crate::core::paint::{Color, PaintSink, TextBackgroundSpan};
use crate::core::text_engine::{Glyph, GlyphRun, TextEngine, TextMetrics};
use crate::platform::font_loader::load_inter_font;
use crate::renderer::glyph_atlas::GlyphMetrics;
use glyphon::fontdb;
use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};

const SHAPE_CACHE_CAPACITY: usize = 4096;

/// Key for shape cache lookups
#[derive(Clone)]
struct ShapeKey {
    text: String,
    font_size_bits: u32, // f32 bits for exact comparison
}

impl PartialEq for ShapeKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text && self.font_size_bits == other.font_size_bits
    }
}

impl Eq for ShapeKey {}

impl Hash for ShapeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font_size_bits.hash(state);
    }
}

impl ShapeKey {
    fn new(text: &str, font_size: f32) -> Self {
        Self {
            text: text.to_string(),
            font_size_bits: font_size.to_bits(),
        }
    }
}

/// Cached shaping result
struct ShapeCacheEntry {
    run: GlyphRun,
    metrics: TextMetrics,
}

/// Glyphon-based text engine implementation
/// All shaping and metric computation is cached by ShapeCache.
/// measure() and metrics() are derived from the same shaping pipeline.
/// draw() uses top-left origin; baseline may be introduced in future SPEC.
pub struct GlyphonTextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
    shape_cache: HashMap<ShapeKey, ShapeCacheEntry>,
    shape_cache_order: VecDeque<ShapeKey>,
    /// Cached font metrics (ascent, descent, line_gap) per font_size
    font_metrics_cache: HashMap<u32, TextMetrics>,
    /// Cached raw font data for metric extraction
    font_data: Option<Vec<u8>>,
}

impl GlyphonTextEngine {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();

        // Load Inter font into the font database
        let _inter_font_id = load_inter_font(font_system.db_mut());

        let mut engine = Self {
            font_system,
            swash_cache: SwashCache::new(),
            shape_cache: HashMap::new(),
            shape_cache_order: VecDeque::new(),
            font_metrics_cache: HashMap::new(),
            font_data: None,
        };
        engine.load_font_data();
        engine
    }

    /// Load raw font data for metric extraction
    fn load_font_data(&mut self) {
        let db = self.font_system.db();
        if let Some(face_info) = db.faces().next() {
            if let Some((source, _index)) = db.face_source(face_info.id) {
                // Extract bytes from fontdb Source
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

    /// Get real font metrics from TTF data
    fn get_font_metrics(&mut self, font_size: f32) -> TextMetrics {
        let size_bits = font_size.to_bits();

        if let Some(metrics) = self.font_metrics_cache.get(&size_bits) {
            return *metrics;
        }

        // Try to extract real metrics from font data
        let (ascent, descent, line_gap) = if let Some(ref font_data) = self.font_data {
            if let Ok(face) = ttf_parser::Face::parse(font_data, 0) {
                let units_per_em = face.units_per_em() as f32;
                let scale = font_size / units_per_em;

                let ascent = face.ascender() as f32 * scale;
                let descent = (-face.descender() as f32) * scale; // descender is negative
                let line_gap = face.line_gap() as f32 * scale;

                (ascent, descent, line_gap)
            } else {
                // Fallback if parsing fails
                Self::synthetic_metrics(font_size)
            }
        } else {
            // Fallback if no font data
            Self::synthetic_metrics(font_size)
        };

        let line_height = ascent + descent + line_gap;

        let metrics = TextMetrics {
            ascent,
            descent,
            line_gap,
            line_height,
        };

        self.font_metrics_cache.insert(size_bits, metrics);
        metrics
    }

    /// Synthetic metrics fallback
    fn synthetic_metrics(font_size: f32) -> (f32, f32, f32) {
        let ascent = font_size * 0.8;
        let descent = font_size * 0.2;
        let line_gap = font_size * 0.2;
        (ascent, descent, line_gap)
    }

    /// Get a mutable reference to the font system (for glyphon Buffer creation)
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Get the primary font ID (0 for now, until multi-font support)
    pub fn primary_font_id(&self) -> u64 {
        0
    }

    /// Rasterize a single glyph to a bitmap.
    ///
    /// Returns (bitmap_data, metrics) or None if rasterization fails.
    /// The bitmap is in R8 format (grayscale alpha).
    ///
    /// IMPORTANT: font_id must be the exact font ID from glyphon's shaping step
    /// to ensure we rasterize the same glyph that was shaped.
    pub fn rasterize_glyph(
        &mut self,
        glyph_id: u32,
        font_id: fontdb::ID,
        font_size: f32,
        scale_factor: f32,
    ) -> Option<(Vec<u8>, GlyphMetrics)> {
        // Build cache key for swash using the font_id from glyphon shaping
        let physical_size = font_size * scale_factor;
        let (cache_key, _, _) = glyphon::CacheKey::new(
            font_id,
            glyph_id as u16,
            physical_size,
            (0.0, 0.0),
            glyphon::cosmic_text::CacheKeyFlags::empty(),
        );

        // Rasterize using swash cache
        let image = self
            .swash_cache
            .get_image_uncached(&mut self.font_system, cache_key)?;

        // Convert SwashContent to R8 format
        let (width, height) = (image.placement.width as u32, image.placement.height as u32);

        let mut bitmap = match image.content {
            glyphon::SwashContent::Mask => {
                // Already grayscale - use directly
                image.data.clone()
            }
            glyphon::SwashContent::Color => {
                // Convert RGBA to grayscale alpha
                let mut grayscale = Vec::with_capacity((width * height) as usize);
                for chunk in image.data.chunks(4) {
                    // Use alpha channel as the mask
                    grayscale.push(chunk.get(3).copied().unwrap_or(255));
                }
                grayscale
            }
            glyphon::SwashContent::SubpixelMask => {
                // Convert RGB subpixel to grayscale
                let mut grayscale = Vec::with_capacity((width * height) as usize);
                for chunk in image.data.chunks(3) {
                    // Average RGB channels
                    let avg = (chunk[0] as u32 + chunk[1] as u32 + chunk[2] as u32) / 3;
                    grayscale.push(avg as u8);
                }
                grayscale
            }
        };

        // Gamma correction disabled for debugging
        // for px in &mut bitmap {
        //     let a = *px as f32 / 255.0;
        //     let a = a.powf(1.2);
        //     *px = (a * 255.0).clamp(0.0, 255.0) as u8;
        // }
        let _ = &mut bitmap; // suppress unused warning

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

        // Extract font metrics for future use
        let (ascender, descender, line_height_val) = if let Some(ref font_data) = self.font_data {
            if let Ok(face) = ttf_parser::Face::parse(font_data, 0) {
                let units_per_em = face.units_per_em() as f32;
                let scale = physical_size / units_per_em;
                let asc = face.ascender() as f32 * scale;
                let desc = face.descender() as f32 * scale; // negative value
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

    /// Internal shaping function - all shaping goes through here
    fn shape_internal(&mut self, text: &str, font_size: f32) -> (GlyphRun, TextMetrics) {
        let key = ShapeKey::new(text, font_size);

        // Check cache first
        if let Some(entry) = self.shape_cache.get(&key) {
            return (entry.run.clone(), entry.metrics);
        }

        // Use glyphon's default line height (font_size * 1.25 as temporary fallback)
        let line_height = font_size * 1.25;

        // Create buffer and shape using Inter font
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
        let attrs = Attrs::new().family(Family::Name("Inter"));
        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Extract glyphs and compute dimensions
        let mut glyphs = Vec::new();
        let mut max_width = 0.0f32;
        let mut line_count = 0;

        for run in buffer.layout_runs() {
            max_width = max_width.max(run.line_w);
            line_count += 1;

            for glyph in run.glyphs.iter() {
                glyphs.push(Glyph {
                    id: glyph.glyph_id as u32,
                    x: glyph.x,
                    y: glyph.y,
                    advance: glyph.w,
                });
            }
        }

        // Ensure at least one line for empty text
        if line_count == 0 {
            line_count = 1;
        }

        let total_height = line_count as f32 * line_height;

        // Use simple fallback metrics (glyphon handles actual baseline positioning)
        let font_metrics = TextMetrics {
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
            line_gap: font_size * 0.05,
            line_height,
        };

        let run = GlyphRun {
            glyphs,
            width: max_width,
            height: total_height,
            text: text.to_string(),
            font_size,
            ascent: font_metrics.ascent,
            descent: font_metrics.descent,
        };

        let entry = ShapeCacheEntry {
            run: run.clone(),
            metrics: font_metrics,
        };
        self.insert_cache_entry(key, entry);

        (run, font_metrics)
    }

    fn insert_cache_entry(&mut self, key: ShapeKey, entry: ShapeCacheEntry) {
        while self.shape_cache.len() >= SHAPE_CACHE_CAPACITY {
            let Some(oldest) = self.shape_cache_order.pop_front() else {
                self.shape_cache.clear();
                break;
            };
            self.shape_cache.remove(&oldest);
        }
        self.shape_cache_order.push_back(key.clone());
        self.shape_cache.insert(key, entry);
    }
}

impl Default for GlyphonTextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine for GlyphonTextEngine {
    fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        let (run, _) = self.shape_internal(text, font_size);
        (run.width, run.height)
    }

    fn metrics(&mut self, _text: &str, font_size: f32) -> TextMetrics {
        // Get real font metrics (not text-dependent for basic metrics)
        self.get_font_metrics(font_size)
    }

    fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun {
        let (run, _) = self.shape_internal(text, font_size);
        run
    }

    fn draw(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        z_index: crate::core::paint::ZIndex,
        sink: &mut dyn PaintSink,
    ) {
        sink.push_text_with_backgrounds(run, x, y, color, Vec::new(), z_index, None);
    }

    fn draw_with_backgrounds(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        backgrounds: Vec<TextBackgroundSpan>,
        z_index: crate::core::paint::ZIndex,
        sink: &mut dyn PaintSink,
    ) {
        // SPEC-EXIT-A1: Push text with background spans for selection/caret
        sink.push_text_with_backgrounds(run, x, y, color, backgrounds, z_index, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scene::{SceneBuilder, SceneNode};

    fn assert_single_text_z(scene: SceneBuilder, expected: crate::core::paint::ZIndex) {
        let graph = scene.build();
        assert_eq!(graph.root.len(), 1);
        let SceneNode::Text(text) = &graph.root[0] else {
            panic!("expected a text node");
        };
        assert_eq!(text.z_index, expected);
    }

    #[test]
    fn explicit_text_z_draw_entry_points_forward() {
        let mut engine = GlyphonTextEngine::new();
        let run = engine.shape("z", 12.0);

        let mut plain = SceneBuilder::new();
        engine.draw(&run, 0.0, 0.0, Color::WHITE, 37, &mut plain);
        assert_single_text_z(plain, 37);

        let mut with_backgrounds = SceneBuilder::new();
        engine.draw_with_backgrounds(
            &run,
            0.0,
            0.0,
            Color::WHITE,
            Vec::new(),
            41,
            &mut with_backgrounds,
        );
        assert_single_text_z(with_backgrounds, 41);
    }
}
