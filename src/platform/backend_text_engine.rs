//! SPEC-HAL-047 + SPEC-HAL-065: BackendTextEngine<B> adapter.
//!
//! Wraps any `NativeFontBackend` to implement the existing `TextEngine` trait.
//! Per SPEC-HAL-065, shaping is handled by TextShaper (rustybuzz), not the backend.
//! Widgets and layout code continue using `TextEngine` unchanged.

use crate::core::math::Point;
use crate::core::paint::{Color, PaintSink, TextBackgroundSpan, ZIndex};
use crate::core::paragraph::{Paragraph, ParagraphLayoutEngine, Run, TextStyle};
use crate::core::text_engine::{Glyph, GlyphRun, TextEngine, TextMetrics};
use crate::core::text_engine::{
    QualifiedTextClusterRange, ResolvedTextStyle, SourceInkError, SourceInkGeometry, TextBinding,
    TextInputError, TextRangeEffectError, TextSourceRangeEffect, bound_glyph_run, source_ink,
};
use crate::core::text_shaper::TextShaper;
use crate::platform::font_manager::FontFaceCatalog;
use crate::platform::native_font_backend::NativeFontBackend;
use crate::renderer::glyph_atlas::GlyphKey;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const SHAPE_CACHE_CAPACITY: usize = 4096;
const METRICS_CACHE_CAPACITY: usize = 64;

/// Exact identity for native font metrics reuse (SPEC-HAL-104 A.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MetricsKey {
    font_id: u64,
    font_size_bits: u32,
    scale_bits: u32,
}

impl MetricsKey {
    fn new(font_id: u64, font_size: f32, scale: f32) -> Self {
        Self {
            font_id,
            font_size_bits: font_size.to_bits(),
            scale_bits: scale.to_bits(),
        }
    }
}

/// Cache key for shaped text results.
#[derive(Clone)]
struct ShapeKey {
    text: String,
    font_size_bits: u32,
    binding: Option<TextBinding>,
}

impl PartialEq for ShapeKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.font_size_bits == other.font_size_bits
            && self.binding == other.binding
    }
}

impl Eq for ShapeKey {}

impl Hash for ShapeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font_size_bits.hash(state);
        self.binding.hash(state);
    }
}

impl ShapeKey {
    fn new(text: &str, font_size: f32) -> Self {
        Self {
            text: text.to_string(),
            font_size_bits: font_size.to_bits(),
            binding: None,
        }
    }
}

/// Cached shaping result with glyph keys for rasterization.
struct ShapeCacheEntry {
    run: GlyphRun,
    metrics: TextMetrics,
    /// Glyph keys for atlas lookup/rasterization
    glyph_keys: Vec<GlyphKey>,
}

/// Adapter that implements `TextEngine` by delegating to a `NativeFontBackend`.
///
/// Per SPEC-HAL-065, shaping is done via TextShaper (rustybuzz), and backends
/// only handle rasterization. This allows consistent Unicode shaping across platforms.
pub struct BackendTextEngine<B: NativeFontBackend> {
    catalog: Option<FontFaceCatalog>,
    native_faces: HashMap<u64, B>,
    backend: B,
    /// Text shaper using rustybuzz
    shaper: Option<Arc<TextShaper>>,
    /// Font ID for glyph keys (from the loaded font)
    font_id: u64,
    shape_cache: HashMap<ShapeKey, ShapeCacheEntry>,
    shape_cache_order: VecDeque<ShapeKey>,
    metrics_cache: HashMap<MetricsKey, TextMetrics>,
    metrics_cache_order: VecDeque<MetricsKey>,
    /// Current scale factor (logical → physical).
    /// Default 1.0, updated by renderer when known.
    scale: f32,
}

impl<B: NativeFontBackend> BackendTextEngine<B> {
    /// Create a new BackendTextEngine wrapping the given backend.
    ///
    /// Note: Call `set_shaper` to enable rustybuzz shaping. Without a shaper,
    /// shaping will return empty runs.
    pub fn new(backend: B) -> Self {
        Self {
            catalog: None,
            native_faces: HashMap::new(),
            backend,
            shaper: None,
            font_id: 0,
            shape_cache: HashMap::new(),
            shape_cache_order: VecDeque::new(),
            metrics_cache: HashMap::new(),
            metrics_cache_order: VecDeque::new(),
            scale: 1.0,
        }
    }

    /// Set the text shaper (rustybuzz) and font ID for glyph keys.
    ///
    /// The font_id should match the font used by the backend for rasterization.
    pub fn set_shaper(&mut self, shaper: Arc<TextShaper>, font_id: u64) {
        self.shaper = Some(shaper);
        self.font_id = font_id;
        // Clear cache since shaper changed
        self.shape_cache.clear();
        self.shape_cache_order.clear();
        self.metrics_cache.clear();
        self.metrics_cache_order.clear();
    }

    /// Check if a shaper is configured.
    pub fn has_shaper(&self) -> bool {
        self.shaper.is_some()
    }

    /// Bootstrap only: all native resources are initialized before control measurement.
    pub fn admit_face_catalog(&mut self, catalog: FontFaceCatalog) -> Result<(), TextInputError> {
        let mut resources = HashMap::new();
        for face in catalog.faces() {
            let backend = B::from_font_data(face.binding.data().clone(), face.binding.font_id())
                .ok_or(TextInputError::FontUnavailable)?;
            resources.insert(face.binding.font_id(), backend);
        }
        self.native_faces = resources;
        self.catalog = Some(catalog);
        Ok(())
    }

    pub fn shape_bound_with_keys(
        &mut self,
        text: &str,
        font_size: f32,
        binding: &TextBinding,
    ) -> Result<(GlyphRun, Vec<GlyphKey>), TextInputError> {
        let binding = binding.shape_binding();
        let binding = binding.as_ref();
        let face = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.get(binding.face().font_id()))
            .filter(|face| &face.binding == binding.face())
            .ok_or(TextInputError::FontUnavailable)?;
        let shaper = face.shaper.clone();
        let mut key = ShapeKey::new(text, font_size);
        key.binding = Some(binding.clone());
        if let Some(entry) = self.shape_cache.get(&key) {
            return Ok((entry.run.clone(), entry.glyph_keys.clone()));
        }
        let metrics = self.metrics_bound(font_size, binding)?;
        let style = ResolvedTextStyle {
            binding: binding.clone(),
            font_size,
        };
        let run = bound_glyph_run(text, &style, &shaper, metrics, self.scale)?;
        let keys: Vec<_> = run
            .glyphs
            .iter()
            .map(|glyph| GlyphKey::new(binding.face().font_id(), font_size, glyph.id, self.scale))
            .collect();
        self.insert_cache_entry(
            key,
            ShapeCacheEntry {
                run: run.clone(),
                metrics,
                glyph_keys: keys.clone(),
            },
        );
        Ok((run, keys))
    }

    /// HAL048: reproduce the complete run using its actual admitted face only.
    pub(crate) fn verified_glyph_source_ranges(
        &self,
        run: &GlyphRun,
    ) -> Option<Vec<std::ops::Range<usize>>> {
        let binding = run.binding.as_ref()?;
        let face = self.catalog.as_ref()?.get(binding.face().font_id())?;
        if &face.binding != binding.face() {
            return None;
        }
        crate::core::text_engine::glyph_source_ranges(run, Some(&face.shaper), self.scale)
    }

    pub(crate) fn metrics_bound(
        &mut self,
        font_size: f32,
        binding: &TextBinding,
    ) -> Result<TextMetrics, TextInputError> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(TextInputError::InvalidFontSize);
        }
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(TextInputError::InvalidScale);
        }
        let id = binding.face().font_id();
        let face = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.get(id))
            .filter(|face| &face.binding == binding.face())
            .ok_or(TextInputError::FontUnavailable)?;
        let key = MetricsKey::new(id, font_size, self.scale);
        if let Some(metrics) = self.metrics_cache.get(&key) {
            return Ok(*metrics);
        }
        let _ = face;
        let metrics = self
            .native_faces
            .get_mut(&id)
            .ok_or(TextInputError::FontUnavailable)?
            .font_metrics(font_size, self.scale);
        if ![
            metrics.ascent,
            metrics.descent,
            metrics.line_gap,
            metrics.line_height,
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            return Err(TextInputError::PositionOverflow);
        }
        while self.metrics_cache.len() >= METRICS_CACHE_CAPACITY {
            if let Some(oldest) = self.metrics_cache_order.pop_front() {
                self.metrics_cache.remove(&oldest);
            } else {
                break;
            }
        }
        self.metrics_cache_order.push_back(key);
        self.metrics_cache.insert(key, metrics);
        Ok(metrics)
    }

    pub(crate) fn rasterize_bound_glyph(
        &mut self,
        key: &GlyphKey,
        scale: f32,
    ) -> Option<crate::platform::native_font_backend::GlyphBitmap> {
        if let Some(backend) = self.native_faces.get_mut(&key.font_id) {
            backend.rasterize_glyph(key, scale)
        } else if key.font_id == self.font_id {
            self.backend.rasterize_glyph(key, scale)
        } else {
            None
        }
    }

    /// Set the scale factor for shaping.
    ///
    /// Called by the renderer when scale factor changes.
    pub fn set_scale(&mut self, scale: f32) {
        if self.scale.to_bits() != scale.to_bits() {
            // Scale changed - invalidate cache
            self.shape_cache.clear();
            self.shape_cache_order.clear();
            self.metrics_cache.clear();
            self.metrics_cache_order.clear();
            self.scale = scale;
        }
    }

    /// Get the current scale factor.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Get the font_id used for glyph keys.
    pub fn font_id(&self) -> u64 {
        self.font_id
    }

    /// Get mutable access to the underlying backend.
    ///
    /// Needed for renderer to call `rasterize_glyph`.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Get immutable access to the underlying backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub(crate) fn metrics_for_scale(&mut self, font_size: f32, scale: f32) -> TextMetrics {
        let key = MetricsKey::new(self.font_id, font_size, scale);
        if let Some(metrics) = self.metrics_cache.get(&key) {
            return *metrics;
        }

        let metrics = self.backend.font_metrics(font_size, scale);
        while self.metrics_cache.len() >= METRICS_CACHE_CAPACITY {
            let Some(oldest) = self.metrics_cache_order.pop_front() else {
                self.metrics_cache.clear();
                break;
            };
            self.metrics_cache.remove(&oldest);
        }
        self.metrics_cache_order.push_back(key);
        self.metrics_cache.insert(key, metrics);
        metrics
    }

    fn metrics_for(&mut self, font_size: f32) -> TextMetrics {
        self.metrics_for_scale(font_size, self.scale)
    }

    /// Internal shaping using TextShaper (rustybuzz).
    fn shape_internal(
        &mut self,
        text: &str,
        font_size: f32,
    ) -> (GlyphRun, TextMetrics, Vec<GlyphKey>) {
        let key = ShapeKey::new(text, font_size);

        // Check cache
        if let Some(entry) = self.shape_cache.get(&key) {
            return (entry.run.clone(), entry.metrics, entry.glyph_keys.clone());
        }

        // Get metrics from backend
        let metrics = self.metrics_for(font_size);

        // Shape via TextShaper if available
        let Some(shaper) = &self.shaper else {
            // No shaper configured - return empty run
            let run = GlyphRun {
                binding: None,
                clusters: Vec::new(),
                glyphs: Vec::new(),
                width: 0.0,
                height: metrics.line_height,
                text: text.to_string(),
                font_size,
                ascent: metrics.ascent,
                descent: metrics.descent,
            };
            return (run, metrics, Vec::new());
        };

        // Shape text using rustybuzz
        let shaped = shaper
            .shape_with_gap(text, font_size, self.scale, Default::default())
            .expect("default text shaping requires finite input");
        let shaped_clusters = shaped.glyphs;

        // SPEC-HAL-104: Convert ShapedCluster to Glyph using INTEGER accumulation
        // Use f32 accumulation to preserve subpixel precision
        // Integer truncation caused horizontal quantization artifacts (staircase pattern)
        let mut glyphs = Vec::with_capacity(shaped_clusters.len());
        let mut glyph_keys = Vec::with_capacity(shaped_clusters.len());

        // Use f32 accumulation for subpixel precision
        let mut pen_x_px: f32 = 0.0;

        for cluster in &shaped_clusters {
            // Use f32 values - no integer truncation
            let glyph_x_px = pen_x_px + cluster.x_offset_px;

            glyphs.push(Glyph {
                id: cluster.glyph_id,
                // Convert back to logical for Glyph struct (required by current API)
                x: glyph_x_px / self.scale,
                y: cluster.y_offset_px / self.scale,
                advance: cluster.advance_px / self.scale,
            });

            glyph_keys.push(GlyphKey::new(
                self.font_id,
                font_size,
                cluster.glyph_id,
                self.scale,
            ));

            // f32 accumulation - preserves subpixel precision
            pen_x_px += cluster.advance_px;
        }

        // Compute dimensions from f32 accumulator
        let width = pen_x_px / self.scale;
        let height = metrics.line_height;

        let run = GlyphRun {
            binding: self
                .catalog
                .as_ref()
                .and_then(|catalog| catalog.get(self.font_id))
                .map(|face| TextBinding::new(face.binding.clone(), Default::default())),
            clusters: shaped.clusters,
            glyphs,
            width,
            height,
            text: text.to_string(),
            font_size,
            ascent: metrics.ascent,
            descent: metrics.descent,
        };

        let entry = ShapeCacheEntry {
            run: run.clone(),
            metrics,
            glyph_keys: glyph_keys.clone(),
        };
        self.insert_cache_entry(key, entry);

        (run, metrics, glyph_keys)
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

    /// Shape text and return glyph keys for rasterization.
    ///
    /// Used by renderer to get positioned glyph keys for atlas lookup.
    pub fn shape_with_keys(&mut self, text: &str, font_size: f32) -> (GlyphRun, Vec<GlyphKey>) {
        let (run, _, keys) = self.shape_internal(text, font_size);
        (run, keys)
    }
}

impl<B: NativeFontBackend> TextEngine for BackendTextEngine<B> {
    fn current_shaping_scale(&self) -> Result<f32, TextInputError> {
        let scale = self.scale;
        if !scale.is_finite() || scale <= 0.0 {
            return Err(TextInputError::InvalidScale);
        }
        Ok(scale)
    }

    fn font_face_catalog(&self) -> Option<FontFaceCatalog> {
        self.catalog.clone()
    }

    fn shape_laid_out_paragraph_run(
        &mut self,
        paragraph: &Paragraph,
        run: &Run,
    ) -> Result<GlyphRun, TextInputError> {
        let scale = self.scale;
        ParagraphLayoutEngine::reconstruct_bound_run(paragraph, run, self, scale)
    }

    fn qualify_source_range(
        &mut self,
        run: &GlyphRun,
        utf8: std::ops::Range<usize>,
    ) -> Result<QualifiedTextClusterRange, TextRangeEffectError> {
        let scale = self.scale;
        source_ink::qualify_source_range(self, run, utf8, scale)
    }

    fn draw_with_source_range_effects(
        &mut self,
        run: &GlyphRun,
        origin: Point,
        color: Color,
        z_index: ZIndex,
        effects: &[TextSourceRangeEffect],
        sink: &mut dyn PaintSink,
    ) -> Result<(), TextRangeEffectError> {
        let scale = self.scale;
        source_ink::draw_source_range_effects(self, run, origin, color, z_index, effects, sink, scale)
    }

    fn source_ink_geometry<'run>(
        &mut self,
        run: &'run GlyphRun,
        range: std::ops::Range<usize>,
    ) -> Result<SourceInkGeometry<'run>, SourceInkError> {
        let scale = self.scale;
        SourceInkGeometry::qualify(self, run, range, scale)
    }

    fn shape_bound_text(
        &mut self,
        text: &str,
        font_size: f32,
        binding: &TextBinding,
    ) -> Result<GlyphRun, TextInputError> {
        self.shape_bound_with_keys(text, font_size, binding)
            .map(|v| v.0)
    }

    fn metrics_bound_text(
        &mut self,
        font_size: f32,
        binding: &TextBinding,
    ) -> Result<TextMetrics, TextInputError> {
        self.metrics_bound(font_size, binding)
    }
    fn resolve_text_style(&self, style: &TextStyle) -> Result<ResolvedTextStyle, TextInputError> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or(TextInputError::FontUnavailable)?;
        catalog.resolve(self.font_id, style)
    }

    fn shape_styled(
        &mut self,
        text: &str,
        style: Option<&TextStyle>,
        default_size: f32,
    ) -> Result<GlyphRun, TextInputError> {
        match style {
            None => Ok(self.shape(text, default_size)),
            Some(style) => {
                let resolved = self.resolve_text_style(style)?;
                self.shape_bound_with_keys(text, resolved.font_size, &resolved.binding)
                    .map(|(run, _)| run)
            }
        }
    }

    fn metrics_styled(
        &mut self,
        style: Option<&TextStyle>,
        default_size: f32,
    ) -> Result<TextMetrics, TextInputError> {
        match style {
            None => Ok(TextEngine::font_metrics(self, default_size)),
            Some(style) => {
                let resolved = self.resolve_text_style(style)?;
                self.metrics_bound(resolved.font_size, &resolved.binding)
            }
        }
    }
    fn presentation_text_generation(
        &self,
    ) -> Option<crate::core::text_engine::TextGenerationLease> {
        self.shaper.as_ref().map(|shaper| {
            crate::core::text_engine::TextGenerationLease::new(self.font_id, shaper.font_data())
        })
    }

    fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        // HAL_PROFILE_RESIZE: Count text measure calls
        crate::core::app::profiler_inc_text_measure_call();
        let (run, _, _) = self.shape_internal(text, font_size);
        (run.width, run.height)
    }

    fn metrics(&mut self, _text: &str, font_size: f32) -> TextMetrics {
        self.metrics_for(font_size)
    }

    fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun {
        let (run, _, _) = self.shape_internal(text, font_size);
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

    fn shaper(&self) -> Option<&TextShaper> {
        self.shaper.as_ref().map(|s| s.as_ref())
    }
}

impl<B: NativeFontBackend> NativeFontBackend for BackendTextEngine<B> {
    fn font_metrics(&mut self, font_size: f32, scale: f32) -> TextMetrics {
        self.metrics_for_scale(font_size, scale)
    }

    fn rasterize_glyph(
        &mut self,
        key: &GlyphKey,
        scale: f32,
    ) -> Option<crate::platform::native_font_backend::GlyphBitmap> {
        self.rasterize_bound_glyph(key, scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scene::{SceneBuilder, SceneNode};
    use crate::platform::native_font_backend::GlyphBitmap;

    struct StubBackend;

    impl NativeFontBackend for StubBackend {
        fn font_metrics(&mut self, font_size: f32, _scale: f32) -> TextMetrics {
            TextMetrics {
                ascent: font_size,
                descent: 0.0,
                line_gap: 0.0,
                line_height: font_size,
            }
        }

        fn rasterize_glyph(&mut self, _key: &GlyphKey, _scale: f32) -> Option<GlyphBitmap> {
            None
        }
    }

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
        let mut engine = BackendTextEngine::new(StubBackend);
        let run = GlyphRun {
            binding: None,
            clusters: Vec::new(),
            glyphs: Vec::new(),
            width: 1.0,
            height: 1.0,
            text: "z".to_string(),
            font_size: 12.0,
            ascent: 1.0,
            descent: 0.0,
        };

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
