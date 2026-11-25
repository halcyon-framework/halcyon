//! SPEC-HAL-103 Stage 2: Single-font text engine with generic font identity.
//!
//! This is a lightweight, Send-safe text engine for the UI thread that:
//! - Uses the legacy embedded Inter font (until OS font discovery is implemented)
//! - Computes font_id using generic `compute_font_id()` with `FontIdentity`
//! - Ensures UI thread shaping and platform thread rasterization use identical font identity
//!
//! IMPORTANT: This engine does NOT hold or interact with FontManager at runtime.
//! It only ensures font_id compatibility so that GlyphKeys produced here will
//! correctly match the font registered in FontManager on the platform thread.
//!
//! For actual multi-font support with FontResolver-based fallback, use
//! `ParagraphLayoutEngine::layout_with_fonts()` which takes a `&FontManager`.

use crate::core::math::Point;
use crate::core::paint::{Color, PaintSink, TextBackgroundSpan, ZIndex};
use crate::core::paragraph::{Paragraph, ParagraphLayoutEngine, Run, TextStyle};
use crate::core::text_engine::{
    FontFaceBinding, GlyphRun, QualifiedTextClusterRange, ResolvedTextStyle, TextBinding,
    TextEngine, TextInputError, TextMetrics, TextRangeEffectError, TextSourceRangeEffect,
    bound_glyph_run, source_ink,
};
use crate::core::text_shaper::TextShaper;
use crate::platform::font_manager::{
    FontAsset, FontFaceCatalog, FontLoadError, FontManager, FontWeight,
};
use crate::platform::system_font_provider;
use crate::renderer::glyph_atlas::GlyphKey;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const SHAPE_CACHE_CAPACITY: usize = 4096;

// ============================================================================
// SPEC-HAL-103 Stage 2: FontIdentity - Generic font identification
// ============================================================================

/// Font identity descriptor for generic font ID computation.
///
/// SPEC-HAL-103 Stage 2: This struct captures all identity components needed
/// to compute a stable, unique font_id. No font-specific assumptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontIdentity {
    /// Human-readable family name (e.g., "Inter", "Segoe UI", ".AppleSystemUIFont")
    pub family_name: String,
    /// PostScript name if available (preferred for precise identification)
    pub postscript_name: Option<String>,
    /// Font weight
    pub weight: FontWeight,
    /// Whether the font is italic
    pub italic: bool,
}

impl FontIdentity {
    /// Create a new FontIdentity.
    pub fn new(
        family_name: impl Into<String>,
        postscript_name: Option<String>,
        weight: FontWeight,
        italic: bool,
    ) -> Self {
        Self {
            family_name: family_name.into(),
            postscript_name,
            weight,
            italic,
        }
    }

    /// Create FontIdentity for legacy embedded Inter font.
    pub fn legacy_inter() -> Self {
        Self {
            family_name: "Inter".to_string(),
            postscript_name: Some("Inter-Regular".to_string()),
            weight: FontWeight::Regular,
            italic: false,
        }
    }

    /// Extract FontIdentity from font data using ttf-parser.
    ///
    /// Falls back to provided defaults if parsing fails.
    pub fn from_font_data(data: &[u8], fallback_family: &str) -> Self {
        let (family_name, postscript_name, weight, italic) =
            if let Ok(face) = ttf_parser::Face::parse(data, 0) {
                let family = face
                    .names()
                    .into_iter()
                    .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
                    .find_map(|n| n.to_string())
                    .unwrap_or_else(|| fallback_family.to_string());

                let ps_name = face
                    .names()
                    .into_iter()
                    .filter(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
                    .find_map(|n| n.to_string());

                let weight = face
                    .tables()
                    .os2
                    .map(|os2| FontWeight::from_value(os2.weight().to_number()))
                    .unwrap_or(FontWeight::Regular);

                let italic = face.is_italic();

                (family, ps_name, weight, italic)
            } else {
                (
                    fallback_family.to_string(),
                    None,
                    FontWeight::Regular,
                    false,
                )
            };

        Self {
            family_name,
            postscript_name,
            weight,
            italic,
        }
    }
}

/// Compute a stable font_id from identity and font data.
///
/// SPEC-HAL-103 Stage 2: Generic font ID computation. No font-specific assumptions.
/// Uses the same algorithm as `FontManager::compute_font_id`.
///
/// Hash inputs (in order):
/// 1. family_name
/// 2. postscript_name (or empty string if None)
/// 3. weight (as u16)
/// 4. italic (bool)
/// 5. SHA-256 hash of font bytes
pub fn compute_font_id(identity: &FontIdentity, data: &[u8]) -> u64 {
    use sha2::{Digest, Sha256};
    use std::collections::hash_map::DefaultHasher;

    // Compute SHA-256 of full font data
    let mut data_hasher = Sha256::new();
    data_hasher.update(data);
    let data_hash = data_hasher.finalize();

    // Combine all identity components into final hash
    // MUST match FontManager::compute_font_id exactly
    let mut hasher = DefaultHasher::new();
    identity.family_name.hash(&mut hasher);
    // PostScript name: use actual value or empty string for None
    identity
        .postscript_name
        .as_deref()
        .unwrap_or("")
        .hash(&mut hasher);
    identity.weight.to_value().hash(&mut hasher);
    identity.italic.hash(&mut hasher);
    data_hash.as_slice().hash(&mut hasher);
    hasher.finish()
}

/// Cache key for shaped text results.
#[derive(Clone)]
struct ShapeKey {
    binding: TextBinding,
    text: String,
    font_size_bits: u32,
}

impl PartialEq for ShapeKey {
    fn eq(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.text == other.text
            && self.font_size_bits == other.font_size_bits
    }
}

impl Eq for ShapeKey {}

impl Hash for ShapeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.binding.hash(state);
        self.text.hash(state);
        self.font_size_bits.hash(state);
    }
}

impl ShapeKey {
    fn new(text: &str, font_size: f32, binding: TextBinding) -> Self {
        Self {
            binding,
            text: text.to_string(),
            font_size_bits: font_size.to_bits(),
        }
    }
}

/// Cached shaping result.
struct ShapeCacheEntry {
    run: GlyphRun,
    metrics: TextMetrics,
    glyph_keys: Vec<GlyphKey>,
}

/// Text engine backed by an immutable FontManager face catalog.
///
/// This is a Send-safe TextEngine implementation for the UI thread.
/// It shapes text using rustybuzz and produces GlyphKeys with a font_id
/// that matches what FontManager would assign for the same font bytes.
///
/// Key properties:
/// - Legacy constructors preserve their bootstrap face; catalogs admit explicit faces
/// - font_id computed via generic `compute_font_id()` with `FontIdentity`
/// - Does NOT hold FontManager reference (lightweight, Send-safe)
/// - Suitable for simple UI text (labels, buttons, etc.)
///
/// For multi-font paragraphs with fallback, use `ParagraphLayoutEngine::layout_with_fonts()`.
pub struct FontManagerTextEngine {
    catalog: crate::platform::font_manager::FontFaceCatalog,
    /// The text shaper (rustybuzz-based)
    shaper: Arc<TextShaper>,
    /// Font data for metrics
    font_data: Arc<Vec<u8>>,
    /// Units per em from the font
    units_per_em: u16,
    /// Font ID from FontManager (MUST match renderer's font_id)
    font_id: u64,
    /// Font identity (SPEC-HAL-103 Stage 2)
    identity: FontIdentity,
    /// Shape cache
    shape_cache: HashMap<ShapeKey, ShapeCacheEntry>,
    shape_cache_order: VecDeque<ShapeKey>,
    /// Current scale factor
    scale: f32,
}

impl FontManagerTextEngine {
    fn catalog_for(data: Arc<Vec<u8>>, identity: &FontIdentity) -> FontFaceCatalog {
        let id = compute_font_id(identity, &data);
        let shaper =
            Arc::new(TextShaper::from_data(data.clone()).expect("bootstrap face must shape"));
        FontFaceCatalog::single(
            FontFaceBinding::new(
                id,
                data,
                identity.family_name.clone(),
                identity.weight,
                identity.italic,
            ),
            shaper,
            identity.postscript_name.clone().unwrap_or_default(),
        )
    }

    /// Construct the existing Send adapter from manager-admitted immutable data.
    pub fn with_face_catalog(
        catalog: FontFaceCatalog,
        default_id: u64,
    ) -> Result<Self, TextInputError> {
        let face = catalog
            .get(default_id)
            .ok_or(TextInputError::FontUnavailable)?;
        let data = face.binding.data().clone();
        let units_per_em = ttf_parser::Face::parse(&data, 0)
            .map_err(|_| TextInputError::FontUnavailable)?
            .units_per_em();
        let shaper = face.shaper.clone();
        let identity = FontIdentity::new(
            face.binding.family_name(),
            Some(face.resolved.postscript_name.clone()),
            face.binding.weight(),
            face.binding.italic(),
        );
        Ok(Self {
            catalog,
            shaper,
            font_data: data,
            units_per_em,
            font_id: default_id,
            identity,
            shape_cache: HashMap::new(),
            shape_cache_order: VecDeque::new(),
            scale: 1.0,
        })
    }

    pub fn face_catalog(&self) -> FontFaceCatalog {
        self.catalog.clone()
    }
    /// Create a new FontManagerTextEngine with the legacy embedded Inter font.
    ///
    /// The font_id is computed using the same algorithm as FontManager
    /// to ensure consistency between UI thread shaping and platform thread rasterization.
    ///
    /// SPEC-HAL-103: Currently uses legacy Inter. Future stages will try
    /// `load_default_system_ui_font()` first, falling back to Inter.
    pub fn new() -> Self {
        // SPEC-HAL-103 Stage 1: Use legacy Inter explicitly
        let descriptor = system_font_provider::load_legacy_embedded_inter();
        let font_data = descriptor.data.clone();

        // Create shaper (from_data infers units_per_em from font)
        let shaper = Arc::new(
            TextShaper::from_data(font_data.clone())
                .expect("Failed to create shaper for legacy Inter font"),
        );

        // Get units_per_em from ttf-parser
        let units_per_em = ttf_parser::Face::parse(&font_data, 0)
            .map(|f| f.units_per_em())
            .unwrap_or(1000);

        // SPEC-HAL-103 Stage 2: Construct identity explicitly, no Inter assumptions
        let identity = FontIdentity::new(
            descriptor.family_name,
            Self::extract_postscript_name(&font_data),
            descriptor.weight,
            descriptor.italic,
        );

        // Compute font_id using generic compute_font_id (SPEC-HAL-103 Stage 2)
        let font_id = compute_font_id(&identity, &font_data);

        let mut manager = FontManager::new();
        let registered = manager
            .register_font_asset(FontAsset {
                family_name: identity.family_name.clone(),
                weight: identity.weight,
                italic: identity.italic,
                data: font_data.clone(),
            })
            .expect("embedded Regular face must be admitted");
        assert_eq!(
            registered, font_id,
            "embedded Regular identity remains unchanged"
        );
        for descriptor in system_font_provider::load_legacy_embedded_inter_styles() {
            manager
                .load_font_memory(descriptor.data, Some(descriptor.family_name))
                .expect("embedded style face must be admitted");
        }

        Self {
            catalog: manager.face_catalog(),
            shaper,
            font_data,
            units_per_em,
            font_id,
            identity,
            shape_cache: HashMap::new(),
            shape_cache_order: VecDeque::new(),
            scale: 1.0,
        }
    }

    /// Create a new FontManagerTextEngine, trying OS SystemUI font first.
    ///
    /// SPEC-HAL-103 Stage 3: This is the future-proof entry point.
    /// - First attempts to load the OS system UI font via `load_default_system_ui_font()`
    /// - Falls back to legacy embedded Inter if OS font unavailable
    ///
    /// The renderer should use this constructor to get the appropriate font.
    pub fn new_with_system_ui() -> Self {
        // 1. Try OS SystemUI
        if let Some(descriptor) = system_font_provider::load_default_system_ui_font() {
            let font_data = descriptor.data.clone();
            let shaper = Arc::new(
                TextShaper::from_data(font_data.clone())
                    .expect("Failed to create shaper for SystemUI font"),
            );

            let units_per_em = ttf_parser::Face::parse(&font_data, 0)
                .ok()
                .map(|f| f.units_per_em())
                .unwrap_or(1000);

            let identity = FontIdentity::new(
                descriptor.family_name,
                Self::extract_postscript_name(&font_data),
                descriptor.weight,
                descriptor.italic,
            );

            let font_id = compute_font_id(&identity, &font_data);

            return Self {
                catalog: Self::catalog_for(font_data.clone(), &identity),
                shaper,
                font_data,
                units_per_em,
                font_id,
                identity,
                shape_cache: HashMap::new(),
                shape_cache_order: VecDeque::new(),
                scale: 1.0,
            };
        }

        // 2. Fallback — legacy Inter
        Self::new()
    }

    /// Preserve every admitted bootstrap face when constructing the renderer's
    /// existing manager. Copying only the primary face silently loses styles.
    pub(crate) fn bootstrap_font_manager(&self) -> Result<FontManager, FontLoadError> {
        let mut manager = FontManager::new();
        for face in self.catalog.faces() {
            let id = manager.register_font_asset(FontAsset {
                family_name: face.binding.family_name().to_string(),
                weight: face.binding.weight(),
                italic: face.binding.italic(),
                data: face.binding.data().clone(),
            })?;
            assert_eq!(
                id,
                face.binding.font_id(),
                "bootstrap face identity remains unchanged"
            );
        }
        Ok(manager)
    }

    /// Extract PostScript name from font data.
    pub fn extract_postscript_name(data: &[u8]) -> Option<String> {
        ttf_parser::Face::parse(data, 0).ok().and_then(|face| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
                .find_map(|n| n.to_string())
        })
    }

    /// Create with explicit font_id from FontManager.
    ///
    /// Use this when you have already registered the font with FontManager
    /// and have the authoritative font_id.
    pub fn with_font_id(font_data: Arc<Vec<u8>>, font_id: u64) -> Option<Self> {
        let shaper = Arc::new(TextShaper::from_data(font_data.clone())?);
        let units_per_em = ttf_parser::Face::parse(&font_data, 0)
            .map(|f| f.units_per_em())
            .unwrap_or(1000);

        // SPEC-HAL-103 Stage 2: Extract identity from font data
        let identity = FontIdentity::from_font_data(&font_data, "Unknown");

        Some(Self {
            catalog: FontFaceCatalog::single(
                FontFaceBinding::new(
                    font_id,
                    font_data.clone(),
                    identity.family_name.clone(),
                    identity.weight,
                    identity.italic,
                ),
                shaper.clone(),
                identity.postscript_name.clone().unwrap_or_default(),
            ),
            shaper,
            font_data,
            units_per_em,
            font_id,
            identity,
            shape_cache: HashMap::new(),
            shape_cache_order: VecDeque::new(),
            scale: 1.0,
        })
    }

    /// Compute font_id for embedded Inter font.
    ///
    /// SPEC-HAL-103 Stage 2: This is now a thin wrapper around `compute_font_id()`.
    /// Kept for backward compatibility with existing call sites.
    ///
    /// This ensures the UI thread and platform thread use identical font_ids.
    #[deprecated(
        since = "0.1.3",
        note = "Use compute_font_id() with FontIdentity instead"
    )]
    pub fn compute_font_id_for_inter(data: &[u8]) -> u64 {
        // SPEC-HAL-103 Stage 2: Construct identity from data, use generic compute_font_id
        let identity = FontIdentity::new(
            "Inter",
            Self::extract_postscript_name(data),
            FontWeight::Regular,
            false,
        );
        compute_font_id(&identity, data)
    }

    /// Get the legacy embedded Inter font data.
    ///
    /// SPEC-HAL-103 Stage 1: Explicitly named to indicate this is legacy Inter,
    /// not the OS system UI font.
    pub fn legacy_inter_data() -> Arc<Vec<u8>> {
        system_font_provider::load_legacy_embedded_inter().data
    }

    /// Backwards compatibility alias for `legacy_inter_data()`.
    #[deprecated(since = "0.1.3", note = "Use legacy_inter_data() instead")]
    pub fn embedded_inter_data() -> Arc<Vec<u8>> {
        Self::legacy_inter_data()
    }

    /// Get the font ID.
    pub fn font_id(&self) -> u64 {
        self.font_id
    }

    /// Get the font identity (SPEC-HAL-103 Stage 2).
    pub fn identity(&self) -> &FontIdentity {
        &self.identity
    }

    /// Get the font data.
    pub fn font_data(&self) -> &Arc<Vec<u8>> {
        &self.font_data
    }

    /// Set the scale factor.
    pub fn set_scale(&mut self, scale: f32) {
        if self.scale.to_bits() != scale.to_bits() {
            self.shape_cache.clear();
            self.shape_cache_order.clear();
            self.scale = scale;
        }
    }

    /// Get current scale factor.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Internal shaping function.
    fn shape_internal(
        &mut self,
        text: &str,
        font_size: f32,
    ) -> (GlyphRun, TextMetrics, Vec<GlyphKey>) {
        let style = ResolvedTextStyle {
            binding: TextBinding::new(
                self.catalog
                    .get(self.font_id)
                    .expect("bootstrap catalog face")
                    .binding
                    .clone(),
                Default::default(),
            ),
            font_size,
        };
        self.shape_bound(text, &style)
            .expect("default text input must be finite")
    }

    fn metrics_bound(&self, style: &ResolvedTextStyle) -> Result<TextMetrics, TextInputError> {
        let face = self
            .catalog
            .get(style.binding.face().font_id())
            .ok_or(TextInputError::FontUnavailable)?;
        if face.binding != *style.binding.face() {
            return Err(TextInputError::FontUnavailable);
        }
        let parsed = ttf_parser::Face::parse(face.binding.data(), 0)
            .map_err(|_| TextInputError::FontUnavailable)?;
        let unit = style.font_size / parsed.units_per_em() as f32;
        let ascent = parsed.ascender() as f32 * unit;
        let descent = -(parsed.descender() as f32) * unit;
        let line_gap = parsed.line_gap() as f32 * unit;
        Ok(TextMetrics {
            ascent,
            descent,
            line_gap,
            line_height: ascent + descent + line_gap,
        })
    }

    fn shape_bound(
        &mut self,
        text: &str,
        style: &ResolvedTextStyle,
    ) -> Result<(GlyphRun, TextMetrics, Vec<GlyphKey>), TextInputError> {
        let key = ShapeKey::new(text, style.font_size, style.binding.clone());
        if let Some(entry) = self.shape_cache.get(&key) {
            return Ok((entry.run.clone(), entry.metrics, entry.glyph_keys.clone()));
        }
        let metrics = self.metrics_bound(style)?;
        let face = self
            .catalog
            .get(style.binding.face().font_id())
            .ok_or(TextInputError::FontUnavailable)?;
        let run = bound_glyph_run(text, style, &face.shaper, metrics, self.scale)?;
        let keys = run
            .glyphs
            .iter()
            .map(|glyph| {
                GlyphKey::new(
                    face.binding.font_id(),
                    style.font_size,
                    glyph.id,
                    self.scale,
                )
            })
            .collect::<Vec<_>>();
        self.insert_cache_entry(
            key,
            ShapeCacheEntry {
                run: run.clone(),
                metrics,
                glyph_keys: keys.clone(),
            },
        );
        Ok((run, metrics, keys))
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

    /// Get font metrics for a given size.
    fn get_metrics(&self, font_size: f32) -> TextMetrics {
        let scale = font_size / self.units_per_em as f32;

        // Get metrics from ttf-parser
        if let Ok(parsed) = ttf_parser::Face::parse(&self.font_data, 0) {
            let ascent = parsed.ascender() as f32 * scale;
            let descent = (-parsed.descender() as f32) * scale;
            let line_gap = parsed.line_gap() as f32 * scale;
            let line_height = ascent + descent + line_gap;

            return TextMetrics {
                ascent,
                descent,
                line_gap,
                line_height,
            };
        }

        // Fallback synthetic metrics
        let ascent = font_size * 0.8;
        let descent = font_size * 0.2;
        let line_gap = font_size * 0.1;
        TextMetrics {
            ascent,
            descent,
            line_gap,
            line_height: ascent + descent + line_gap,
        }
    }

    /// Shape text and return glyph keys for rasterization.
    pub fn shape_with_keys(&mut self, text: &str, font_size: f32) -> (GlyphRun, Vec<GlyphKey>) {
        let (run, _, keys) = self.shape_internal(text, font_size);
        (run, keys)
    }
}

impl Default for FontManagerTextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine for FontManagerTextEngine {
    fn current_shaping_scale(&self) -> Result<f32, TextInputError> {
        let scale = self.scale;
        if !scale.is_finite() || scale <= 0.0 {
            return Err(TextInputError::InvalidScale);
        }
        Ok(scale)
    }

    fn font_face_catalog(&self) -> Option<FontFaceCatalog> {
        Some(self.catalog.clone())
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
    ) -> Result<crate::core::text_engine::SourceInkGeometry<'run>, crate::core::text_engine::SourceInkError> {
        let scale = self.scale;
        crate::core::text_engine::SourceInkGeometry::qualify(self, run, range, scale)
    }

    fn shape_bound_text(
        &mut self,
        text: &str,
        font_size: f32,
        binding: &TextBinding,
    ) -> Result<GlyphRun, TextInputError> {
        self.shape_bound(
            text,
            &ResolvedTextStyle {
                binding: binding.shape_binding().into_owned(),
                font_size,
            },
        )
        .map(|v| v.0)
    }

    fn metrics_bound_text(
        &mut self,
        font_size: f32,
        binding: &TextBinding,
    ) -> Result<TextMetrics, TextInputError> {
        self.metrics_bound(&ResolvedTextStyle {
            binding: binding.clone(),
            font_size,
        })
    }
    fn resolve_text_style(&self, style: &TextStyle) -> Result<ResolvedTextStyle, TextInputError> {
        self.catalog.resolve(self.font_id, style)
    }

    fn shape_styled(
        &mut self,
        text: &str,
        style: Option<&TextStyle>,
        default_size: f32,
    ) -> Result<GlyphRun, TextInputError> {
        match style {
            None => Ok(self.shape_internal(text, default_size).0),
            Some(style) => {
                let resolved = self.resolve_text_style(style)?;
                Ok(self.shape_bound(text, &resolved)?.0)
            }
        }
    }

    fn metrics_styled(
        &mut self,
        style: Option<&TextStyle>,
        default_size: f32,
    ) -> Result<TextMetrics, TextInputError> {
        match style {
            None => Ok(self.get_metrics(default_size)),
            Some(style) => self.metrics_bound(&self.resolve_text_style(style)?),
        }
    }
    fn presentation_text_generation(
        &self,
    ) -> Option<crate::core::text_engine::TextGenerationLease> {
        Some(crate::core::text_engine::TextGenerationLease::new(
            self.font_id,
            self.font_data.clone(),
        ))
    }

    fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        // HAL_PROFILE_RESIZE: Count text measure calls
        crate::core::app::profiler_inc_text_measure_call();
        let (run, _, _) = self.shape_internal(text, font_size);
        (run.width, run.height)
    }

    fn metrics(&mut self, _text: &str, font_size: f32) -> TextMetrics {
        self.get_metrics(font_size)
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
        Some(&self.shaper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scene::{SceneBuilder, SceneNode};
    use crate::platform::font_manager::FontManager;

    #[cfg(any(target_arch = "wasm32", target_os = "macos"))]
    #[test]
    fn wasm_font_bootstrap_resolves_real_weight_and_slant() {
        use crate::platform::backend_text_engine::BackendTextEngine;
        use crate::platform::native_font_backend::NativeFontBackend;
        #[cfg(target_arch = "wasm32")]
        use crate::platform::native_font_backend_glyphon::GlyphonBackend as BootstrapRasterizer;
        #[cfg(target_os = "macos")]
        use crate::platform::native_font_backend_coretext::CoreTextBackend as BootstrapRasterizer;

        // WASM has no OS font discovery. Native execution qualifies that same
        // ordinary embedded fallback, without substituting fonts or a renderer.
        #[cfg(target_arch = "wasm32")]
        let mut bootstrap = FontManagerTextEngine::new_with_system_ui();
        #[cfg(not(target_arch = "wasm32"))]
        let mut bootstrap = FontManagerTextEngine::new();
        let legacy = system_font_provider::load_legacy_embedded_inter();
        assert_eq!(bootstrap.font_data().as_slice(), legacy.data.as_slice());
        assert_eq!(
            bootstrap.font_id(),
            compute_font_id(bootstrap.identity(), &legacy.data)
        );

        // This is the exact manager handoff used by WgpuRenderer bootstrap.
        let manager = bootstrap.bootstrap_font_manager().unwrap();
        let catalog = manager.face_catalog();
        assert_eq!(catalog.faces().count(), 4);
        let backend =
            BootstrapRasterizer::from_font_data(bootstrap.font_data().clone(), bootstrap.font_id())
                .unwrap();
        let mut engine = BackendTextEngine::new(backend);
        engine.set_shaper(
            Arc::new(TextShaper::from_data(legacy.data.clone()).unwrap()),
            bootstrap.font_id(),
        );
        engine.admit_face_catalog(catalog).unwrap();

        let caption = "AV Получить доступ";
        let mut original =
            FontManagerTextEngine::with_font_id(legacy.data, bootstrap.font_id()).unwrap();
        let original_regular = original.shape(caption, 24.0);
        let current_regular = bootstrap.shape(caption, 24.0);
        assert_eq!(original_regular.width, current_regular.width);
        assert_eq!(original_regular.height, current_regular.height);
        let mut ids = std::collections::HashSet::new();
        let mut bitmaps = Vec::new();
        for (weight, italic) in [
            (FontWeight::Regular, false),
            (FontWeight::Bold, false),
            (FontWeight::Regular, true),
            (FontWeight::Bold, true),
        ] {
            let style = TextStyle {
                font_size: 24.0,
                weight,
                italic,
                ..TextStyle::default()
            };
            let resolved = engine.resolve_text_style(&style).unwrap();
            let face = resolved.binding.face();
            assert_eq!(face.weight(), weight);
            assert_eq!(face.italic(), italic);
            let parsed = ttf_parser::Face::parse(face.data(), 0).unwrap();
            assert_eq!(parsed.weight().to_number(), weight.to_value());
            assert_eq!(parsed.is_italic(), italic);
            assert!(
                ids.insert(face.font_id()),
                "real faces have distinct identities"
            );
            if weight == FontWeight::Regular && !italic {
                assert_eq!(face.font_id(), bootstrap.font_id());
                assert_eq!(face.data().as_slice(), bootstrap.font_data().as_slice());
            }
            let run = engine.shape_styled(caption, Some(&style), 13.0).unwrap();
            assert_eq!(run.binding.as_ref().unwrap().face(), face);
            let (_, keys) = engine
                .shape_bound_with_keys(caption, 24.0, &resolved.binding)
                .unwrap();
            assert!(!keys.is_empty() && keys.iter().all(|key| key.glyph_id != 0));
            let bitmap = engine.rasterize_bound_glyph(&keys[0], 1.0).unwrap();
            assert!(bitmap.metrics.width > 0 && bitmap.metrics.height > 0);
            assert!(bitmap.pixels.iter().any(|value| *value > 0));
            bitmaps.push((bitmap.metrics.width, bitmap.metrics.height, bitmap.pixels));
        }
        for variant in &bitmaps[1..] {
            assert_ne!(
                variant, &bitmaps[0],
                "requested style must produce actual different ink"
            );
        }
    }

    #[test]
    fn ui_typography_catalog_preserves_default_and_reports_actual_weight() {
        let mut default = FontManagerTextEngine::new_with_system_ui();
        let legacy = default.shape("AV Привет", 24.0);
        let style = TextStyle::default().with_size(24.0);
        let explicit = default
            .shape_styled("AV Привет", Some(&style), 13.0)
            .unwrap();
        assert_eq!(legacy.width, explicit.width);
        assert_eq!(
            explicit.binding.as_ref().unwrap().face().font_id(),
            default.font_id()
        );
        assert_eq!(
            explicit.binding.as_ref().unwrap().face().data().as_slice(),
            default.font_data().as_slice()
        );
        let zero = default
            .shape_styled(
                "AV Привет",
                Some(&style.with_cluster_gap(0.0).unwrap()),
                13.0,
            )
            .unwrap();
        assert_eq!(zero.width, legacy.width);
        #[cfg(target_os = "macos")]
        {
            let mut manager = FontManager::new();
            let regular = manager
                .load_font_memory(
                    Arc::new(
                        std::fs::read("/System/Library/Fonts/Supplemental/Arial.ttf").unwrap(),
                    ),
                    None,
                )
                .unwrap();
            let bold = manager
                .load_font_memory(
                    Arc::new(
                        std::fs::read("/System/Library/Fonts/Supplemental/Arial Bold.ttf").unwrap(),
                    ),
                    None,
                )
                .unwrap();
            assert_ne!(regular, bold);
            let mut engine =
                FontManagerTextEngine::with_face_catalog(manager.face_catalog(), regular).unwrap();
            let family = manager.get(regular).unwrap().family_name.clone();
            let request = TextStyle::default()
                .with_family(family)
                .with_size(24.0)
                .with_weight(FontWeight::Medium);
            let resolved = engine.resolve_text_style(&request).unwrap();
            assert_eq!(resolved.binding.face().font_id(), regular);
            assert_eq!(resolved.binding.face().weight(), FontWeight::Regular);
            let heavy = engine
                .resolve_text_style(&request.clone().with_weight(FontWeight::Bold))
                .unwrap();
            assert_eq!(heavy.binding.face().font_id(), bold);
            assert_eq!(heavy.binding.face().weight(), FontWeight::Bold);
            let a = engine
                .shape_styled("AV Привет", Some(&request), 13.0)
                .unwrap();
            let b = engine
                .shape_styled(
                    "AV Привет",
                    Some(&request.with_weight(FontWeight::Bold)),
                    13.0,
                )
                .unwrap();
            assert_ne!(a.binding, b.binding);
            assert_ne!(a.width, b.width);
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
        let mut engine = FontManagerTextEngine::new();
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

    // ========================================================================
    // SPEC-HAL-103 Stage 2: FontIdentity and compute_font_id tests
    // ========================================================================

    /// SPEC-HAL-103 Stage 2 TEST: Same identity + same data → same ID (determinism)
    #[test]
    fn test_compute_font_id_determinism() {
        let data = FontManagerTextEngine::legacy_inter_data();
        let identity = FontIdentity::legacy_inter();

        let id1 = compute_font_id(&identity, &data);
        let id2 = compute_font_id(&identity, &data);

        assert_eq!(id1, id2, "compute_font_id must be deterministic");
    }

    /// SPEC-HAL-103 Stage 2 TEST: Same bytes + different family name → different ID
    #[test]
    fn test_compute_font_id_identity_sensitivity() {
        let data = FontManagerTextEngine::legacy_inter_data();

        let identity_inter = FontIdentity::new(
            "Inter",
            Some("Inter-Regular".to_string()),
            FontWeight::Regular,
            false,
        );

        let identity_other = FontIdentity::new(
            "OtherFont", // Different family name
            Some("Inter-Regular".to_string()),
            FontWeight::Regular,
            false,
        );

        let id_inter = compute_font_id(&identity_inter, &data);
        let id_other = compute_font_id(&identity_other, &data);

        assert_ne!(
            id_inter, id_other,
            "Different family name must produce different font_id"
        );
    }

    /// SPEC-HAL-103 Stage 2 TEST: Backward compatibility - Inter ID must match previous
    #[test]
    #[allow(deprecated)]
    fn test_compute_font_id_backward_compatibility() {
        let data = FontManagerTextEngine::legacy_inter_data();

        // Old API (deprecated)
        let old_id = FontManagerTextEngine::compute_font_id_for_inter(&data);

        // New API with explicit identity
        let identity = FontIdentity::new(
            "Inter",
            FontManagerTextEngine::extract_postscript_name(&data),
            FontWeight::Regular,
            false,
        );
        let new_id = compute_font_id(&identity, &data);

        assert_eq!(
            old_id, new_id,
            "New compute_font_id must produce same ID as deprecated compute_font_id_for_inter"
        );
    }

    /// SPEC-HAL-103 Stage 2 TEST: FontIdentity::legacy_inter() produces correct identity
    #[test]
    fn test_font_identity_legacy_inter() {
        let identity = FontIdentity::legacy_inter();
        assert_eq!(identity.family_name, "Inter");
        assert_eq!(identity.postscript_name, Some("Inter-Regular".to_string()));
        assert_eq!(identity.weight, FontWeight::Regular);
        assert!(!identity.italic);
    }

    /// SPEC-HAL-103 Stage 2 TEST: FontIdentity::from_font_data extracts correct values
    #[test]
    fn test_font_identity_from_font_data() {
        let data = FontManagerTextEngine::legacy_inter_data();
        let identity = FontIdentity::from_font_data(&data, "Fallback");

        // InterVariable.ttf FAMILY name (name ID 1) is "Inter Variable"
        assert_eq!(identity.family_name, "Inter Variable");
        assert!(identity.postscript_name.is_some());
        assert_eq!(identity.weight, FontWeight::Regular);
        assert!(!identity.italic);
    }

    // ========================================================================
    // Original tests (updated for Stage 2)
    // ========================================================================

    /// SPEC-049/051 INVARIANT TEST: FontManagerTextEngine and FontManager
    /// must produce identical font_ids for the same font data.
    #[test]
    fn test_font_id_consistency_with_font_manager() {
        // Get font_id from FontManagerTextEngine
        let text_engine = FontManagerTextEngine::new();
        let text_engine_font_id = text_engine.font_id();

        // Get font_id from FontManager by registering the same font
        let mut font_manager = FontManager::new();
        let font_data = FontManagerTextEngine::legacy_inter_data();
        let font_manager_font_id = font_manager.load_font_bytes(
            font_data,
            "Inter".to_string(),
            FontWeight::Regular,
            false,
        );

        // CRITICAL: These MUST be equal for SPEC-049/051 compliance
        assert_eq!(
            text_engine_font_id, font_manager_font_id,
            "FontManagerTextEngine font_id ({}) must match FontManager font_id ({}) for same font data",
            text_engine_font_id, font_manager_font_id
        );
    }

    /// Test that FontManagerTextEngine correctly shapes text and produces valid glyph keys.
    #[test]
    fn test_shape_produces_valid_glyph_keys() {
        let mut engine = FontManagerTextEngine::new();
        let (run, keys) = engine.shape_with_keys("Hello", 16.0);

        // Should have glyphs
        assert!(!run.glyphs.is_empty(), "Should produce glyphs for 'Hello'");
        assert_eq!(
            run.glyphs.len(),
            keys.len(),
            "Glyph count should match key count"
        );

        // All keys should have the engine's font_id
        for key in &keys {
            assert_eq!(
                key.font_id,
                engine.font_id(),
                "GlyphKey font_id must match engine's font_id"
            );
        }
    }

    /// Test that legacy_inter_data returns consistent data.
    #[test]
    fn test_legacy_inter_data_consistency() {
        let data1 = FontManagerTextEngine::legacy_inter_data();
        let data2 = FontManagerTextEngine::legacy_inter_data();

        assert_eq!(
            data1.len(),
            data2.len(),
            "Legacy Inter data should be consistent"
        );
        assert_eq!(&*data1, &*data2, "Legacy Inter data bytes should match");
    }

    /// Test that FontManagerTextEngine's font_id matches what WgpuRenderer would use.
    /// This ensures UI thread shaping and platform thread rasterization use the same font.
    #[test]
    #[allow(deprecated)]
    fn test_font_id_matches_renderer_computation() {
        // Simulate what WgpuRenderer does (still uses deprecated API for now)
        let font_data = FontManagerTextEngine::legacy_inter_data();
        let renderer_font_id = FontManagerTextEngine::compute_font_id_for_inter(&font_data);

        // Create text engine
        let engine = FontManagerTextEngine::new();

        assert_eq!(
            engine.font_id(),
            renderer_font_id,
            "FontManagerTextEngine font_id must match renderer's font_id computation"
        );
    }

    /// SPEC-HAL-103 Stage 2 TEST: Engine stores and exposes identity
    #[test]
    fn test_engine_stores_identity() {
        let engine = FontManagerTextEngine::new();
        let identity = engine.identity();

        assert_eq!(identity.family_name, "Inter");
        assert_eq!(identity.weight, FontWeight::Regular);
        assert!(!identity.italic);
    }

    // ========================================================================
    // SPEC-HAL-103 Stage 3-4: SystemUI bootstrap tests
    // ========================================================================

    /// SPEC-HAL-103 Stage 3 TEST: new_with_system_ui falls back to Inter on Linux
    #[test]
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
    fn test_new_with_system_ui_falls_back_to_inter() {
        let engine = FontManagerTextEngine::new_with_system_ui();
        let id = engine.identity();
        assert_eq!(id.family_name, "Inter");
    }

    /// SPEC-HAL-103 Stage 4 TEST: new_with_system_ui returns Segoe UI on Windows
    #[test]
    #[cfg(target_os = "windows")]
    fn test_new_with_system_ui_returns_segoe_ui_on_windows() {
        let engine = FontManagerTextEngine::new_with_system_ui();
        let id = engine.identity();
        assert_eq!(id.family_name, "Segoe UI");
    }

    /// SPEC-HAL-103 Stage 5 TEST: new_with_system_ui returns system font on macOS
    #[test]
    #[cfg(target_os = "macos")]
    fn test_new_with_system_ui_returns_system_font_on_macos() {
        let engine = FontManagerTextEngine::new_with_system_ui();
        let id = engine.identity();
        // macOS system font is typically ".AppleSystemUIFont" or ".SF NS"
        assert!(!id.family_name.is_empty());
        assert_ne!(
            id.family_name, "Inter",
            "Should not fall back to Inter on macOS"
        );
    }
}
