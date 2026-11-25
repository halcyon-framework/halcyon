use crate::core::math::{Point, RectF};
use crate::core::paint::{Color, PaintSink, TextBackgroundSpan, ZIndex};
use crate::core::text_shaper::{SpacedShape, TextShaper};
use std::sync::Arc;

pub(crate) mod source_ink;
pub use source_ink::TextRangeEffectError as SourceInkError;
pub use source_ink::{
    QualifiedTextClusterRange, SourceInkEffect, SourceInkGeometry, TextRangeEffectError,
    TextSourceClusterInk, TextSourceRangeEffect,
};
pub use crate::core::text_shaper::{ClusterGap, SourceCluster, TextInputError};

/// An admitted immutable face, without native handles or shaped text.
#[derive(Debug, Clone)]
pub struct FontFaceBinding {
    font_id: u64,
    data: Arc<Vec<u8>>,
    digest: [u8; 32],
    family_name: String,
    weight: crate::platform::font_manager::FontWeight,
    italic: bool,
}

impl FontFaceBinding {
    pub(crate) fn new(
        font_id: u64,
        data: Arc<Vec<u8>>,
        family_name: String,
        weight: crate::platform::font_manager::FontWeight,
        italic: bool,
    ) -> Self {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(data.as_slice()).into();
        Self {
            font_id,
            data,
            digest,
            family_name,
            weight,
            italic,
        }
    }

    pub fn font_id(&self) -> u64 {
        self.font_id
    }
    pub fn data(&self) -> &Arc<Vec<u8>> {
        &self.data
    }
    pub fn family_name(&self) -> &str {
        &self.family_name
    }
    pub fn weight(&self) -> crate::platform::font_manager::FontWeight {
        self.weight
    }
    pub fn italic(&self) -> bool {
        self.italic
    }
}

impl PartialEq for FontFaceBinding {
    fn eq(&self, other: &Self) -> bool {
        self.font_id == other.font_id
            && self.digest == other.digest
            && self.family_name == other.family_name
            && self.weight == other.weight
            && self.italic == other.italic
    }
}
impl Eq for FontFaceBinding {}
impl std::hash::Hash for FontFaceBinding {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.font_id.hash(state);
        self.digest.hash(state);
        self.family_name.hash(state);
        self.weight.hash(state);
        self.italic.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextBinding {
    face: FontFaceBinding,
    gap: ClusterGap,
    // HAL049 §15: inherited paragraph direction, opaque outside the text owner.
    rtl: Option<bool>,
    // Immutable foreground values for the logical scene run. Shaping ignores them.
    source_ink: Option<Arc<source_ink::SourceInkPaint>>,
}

impl TextBinding {
    pub(crate) fn new(face: FontFaceBinding, gap: ClusterGap) -> Self {
        Self {
            face,
            gap,
            rtl: None,
            source_ink: None,
        }
    }
    pub(crate) fn with_direction(
        mut self,
        direction: crate::core::text_shaper::TextDirection,
    ) -> Self {
        self.rtl = Some(direction == crate::core::text_shaper::TextDirection::RightToLeft);
        self
    }
    pub(crate) fn direction_for(&self, text: &str) -> crate::core::text_shaper::TextDirection {
        use crate::core::text_shaper::TextDirection;
        match self.rtl {
            Some(true) => TextDirection::RightToLeft,
            Some(false) => TextDirection::LeftToRight,
            None => TextDirection::detect(text),
        }
    }
    pub fn face(&self) -> &FontFaceBinding {
        &self.face
    }
    pub fn cluster_gap(&self) -> ClusterGap {
        self.gap
    }

    pub(crate) fn source_ink(&self) -> Option<&source_ink::SourceInkPaint> {
        self.source_ink.as_deref()
    }

    pub(crate) fn shape_binding(&self) -> std::borrow::Cow<'_, Self> {
        if self.source_ink.is_none() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut binding = self.clone();
        binding.source_ink = None;
        std::borrow::Cow::Owned(binding)
    }
}

/// The actual face is independent of the product's family/weight request.
#[derive(Debug, Clone)]
pub struct ResolvedTextStyle {
    pub binding: TextBinding,
    pub font_size: f32,
}

pub(crate) fn bound_glyph_run(
    text: &str,
    style: &ResolvedTextStyle,
    shaper: &TextShaper,
    metrics: TextMetrics,
    scale: f32,
) -> Result<GlyphRun, TextInputError> {
    let shaped = shaper.shape_with_gap_direction(
        text,
        style.font_size,
        scale,
        style.binding.cluster_gap(),
        style.binding.direction_for(text),
    )?;
    bound_glyph_run_from_shape(text, style, shaped, metrics, scale)
}

pub(crate) fn bound_glyph_run_from_shape(
    text: &str,
    style: &ResolvedTextStyle,
    shaped: SpacedShape,
    metrics: TextMetrics,
    scale: f32,
) -> Result<GlyphRun, TextInputError> {
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
    let mut pen = 0.0_f32;
    let glyphs = shaped
        .glyphs
        .iter()
        .map(|glyph| {
            let result = Glyph {
                id: glyph.glyph_id,
                x: (pen + glyph.x_offset_phys) / scale,
                y: glyph.y_offset_phys / scale,
                advance: glyph.advance_phys / scale,
            };
            pen += glyph.advance_phys;
            result
        })
        .collect();
    Ok(GlyphRun {
        binding: Some(style.binding.clone()),
        clusters: shaped.clusters,
        glyphs,
        width: shaped.width,
        height: metrics.line_height,
        text: text.to_string(),
        font_size: style.font_size,
        ascent: metrics.ascent,
        descent: metrics.descent,
    })
}

/// Qualify ephemeral glyph/source attribution against the exact complete run.
/// This never changes shaping input or retains glyphs beyond the caller's phase.
pub(crate) fn glyph_source_ranges(
    run: &GlyphRun,
    shaper: Option<&TextShaper>,
    scale: f32,
) -> Option<Vec<std::ops::Range<usize>>> {
    verify_run_source_ranges(run, shaper?, scale)
}

/// Internal whole-run verifier shared by the existing and scoped range owners.
fn verify_run_source_ranges(
    run: &GlyphRun,
    shaper: &TextShaper,
    scale: f32,
) -> Option<Vec<std::ops::Range<usize>>> {
    let binding = run.binding.as_ref()?;
    if ![
        run.font_size,
        run.width,
        run.height,
        run.ascent,
        run.descent,
        scale,
    ]
    .iter()
    .all(|value| value.is_finite())
        || run.font_size <= 0.0
        || scale <= 0.0
    {
        return None;
    }
    let data = shaper.font_data();
    if !Arc::ptr_eq(&data, binding.face().data())
        && data.as_slice() != binding.face().data().as_slice()
    {
        return None;
    }
    let face = ttf_parser::Face::parse(binding.face().data(), 0).ok()?;
    if shaper.units_per_em() != face.units_per_em() {
        return None;
    }
    let shaped = shaper
        .shape_with_gap_direction(
            &run.text,
            run.font_size,
            scale,
            binding.cluster_gap(),
            binding.direction_for(&run.text),
        )
        .ok()?;
    if shaped.glyphs.len() != run.glyphs.len()
        || shaped.width != run.width
        || shaped.clusters != run.clusters
    {
        return None;
    }
    let mut sources = std::collections::BTreeMap::new();
    for cluster in &shaped.clusters {
        if cluster.source.start >= cluster.source.end
            || cluster.source.end > run.text.len()
            || !run.text.is_char_boundary(cluster.source.start)
            || !run.text.is_char_boundary(cluster.source.end)
            || !cluster.x_start.is_finite()
            || !cluster.x_end.is_finite()
        {
            return None;
        }
        if let Some(previous) = sources.insert(cluster.source.start, cluster.source.clone()) {
            if previous != cluster.source {
                return None;
            }
        }
    }
    let mut covered = 0;
    for range in sources.values() {
        if range.start != covered {
            return None;
        }
        covered = range.end;
    }
    if covered != run.text.len() {
        return None;
    }
    let mut pen = 0.0_f32;
    let mut ranges = Vec::with_capacity(shaped.glyphs.len());
    for (raw, glyph) in shaped.glyphs.iter().zip(&run.glyphs) {
        let x = (pen + raw.x_offset_phys) / scale;
        let y = raw.y_offset_phys / scale;
        let advance = raw.advance_phys / scale;
        if ![x, y, advance, glyph.x, glyph.y, glyph.advance]
            .iter()
            .all(|value| value.is_finite())
            || glyph.id != raw.glyph_id
            || glyph.x != x
            || glyph.y != y
            || glyph.advance != advance
        {
            return None;
        }
        pen += raw.advance_phys;
        ranges.push(sources.get(&(raw.cluster as usize))?.clone());
    }
    Some(ranges)
}

/// Exact immutable font backing for re-realizable presentation text. This holds
/// font bytes, never a shaped run, glyph atlas address or compiled text quad.
#[derive(Debug, Clone)]
pub struct TextGenerationLease {
    font_id: u64,
    data: Arc<Vec<u8>>,
}

impl TextGenerationLease {
    pub(crate) fn new(font_id: u64, data: Arc<Vec<u8>>) -> Self {
        Self { font_id, data }
    }

    pub(crate) fn same_generation(&self, other: &Self) -> bool {
        self.font_id == other.font_id && Arc::ptr_eq(&self.data, &other.data)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.data.capacity() + std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>()
    }
}

/// A single shaped glyph with position information
#[derive(Debug, Clone)]
pub struct Glyph {
    /// Glyph ID from the font
    pub id: u32,
    /// X position relative to text origin
    pub x: f32,
    /// Y position relative to text origin
    pub y: f32,
    /// Horizontal advance to next glyph
    pub advance: f32,
}

/// A shaped run of glyphs ready for rendering
#[derive(Debug, Clone)]
pub struct GlyphRun {
    /// Logical immutable input for renderer re-shaping, never glyph storage.
    pub binding: Option<TextBinding>,
    /// Ephemeral visual cluster mapping for selection; never emitted to SceneGraph.
    pub clusters: Vec<SourceCluster>,
    /// Ephemeral shaped glyphs in visual order, using logical coordinates.
    pub glyphs: Vec<Glyph>,
    /// Total width of the text run
    pub width: f32,
    /// Total height of the text run
    pub height: f32,
    /// Original text (needed for rendering)
    pub text: String,
    /// Font size used for shaping
    pub font_size: f32,
    /// Ascent from baseline (positive, upward)
    pub ascent: f32,
    /// Descent from baseline (positive, downward)
    pub descent: f32,
}

// =============================================================================
// TEXT PLACEMENT INVARIANTS (Engine-Grade)
// =============================================================================
//
// INVARIANT 1: Single Semantic Meaning
//   `draw(run, x, y)` always interprets `y` as TOP OF TEXT BOX.
//   The renderer is solely responsible for baseline placement.
//   Callers MUST NEVER add ascent/descent manually.
//
// INVARIANT 2: Forbidden Naming
//   Variables named `baseline_y` are FORBIDDEN at widget call sites.
//   Use: `text_top_y`, `line_top_y`, or the helper function below.
//   Baseline is an internal renderer concern only.
//
// INVARIANT 3: Centralized Helper
//   Use `text_top_y_in_rect()` for vertical centering.
//   No widget should compute this manually.
//
// INVARIANT 4: Debug Detection
//   In debug builds, misuse will be caught by code review and grep audits.
//   Pattern to detect: `run.ascent` used near `draw()` calls.
//
// =============================================================================

/// Compute text top Y for vertical centering within a rectangle.
///
/// # Deprecation Notice
/// This function centers based on **actual run height** (`run.height`), which
/// varies with text content. For controls (Button, TextField, Tag, etc.),
/// use [`compute_control_text_top_y`] instead, which uses consistent theme
/// typography for proper baseline alignment across controls.
///
/// # When to Use This Function
/// - List cells or table cells with variable content height
/// - Non-control contexts where content-dependent centering is intentional
/// - Legacy code that hasn't migrated to control text placement
///
/// # When NOT to Use This Function
/// - Button, TextField, Tag, Dropdown, or any theme-defined control
/// - When consistent baseline alignment is required across widgets
///
/// # Usage
/// ```ignore
/// let text_y = text_top_y_in_rect(&rect, &run);
/// cx.text_engine.draw(&run, x, text_y, color, z_index, scene);
/// ```
///
/// # FORBIDDEN
/// - Adding `run.ascent` to the result
/// - Adding `run.descent` to the result
/// - Manual baseline calculations at call sites
#[inline]
#[deprecated(
    since = "0.1.3",
    note = "For controls, use compute_control_text_top_y() instead. \
            This function is retained for list cells and content-dependent centering."
)]
pub fn text_top_y_in_rect(rect: &RectF, run: &GlyphRun) -> f32 {
    rect.y + (rect.height - run.height) / 2.0
}

/// Compute text top Y for vertical centering given explicit coordinates.
///
/// Use this when you have Y and height values but not a full RectF.
#[inline]
pub fn text_top_y_centered(container_y: f32, container_height: f32, text_height: f32) -> f32 {
    container_y + (container_height - text_height) / 2.0
}

/// Compute text top Y offset for vertical centering within a control.
///
/// # Deprecation Notice
/// **DEPRECATED for control text placement.** Use [`compute_control_text_baseline_y`]
/// instead, which uses actual font metrics (ascent + descent) rather than line_height.
///
/// This function centers based on `line_height`, but the renderer positions glyphs
/// using `ascent`. This mismatch causes glyphs to visually touch or cross the
/// control's top edge even when the math appears correct.
///
/// # When to Still Use This Function
/// - Layout spacing calculations (line box stacking, multiline flow)
/// - Non-control contexts where line_height is the correct abstraction
///
/// # When NOT to Use This Function
/// - Button, TextField, Tag, Dropdown, or any control text placement
/// - When glyph containment within control bounds is required
///
/// Per SPEC-HAL-092 Section 8 (Control Text Placement) - **SUPERSEDED by Section 8A**:
/// ```ignore
/// text_top_y = (control_height - line_height) / 2 + baseline_correction
/// ```
#[inline]
#[deprecated(
    since = "0.1.4",
    note = "For control text placement, use compute_control_text_baseline_y() instead. \
            line_height centering causes glyph-to-edge clipping. \
            This function is retained for layout spacing calculations only."
)]
pub fn compute_control_text_top_y(
    control_height: f32,
    line_height: f32,
    baseline_correction: f32,
) -> f32 {
    (control_height - line_height) / 2.0 + baseline_correction
}

/// Compute baseline Y for control text placement.
///
/// # INV-TYPO-CTRL-1 (SPEC-HAL-092 Section 8A)
/// Control text vertical placement MUST be computed from actual font metrics
/// (ascent + descent), not from line_height.
///
/// # INV-TYPO-CTRL-2
/// The glyph bounding box (ascent + descent) MUST be fully contained within the
/// control's content rect.
///
/// # Contract
/// - `baseline_y` is the Y coordinate where glyph baseline is placed.
/// - Glyphs (ascent above + descent below baseline) fit inside control content box.
/// - This function is the ONLY allowed text placement entry for controls.
///
/// # Inputs (logical space)
/// - `control_height`: Height of the control in logical units
/// - `ascent`: Font ascent in logical units (from TextMetrics)
/// - `descent`: Font descent in logical units (from TextMetrics)
/// - `baseline_offset`: Vertical adjustment from theme (default 0)
///
/// # Output
/// - Baseline Y position relative to control top edge (logical units)
///
/// # Usage
/// ```ignore
/// let metrics = cx.text_engine.font_metrics(font_size);
/// let baseline_y = rect.y + compute_control_text_baseline_y(
///     rect.height,
///     metrics.ascent,
///     metrics.descent,
///     cx.theme.typography.baseline_offset,
/// );
/// // draw() takes top-of-text-box, so subtract ascent from baseline
/// let text_top_y = baseline_y - metrics.ascent;
/// cx.text_engine.draw(&run, x, text_top_y, color, z_index, scene);
/// ```
///
/// # CANONICAL HELPER (SPEC-HAL-092 Section 8A)
/// All controls (Button, TextField, Tag, Dropdown, etc.) MUST use this function.
/// No widget should compute control text placement manually.
/// Using line_height for control placement is architecturally invalid.
#[inline]
pub fn compute_control_text_baseline_y(
    control_height: f32,
    ascent: f32,
    descent: f32,
    baseline_offset: f32,
) -> f32 {
    // Center actual glyph box (ascent + descent), not line_height
    let glyph_height = ascent + descent;
    let top = (control_height - glyph_height) * 0.5;
    // Baseline = top of glyph box + ascent
    top + ascent + baseline_offset
}

/// Minimum content row that contains the font under the existing optical baseline.
pub(crate) fn compute_control_line_height(
    configured: f32,
    metrics: &TextMetrics,
    baseline_offset: f32,
) -> f32 {
    configured
        .max(metrics.line_height)
        .max(metrics.ascent + metrics.descent + 2.0 * baseline_offset.abs())
}

/// Font metrics for accurate layout calculations.
///
/// # INVARIANT: All values are in LOGICAL PIXELS
///
/// - `ascent`, `descent`, `line_gap`, `line_height` are logical px
/// - Metrics scale proportionally with font_size (also logical)
/// - Physical scaling is renderer's job, not layout's
/// - Layout owns all vertical positioning; renderer is placement-dumb
#[derive(Debug, Clone, Copy)]
pub struct TextMetrics {
    /// Distance from baseline to top of tallest glyph (logical px, positive)
    pub ascent: f32,
    /// Distance from baseline to bottom of lowest descender (logical px, positive)
    pub descent: f32,
    /// Additional spacing between lines (logical px)
    pub line_gap: f32,
    /// Total line height: ascent + descent + line_gap (logical px)
    pub line_height: f32,
}

/// Core text rendering abstraction
/// Backends can implement this for different text engines (glyphon, cosmic-text, etc.)
pub trait TextEngine {
    /// Active scale used by this engine to shape text. Use it for
    /// `LayoutParams::scale` when reproducing catalog paragraph runs with this engine.
    ///
    /// Returns `UnsupportedTypography` when the engine cannot report its scale,
    /// or `InvalidScale` when its current scale is nonfinite or nonpositive.
    fn current_shaping_scale(&self) -> Result<f32, TextInputError> {
        Err(TextInputError::UnsupportedTypography)
    }

    /// HAL049 §15: immutable admitted faces; absence preserves legacy capability.
    fn font_face_catalog(&self) -> Option<crate::platform::font_manager::FontFaceCatalog> {
        None
    }

    fn shape_bound_text(
        &mut self,
        _text: &str,
        _font_size: f32,
        _binding: &TextBinding,
    ) -> Result<GlyphRun, TextInputError> {
        Err(TextInputError::UnsupportedTypography)
    }

    /// Engine implementation of `ParagraphLayoutEngine::shape_laid_out_run`
    /// with access to the current shaping scale.
    fn shape_laid_out_paragraph_run(
        &mut self,
        _paragraph: &crate::core::paragraph::Paragraph,
        _run: &crate::core::paragraph::Run,
    ) -> Result<GlyphRun, TextInputError> {
        Err(TextInputError::UnsupportedTypography)
    }

    fn metrics_bound_text(
        &mut self,
        _font_size: f32,
        _binding: &TextBinding,
    ) -> Result<TextMetrics, TextInputError> {
        Err(TextInputError::UnsupportedTypography)
    }

    fn resolve_text_style(
        &self,
        _style: &crate::core::paragraph::TextStyle,
    ) -> Result<ResolvedTextStyle, TextInputError> {
        Err(TextInputError::UnsupportedTypography)
    }

    fn shape_styled(
        &mut self,
        text: &str,
        style: Option<&crate::core::paragraph::TextStyle>,
        default_size: f32,
    ) -> Result<GlyphRun, TextInputError> {
        match style {
            None => Ok(self.shape(text, default_size)),
            Some(_) => Err(TextInputError::UnsupportedTypography),
        }
    }

    fn metrics_styled(
        &mut self,
        style: Option<&crate::core::paragraph::TextStyle>,
        default_size: f32,
    ) -> Result<TextMetrics, TextInputError> {
        match style {
            None => Ok(self.font_metrics(default_size)),
            Some(_) => Err(TextInputError::UnsupportedTypography),
        }
    }
    /// Absent means historical text is not admitted by this text owner.
    fn presentation_text_generation(&self) -> Option<TextGenerationLease> {
        None
    }

    /// Measure text dimensions without full shaping
    /// Returns (width, height) in logical pixels
    fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32);

    /// Get font metrics for accurate layout calculations
    /// Returns ascent, descent, and line_height in logical pixels
    fn metrics(&mut self, text: &str, font_size: f32) -> TextMetrics;

    /// Get font-global metrics independent of text content.
    /// Returns consistent ascent/descent/line_height for any font size.
    /// Use this for layout calculations where metrics must not vary with text content.
    fn font_metrics(&mut self, font_size: f32) -> TextMetrics {
        // Default implementation delegates to metrics with empty string
        self.metrics("", font_size)
    }

    /// Shape text into positioned glyphs
    /// Performs full layout, BiDi, shaping, etc.
    fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun;

    /// Draw a shaped glyph run into the scene graph
    /// x, y specify the top-left origin position (not baseline).
    /// `z_index` is required producer authority and is forwarded unchanged.
    /// Baseline-origin semantics will be introduced in the typography overhaul.
    fn draw(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        z_index: ZIndex,
        sink: &mut dyn PaintSink,
    );

    /// Qualify a UTF-8 range against the complete current bound run. Each
    /// returned cluster has its original source range and optional unscaled ink.
    fn qualify_source_range(
        &mut self,
        _run: &GlyphRun,
        _utf8: std::ops::Range<usize>,
    ) -> Result<QualifiedTextClusterRange, TextRangeEffectError> {
        Err(TextRangeEffectError::UnsupportedEngine)
    }

    /// Validate and paint one complete bound run with disjoint source effects.
    /// Positions and advances come from the original run. An empty effect list
    /// uses ordinary drawing. A rejected request emits no text node.
    fn draw_with_source_range_effects(
        &mut self,
        run: &GlyphRun,
        origin: Point,
        color: Color,
        z_index: ZIndex,
        effects: &[TextSourceRangeEffect],
        sink: &mut dyn PaintSink,
    ) -> Result<(), TextRangeEffectError> {
        if effects.is_empty() {
            self.draw(run, origin.x, origin.y, color, z_index, sink);
            return Ok(());
        }
        Err(TextRangeEffectError::UnsupportedEngine)
    }

    /// Qualify whole source clusters in a current, already shaped non-editable run.
    /// Boxes are local to the same top-left text origin accepted by `draw`.
    fn source_ink_geometry<'run>(
        &mut self,
        _run: &'run GlyphRun,
        _range: std::ops::Range<usize>,
    ) -> Result<SourceInkGeometry<'run>, SourceInkError> {
        Err(SourceInkError::UnsupportedEngine)
    }

    /// Draw one complete run with optional checked, disjoint foreground effects.
    /// An empty effect list uses the ordinary draw path. Failure emits no text node.
    fn draw_with_source_ink(
        &mut self,
        run: &GlyphRun,
        origin: Point,
        color: Color,
        z_index: ZIndex,
        effects: &[SourceInkEffect<'_>],
        sink: &mut dyn PaintSink,
    ) -> Result<(), SourceInkError> {
        if effects.is_empty() {
            self.draw(run, origin.x, origin.y, color, z_index, sink);
            return Ok(());
        }
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return Err(SourceInkError::InvalidGeometry);
        }
        let catalog = self
            .font_face_catalog()
            .ok_or(SourceInkError::UnsupportedEngine)?;
        let effects = effects
            .iter()
            .map(|effect| effect.for_run(run, &catalog))
            .collect::<Result<Vec<_>, _>>()?;
        self.draw_with_source_range_effects(run, origin, color, z_index, &effects, sink)
    }

    /// SPEC-EXIT-A1: Draw a shaped glyph run with background spans.
    ///
    /// Background spans are rendered BEFORE glyphs in the text pass, ensuring
    /// selection highlights and carets appear behind text without z-index hacks.
    /// The producer supplies the shared Z value for that ordered run.
    ///
    /// x, y specify the top-left origin position. Background span offsets are
    /// relative to this position.
    fn draw_with_backgrounds(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        backgrounds: Vec<TextBackgroundSpan>,
        z_index: ZIndex,
        sink: &mut dyn PaintSink,
    );

    /// Optional access to the underlying TextShaper implementation.
    ///
    /// Default is None. Engines that own a shaper (like FontManagerTextEngine)
    /// override this to expose their shaper for paragraph layout.
    fn shaper(&self) -> Option<&TextShaper> {
        None
    }
}

// =============================================================================
// TEXT DEBUG OVERLAY (Visual Truth Probe)
// =============================================================================
//
// Enable via: HALCYON_TEXT_DEBUG=1
//
// Draws diagnostic lines for every text placement:
// - Control rect bounds (cyan border)
// - Computed baseline (green line)
// - Glyph box top/bottom derived from ascent/descent (magenta lines)
// =============================================================================

/// Check if text debug overlay is enabled via environment variable.
#[inline]
pub fn text_debug_enabled() -> bool {
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("HALCYON_TEXT_DEBUG")
            .map(|v| v == "1")
            .unwrap_or(false)
    })
}

/// Draw text placement debug overlay.
///
/// Call this after drawing text to visualize the placement geometry.
///
/// # Arguments
/// * `scene` - Scene builder to draw into
/// * `control_rect` - The control's content rect (bounding box)
/// * `text_top_y` - The Y coordinate passed to draw() (top of text box)
/// * `baseline_y` - The computed baseline Y
/// * `metrics` - Font metrics used for placement (ascent, descent)
/// * `layer` - Z-layer for overlay (should be above text)
pub fn draw_text_debug_overlay(
    sink: &mut dyn PaintSink,
    control_rect: RectF,
    text_top_y: f32,
    baseline_y: f32,
    metrics: &TextMetrics,
    layer: u32,
) {
    if !text_debug_enabled() {
        return;
    }

    let cyan = Color::from_srgb(0.0, 1.0, 1.0, 0.8);
    let green = Color::from_srgb(0.0, 1.0, 0.0, 0.8);
    let magenta = Color::from_srgb(1.0, 0.0, 1.0, 0.6);

    // Control rect bounds (cyan border) - draw as 4 edge rects
    let bw = 1.0; // border width
    // Top edge
    sink.push_rect(
        RectF::new(control_rect.x, control_rect.y, control_rect.width, bw),
        cyan,
        layer,
        None,
    );
    // Bottom edge
    sink.push_rect(
        RectF::new(
            control_rect.x,
            control_rect.y + control_rect.height - bw,
            control_rect.width,
            bw,
        ),
        cyan,
        layer,
        None,
    );
    // Left edge
    sink.push_rect(
        RectF::new(control_rect.x, control_rect.y, bw, control_rect.height),
        cyan,
        layer,
        None,
    );
    // Right edge
    sink.push_rect(
        RectF::new(
            control_rect.x + control_rect.width - bw,
            control_rect.y,
            bw,
            control_rect.height,
        ),
        cyan,
        layer,
        None,
    );

    // Baseline (green line)
    sink.push_rect(
        RectF::new(control_rect.x, baseline_y, control_rect.width, 1.0),
        green,
        layer + 1,
        None,
    );

    // Glyph box top (magenta line) - derived from text_top_y
    sink.push_rect(
        RectF::new(control_rect.x, text_top_y, control_rect.width, 1.0),
        magenta,
        layer,
        None,
    );

    // Glyph box bottom (magenta line) - baseline + descent
    let glyph_bottom_y = baseline_y + metrics.descent;
    sink.push_rect(
        RectF::new(control_rect.x, glyph_bottom_y, control_rect.width, 1.0),
        magenta,
        layer,
        None,
    );
}
