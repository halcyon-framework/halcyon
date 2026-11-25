//! SPEC-HAL-048: Paragraph & Text Layout Engine
//! SPEC-HAL-049: Font Resolution Integration
//!
//! Multi-line text layout with:
//! - Per-span styling (font, size, weight, color)
//! - Cluster-aware line breaking
//! - Run-based shaping integration
//! - Proper baseline alignment
//! - FontManager-based font resolution (SPEC-049)
//! - Missing glyph fallback
//! - Text alignment (left, center, right)
//!
//! Pipeline: Paragraph → Spans → Runs → Lines
//!
//! This is the CANONICAL paragraph layout system. Use `ParagraphLayoutEngine`
//! for all text layout needs. The legacy `LineBreaker` in `text_linebreaker.rs`
//! is deprecated.

use crate::core::paint::Color;
use crate::core::text_engine::{
    GlyphRun, ResolvedTextStyle, TextBinding, TextEngine, TextInputError, TextMetrics,
    bound_glyph_run_from_shape,
};
use crate::core::text_shaper::{ShapedCluster, TextDirection, TextShaper};
use crate::platform::font_manager::{
    CatalogFace, FontFaceCatalog, FontFamily as FmFontFamily, FontManager, FontStack, FontWeight,
};
use crate::platform::font_resolver::FontResolver;
use std::ops::Range;

// ============================================================================
// Text Alignment (SPEC-048 Section 4.6)
// ============================================================================

/// Text alignment options.
///
/// Per SPEC-048 Section 4.6. This is the CANONICAL alignment enum.
/// Alignment is applied during rendering, not during layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Left-aligned (default for LTR text)
    #[default]
    Left,
    /// Center-aligned
    Center,
    /// Right-aligned (default for RTL text)
    Right,
}

impl TextAlign {
    /// Calculate X offset for a line given container width.
    ///
    /// Returns the offset to add to the line's X position.
    pub fn offset(&self, line_width: f32, container_width: f32) -> f32 {
        match self {
            TextAlign::Left => 0.0,
            TextAlign::Center => (container_width - line_width) * 0.5,
            TextAlign::Right => container_width - line_width,
        }
    }
}

// ============================================================================
// Data Structures (SPEC-048 Section 4)
// ============================================================================

/// Font family identifier.
///
/// Wraps a String and converts to FontManager's FontFamily for resolution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FontFamily(pub String);

impl FontFamily {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    /// Convert to FontManager's FontFamily for resolution.
    pub fn to_fm_family(&self) -> FmFontFamily {
        match self.0.as_str() {
            "" | "SansSerif" => FmFontFamily::SansSerif,
            "SystemUI" => FmFontFamily::SystemUI,
            "Serif" => FmFontFamily::Serif,
            "Monospace" => FmFontFamily::Monospace,
            _ => FmFontFamily::named(&self.0),
        }
    }

    /// Create a FontStack from this family with SansSerif fallback.
    pub fn to_font_stack(&self) -> FontStack {
        FontStack::single(self.to_fm_family()).with_fallback(FmFontFamily::SansSerif)
    }
}

impl From<&str> for FontFamily {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for FontFamily {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Text style for a span.
///
/// Per SPEC-048 Section 4.3.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub cluster_gap: crate::core::text_shaper::ClusterGap,
    pub font_family: FontFamily,
    pub font_size: f32,
    pub weight: FontWeight,
    pub italic: bool,
    pub color: Color,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            cluster_gap: crate::core::text_shaper::ClusterGap::default(),
            font_family: FontFamily::default(),
            font_size: 16.0,
            weight: FontWeight::Regular,
            italic: false,
            color: Color::WHITE,
        }
    }
}

impl TextStyle {
    pub fn with_cluster_gap(
        mut self,
        gap: f32,
    ) -> Result<Self, crate::core::text_shaper::TextInputError> {
        self.cluster_gap = crate::core::text_shaper::ClusterGap::new(gap)?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), crate::core::text_shaper::TextInputError> {
        if !self.font_size.is_finite() || self.font_size <= 0.0 {
            return Err(crate::core::text_shaper::TextInputError::InvalidFontSize);
        }
        Ok(())
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    pub fn with_family(mut self, family: impl Into<FontFamily>) -> Self {
        self.font_family = family.into();
        self
    }

    /// Check if two styles are identical (for span merging).
    pub fn same_as(&self, other: &TextStyle) -> bool {
        self.font_family == other.font_family
            && self.cluster_gap == other.cluster_gap
            && self.font_size.to_bits() == other.font_size.to_bits()
            && self.weight == other.weight
            && self.italic == other.italic
            && self.color == other.color
    }
}

/// A styled span within a paragraph.
///
/// Per SPEC-048 Section 4.2.
#[derive(Debug, Clone)]
pub struct Span {
    /// Byte range into paragraph text (start..end)
    pub range: Range<usize>,
    /// Style for this span
    pub style: TextStyle,
}

impl Span {
    pub fn new(range: Range<usize>, style: TextStyle) -> Self {
        Self { range, style }
    }

    /// Create a span covering the entire text with default style.
    pub fn full(text_len: usize) -> Self {
        Self {
            range: 0..text_len,
            style: TextStyle::default(),
        }
    }
}

/// A paragraph of text with styled spans.
///
/// Per SPEC-048 Section 4.1.
#[derive(Debug, Clone)]
pub struct Paragraph {
    /// The full text content
    pub text: String,
    /// Styled spans (must cover entire text)
    pub spans: Vec<Span>,
}

impl Paragraph {
    /// Create a paragraph with uniform style.
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        let text = text.into();
        let len = text.len();
        Self {
            text,
            spans: vec![Span::new(0..len, style)],
        }
    }

    /// Create a paragraph with explicit spans.
    ///
    /// Spans must cover the entire text without gaps or overlaps.
    pub fn with_spans(text: impl Into<String>, spans: Vec<Span>) -> Self {
        Self {
            text: text.into(),
            spans,
        }
    }

    /// Create an empty paragraph.
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            spans: Vec::new(),
        }
    }

    /// Create a paragraph from plain text with uniform styling.
    ///
    /// Convenience constructor for TextBlock and similar widgets.
    pub fn from_plain_text(text: String, size: f32, color: Color) -> Self {
        let style = TextStyle::default().with_size(size).with_color(color);
        Self::new(text, style)
    }
}

/// A shaped glyph with logical positions.
///
/// Wraps ShapedCluster with positions converted to logical space.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID from the font
    pub glyph_id: u32,
    /// X offset from run origin in logical pixels
    pub x_offset: f32,
    /// Y offset from baseline in logical pixels
    pub y_offset: f32,
    /// Horizontal advance in logical pixels
    pub advance: f32,
    /// Original cluster index (byte offset in source text)
    pub cluster: u32,
}

impl ShapedGlyph {
    /// Create from ShapedCluster, converting physical to logical coordinates.
    pub fn from_cluster(cluster: &ShapedCluster, scale: f32) -> Self {
        Self {
            glyph_id: cluster.glyph_id,
            x_offset: cluster.x_offset_phys / scale,
            y_offset: cluster.y_offset_phys / scale,
            advance: cluster.advance_phys / scale,
            cluster: cluster.cluster,
        }
    }
}

/// A shaped run of glyphs with uniform style.
///
/// Per SPEC-048 Section 4.4.
#[derive(Debug, Clone)]
pub struct Run {
    /// Byte range into paragraph text
    pub range: Range<usize>,
    /// Style for this run
    pub style: TextStyle,
    /// Font identifier (from FontManager)
    pub font_id: u64,
    /// Shaped glyphs in visual order (logical coordinates)
    pub glyphs: Vec<ShapedGlyph>,
    /// Font metrics for this run (logical units)
    pub metrics: TextMetrics,
    /// Total advance width in logical pixels
    pub width: f32,
}

/// A laid-out line of text.
///
/// Per SPEC-048 Section 4.5.
#[derive(Debug, Clone)]
pub struct Line {
    /// Runs in this line (in visual order)
    pub runs: Vec<Run>,
    /// Total width in logical pixels
    pub width: f32,
    /// Maximum ascent among all runs
    pub ascent: f32,
    /// Maximum descent among all runs
    pub descent: f32,
    /// Line height (ascent + descent)
    pub height: f32,
    /// Baseline offset from line top
    pub baseline: f32,
}

impl Line {
    /// Create an empty line with given metrics.
    ///
    /// L1.INV2: Height is computed as ascent + descent, consistent with
    /// assemble_line() which uses the same formula for lines with content.
    pub fn empty(metrics: TextMetrics) -> Self {
        Self {
            runs: Vec::new(),
            width: 0.0,
            ascent: metrics.ascent,
            descent: metrics.descent,
            height: metrics.ascent + metrics.descent,
            baseline: metrics.ascent,
        }
    }
}

/// A line box and its first logical source insertion position.
pub(crate) struct LineWithSource {
    pub line: Line,
    pub source_start: usize,
}

impl LineWithSource {
    pub(crate) fn empty(metrics: TextMetrics, source_start: usize) -> Self {
        Self {
            line: Line::empty(metrics),
            source_start,
        }
    }

    fn into_lines(lines: Vec<Self>) -> Vec<Line> {
        lines.into_iter().map(|layout| layout.line).collect()
    }
}

// ============================================================================
// Layout Parameters
// ============================================================================

/// Parameters for paragraph layout.
#[derive(Debug, Clone)]
pub struct LayoutParams {
    /// Maximum width for line breaking (logical pixels)
    pub max_width: f32,
    /// Device scale factor (logical to physical)
    pub scale: f32,
    /// L2.INV1: Canonical font metrics for ALL line boxes (empty and non-empty).
    /// This is the single source of truth for vertical metrics.
    /// Line height = canonical_metrics.ascent + canonical_metrics.descent.
    pub canonical_metrics: TextMetrics,
}

impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            max_width: f32::INFINITY,
            scale: 1.0,
            canonical_metrics: TextMetrics {
                ascent: 14.0,
                descent: 4.0,
                line_gap: 0.0,
                line_height: 18.0,
            },
        }
    }
}

// ============================================================================
// ParagraphLayoutEngine
// ============================================================================

/// Main paragraph layout engine.
///
/// Implements SPEC-048 + SPEC-049 pipeline:
/// 1. Span segmentation (merge identical styles)
/// 2. Font resolution (SPEC-049)
/// 3. Run creation (shape each span with resolved font)
/// 4. Missing glyph fallback (SPEC-049)
/// 5. Line breaking (cluster-aware)
/// 6. Line assembly (baseline alignment)
pub struct ParagraphLayoutEngine;

impl ParagraphLayoutEngine {
    /// Layout a paragraph into lines using FontManager for font resolution.
    ///
    /// # Arguments
    /// - `paragraph`: The paragraph to layout
    /// - `font_manager`: FontManager for font resolution and shaping
    /// - `params`: Layout parameters
    ///
    /// # Returns
    /// Vector of laid-out lines ready for rendering.
    pub fn layout_with_fonts(
        paragraph: &Paragraph,
        font_manager: &FontManager,
        params: &LayoutParams,
    ) -> Vec<Line> {
        Self::try_layout_catalog(paragraph, &font_manager.face_catalog(), None, params)
            .map(LineWithSource::into_lines)
            .expect("UI-TYPOGRAPHY-INPUT: registered paragraph layout failed")
    }

    /// HAL049 §15: resolve only admitted faces, preserving every source cluster.
    pub fn try_layout_with_catalog(
        paragraph: &Paragraph,
        catalog: &crate::platform::font_manager::FontFaceCatalog,
        default_font_id: u64,
        params: &LayoutParams,
    ) -> Result<Vec<Line>, crate::core::text_engine::TextInputError> {
        Self::try_layout_catalog(paragraph, catalog, Some(default_font_id), params)
            .map(LineWithSource::into_lines)
    }

    /// Reproduce one final face run from catalog layout.
    ///
    /// Use the original paragraph and a final `Run` from its layout. The
    /// text engine must admit the recorded face and reproduce the run at its
    /// current scale. Keep the original line's width, baseline, and visual run
    /// order. Paint at the original run X and
    /// `line_top + line.baseline - glyph_run.ascent`.
    pub fn shape_laid_out_run(
        paragraph: &Paragraph,
        run: &Run,
        text_engine: &mut dyn TextEngine,
    ) -> Result<GlyphRun, TextInputError> {
        text_engine.shape_laid_out_paragraph_run(paragraph, run)
    }

    /// Resolve the same whole face run admitted by canonical paragraph layout.
    pub(crate) fn reconstruct_bound_run(
        paragraph: &Paragraph,
        original: &Run,
        engine: &mut dyn TextEngine,
        scale: f32,
    ) -> Result<GlyphRun, TextInputError> {
        let spans = Self::catalog_spans(paragraph)?;
        let origin = Self::originating_span(&spans, &original.range)
            .filter(|span| span.style.same_as(&original.style))
            .ok_or(TextInputError::UnsupportedTypography)?;
        let source = paragraph
            .text
            .get(original.range.clone())
            .filter(|text| !text.is_empty())
            .ok_or(TextInputError::PositionOverflow)?;
        let direction = TextDirection::detect(&paragraph.text[origin.range.clone()]);
        let catalog = engine
            .font_face_catalog()
            .ok_or(TextInputError::FontUnavailable)?;
        let (face, binding) = Self::bind_catalog_run(&catalog, original, direction)?;
        let style = ResolvedTextStyle {
            binding,
            font_size: original.style.font_size,
        };
        let shaped = face.shaper.shape_with_gap_direction(
            source,
            style.font_size,
            scale,
            original.style.cluster_gap,
            direction,
        )?;
        Self::catalog_shape_ranges(source, &shaped)?;
        let design = Self::catalog_run_metrics(face, style.font_size)?;
        if shaped.width != original.width
            || design.ascent != original.metrics.ascent
            || design.descent != original.metrics.descent
            || design.line_gap != original.metrics.line_gap
            || design.line_height != original.metrics.line_height
            || shaped.glyphs.len() != original.glyphs.len()
            || shaped
                .glyphs
                .iter()
                .zip(&original.glyphs)
                .any(|(glyph, expected)| {
                    let actual = ShapedGlyph::from_cluster(glyph, scale);
                    actual.glyph_id != expected.glyph_id
                        || actual.cluster != expected.cluster
                        || actual.x_offset != expected.x_offset
                        || actual.y_offset != expected.y_offset
                        || actual.advance != expected.advance
                })
        {
            return Err(TextInputError::UnsupportedTypography);
        }
        // Use the same paint ascent and other native metrics as an ordinary
        // bound run. The caller still places it on the original line baseline.
        let metrics = engine.metrics_bound_text(style.font_size, &style.binding)?;
        bound_glyph_run_from_shape(source, &style, shaped, metrics, scale)
    }

    fn bind_catalog_run<'a>(
        catalog: &'a FontFaceCatalog,
        run: &Run,
        direction: TextDirection,
    ) -> Result<(&'a CatalogFace, TextBinding), TextInputError> {
        let face = catalog
            .get(run.font_id)
            .ok_or(TextInputError::FontUnavailable)?;
        let binding =
            TextBinding::new(face.binding.clone(), run.style.cluster_gap).with_direction(direction);
        Ok((face, binding))
    }

    /// Bind the existing uniform widget paragraph to its cached catalog.
    /// Widget layout owns logical geometry and the engine owns paint resampling.
    pub(crate) fn shape_uniform_run(
        text: &str,
        run: &Run,
        catalog: Option<&FontFaceCatalog>,
        engine: &mut dyn TextEngine,
    ) -> Result<GlyphRun, TextInputError> {
        let source = text
            .get(run.range.clone())
            .ok_or(TextInputError::PositionOverflow)?;
        let Some(catalog) = catalog else {
            return Ok(engine.shape(source, run.style.font_size));
        };
        let (_, binding) = Self::bind_catalog_run(catalog, run, TextDirection::detect(text))?;
        engine.shape_bound_text(source, run.style.font_size, &binding)
    }

    pub(crate) fn try_layout_catalog(
        paragraph: &Paragraph,
        catalog: &crate::platform::font_manager::FontFaceCatalog,
        default_font_id: Option<u64>,
        params: &LayoutParams,
    ) -> Result<Vec<LineWithSource>, crate::core::text_engine::TextInputError> {
        use crate::core::text_engine::TextInputError;
        if !params.scale.is_finite() || params.scale <= 0.0 {
            return Err(TextInputError::InvalidScale);
        }
        if params.max_width.is_nan()
            || params.max_width < 0.0
            || [
                params.canonical_metrics.ascent,
                params.canonical_metrics.descent,
                params.canonical_metrics.line_gap,
                params.canonical_metrics.line_height,
            ]
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err(TextInputError::PositionOverflow);
        }
        let spans = Self::catalog_spans(paragraph)?;
        if paragraph.text.is_empty() {
            return Ok(vec![LineWithSource::empty(params.canonical_metrics, 0)]);
        }
        if let Some(id) = default_font_id {
            catalog.get(id).ok_or(TextInputError::FontUnavailable)?;
        }
        let runs = Self::catalog_runs(
            &paragraph.text,
            &spans,
            &spans,
            catalog,
            default_font_id,
            params,
            false,
        )?;
        let mut lines = Self::break_and_assemble(&paragraph.text, runs, params);
        // A line boundary changes shaping context. Reproduce each final complete
        // face run before committing its width/geometry; a newly required wrap
        // adds a source cut, so this refinement is bounded by the source length.
        for _ in 0..=paragraph.text.chars().count() {
            let mut output = Vec::new();
            let mut added_cut = false;
            for layout in lines {
                let start = layout.source_start;
                let Some(finish) = layout.line.runs.iter().map(|run| run.range.end).max() else {
                    output.push(layout);
                    continue;
                };
                let first = spans.partition_point(|span| span.range.end <= start);
                let pieces: Vec<_> = spans[first..]
                    .iter()
                    .take_while(|span| span.range.start < finish)
                    .filter_map(|span| {
                        let a = span.range.start.max(start);
                        let b = span.range.end.min(finish);
                        (a < b).then(|| Span::new(a..b, span.style.clone()))
                    })
                    .collect();
                let runs = Self::catalog_runs(
                    &paragraph.text,
                    &pieces,
                    &spans,
                    catalog,
                    default_font_id,
                    params,
                    true,
                )?;
                let final_line = if runs.is_empty() {
                    LineWithSource::empty(params.canonical_metrics, start)
                } else {
                    Self::assemble_line(runs, params.canonical_metrics)
                };
                if final_line.line.width > params.max_width {
                    let mut logical = final_line.line.runs.clone();
                    logical.sort_by_key(|run| run.range.start);
                    let split = Self::break_and_assemble(&paragraph.text, logical, params);
                    if split.len() > 1 {
                        output.extend(split);
                        added_cut = true;
                        continue;
                    }
                }
                output.push(final_line);
            }
            if !added_cut {
                return Ok(output);
            }
            lines = output;
        }
        Err(TextInputError::PositionOverflow)
    }

    fn catalog_spans(paragraph: &Paragraph) -> Result<Vec<Span>, TextInputError> {
        let mut spans = paragraph.spans.clone();
        spans.sort_by_key(|span| span.range.start);
        let mut end = 0;
        for span in &spans {
            span.style.validate()?;
            if span.range.start != end || paragraph.text.get(span.range.clone()).is_none() {
                return Err(TextInputError::PositionOverflow);
            }
            end = span.range.end;
        }
        if end != paragraph.text.len() {
            return Err(TextInputError::PositionOverflow);
        }
        Ok(Self::merge_spans(&spans))
    }

    fn originating_span<'a>(spans: &'a [Span], range: &Range<usize>) -> Option<&'a Span> {
        spans.get(spans.partition_point(|span| span.range.end <= range.start))
            .filter(|span| span.range.start <= range.start && span.range.end >= range.end)
    }

    /// Layout a paragraph into lines (legacy API using TextShaper directly).
    ///
    /// # Arguments
    /// - `paragraph`: The paragraph to layout
    /// - `shaper`: Text shaper for glyph shaping
    /// - `params`: Layout parameters
    ///
    /// # Returns
    /// Vector of laid-out lines ready for rendering.
    pub fn layout(paragraph: &Paragraph, shaper: &TextShaper, params: &LayoutParams) -> Vec<Line> {
        Self::try_layout(paragraph, shaper, params)
            .expect("UI-TYPOGRAPHY-INPUT: paragraph layout failed")
    }

    pub fn try_layout(
        paragraph: &Paragraph,
        shaper: &TextShaper,
        params: &LayoutParams,
    ) -> Result<Vec<Line>, crate::core::text_engine::TextInputError> {
        Self::try_layout_with_line_sources(paragraph, shaper, params).map(LineWithSource::into_lines)
    }

    pub(crate) fn try_layout_with_line_sources(
        paragraph: &Paragraph,
        shaper: &TextShaper,
        params: &LayoutParams,
    ) -> Result<Vec<LineWithSource>, crate::core::text_engine::TextInputError> {
        for span in &paragraph.spans {
            span.style.validate()?;
        }
        if paragraph.text.is_empty() {
            return Ok(vec![LineWithSource::empty(params.canonical_metrics, 0)]);
        }

        // Stage 1: Merge adjacent spans with identical style
        let merged_spans = Self::merge_spans(&paragraph.spans);

        // Stage 2: Create runs from spans (shape each span)
        let runs = Self::create_runs(paragraph, &merged_spans, shaper, params)?;

        // Stage 3 & 4: Break into lines and assemble
        Ok(Self::break_and_assemble(&paragraph.text, runs, params))
    }

    /// Stage 1: Merge adjacent spans with identical style.
    fn merge_spans(spans: &[Span]) -> Vec<Span> {
        if spans.is_empty() {
            return Vec::new();
        }

        let mut merged = Vec::with_capacity(spans.len());
        let mut current = spans[0].clone();

        for span in spans.iter().skip(1) {
            if current.range.end == span.range.start && current.style.same_as(&span.style) {
                // Merge: extend current span
                current.range.end = span.range.end;
            } else {
                // Different style or gap: push current, start new
                merged.push(current);
                current = span.clone();
            }
        }
        merged.push(current);

        merged
    }

    /// Stage 2: Create runs by shaping each span (legacy API).
    fn create_runs(
        paragraph: &Paragraph,
        spans: &[Span],
        shaper: &TextShaper,
        params: &LayoutParams,
    ) -> Result<Vec<Run>, crate::core::text_engine::TextInputError> {
        let mut runs = Vec::with_capacity(spans.len());

        for span in spans {
            let span_text = &paragraph.text[span.range.clone()];
            if span_text.is_empty() {
                continue;
            }

            // Shape the span text
            let clusters = shaper
                .shape_with_gap(
                    span_text,
                    span.style.font_size,
                    params.scale,
                    span.style.cluster_gap,
                )?
                .glyphs;

            // Convert to logical coordinates
            let glyphs: Vec<ShapedGlyph> = clusters
                .iter()
                .map(|c| ShapedGlyph::from_cluster(c, params.scale))
                .collect();

            // Calculate metrics
            let width: f32 = glyphs.iter().map(|g| g.advance).sum();

            // Get font metrics (using ttf-parser would be ideal, but we use defaults scaled by font size)
            let metrics = Self::estimate_metrics(span.style.font_size, params);

            runs.push(Run {
                range: span.range.clone(),
                style: span.style.clone(),
                font_id: 0, // Legacy single-font path
                glyphs,
                metrics,
                width,
            });
        }

        Ok(runs)
    }

    /// Validate logical source coverage without sorting the actual visual glyphs.
    fn catalog_shape_ranges(
        text: &str,
        shape: &crate::core::text_shaper::SpacedShape,
    ) -> Result<Vec<Range<usize>>, crate::core::text_engine::TextInputError> {
        use crate::core::text_engine::TextInputError;
        let mut ranges: Vec<_> = shape.clusters.iter().map(|c| c.source.clone()).collect();
        ranges.sort_by_key(|r| r.start);
        ranges.dedup();
        let mut end = 0;
        for range in &ranges {
            if range.start != end || range.end <= range.start || text.get(range.clone()).is_none() {
                return Err(TextInputError::PositionOverflow);
            }
            end = range.end;
        }
        let starts: std::collections::HashSet<_> = ranges.iter().map(|range| range.start).collect();
        if end != text.len()
            || shape
                .glyphs
                .iter()
                .any(|glyph| !starts.contains(&(glyph.cluster as usize)))
        {
            return Err(TextInputError::UnsupportedTypography);
        }
        Ok(ranges)
    }

    fn catalog_run(
        text: &str,
        range: Range<usize>,
        style: &TextStyle,
        font_id: u64,
        catalog: &crate::platform::font_manager::FontFaceCatalog,
        params: &LayoutParams,
        direction: crate::core::text_shaper::TextDirection,
    ) -> Result<Run, crate::core::text_engine::TextInputError> {
        use crate::core::text_engine::TextInputError;
        let source = text
            .get(range.clone())
            .ok_or(TextInputError::PositionOverflow)?;
        let face = catalog
            .get(font_id)
            .ok_or(TextInputError::FontUnavailable)?;
        let shape = face.shaper.shape_with_gap_direction(
            source,
            style.font_size,
            params.scale,
            style.cluster_gap,
            direction,
        )?;
        Self::catalog_shape_ranges(source, &shape)?;
        Ok(Run {
            range,
            style: style.clone(),
            font_id,
            glyphs: shape
                .glyphs
                .iter()
                .map(|g| ShapedGlyph::from_cluster(g, params.scale))
                .collect(),
            metrics: Self::catalog_run_metrics(face, style.font_size)?,
            width: shape.width,
        })
    }

    fn catalog_run_metrics(face: &CatalogFace, font_size: f32) -> Result<TextMetrics, TextInputError> {
        let parsed = ttf_parser::Face::parse(face.binding.data(), 0)
            .map_err(|_| TextInputError::FontUnavailable)?;
        let unit = font_size / f32::from(parsed.units_per_em());
        let ascent = f32::from(parsed.ascender()) * unit;
        let descent = -f32::from(parsed.descender()) * unit;
        let line_gap = f32::from(parsed.line_gap()) * unit;
        Ok(TextMetrics {
            ascent,
            descent,
            line_gap,
            line_height: ascent + descent + line_gap,
        })
    }

    fn catalog_runs(
        text: &str,
        spans: &[Span],
        origins: &[Span],
        catalog: &crate::platform::font_manager::FontFaceCatalog,
        default_id: Option<u64>,
        params: &LayoutParams,
        visual: bool,
    ) -> Result<Vec<Run>, crate::core::text_engine::TextInputError> {
        use crate::core::text_engine::TextInputError;
        use std::collections::HashSet;
        let resolver = FontResolver::with_catalog(catalog);
        let mut result = Vec::new();
        for span in spans {
            let source = text
                .get(span.range.clone())
                .ok_or(TextInputError::PositionOverflow)?;
            if source.is_empty() {
                continue;
            }
            let original = Self::originating_span(origins, &span.range)
                .ok_or(TextInputError::PositionOverflow)?;
            let direction =
                crate::core::text_shaper::TextDirection::detect(&text[original.range.clone()]);
            let primary = if let Some(id) = default_id {
                catalog.resolve(id, &span.style)?.binding.face().font_id()
            } else {
                resolver
                    .resolve(
                        &span.style.font_family.to_font_stack(),
                        span.style.weight,
                        span.style.italic,
                    )
                    .ok_or(TextInputError::FontUnavailable)?
                    .font_id
            };
            let face = catalog
                .get(primary)
                .ok_or(TextInputError::FontUnavailable)?;
            let shape = face.shaper.shape_with_gap_direction(
                source,
                span.style.font_size,
                params.scale,
                span.style.cluster_gap,
                direction,
            )?;
            let ranges = Self::catalog_shape_ranges(source, &shape)?;
            let missing_offsets: HashSet<_> = shape
                .glyphs
                .iter()
                .filter(|glyph| glyph.glyph_id == 0)
                .map(|glyph| glyph.cluster as usize)
                .collect();
            let missing: Vec<_> = ranges
                .iter()
                .map(|range| missing_offsets.contains(&range.start))
                .collect();
            let candidates = resolver.fallback_candidates_for_cluster(
                source,
                span.style.weight,
                span.style.italic,
            );
            let mut rejected = vec![HashSet::new(); ranges.len()];
            let limit = ranges
                .len()
                .checked_mul(candidates.len())
                .ok_or(TextInputError::PositionOverflow)?;
            let mut completed = None;
            for _ in 0..=limit {
                let mut chosen = vec![primary; ranges.len()];
                for (i, range) in ranges.iter().enumerate() {
                    if !missing[i] {
                        continue;
                    }
                    let cluster = &source[range.clone()];
                    for candidate in &candidates {
                        if candidate.font_id == primary
                            || rejected[i].contains(&candidate.font_id)
                            || !cluster.chars().any(|ch| candidate.coverage_map.covers(ch))
                        {
                            continue;
                        }
                        let candidate_face = catalog
                            .get(candidate.font_id)
                            .ok_or(TextInputError::FontUnavailable)?;
                        let trial = candidate_face.shaper.shape_with_gap_direction(
                            cluster,
                            span.style.font_size,
                            params.scale,
                            span.style.cluster_gap,
                            direction,
                        )?;
                        Self::catalog_shape_ranges(cluster, &trial)?;
                        if trial.glyphs.iter().all(|g| g.glyph_id != 0) {
                            chosen[i] = candidate.font_id;
                            break;
                        }
                    }
                }
                let mut runs = Vec::new();
                let mut retry = false;
                let mut i = 0;
                while i < ranges.len() {
                    let start = i;
                    let id = chosen[i];
                    i += 1;
                    while i < ranges.len() && chosen[i] == id {
                        i += 1;
                    }
                    let range = (span.range.start + ranges[start].start)
                        ..(span.range.start + ranges[i - 1].end);
                    let run = Self::catalog_run(
                        text,
                        range,
                        &span.style,
                        id,
                        catalog,
                        params,
                        direction,
                    )?;
                    if id != primary && run.glyphs.iter().any(|g| g.glyph_id == 0) {
                        // Each failed coalesced candidate permanently removes
                        // one finite trial for each affected source cluster.
                        for set in &mut rejected[start..i] {
                            set.insert(id);
                        }
                        retry = true;
                    }
                    runs.push(run);
                }
                if !retry {
                    completed = Some(runs);
                    break;
                }
            }
            let mut runs = completed.ok_or(TextInputError::UnsupportedTypography)?;
            if visual {
                // Restore the primary span's visual run order only after logical
                // line breaking. Never interpret glyph ordinal as a source index.
                let mut ranks = vec![usize::MAX; runs.len()];
                for (rank, glyph) in shape.glyphs.iter().enumerate() {
                    let offset = span.range.start + glyph.cluster as usize;
                    let index = runs.partition_point(|run| run.range.end <= offset);
                    if index < runs.len() && runs[index].range.contains(&offset) {
                        ranks[index] = ranks[index].min(rank);
                    }
                }
                let mut ordered: Vec<_> = runs.into_iter().zip(ranks).collect();
                ordered.sort_by_key(|(_, rank)| *rank);
                runs = ordered.into_iter().map(|(run, _)| run).collect();
            }
            result.extend(runs);
        }
        Ok(result)
    }

    /// Estimate text metrics from font size.
    ///
    /// TODO: Get actual metrics from font via TextShaper or FontManager.
    fn estimate_metrics(font_size: f32, _params: &LayoutParams) -> TextMetrics {
        // Typical proportions for Latin fonts
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

    /// Stage 3 & 4: Break runs into lines (cluster-aware) and assemble.
    fn break_and_assemble(text: &str, runs: Vec<Run>, params: &LayoutParams) -> Vec<LineWithSource> {
        if runs.is_empty() {
            return vec![LineWithSource::empty(params.canonical_metrics, 0)];
        }

        let mut lines = Vec::new();
        let mut current_line_runs: Vec<Run> = Vec::new();
        // A hard break opens a following logical line even when it has no runs yet.
        let mut current_line_open = false;
        let mut open_line_start = runs[0].range.start;
        let mut current_line_width: f32 = 0.0;
        let mut last_break_opportunity: Option<BreakOpportunity> = None;

        for run in runs {
            let run_result = Self::process_run_for_line(
                &run,
                text,
                params,
                &mut current_line_width,
                &mut last_break_opportunity,
            );

            match run_result {
                RunLineResult::FitsEntirely(processed_run) => {
                    current_line_runs.push(processed_run);
                    current_line_open = true;
                }
                RunLineResult::NeedsBreak(before_break, after_break) => {
                    // Emit line with content before break
                    if let Some(before) = before_break {
                        current_line_runs.push(before);
                        current_line_open = true;
                    }
                    if !current_line_runs.is_empty() {
                        lines.push(Self::assemble_line(
                            current_line_runs,
                            params.canonical_metrics,
                        ));
                        current_line_runs = Vec::new();
                        current_line_open = false;
                        current_line_width = 0.0;
                        last_break_opportunity = None;
                    }

                    // Handle remaining content
                    if let Some(after) = after_break {
                        // Recursively process remaining run
                        let remaining_runs = vec![after];
                        let mut remaining_lines =
                            Self::break_and_assemble(text, remaining_runs, params);
                        if let Some(last) = remaining_lines.pop() {
                            // Completed recursive lines keep their source order.
                            // Retain only the last line so a following run can
                            // still join it when space remains.
                            lines.extend(remaining_lines);
                            open_line_start = last.source_start;
                            current_line_runs.extend(last.line.runs);
                            current_line_width = current_line_runs.iter().map(|r| r.width).sum();
                            current_line_open = true;
                        }
                    }
                }
                RunLineResult::HardBreak {
                    before: before_break,
                    newline,
                    after: after_break,
                } => {
                    // Content before the newline may exceed max_width.
                    // Recursively process it through break_and_assemble so
                    // it gets soft-broken at whitespace if needed.
                    //
                    // Combine any accumulated current_line_runs with the
                    // "before" content, then break_and_assemble the lot.
                    if let Some(before) = before_break {
                        current_line_runs.push(before);
                    }
                    if !current_line_runs.is_empty() {
                        // Check if we even need recursive processing.
                        let total_width: f32 = current_line_runs.iter().map(|r| r.width).sum();
                        if total_width <= params.max_width {
                            // Fits on one line — emit directly (fast path).
                            let taken = std::mem::take(&mut current_line_runs);
                            lines.push(Self::assemble_line(taken, params.canonical_metrics));
                        } else {
                            // Exceeds max_width — recursively soft-break.
                            let taken = std::mem::take(&mut current_line_runs);
                            let broken_lines = Self::break_and_assemble(text, taken, params);
                            lines.extend(broken_lines);
                        }
                    } else {
                        // L2.INV1: Empty content before newline still needs
                        // a visual line box.
                        lines.push(LineWithSource::empty(params.canonical_metrics, newline));
                    }
                    current_line_open = false;
                    open_line_start = newline + 1;
                    current_line_width = 0.0;
                    last_break_opportunity = None;

                    // Recursively process remaining content (may contain more
                    // newlines). Completed lines stay closed, while the last
                    // post-break line remains open for a following styled run.
                    if let Some(after) = after_break {
                        let remaining_runs = vec![after];
                        let mut remaining_lines =
                            Self::break_and_assemble(text, remaining_runs, params);
                        if let Some(last) = remaining_lines.pop() {
                            lines.extend(remaining_lines);
                            open_line_start = last.source_start;
                            current_line_runs.extend(last.line.runs);
                            current_line_width = current_line_runs.iter().map(|r| r.width).sum();
                            current_line_open = true;
                        }
                    } else {
                        // Preserve the empty logical line after the newline.
                        // A later run may fill it; function exit emits it once.
                        current_line_open = true;
                    }
                }
            }
        }

        // Emit final line
        if !current_line_runs.is_empty() {
            lines.push(Self::assemble_line(
                current_line_runs,
                params.canonical_metrics,
            ));
        } else if current_line_open {
            lines.push(LineWithSource::empty(params.canonical_metrics, open_line_start));
        }

        // Ensure at least one line
        if lines.is_empty() {
            lines.push(LineWithSource::empty(params.canonical_metrics, open_line_start));
        }

        lines
    }

    /// Process a run for the current line, determining if/where to break.
    fn process_run_for_line(
        run: &Run,
        text: &str,
        params: &LayoutParams,
        current_width: &mut f32,
        _last_break: &mut Option<BreakOpportunity>,
    ) -> RunLineResult {
        let run_text = &text[run.range.clone()];

        // Check for hard breaks (newlines)
        if let Some(newline_pos) = run_text.find('\n') {
            let byte_offset = run.range.start + newline_pos;

            // Split run at newline
            let before = if newline_pos > 0 {
                Some(Self::slice_run(run, run.range.start..byte_offset))
            } else {
                None
            };

            let after_start = byte_offset + 1; // Skip the newline
            let after = if after_start < run.range.end {
                Some(Self::slice_run(run, after_start..run.range.end))
            } else {
                None
            };

            return RunLineResult::HardBreak {
                before,
                newline: byte_offset,
                after,
            };
        }

        // Check if run fits entirely
        if *current_width + run.width <= params.max_width {
            *current_width += run.width;
            return RunLineResult::FitsEntirely(run.clone());
        }

        // Need to break this run - find cluster-aware break point
        if let Some((before, after)) =
            Self::find_break_in_run(run, text, params.max_width - *current_width)
        {
            RunLineResult::NeedsBreak(Some(before), Some(after))
        } else if *current_width > 0.0 {
            // Can't break within run, but we have content - break before this run
            RunLineResult::NeedsBreak(None, Some(run.clone()))
        } else {
            // Forced break within run (single word wider than max_width)
            if let Some((before, after)) = Self::force_break_in_run(run, params.max_width) {
                RunLineResult::NeedsBreak(Some(before), Some(after))
            } else {
                // Can't break at all - just include it
                *current_width += run.width;
                RunLineResult::FitsEntirely(run.clone())
            }
        }
    }

    /// Logical source clusters with measured advances and last visual glyph.
    fn logical_cluster_measures(run: &Run) -> Vec<(Range<usize>, f32, usize)> {
        let mut measures = std::collections::BTreeMap::<usize, (f32, usize)>::new();
        for (index, glyph) in run.glyphs.iter().enumerate() {
            let entry = measures.entry(glyph.cluster as usize).or_default();
            entry.0 += glyph.advance;
            entry.1 = index;
        }
        let entries: Vec<_> = measures.into_iter().collect();
        entries
            .iter()
            .enumerate()
            .map(|(i, (start, (width, last)))| {
                let end = entries.get(i + 1).map_or(run.range.len(), |e| e.0);
                (*start..end, *width, *last)
            })
            .collect()
    }

    fn find_break_in_run(run: &Run, text: &str, available_width: f32) -> Option<(Run, Run)> {
        let mut width = 0.0;
        let mut last_visual = 0;
        let mut soft = None;
        for (range, advance, last) in Self::logical_cluster_measures(run) {
            width += advance;
            last_visual = last_visual.max(last);
            let terminal_gap = if last_visual + 1 < run.glyphs.len() {
                run.style.cluster_gap.value()
            } else {
                0.0
            };
            if width - terminal_gap > available_width {
                let at = soft?;
                if at == run.range.end {
                    return None;
                }
                return Some((
                    Self::slice_run(run, run.range.start..at),
                    Self::slice_run(run, at..run.range.end),
                ));
            }
            // A soft boundary includes its separator's actual advance. Never
            // select an overflowing separator: reshaping that over-wide prefix
            // would otherwise force the separator onto a line of its own.
            let start = run.range.start + range.start;
            if text
                .get(start..)
                .and_then(|s| s.chars().next())
                .is_some_and(|ch| ch == ' ' || ch == '\t')
            {
                soft = Some(run.range.start + range.end);
            }
        }
        None
    }

    fn force_break_in_run(run: &Run, max_width: f32) -> Option<(Run, Run)> {
        let mut width = 0.0;
        let mut last_visual = 0;
        for (index, (range, advance, last)) in
            Self::logical_cluster_measures(run).into_iter().enumerate()
        {
            width += advance;
            last_visual = last_visual.max(last);
            let terminal_gap = if last_visual + 1 < run.glyphs.len() {
                run.style.cluster_gap.value()
            } else {
                0.0
            };
            if width - terminal_gap > max_width {
                let at = run.range.start + if index == 0 { range.end } else { range.start };
                if at == run.range.end {
                    return None;
                }
                return Some((
                    Self::slice_run(run, run.range.start..at),
                    Self::slice_run(run, at..run.range.end),
                ));
            }
        }
        None
    }

    /// Slice a run by byte range.
    ///
    /// IMPORTANT: Rebases glyph.cluster values to be relative to the new range.start.
    /// This ensures cluster offsets remain valid when used as indices into the sliced text.
    fn slice_run(run: &Run, range: Range<usize>) -> Run {
        let range_start = range.start.saturating_sub(run.range.start);
        let range_end = range.end.saturating_sub(run.range.start);

        let mut glyphs: Vec<ShapedGlyph> = run
            .glyphs
            .iter()
            .filter(|g| {
                let cluster = g.cluster as usize;
                cluster >= range_start && cluster < range_end
            })
            .map(|g| {
                // Rebase cluster to be relative to new range.start
                ShapedGlyph {
                    cluster: g.cluster.saturating_sub(range_start as u32),
                    ..*g
                }
            })
            .collect();

        if let Some(last) = run.glyphs.iter().rposition(|glyph| {
            let cluster = glyph.cluster as usize;
            cluster >= range_start && cluster < range_end
        }) {
            if last + 1 < run.glyphs.len()
                && run.glyphs[last].cluster != run.glyphs[last + 1].cluster
            {
                if let Some(glyph) = glyphs.last_mut() {
                    glyph.advance -= run.style.cluster_gap.value();
                }
            }
        }

        let width = glyphs.iter().map(|g| g.advance).sum();

        Run {
            range,
            style: run.style.clone(),
            font_id: run.font_id,
            glyphs,
            metrics: run.metrics,
            width,
        }
    }

    /// Stage 4: Assemble a line from runs.
    ///
    /// L2.INV1-3: Uses canonical metrics for height/baseline, not glyph-derived.
    /// This ensures empty and non-empty lines have identical vertical metrics.
    fn assemble_line(runs: Vec<Run>, canonical: TextMetrics) -> LineWithSource {
        let source_start = runs.iter().map(|run| run.range.start).min().unwrap();
        // L2.INV2: Line height = canonical.ascent + canonical.descent
        // L2.INV3: Baseline = canonical.ascent
        // Glyph-derived metrics are stored but do NOT affect line box height.
        let width = runs.iter().map(|r| r.width).sum();

        LineWithSource {
            source_start,
            line: Line {
                runs,
                width,
                ascent: canonical.ascent,
                descent: canonical.descent,
                height: canonical.ascent + canonical.descent,
                baseline: canonical.ascent,
            },
        }
    }
}

/// Break opportunity tracking.
#[derive(Debug, Clone, Copy)]
struct BreakOpportunity {
    /// Glyph index in current run
    #[allow(dead_code)]
    glyph_idx: usize,
    /// Byte offset in text
    #[allow(dead_code)]
    byte_offset: usize,
    /// Accumulated width up to this point
    #[allow(dead_code)]
    width: f32,
}

/// Result of processing a run for line breaking.
enum RunLineResult {
    /// Run fits entirely on current line
    FitsEntirely(Run),
    /// Run needs soft break: (content before break, content after break)
    NeedsBreak(Option<Run>, Option<Run>),
    /// The consumed LF and the content on either side of it.
    HardBreak {
        before: Option<Run>,
        newline: usize,
        after: Option<Run>,
    },
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text_engine::TextEngine;
    use crate::platform::font_manager::FontWeight as FmFontWeight;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;

    const SOFT_WRAP_CASES: [(&str, f32, f32); 3] = [
        ("Получите ранний доступ к платформе", 22.0, 270.0),
        ("ИИ-арбитраж и маржа для дилеров", 17.0, 216.0),
        (
            "One platform bringing together vehicle listings from sellers across the EU. For dealers, importers, exporters and businesses.",
            20.0,
            296.0,
        ),
    ];

    #[test]
    fn paragraph_soft_wrap_catalog_keeps_spaces_with_content_and_source() {
        let (fonts, font_id) = make_test_font_manager().expect("existing registered Inter asset");
        let catalog = fonts.face_catalog();
        let face = catalog.get(font_id).unwrap();
        let parsed = ttf_parser::Face::parse(face.binding.data(), 0).unwrap();
        let mut failures = Vec::new();
        for (source, size, width) in SOFT_WRAP_CASES {
            assert!(!source.contains('\n'));
            let paragraph = Paragraph::new(source, TextStyle::default().with_size(size));
            for scale in [1.0, 2.0] {
                let unit = size / f32::from(parsed.units_per_em());
                let ascent = f32::from(parsed.ascender()) * unit;
                let descent = -f32::from(parsed.descender()) * unit;
                let gap = f32::from(parsed.line_gap()) * unit;
                let params = LayoutParams {
                    max_width: width,
                    scale,
                    canonical_metrics: TextMetrics {
                        ascent,
                        descent,
                        line_gap: gap,
                        line_height: ascent + descent + gap,
                    },
                };
                let lines = ParagraphLayoutEngine::try_layout_with_catalog(
                    &paragraph, &catalog, font_id, &params,
                )
                .unwrap();
                let describe = |lines: &[Line]| {
                    lines
                        .iter()
                        .map(|line| {
                            (
                                line.runs
                                    .iter()
                                    .map(|run| &source[run.range.clone()])
                                    .collect::<String>(),
                                line.width,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let actual = describe(&lines);
                if actual.iter().any(|(text, _)| text.trim().is_empty()) {
                    failures.push(format!(
                        "source={source:?}, scale={scale}, max={width}, lines={actual:?}"
                    ));
                }
                assert_eq!(
                    actual
                        .iter()
                        .map(|(text, _)| text.as_str())
                        .collect::<String>(),
                    source
                );
                let mut end = 0;
                for line in &lines {
                    assert!(line.width <= width);
                    assert_eq!(line.height, ascent + descent);
                    assert_eq!(line.baseline, ascent);
                    for run in &line.runs {
                        assert_eq!(run.range.start, end);
                        assert_eq!(run.font_id, font_id);
                        let shaped = face
                            .shaper
                            .shape_with_gap(
                                &source[run.range.clone()],
                                size,
                                scale,
                                run.style.cluster_gap,
                            )
                            .unwrap();
                        assert_eq!(run.width, shaped.width);
                        assert_eq!(run.glyphs.len(), shaped.glyphs.len());
                        for (actual, expected) in run.glyphs.iter().zip(&shaped.glyphs) {
                            assert_ne!(actual.glyph_id, 0);
                            assert_eq!(actual.glyph_id, expected.glyph_id);
                            assert_eq!(actual.cluster, expected.cluster);
                            assert_eq!(actual.advance, expected.advance_phys / scale);
                        }
                        end = run.range.end;
                    }
                }
                assert_eq!(end, source.len());
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn paragraph_soft_wrap_preserves_real_hard_break_empty_lines() {
        let (fonts, font_id) = make_test_font_manager().expect("existing registered Inter asset");
        let catalog = fonts.face_catalog();
        let mut engine =
            FontManagerTextEngine::with_face_catalog(catalog.clone(), font_id).unwrap();
        for scale in [1.0, 2.0] {
            engine.set_scale(scale);
            for (source, expected) in [
                (
                    "\nПолучите ранний доступ к платформе\n\nTail\n",
                    vec!["", "Получите ранний ", "доступ к платформе", "", "Tail", ""],
                ),
                ("a\n \nb\n\n", vec!["a", " ", "b", "", ""]),
            ] {
                let params = LayoutParams {
                    max_width: 270.0,
                    scale,
                    canonical_metrics: engine.metrics("", 22.0),
                };
                let paragraph = Paragraph::new(source, TextStyle::default().with_size(22.0));
                let lines = ParagraphLayoutEngine::try_layout_with_catalog(
                    &paragraph, &catalog, font_id, &params,
                )
                .unwrap();
                let actual: Vec<_> = lines
                    .iter()
                    .map(|line| {
                        line.runs
                            .iter()
                            .map(|run| &source[run.range.clone()])
                            .collect::<String>()
                    })
                    .collect();
                assert_eq!(actual, expected);
                let mut covered = vec![false; source.len()];
                for line in &lines {
                    assert_eq!(
                        line.height,
                        params.canonical_metrics.ascent + params.canonical_metrics.descent
                    );
                    assert_eq!(line.baseline, params.canonical_metrics.ascent);
                    assert!(line.width <= params.max_width);
                    for run in &line.runs {
                        for byte in &mut covered[run.range.clone()] {
                            assert!(!*byte, "no duplicate source span");
                            *byte = true;
                        }
                    }
                }
                for (covered, byte) in covered.into_iter().zip(source.bytes()) {
                    assert_eq!(covered, byte != b'\n');
                }
            }
        }
    }

    #[test]
    fn paragraph_soft_wrap_reaches_textblock_scene_at_exact_measured_height() {
        use crate::core::animation::Timeline;
        use crate::core::event::KeyboardAuthorityMode;
        use crate::core::icon::IconRegistry;
        use crate::core::layout::Constraints;
        use crate::core::layout_transition::InterpolatedVisualRects;
        use crate::core::math::RectF;
        use crate::core::rect_authority::{LayoutAuthorityGuard, PaintAuthorityGuard};
        use crate::core::scene::{SceneBuilder, SceneNode};
        use crate::core::theme::Theme;
        use crate::core::widget::{LayoutContext, PaintContext, Widget};
        use crate::core::widget_tree::WidgetTree;
        use crate::widgets::text_block::TextBlock;

        let (fonts, font_id) = make_test_font_manager().expect("existing registered Inter asset");
        let catalog = fonts.face_catalog();
        for scale in [1.0, 2.0] {
            for (source, size, width) in SOFT_WRAP_CASES {
                let mut engine =
                    FontManagerTextEngine::with_face_catalog(catalog.clone(), font_id).unwrap();
                engine.set_scale(scale);
                let mut block = TextBlock::new(source).with_size(size);
                let mut tree = WidgetTree::new();
                let theme = Theme::dark();
                let measured = {
                    let _layout = LayoutAuthorityGuard::enter();
                    block
                        .layout(
                            &mut LayoutContext::new(&mut tree, &mut engine, &theme),
                            Constraints::new(0.0, width, 0.0, f32::INFINITY),
                            &[],
                        )
                        .size
                };
                let params = LayoutParams {
                    max_width: width,
                    scale: 1.0,
                    canonical_metrics: engine.metrics("", size),
                };
                let paragraph = Paragraph::new(source, TextStyle::default().with_size(size));
                let lines = ParagraphLayoutEngine::try_layout_with_catalog(
                    &paragraph, &catalog, font_id, &params,
                )
                .unwrap();
                assert!(lines.len() > 1);
                assert_eq!(measured.width, width);
                assert_eq!(
                    measured.height,
                    lines.iter().map(|line| line.height).sum::<f32>()
                );
                let first_line = lines[0]
                    .runs
                    .iter()
                    .map(|run| &source[run.range.clone()])
                    .collect::<String>();
                for (height, expected) in [
                    (measured.height, source),
                    (lines[0].height, first_line.as_str()),
                    (0.0, ""),
                ] {
                    let icons = IconRegistry::new(&[]);
                    let timeline = Timeline::default();
                    let transitions = InterpolatedVisualRects::default();
                    let mut scene = SceneBuilder::with_scale_factor(scale);
                    let mut overlays = Vec::new();
                    let mut tooltips = Vec::new();
                    {
                        let _paint = PaintAuthorityGuard::enter();
                        block.paint(
                            &mut PaintContext::new(
                                &tree,
                                &mut scene,
                                &theme,
                                &mut engine,
                                &icons,
                                0,
                                &timeline,
                                &transitions,
                                &mut overlays,
                                &mut tooltips,
                                scale,
                                None,
                                KeyboardAuthorityMode::Navigation,
                            ),
                            RectF::new(7.25, 3.5, measured.width, height),
                            &[],
                        );
                    }
                    let nodes: Vec<_> = scene
                        .build()
                        .root
                        .into_iter()
                        .filter_map(|node| match node {
                            SceneNode::Text(text) => Some(text),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(
                        nodes
                            .iter()
                            .map(|node| node.text.as_str())
                            .collect::<String>(),
                        expected,
                        "complete real TextBlock output at scale={scale}, height={height}"
                    );
                    assert!(nodes.iter().all(|node| !node.text.trim().is_empty()));
                    assert!(nodes.iter().all(|node| {
                        node.binding.as_ref().unwrap().face().font_id() == font_id
                    }));
                    let mut line_top = 0.0;
                    for (line, node) in lines.iter().zip(&nodes) {
                        let shaped = engine.shape(&node.text, size);
                        assert_eq!(
                            node.position.y,
                            3.5 + line_top + line.baseline - shaped.ascent
                        );
                        line_top += line.height;
                    }
                }
            }
        }
    }

    #[test]
    fn ui_typography_wrapped_line_has_no_terminal_gap() {
        let shaper = make_test_shaper().expect("existing Inter asset");
        let style = TextStyle::default()
            .with_size(24.0)
            .with_cluster_gap(3.0)
            .unwrap();
        let paragraph = Paragraph::new("office Привет e\u{301} alpha beta gamma", style.clone());
        let mut params = LayoutParams::default();
        params.max_width = 150.0;
        for scale in [1.0, 1.25, 2.0] {
            params.scale = scale;
            let lines = ParagraphLayoutEngine::try_layout(&paragraph, &shaper, &params).unwrap();
            assert!(lines.len() > 1);
            for line in lines {
                for run in &line.runs {
                    let shaped = shaper
                        .shape_with_gap(
                            &paragraph.text[run.range.clone()],
                            style.font_size,
                            scale,
                            style.cluster_gap,
                        )
                        .unwrap();
                    assert!(
                        (run.width - shaped.width).abs() < 0.01,
                        "wrapped run has no terminal cluster gap: {:?}",
                        run.range
                    );
                }
                assert!(
                    (line.width - line.runs.iter().map(|run| run.width).sum::<f32>()).abs() < 0.001
                );
            }
        }
    }

    #[test]
    fn ui_typography_forced_wrap_uses_complete_cluster_width() {
        let shaper = make_test_shaper().expect("existing Inter asset");
        for scale in [1.0, 1.25, 2.0] {
            for gap in [0.0, 3.0] {
                let style = TextStyle::default()
                    .with_size(24.0)
                    .with_cluster_gap(gap)
                    .unwrap();
                for source in ["HHH", "ffiHH", "H\u{301}HH"] {
                    let shaped = shaper
                        .shape_with_gap(source, 24.0, scale, style.cluster_gap)
                        .unwrap();
                    let terminal = match source {
                        "HHH" => 1,
                        "ffiHH" => shaped.clusters.len() - 2,
                        _ => 0,
                    };
                    let first_end = shaped.clusters[terminal].source.end;
                    let mut params = LayoutParams::default();
                    params.scale = scale;
                    params.max_width = shaped.clusters[terminal].x_end;
                    let paragraph = Paragraph::new(source, style.clone());
                    let lines =
                        ParagraphLayoutEngine::try_layout(&paragraph, &shaper, &params).unwrap();
                    assert!(lines.len() > 1);
                    assert_eq!(lines[0].runs[0].range, 0..first_end);
                    if source == "HHH" {
                        assert_eq!(&source[lines[0].runs[0].range.clone()], "HH");
                    }
                    if source == "ffiHH" {
                        assert_eq!(&source[lines[0].runs[0].range.clone()], "ffiH");
                    }
                    if source == "H\u{301}HH" {
                        assert_eq!(first_end, "H\u{301}".len());
                        assert!(
                            shaped
                                .glyphs
                                .iter()
                                .filter(|glyph| glyph.cluster == 0)
                                .count()
                                > 1
                        );
                    }
                    let mut cursor = 0;
                    for line in &lines {
                        assert!(
                            line.width <= params.max_width + 0.001,
                            "source={source:?}, scale={scale}, gap={gap}, width={}, max={}",
                            line.width,
                            params.max_width
                        );
                        for run in &line.runs {
                            assert_eq!(run.range.start, cursor);
                            let groups = shaped
                                .clusters
                                .iter()
                                .filter(|group| {
                                    group.source.start < run.range.end
                                        && group.source.end > run.range.start
                                })
                                .collect::<Vec<_>>();
                            assert!(!groups.is_empty());
                            for group in &groups {
                                assert!(run.range.start <= group.source.start);
                                assert!(run.range.end >= group.source.end);
                            }
                            let width = groups.last().unwrap().x_end - groups[0].x_start;
                            assert!((run.width - width).abs() < 0.001);
                            let original = shaped
                                .glyphs
                                .iter()
                                .filter(|glyph| run.range.contains(&(glyph.cluster as usize)))
                                .collect::<Vec<_>>();
                            assert_eq!(run.glyphs.len(), original.len());
                            for (glyph, original) in run.glyphs.iter().zip(original) {
                                assert_eq!(glyph.glyph_id, original.glyph_id);
                                assert_eq!(
                                    glyph.cluster as usize + run.range.start,
                                    original.cluster as usize
                                );
                            }
                            cursor = run.range.end;
                        }
                    }
                    assert_eq!(cursor, source.len());
                }
            }
        }
    }

    fn make_test_shaper() -> Option<TextShaper> {
        // Load Inter font for testing
        let font_path = "assets/fonts/Inter/InterVariable.ttf";
        let font_data = std::fs::read(font_path).ok()?;
        let font_data = std::sync::Arc::new(font_data);
        TextShaper::from_data(font_data)
    }

    /// Create a FontManager with embedded Inter font for testing layout_with_fonts.
    fn make_test_font_manager() -> Option<(FontManager, u64)> {
        let font_path = "assets/fonts/Inter/InterVariable.ttf";
        let font_data = std::fs::read(font_path).ok()?;
        let font_data = std::sync::Arc::new(font_data);

        let mut font_manager = FontManager::new();
        let font_id = font_manager.load_font_bytes(
            font_data,
            "Inter".to_string(),
            FmFontWeight::Regular,
            false,
        );
        Some((font_manager, font_id))
    }

    #[test]
    fn test_text_style_same_as() {
        let s1 = TextStyle::default();
        let s2 = TextStyle::default();
        assert!(s1.same_as(&s2));

        let s3 = TextStyle::default().with_size(20.0);
        assert!(!s1.same_as(&s3));

        let s4 = TextStyle::default().with_weight(FontWeight::Bold);
        assert!(!s1.same_as(&s4));
    }

    #[test]
    fn test_span_merging() {
        let style1 = TextStyle::default();
        let style2 = TextStyle::default().with_size(20.0);

        // Same style - should merge
        let spans = vec![
            Span::new(0..5, style1.clone()),
            Span::new(5..10, style1.clone()),
        ];
        let merged = ParagraphLayoutEngine::merge_spans(&spans);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].range, 0..10);

        // Different styles - should not merge
        let spans2 = vec![
            Span::new(0..5, style1.clone()),
            Span::new(5..10, style2.clone()),
        ];
        let merged2 = ParagraphLayoutEngine::merge_spans(&spans2);
        assert_eq!(merged2.len(), 2);
    }

    #[test]
    fn test_empty_paragraph() {
        let Some(shaper) = make_test_shaper() else {
            return; // Skip if font not available
        };

        let para = Paragraph::empty();
        let params = LayoutParams::default();
        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].runs.is_empty());
    }

    #[test]
    fn test_single_line_no_wrap() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        let para = Paragraph::new("Hello World", TextStyle::default());
        let params = LayoutParams {
            max_width: f32::INFINITY,
            scale: 1.0,
            ..Default::default()
        };

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].runs.is_empty());
        assert!(lines[0].width > 0.0);
    }

    #[test]
    fn paragraph_line_order_preserves_recursive_soft_wrap_sequence() {
        const TEXT: &str = "The left half of the image is visible. Its right half is covered by an orange rectangle.";

        let text_engine = FontManagerTextEngine::new();
        let shaper = text_engine
            .shaper()
            .expect("the production text engine must expose its canonical shaper");
        let para = Paragraph::new(TEXT, TextStyle::default().with_size(23.0));
        let params = LayoutParams {
            max_width: 371.0,
            scale: 1.0,
            ..Default::default()
        };

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);
        let line_texts: Vec<String> = lines
            .iter()
            .map(|line| {
                let mut ranges: Vec<_> = line.runs.iter().map(|run| run.range.clone()).collect();
                ranges.sort_by_key(|range| range.start);
                ranges
                    .into_iter()
                    .map(|range| &TEXT[range])
                    .collect::<String>()
            })
            .collect();

        assert!(line_texts.len() >= 3);
        assert_eq!(line_texts.concat(), TEXT);
    }

    #[test]
    fn test_hard_break_newline() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        let para = Paragraph::new("Line 1\nLine 2\nLine 3", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_line_metrics() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        let para = Paragraph::new("Test", TextStyle::default().with_size(16.0));
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);
        assert_eq!(lines.len(), 1);

        let line = &lines[0];
        assert!(line.ascent > 0.0);
        assert!(line.descent > 0.0);
        assert_eq!(line.height, line.ascent + line.descent);
        assert_eq!(line.baseline, line.ascent);
    }

    #[test]
    fn test_paragraph_with_spans() {
        let text = "Hello World";
        let spans = vec![
            Span::new(0..6, TextStyle::default()), // "Hello "
            Span::new(6..11, TextStyle::default().with_weight(FontWeight::Bold)), // "World"
        ];

        let para = Paragraph::with_spans(text, spans);
        assert_eq!(para.text, "Hello World");
        assert_eq!(para.spans.len(), 2);
    }

    // ========================================================================
    // SPEC-049/051 Tests: layout_with_fonts integration
    // ========================================================================

    #[test]
    fn test_layout_with_fonts_basic() {
        let Some((font_manager, expected_font_id)) = make_test_font_manager() else {
            return; // Skip if font not available
        };

        let para = Paragraph::new("Hello World", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout_with_fonts(&para, &font_manager, &params);

        assert_eq!(lines.len(), 1, "Should produce one line");
        assert!(!lines[0].runs.is_empty(), "Line should have runs");

        // SPEC-049/051: All runs should have the registered font_id
        for run in &lines[0].runs {
            assert_eq!(
                run.font_id, expected_font_id,
                "Run font_id must match FontManager's registered font_id"
            );
        }
    }

    #[test]
    fn test_layout_with_fonts_runs_have_valid_font_id() {
        let Some((font_manager, expected_font_id)) = make_test_font_manager() else {
            return;
        };

        // Use simple text without newlines for this test
        let para = Paragraph::new("Hello World Test", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout_with_fonts(&para, &font_manager, &params);

        assert!(!lines.is_empty(), "Should have at least one line");

        // Verify each run has a valid font_id from FontManager
        for (line_idx, line) in lines.iter().enumerate() {
            for (run_idx, run) in line.runs.iter().enumerate() {
                assert_eq!(
                    run.font_id, expected_font_id,
                    "Line {}, Run {}: font_id {} should match expected {}",
                    line_idx, run_idx, run.font_id, expected_font_id
                );

                // Verify the font_id exists in FontManager
                assert!(
                    font_manager.get_face(run.font_id).is_some(),
                    "font_id {} should exist in FontManager",
                    run.font_id
                );
            }
        }
    }

    #[test]
    fn test_layout_with_fonts_produces_glyphs() {
        let Some((font_manager, _)) = make_test_font_manager() else {
            return;
        };

        let para = Paragraph::new("Test", TextStyle::default().with_size(16.0));
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout_with_fonts(&para, &font_manager, &params);

        assert_eq!(lines.len(), 1);
        let line = &lines[0];

        // Should have at least one run with glyphs
        assert!(!line.runs.is_empty(), "Should have runs");

        for run in &line.runs {
            assert!(!run.glyphs.is_empty(), "Run should have glyphs for 'Test'");
            assert!(run.width > 0.0, "Run should have positive width");
        }

        // Line metrics should be populated
        assert!(line.ascent > 0.0, "Line should have positive ascent");
        assert!(line.descent > 0.0, "Line should have positive descent");
    }

    // ========================================================================
    // SPEC-048 Tests: TextAlign
    // ========================================================================

    #[test]
    fn test_text_align_default() {
        assert_eq!(TextAlign::default(), TextAlign::Left);
    }

    #[test]
    fn test_text_align_offset_left() {
        let align = TextAlign::Left;
        assert_eq!(align.offset(100.0, 200.0), 0.0);
        assert_eq!(align.offset(50.0, 100.0), 0.0);
    }

    #[test]
    fn test_text_align_offset_center() {
        let align = TextAlign::Center;
        assert_eq!(align.offset(100.0, 200.0), 50.0);
        assert_eq!(align.offset(50.0, 100.0), 25.0);
        assert_eq!(align.offset(80.0, 100.0), 10.0);
    }

    #[test]
    fn test_text_align_offset_right() {
        let align = TextAlign::Right;
        assert_eq!(align.offset(100.0, 200.0), 100.0);
        assert_eq!(align.offset(50.0, 100.0), 50.0);
        assert_eq!(align.offset(80.0, 100.0), 20.0);
    }

    // ========================================================================
    // SPEC-HAL-109 Stage L.1 Tests: Empty Line Box Generation
    // ========================================================================

    /// L1.INV1: Leading empty lines must produce line boxes with proper height.
    #[test]
    fn test_l1_leading_empty_lines() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        // Text = "\n\nabc" (two leading empty lines)
        let para = Paragraph::new("\n\nabc", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);

        // Must have 3 lines
        assert_eq!(
            lines.len(),
            3,
            "L1.INV1: Should have 3 lines for '\\n\\nabc'"
        );

        // First two lines are empty but MUST have positive height
        assert!(
            lines[0].height > 0.0,
            "L1.INV1: Empty line 0 must have positive height, got {}",
            lines[0].height
        );
        assert!(
            lines[1].height > 0.0,
            "L1.INV1: Empty line 1 must have positive height, got {}",
            lines[1].height
        );

        // Third line has content
        assert!(
            !lines[2].runs.is_empty(),
            "L1.INV1: Line 2 should have runs for 'abc'"
        );
    }

    /// L1.INV1: Only empty lines must produce line boxes with proper height.
    #[test]
    fn test_l1_only_empty_lines() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        // Text = "\n\n\n" (three empty lines = 4 lines total)
        let para = Paragraph::new("\n\n\n", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);

        // Must have 4 lines (each \n produces a line, plus trailing empty line)
        assert_eq!(
            lines.len(),
            4,
            "L1.INV1: Should have 4 lines for '\\n\\n\\n'"
        );

        // ALL lines must have positive height
        for (i, line) in lines.iter().enumerate() {
            assert!(
                line.height > 0.0,
                "L1.INV1: Line {} must have positive height, got {}",
                i,
                line.height
            );
        }
    }

    /// L1.INV1: Trailing empty line (text ending with \n) must have proper height.
    #[test]
    fn test_l1_trailing_empty_line() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        // Text = "abc\n" (one line + trailing empty line)
        let para = Paragraph::new("abc\n", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);

        // Must have 2 lines
        assert_eq!(lines.len(), 2, "L1.INV1: Should have 2 lines for 'abc\\n'");

        // First line has content
        assert!(
            !lines[0].runs.is_empty(),
            "L1.INV1: Line 0 should have runs for 'abc'"
        );

        // Second line is empty but MUST have positive height
        assert!(
            lines[1].height > 0.0,
            "L1.INV1: Trailing empty line must have positive height, got {}",
            lines[1].height
        );
    }

    /// L2.INV2: Empty line height MUST equal non-empty line height (exact).
    ///
    /// Both empty and non-empty lines use canonical_metrics for height/baseline.
    /// This eliminates vertical twitch when transitioning empty ↔ non-empty.
    #[test]
    fn test_l2_empty_line_height_exact() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        // Text with content line and empty line
        let para = Paragraph::new("abc\n", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);

        assert_eq!(lines.len(), 2);

        let content_line_height = lines[0].height;
        let empty_line_height = lines[1].height;

        // L2.INV2: Heights MUST be exactly equal
        assert!(
            (content_line_height - empty_line_height).abs() < 0.001,
            "L2.INV2: Empty line height ({}) MUST equal content line height ({})",
            empty_line_height,
            content_line_height
        );

        // L2.INV3: Baselines MUST be exactly equal
        let content_baseline = lines[0].baseline;
        let empty_baseline = lines[1].baseline;
        assert!(
            (content_baseline - empty_baseline).abs() < 0.001,
            "L2.INV3: Empty line baseline ({}) MUST equal content line baseline ({})",
            empty_baseline,
            content_baseline
        );
    }

    /// L1.INV1: Consecutive newlines in middle of text.
    #[test]
    fn test_l1_consecutive_newlines_in_middle() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        // Text = "a\n\n\nb" (two empty lines in middle)
        let para = Paragraph::new("a\n\n\nb", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);

        // Must have 4 lines: "a", "", "", "b"
        assert_eq!(
            lines.len(),
            4,
            "L1.INV1: Should have 4 lines for 'a\\n\\n\\nb'"
        );

        // All lines must have positive height
        for (i, line) in lines.iter().enumerate() {
            assert!(
                line.height > 0.0,
                "L1.INV1: Line {} must have positive height, got {}",
                i,
                line.height
            );
        }

        // First and last lines have runs
        assert!(!lines[0].runs.is_empty(), "Line 0 should have runs");
        assert!(!lines[3].runs.is_empty(), "Line 3 should have runs");

        // Middle lines are empty but have height
        assert!(lines[1].runs.is_empty(), "Line 1 should be empty");
        assert!(lines[2].runs.is_empty(), "Line 2 should be empty");
    }

    // ========================================================================
    // SPEC-HAL-109 Stage L.2 Tests: Canonical Line Metrics Unification
    // ========================================================================

    /// L2.INV1-3: All lines (empty and non-empty) MUST have identical height/baseline.
    #[test]
    fn test_l2_all_lines_same_height() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        // Mixed document: "a\n\n\nb" has content, empty, empty, content lines
        let para = Paragraph::new("a\n\n\nb", TextStyle::default());
        let params = LayoutParams::default();

        let lines = ParagraphLayoutEngine::layout(&para, &shaper, &params);

        assert_eq!(lines.len(), 4, "Should have 4 lines");

        // Verify all lines have identical height and baseline
        let canonical_height = params.canonical_metrics.ascent + params.canonical_metrics.descent;
        let canonical_baseline = params.canonical_metrics.ascent;

        for (i, line) in lines.iter().enumerate() {
            assert!(
                (line.height - canonical_height).abs() < 0.001,
                "L2.INV2: Line {} height ({}) MUST equal canonical height ({})",
                i,
                line.height,
                canonical_height
            );
            assert!(
                (line.baseline - canonical_baseline).abs() < 0.001,
                "L2.INV3: Line {} baseline ({}) MUST equal canonical baseline ({})",
                i,
                line.baseline,
                canonical_baseline
            );
        }
    }

    /// L2.INV4: Content height must be stable across empty/non-empty transitions.
    /// Simulates: typing first glyph into empty line should not change content height.
    #[test]
    fn test_l2_no_vertical_reflow_on_content_change() {
        let Some(shaper) = make_test_shaper() else {
            return;
        };

        let params = LayoutParams::default();

        // Document with only empty line
        let para_empty = Paragraph::new("\n", TextStyle::default());
        let lines_empty = ParagraphLayoutEngine::layout(&para_empty, &shaper, &params);

        // Document with content on that line
        let para_content = Paragraph::new("a\n", TextStyle::default());
        let lines_content = ParagraphLayoutEngine::layout(&para_content, &shaper, &params);

        // Both should have same number of lines
        assert_eq!(
            lines_empty.len(),
            lines_content.len(),
            "Line count must match"
        );

        // Total content height must be identical
        let height_empty: f32 = lines_empty.iter().map(|l| l.height).sum();
        let height_content: f32 = lines_content.iter().map(|l| l.height).sum();

        assert!(
            (height_empty - height_content).abs() < 0.001,
            "L2.INV4: Content height after typing ({}) MUST equal empty height ({})",
            height_content,
            height_empty
        );
    }
}
