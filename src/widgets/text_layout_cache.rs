//! SPEC-HAL-052 Consumer: Text Layout Cache
//!
//! Caches shaped lines and cluster boundaries for the TextEdit widget.
//! Invalidates when document version or layout width changes.

// ============================================================================
// SPEC-HAL-141 TEXT GEOMETRY CONSUMPTION INVARIANTS
// ============================================================================
//
// These invariants enforce correct geometry derivation discipline for text widgets.
// All text geometry (caret position, selection bounds, hit-testing) MUST be derived
// from cached layout data, never recomputed during paint/event phases.
//
// INV-TEXT-GEOM-CACHE-ONLY:
//     All text geometry derivation MUST read from TextLayoutCache.
//     Paint and event handlers MUST NOT call text_engine.measure() or similar.
//     Rationale: Geometry is computed once during layout, then read-only.
//
// INV-TEXT-NO-PAINT-MEASURE:
//     The paint phase MUST NOT call text_engine.measure(), text_engine.shape()
//     for geometry computation (shaping for rendering is allowed).
//     Selection X positions, caret positions use cached boundaries only.
//     Rationale: Measuring during paint recomputes what layout already cached.
//
// INV-TEXT-BOUNDARY-LOOKUP:
//     Caret and selection X positions MUST use caret_x_for_byte() or boundaries().
//     These read from pre-computed ClusterBoundary.x_logical values.
//     Never use prefix measurement or substring shaping for position lookup.
//
// INV-TEXT-LINE-Y-LOOKUP:
//     Line Y positions MUST use line_y() which reads from cached Line heights.
//     Never compute line positions by re-summing or re-measuring.
//
// INV-TEXT-CACHE-FRESHNESS:
//     (K.INV1) Cache MUST be rebuilt before any boundary lookup after mutation.
//     is_fresh_for_doc_len() guards against stale cache access.
//
// ============================================================================

use crate::core::editing::ClusterBoundary;
use crate::core::paint::Color;
use crate::core::paragraph::{
    LayoutParams, Line, Paragraph, ParagraphLayoutEngine, Run, TextStyle,
};
use crate::core::text_engine::{GlyphRun, TextBinding, TextEngine, TextInputError, TextMetrics};
use crate::platform::font_manager::FontFaceCatalog;

/// Cached text layout data for efficient rendering and hit-testing.
///
/// Stores:
/// - Shaped lines from ParagraphLayoutEngine
/// - Cluster boundaries for cursor/selection operations
/// - Metrics for line height calculations
///
/// Cache is invalidated when document version or layout width changes.
///
/// K.INV1: After document mutation, cache must be rebuilt before querying.
pub struct TextLayoutCache {
    /// Cached shaped lines
    lines: Vec<Line>,
    /// Cached cluster boundaries (SPEC-HAL-052)
    boundaries: Vec<ClusterBoundary>,
    /// Document version when cache was built
    cached_version: u64,
    /// Layout width when cache was built
    cached_width: f32,
    /// Font size used for shaping
    font_size: f32,
    /// Text color
    color: Color,
    /// Total content height
    content_height: f32,
    /// S1.INV5: Max line width for horizontal scroll
    content_width: f32,
    /// Line height for cursor positioning
    line_height: f32,
    /// K.INV1: Document length when cache was built (for stale detection)
    cached_doc_len: usize,
    /// Caret thickness for content width calculation (theme-backed)
    caret_thickness: f32,
    // HAL049 §18: immutable typography identity, shared by geometry and Paint.
    cached_catalog: Option<FontFaceCatalog>,
    cached_primary: Option<TextBinding>,
    cached_metrics: [u32; 4],
    typography_revision: u64,
    /// SPEC-HAL-141: Debug-only staleness flag.
    /// Set true by mark_stale(), set false by rebuild().
    /// Getters assert this is false in debug builds.
    #[cfg(debug_assertions)]
    is_stale: bool,
}

impl std::fmt::Debug for TextLayoutCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Catalog/face Debug includes complete admitted font bytes. Cache
        // diagnostics describe derived geometry and identity, never that backing.
        f.debug_struct("TextLayoutCache")
            .field("lines", &self.lines)
            .field("boundaries", &self.boundaries)
            .field("cached_version", &self.cached_version)
            .field("cached_width", &self.cached_width)
            .field("font_size", &self.font_size)
            .field("color", &self.color)
            .field("content_height", &self.content_height)
            .field("content_width", &self.content_width)
            .field("line_height", &self.line_height)
            .field("cached_doc_len", &self.cached_doc_len)
            .field("caret_thickness", &self.caret_thickness)
            .field("typography_revision", &self.typography_revision)
            .finish_non_exhaustive()
    }
}

impl TextLayoutCache {
    /// Create a new empty cache.
    pub fn new(font_size: f32, color: Color) -> Self {
        Self {
            lines: Vec::new(),
            boundaries: Vec::new(),
            cached_version: u64::MAX, // Force initial rebuild
            cached_width: -1.0,
            font_size,
            color,
            content_height: 0.0,
            content_width: 0.0,
            line_height: font_size * 1.2, // Default line height
            cached_doc_len: 0,
            caret_thickness: 2.0, // Default matches theme.typography.caret_thickness
            cached_catalog: None,
            cached_primary: None,
            cached_metrics: [0; 4],
            typography_revision: 0,
            #[cfg(debug_assertions)]
            is_stale: true, // Start stale, rebuild() clears
        }
    }

    /// Check if cache needs rebuild.
    pub fn needs_rebuild(&self, document_version: u64, layout_width: f32) -> bool {
        self.cached_version != document_version
            || self.cached_width.to_bits() != layout_width.to_bits()
    }

    fn typography(
        &self,
        engine: &mut dyn TextEngine,
    ) -> Result<(Option<FontFaceCatalog>, Option<TextBinding>, TextMetrics), TextInputError> {
        if !self.font_size.is_finite() || self.font_size <= 0.0 {
            return Err(TextInputError::InvalidFontSize);
        }
        let catalog = engine.font_face_catalog();
        let primary = if let Some(catalog) = &catalog {
            let binding = engine
                .resolve_text_style(&TextStyle::default().with_size(self.font_size))?
                .binding;
            if catalog
                .get(binding.face().font_id())
                .is_none_or(|face| face.binding != *binding.face())
            {
                return Err(TextInputError::FontUnavailable);
            }
            Some(binding)
        } else {
            None
        };
        let metrics = engine.metrics("", self.font_size);
        if catalog.is_some()
            && (![
                metrics.ascent,
                metrics.descent,
                metrics.line_gap,
                metrics.line_height,
            ]
            .into_iter()
            .all(f32::is_finite)
                || metrics.line_height <= 0.0)
        {
            return Err(TextInputError::PositionOverflow);
        }
        Ok((catalog, primary, metrics))
    }

    fn metrics_key(metrics: TextMetrics) -> [u32; 4] {
        [
            metrics.ascent.to_bits(),
            metrics.descent.to_bits(),
            metrics.line_gap.to_bits(),
            metrics.line_height.to_bits(),
        ]
    }

    /// INV-EDIT-CACHE-IDENTITY: an equal document/width cannot bless another
    /// admitted catalog, primary face or current native metric qualification.
    pub(crate) fn typography_changed(
        &self,
        engine: &mut dyn TextEngine,
    ) -> Result<bool, TextInputError> {
        let (catalog, primary, metrics) = self.typography(engine)?;
        let same_catalog = match (&catalog, &self.cached_catalog) {
            (Some(a), Some(b)) => a.same_generation(b),
            (None, None) => true,
            _ => false,
        };
        Ok(!same_catalog
            || primary != self.cached_primary
            || Self::metrics_key(metrics) != self.cached_metrics)
    }

    pub(crate) fn typography_revision(&self) -> u64 {
        self.typography_revision
    }

    /// Get the cached layout width.
    ///
    /// Used by TextArea to check if width has changed for tight-width rebuild logic.
    #[inline]
    pub fn cached_width(&self) -> f32 {
        self.cached_width
    }

    /// K.INV1: Check if cache is fresh for given document state.
    /// Returns true if boundaries are valid for the given doc_len.
    /// MUST be called before any boundary lookup after mutation.
    #[inline]
    pub fn is_fresh_for_doc_len(&self, doc_len: usize) -> bool {
        self.cached_doc_len == doc_len
    }

    /// K.INV1: Get cached document length for debug assertions.
    #[inline]
    pub fn cached_doc_len(&self) -> usize {
        self.cached_doc_len
    }

    /// SPEC-HAL-141: Mark cache as stale after document mutation.
    ///
    /// Must be called whenever the document is mutated. rebuild() clears staleness.
    /// Geometry getters will panic in debug builds if cache is stale.
    #[inline]
    pub fn mark_stale(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.is_stale = true;
        }
    }

    /// SPEC-HAL-141: Hard freshness assertion for debug builds.
    ///
    /// Panics if cache is stale (mark_stale was called without subsequent rebuild).
    /// This is the single enforcement point for cache freshness.
    /// Called automatically by all geometry getters.
    ///
    /// # Arguments
    /// * `context` - Call site identifier for diagnostic messages
    ///
    /// # Panics
    /// Panics in debug builds if `is_stale == true`, indicating
    /// a geometry read after mutation without layout rebuild.
    #[cfg(debug_assertions)]
    #[inline]
    fn assert_fresh(&self, context: &str) {
        assert!(
            !self.is_stale,
            "[{}] TextLayoutCache STALE: mark_stale() was called but rebuild() was not. \
             Geometry read after mutation without layout rebuild is forbidden.",
            context
        );
    }

    /// SPEC-HAL-141: No-op in release builds.
    #[cfg(not(debug_assertions))]
    #[inline]
    fn assert_fresh(&self, _context: &str) {}

    /// Rebuild cache from document text.
    ///
    /// Uses ParagraphLayoutEngine for soft-wrap layout and
    /// ClusterBoundary::build_from_lines for boundary extraction.
    pub fn rebuild(
        &mut self,
        text: &str,
        document_version: u64,
        layout_width: f32,
        text_engine: &mut dyn TextEngine,
    ) {
        self.try_rebuild(text, document_version, layout_width, text_engine)
            .expect("UI-TYPOGRAPHY-INPUT: editable registered layout failed");
    }

    /// HAL049 §18 / HAL383: prepare all derived values before publishing any.
    /// A catalog that cannot certify its binding is an error, never permission
    /// to silently re-enter the single-shaper path.
    pub(crate) fn try_rebuild(
        &mut self,
        text: &str,
        document_version: u64,
        layout_width: f32,
        text_engine: &mut dyn TextEngine,
    ) -> Result<(), TextInputError> {
        if layout_width.is_nan() || layout_width <= 0.0 {
            return Err(TextInputError::PositionOverflow);
        }
        let (catalog, primary, metrics) = self.typography(text_engine)?;
        let same_catalog = match (&catalog, &self.cached_catalog) {
            (Some(a), Some(b)) => a.same_generation(b),
            (None, None) => true,
            _ => false,
        };
        let typography_revision = if same_catalog
            && primary == self.cached_primary
            && Self::metrics_key(metrics) == self.cached_metrics
        {
            self.typography_revision
        } else {
            self.typography_revision
                .checked_add(1)
                .ok_or(TextInputError::PositionOverflow)?
        };

        // Build paragraph with uniform styling
        let paragraph = Paragraph::from_plain_text(text.to_string(), self.font_size, self.color);

        // Layout parameters
        let params = LayoutParams {
            max_width: if layout_width.is_finite() {
                layout_width
            } else {
                f32::INFINITY
            },
            scale: 1.0,
            canonical_metrics: metrics,
        };

        let lines = if let Some(catalog) = &catalog {
            ParagraphLayoutEngine::try_layout_with_catalog(
                &paragraph,
                catalog,
                primary
                    .as_ref()
                    .ok_or(TextInputError::FontUnavailable)?
                    .face()
                    .font_id(),
                &params,
            )?
        } else if let Some(shaper) = text_engine.shaper() {
            ParagraphLayoutEngine::layout(&paragraph, shaper, &params)
        } else {
            // Fallback for engines without shaper
            vec![Line::empty(metrics)]
        };

        // Build cluster boundaries from lines
        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Compute total height
        let content_height: f32 = lines.iter().map(|line| line.height).sum();
        let content_width =
            lines.iter().map(|line| line.width).fold(0.0_f32, f32::max) + self.caret_thickness;
        if catalog.is_some()
            && (!content_height.is_finite()
                || !content_width.is_finite()
                || boundaries
                    .iter()
                    .any(|boundary| !boundary.x_logical.is_finite()))
        {
            return Err(TextInputError::PositionOverflow);
        }
        // INV-EDIT-FACE-COHERENCE / INV-EDIT-CLUSTER-COHERENCE: one atomic
        // publication of the exact final runs and their original boundary map.
        self.lines = lines;
        self.boundaries = boundaries;
        self.content_height = content_height;
        self.content_width = content_width;
        self.line_height = metrics.line_height;
        self.cached_catalog = catalog;
        self.cached_primary = primary;
        self.cached_metrics = Self::metrics_key(metrics);
        self.typography_revision = typography_revision;

        // DEBUG: Trace line layout (gated via HAL_DEBUG_LAYOUT_CACHE)
        {
            static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if *ENABLED.get_or_init(|| std::env::var_os("HAL_DEBUG_LAYOUT_CACHE").is_some()) {
                eprintln!(
                    "[LAYOUT_CACHE] layout_width={:.1}, lines={}, content_height={:.1}, max_line_width={:.1}",
                    layout_width,
                    self.lines.len(),
                    self.content_height,
                    self.lines
                        .iter()
                        .map(|l| l.width)
                        .fold(0.0_f32, |a, b| a.max(b))
                );
            }
        }

        // S1.INV5: Compute max line width for horizontal scroll
        // S1.INV6: Content width must include space for trailing caret
        // This ensures horizontal scroll can reach a position where caret at EOL is visible.
        // Update cache keys
        self.cached_version = document_version;
        self.cached_width = layout_width;
        self.cached_doc_len = text.len();

        // SPEC-HAL-141: Clear staleness after successful rebuild
        #[cfg(debug_assertions)]
        {
            self.is_stale = false;
        }
        Ok(())
    }

    /// Reproduce a final laid-out run through the admitted source/face/direction,
    /// exactly as the registered TextBlock path. No shaped history is retained.
    pub(crate) fn shape_run(
        &self,
        text: &str,
        run: &Run,
        engine: &mut dyn TextEngine,
    ) -> Result<GlyphRun, TextInputError> {
        ParagraphLayoutEngine::shape_uniform_run(text, run, self.cached_catalog.as_ref(), engine)
    }

    /// The existing whole-string content-motion painter cannot represent mixed
    /// faces. Preserve it only when its actual single binding/shape is this cache.
    pub(crate) fn matches_single_run(&self, run: &GlyphRun) -> bool {
        if self.cached_catalog.is_none() {
            return true; // Original no-catalog capability and qualification.
        }
        let Some(binding) = run.binding.as_ref() else {
            return false;
        };
        if self
            .cached_catalog
            .as_ref()
            .unwrap()
            .get(binding.face().font_id())
            .is_none_or(|face| face.binding != *binding.face())
        {
            return false;
        }
        if self.lines.len() != 1
            || self.cached_doc_len != run.text.len()
            || self.font_size != run.font_size
            || (self.lines[0].width - run.width).abs() > 0.001
        {
            return false;
        }
        let runs = &self.lines[0].runs;
        if runs.iter().any(|r| {
            r.font_id != binding.face().font_id() || r.style.cluster_gap != binding.cluster_gap()
        }) {
            return false;
        }
        runs.iter().map(|r| r.glyphs.len()).sum::<usize>() == run.glyphs.len()
            && runs
                .iter()
                .flat_map(|r| &r.glyphs)
                .zip(&run.glyphs)
                .all(|(a, b)| a.glyph_id == b.id && (a.advance - b.advance).abs() <= 0.001)
    }

    /// Get the shaped lines.
    pub fn lines(&self) -> &[Line] {
        self.assert_fresh("lines");
        &self.lines
    }

    /// Get the cluster boundaries.
    pub fn boundaries(&self) -> &[ClusterBoundary] {
        self.assert_fresh("boundaries");
        &self.boundaries
    }

    /// Get total content height.
    pub fn content_height(&self) -> f32 {
        self.assert_fresh("content_height");
        self.content_height
    }

    /// S1.INV5: Get max line width for horizontal scroll.
    pub fn content_width(&self) -> f32 {
        self.assert_fresh("content_width");
        self.content_width
    }

    /// Get line height for cursor calculations.
    pub fn line_height(&self) -> f32 {
        self.assert_fresh("line_height");
        self.line_height
    }

    /// Get font size.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Get text color.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Set font size (will invalidate cache on next rebuild check).
    pub fn set_font_size(&mut self, size: f32) {
        if self.font_size.to_bits() != size.to_bits() {
            self.font_size = size;
            self.cached_version = u64::MAX; // Force rebuild
        }
    }

    /// Set text color.
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    /// Set caret thickness for content width calculation.
    /// Should be synced from theme.typography.caret_thickness during layout.
    pub fn set_caret_thickness(&mut self, thickness: f32) {
        if self.caret_thickness.to_bits() != thickness.to_bits() {
            self.caret_thickness = thickness;
            self.cached_version = u64::MAX;
        }
    }

    /// Get caret thickness (synced from theme during layout).
    pub fn caret_thickness(&self) -> f32 {
        self.caret_thickness
    }

    /// Force cache invalidation.
    pub fn invalidate(&mut self) {
        self.cached_version = u64::MAX;
        // SPEC-HAL-141: Also mark stale since cache is now invalid
        #[cfg(debug_assertions)]
        {
            self.is_stale = true;
        }
    }

    /// Get the Y position for a given line index.
    pub fn line_y(&self, line_index: usize) -> f32 {
        self.assert_fresh("line_y");
        self.lines.iter().take(line_index).map(|l| l.height).sum()
    }

    /// Get line count.
    pub fn line_count(&self) -> usize {
        self.assert_fresh("line_count");
        self.lines.len().max(1)
    }

    /// Get caret x position for a given byte position.
    ///
    /// S1.INV6: This is the authoritative function for caret positioning.
    /// For byte_pos == line_end_byte (sentinel), returns x after the last glyph.
    ///
    /// Returns 0.0 if byte_pos is not found in boundaries.
    pub fn caret_x_for_byte(&self, byte_pos: usize) -> f32 {
        self.assert_fresh("caret_x_for_byte");
        for b in &self.boundaries {
            if b.byte_start == byte_pos {
                return b.x_logical;
            }
        }
        // Fallback: not found (should not happen with valid boundaries)
        0.0
    }
}

impl Default for TextLayoutCache {
    fn default() -> Self {
        // TODO: Default color should be resolved from theme at point of use
        Self::new(16.0, Color::WHITE)
    }
}

// ============================================================================
// Test Harness Primitives (SPEC-HAL-109 Stage A)
// ============================================================================

#[cfg(any(test, feature = "test-harness"))]
impl TextLayoutCache {
    /// Inject pre-built cluster boundaries for testing.
    /// Bypasses layout() entirely - no text shaping, no engine access.
    pub fn set_boundaries_for_test(&mut self, boundaries: Vec<ClusterBoundary>) {
        self.boundaries = boundaries;
    }

    /// K.INV1: Set cached document length for testing.
    /// Must be called after set_boundaries_for_test to satisfy K.INV1 guards.
    pub fn set_cached_doc_len_for_test(&mut self, doc_len: usize) {
        self.cached_doc_len = doc_len;
    }

    /// Set uniform line heights for testing.
    /// Creates `line_count` lines with the given height.
    pub fn set_lines_for_test(&mut self, line_count: usize, line_height: f32) {
        use crate::core::paragraph::Line;
        self.lines = (0..line_count)
            .map(|_| Line {
                runs: Vec::new(),
                width: 100.0,
                ascent: line_height * 0.8,
                descent: line_height * 0.2,
                height: line_height,
                baseline: line_height * 0.8,
            })
            .collect();
        self.line_height = line_height;
        self.content_height = line_count as f32 * line_height;
    }

    /// SPEC-HAL-141: Clear staleness flag for testing.
    /// Must be called after injecting test data to allow getter access.
    pub fn clear_stale_for_test(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.is_stale = false;
        }
    }
}
