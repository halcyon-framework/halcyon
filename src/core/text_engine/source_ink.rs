//! HAL-048 scoped foreground presentation of a complete, non-editing text run.

use super::{ClusterGap, GlyphRun, TextBinding, TextEngine, verify_run_source_ranges};
use crate::core::math::{Point, RectF};
use crate::core::paint::{Color, PaintSink, ZIndex};
use crate::platform::font_manager::{CatalogFace, FontFaceCatalog};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::platform::native_font_backend_coretext::CoreTextOutlineReader;
use sha2::{Digest, Sha256};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use std::cell::OnceCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Weak};
use ttf_parser::{Face, GlyphId};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextRangeEffectError {
    #[error("the text engine does not support source range effects")]
    UnsupportedEngine,
    #[error("the supplied run cannot be reproduced with its admitted face and metrics")]
    UnqualifiedRun,
    #[error("the range must be nonempty and inside the run at UTF-8 boundaries")]
    InvalidRange,
    #[error("the range splits an extended grapheme or a shaped source cluster")]
    SplitCluster,
    #[error("the run contains a missing glyph")]
    MissingGlyph,
    #[error("the selected glyph ink has no supported outline")]
    UnsupportedInk,
    #[error("scaling the range requires visible ink")]
    NoInk,
    #[error("the ink geometry is not finite")]
    InvalidGeometry,
    #[error("the foreground must have finite RGBA components in 0..=1")]
    InvalidColor,
    #[error("the ink scale must be finite and in 0.90..=1.15")]
    InvalidScale,
    #[error("the effect does not belong to this run and current font catalog")]
    StaleGeometry,
    #[error("source range effects must have disjoint ranges")]
    OverlappingRanges,
}

/// An original source cluster and its unscaled outline bounds, if it has ink.
#[derive(Debug)]
pub struct TextSourceClusterInk {
    source: Range<usize>,
    ink: Option<RectF>,
}

impl TextSourceClusterInk {
    pub fn source(&self) -> Range<usize> {
        self.source.clone()
    }

    pub fn ink_bounds(&self) -> Option<RectF> {
        self.ink
    }
}

/// Snapshot of the complete clusters covered by a qualified UTF-8 range.
///
/// Rectangles are local to the run's top-left text origin. Use them in visual
/// order for hit testing and reacquire them after the original geometry changes.
#[derive(Debug)]
pub struct QualifiedTextClusterRange {
    source: Range<usize>,
    clusters: Vec<TextSourceClusterInk>,
    ink: Option<RectF>,
}

impl QualifiedTextClusterRange {
    pub fn source(&self) -> Range<usize> {
        self.source.clone()
    }

    pub fn clusters(&self) -> &[TextSourceClusterInk] {
        &self.clusters
    }

    pub fn ink_bounds(&self) -> Option<RectF> {
        self.ink
    }

    pub fn pivot(&self) -> Option<Point> {
        self.ink.map(ink_center)
    }
}

/// Logical paint values. The text engine validates them against the supplied
/// complete run before publishing them to the scene.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSourceRangeEffect {
    pub source: Range<usize>,
    pub foreground: Color,
    pub scale: f32,
}

fn check_paint(foreground: Color, scale: f32) -> Result<(), TextRangeEffectError> {
    if ![foreground.r, foreground.g, foreground.b, foreground.a]
        .into_iter()
        .all(|value| (0.0..=1.0).contains(&value))
    {
        return Err(TextRangeEffectError::InvalidColor);
    }
    if !(0.90..=1.15).contains(&scale) {
        return Err(TextRangeEffectError::InvalidScale);
    }
    Ok(())
}

pub(crate) fn qualify_source_range(
    engine: &mut dyn TextEngine,
    run: &GlyphRun,
    source: Range<usize>,
    scale: f32,
) -> Result<QualifiedTextClusterRange, TextRangeEffectError> {
    let catalog = engine
        .font_face_catalog()
        .ok_or(TextRangeEffectError::UnsupportedEngine)?;
    let qualified = QualifiedRun::for_engine(engine, run, &catalog, scale)?;
    qualified.check_range(&source)?;
    let mut result = QualifiedTextClusterRange {
        source,
        clusters: Vec::new(),
        ink: None,
    };
    let mut offset = 0;
    for cluster in qualified.sources.chunk_by(|left, right| left == right) {
        let indices = offset..offset + cluster.len();
        offset = indices.end;
        if !result.source.contains(&cluster[0].start) {
            continue;
        }
        let mut ink = None;
        for index in indices {
            if let Some(rect) = qualified.glyph_ink(index)? {
                extend_ink(&mut ink, rect)?;
            }
        }
        if let Some(rect) = ink {
            extend_ink(&mut result.ink, rect)?;
        }
        result.clusters.push(TextSourceClusterInk {
            source: cluster[0].clone(),
            ink,
        });
    }
    Ok(result)
}

/// Local glyph outline boxes in visual order, qualified for one supplied run.
/// Use `TextEngine::qualify_source_range` for source-cluster rectangles.
pub struct SourceInkGeometry<'run> {
    run: &'run GlyphRun,
    catalog: Weak<HashMap<u64, CatalogFace>>,
    range: Range<usize>,
    boxes: Vec<RectF>,
}

impl std::fmt::Debug for SourceInkGeometry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceInkGeometry")
            .field("range", &self.range)
            .field("boxes", &self.boxes)
            .finish_non_exhaustive()
    }
}

impl<'run> SourceInkGeometry<'run> {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn ink_boxes(&self) -> &[RectF] {
        &self.boxes
    }

    pub(crate) fn qualify(
        engine: &mut dyn TextEngine,
        run: &'run GlyphRun,
        range: Range<usize>,
        scale: f32,
    ) -> Result<Self, TextRangeEffectError> {
        let catalog = engine
            .font_face_catalog()
            .ok_or(TextRangeEffectError::UnsupportedEngine)?;
        let qualified = QualifiedRun::for_engine(engine, run, &catalog, scale)?;
        qualified.check_range(&range)?;
        let mut boxes = Vec::new();
        for (index, source) in qualified.sources.iter().enumerate() {
            if range.contains(&source.start)
                && let Some(ink) = qualified.glyph_ink(index)?
            {
                boxes.push(ink);
            }
        }
        if boxes.is_empty() {
            return Err(TextRangeEffectError::NoInk);
        }
        Ok(Self {
            run,
            catalog: catalog.weak_faces(),
            range,
            boxes,
        })
    }
}

/// A foreground effect borrowed from outline geometry on the same run.
#[derive(Debug)]
pub struct SourceInkEffect<'run> {
    geometry: &'run SourceInkGeometry<'run>,
    color: Color,
    scale: f32,
}

impl<'run> SourceInkEffect<'run> {
    pub fn try_new(
        geometry: &'run SourceInkGeometry<'run>,
        color: Color,
        scale: f32,
    ) -> Result<Self, TextRangeEffectError> {
        check_paint(color, scale)?;
        Ok(Self {
            geometry,
            color,
            scale,
        })
    }

    pub(super) fn for_run(
        &self,
        run: &GlyphRun,
        catalog: &FontFaceCatalog,
    ) -> Result<TextSourceRangeEffect, TextRangeEffectError> {
        let geometry = self.geometry;
        if !std::ptr::eq(run, geometry.run) || !catalog.weak_faces().ptr_eq(&geometry.catalog) {
            return Err(TextRangeEffectError::StaleGeometry);
        }
        Ok(TextSourceRangeEffect {
            source: geometry.range.clone(),
            foreground: self.color,
            scale: self.scale,
        })
    }
}

pub(crate) fn draw_source_range_effects(
    engine: &mut dyn TextEngine,
    run: &GlyphRun,
    origin: Point,
    color: Color,
    z_index: ZIndex,
    effects: &[TextSourceRangeEffect],
    sink: &mut dyn PaintSink,
    scale: f32,
) -> Result<(), TextRangeEffectError> {
    if effects.is_empty() {
        engine.draw(run, origin.x, origin.y, color, z_index, sink);
        return Ok(());
    }
    if !(origin.x * scale).is_finite() || !(origin.y * scale).is_finite() {
        return Err(TextRangeEffectError::InvalidGeometry);
    }
    let catalog = engine
        .font_face_catalog()
        .ok_or(TextRangeEffectError::UnsupportedEngine)?;
    let qualified = QualifiedRun::for_engine(engine, run, &catalog, scale)?;
    let paint = SourceInkPaint::new(&qualified, effects)?;
    let mut styled = run.clone();
    styled.binding.as_mut().expect("qualified run").source_ink = Some(Arc::new(paint));
    engine.draw(&styled, origin.x, origin.y, color, z_index, sink);
    Ok(())
}

/// Only logical values cross Scene. Provenance ties them to the admitted full run.
#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct SourceInkPaint {
    source: [u8; 32],
    font_size: u32,
    face_id: u64,
    face_digest: [u8; 32],
    gap: ClusterGap,
    rtl: Option<bool>,
    effects: Vec<LogicalEffect>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct LogicalEffect {
    range: Range<usize>,
    rgba: [u32; 4],
    scale: u32,
}

impl SourceInkPaint {
    fn new(
        run: &QualifiedRun<'_>,
        effects: &[TextSourceRangeEffect],
    ) -> Result<Self, TextRangeEffectError> {
        let mut values = Vec::with_capacity(effects.len());
        for effect in effects {
            check_paint(effect.foreground, effect.scale)?;
            run.check_range(&effect.source)?;
            let Color { r, g, b, a } = effect.foreground;
            values.push(LogicalEffect {
                range: effect.source.clone(),
                rgba: [r, g, b, a].map(f32::to_bits),
                scale: effect.scale.to_bits(),
            });
        }
        values.sort_unstable_by_key(|effect| effect.range.start);
        if values
            .windows(2)
            .any(|pair| pair[0].range.end > pair[1].range.start)
        {
            return Err(TextRangeEffectError::OverlappingRanges);
        }
        let binding = run.binding;
        let paint = Self {
            source: Sha256::digest(run.run.text.as_bytes()).into(),
            font_size: run.run.font_size.to_bits(),
            face_id: binding.face().font_id(),
            face_digest: binding.face().digest,
            gap: binding.cluster_gap(),
            rtl: binding.rtl,
            effects: values,
        };
        paint.prepare(run, |_, _| {})?;
        Ok(paint)
    }

    /// Independently reproduce attribution from current whole-run shaping during Compile.
    pub(crate) fn realize(
        &self,
        run: &GlyphRun,
        catalog: &FontFaceCatalog,
        scale: f32,
    ) -> Result<Vec<Option<GlyphInk>>, TextRangeEffectError> {
        let source: [u8; 32] = Sha256::digest(run.text.as_bytes()).into();
        let qualified = QualifiedRun::new(run, catalog, scale)?;
        let binding = qualified.binding;
        if self.source != source
            || self.font_size != run.font_size.to_bits()
            || self.face_id != binding.face().font_id()
            || self.face_digest != binding.face().digest
            || self.gap != binding.cluster_gap()
            || self.rtl != binding.rtl
        {
            return Err(TextRangeEffectError::StaleGeometry);
        }
        for effect in &self.effects {
            qualified.check_range(&effect.range)?;
        }
        let mut selected = vec![None; run.glyphs.len()];
        let paints = self.prepare(&qualified, |glyph, effect| selected[glyph] = Some(effect))?;
        Ok(selected
            .into_iter()
            .map(|index| index.map(|i| paints[i]))
            .collect())
    }

    fn prepare(
        &self,
        run: &QualifiedRun<'_>,
        mut select: impl FnMut(usize, usize),
    ) -> Result<Vec<GlyphInk>, TextRangeEffectError> {
        let mut bounds = vec![None; self.effects.len()];
        for (glyph, source) in run.sources.iter().enumerate() {
            let index = self
                .effects
                .partition_point(|effect| effect.range.end <= source.start);
            if let Some(effect) = self.effects.get(index)
                && effect.range.contains(&source.start)
                && let Some(rect) = run.glyph_ink(glyph)?
            {
                extend_ink(&mut bounds[index], rect)?;
                select(glyph, index);
            }
        }
        self.effects
            .iter()
            .zip(bounds)
            .map(|(effect, bounds)| {
                let [r, g, b, a] = effect.rgba.map(f32::from_bits);
                let scale = f32::from_bits(effect.scale);
                let transform = if scale == 1.0 {
                    None
                } else {
                    Some((
                        ink_center(bounds.ok_or(TextRangeEffectError::NoInk)?),
                        scale,
                    ))
                };
                Ok(GlyphInk {
                    color: Color { r, g, b, a },
                    transform,
                })
            })
            .collect()
    }
}

/// Prepared foreground projection for a glyph in this Compile only.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GlyphInk {
    pub color: Color,
    transform: Option<(Point, f32)>,
}

impl GlyphInk {
    /// Reuse existing oversampled atlas coverage and leave other quads unchanged.
    pub(crate) fn project(self, rect: RectF, origin: Point) -> Option<RectF> {
        let Some((center, scale)) = self.transform else {
            return Some(rect);
        };
        let x = origin.x + center.x;
        let y = origin.y + center.y;
        let rect = RectF::new(
            x + (rect.x - x) * scale,
            y + (rect.y - y) * scale,
            rect.width * scale,
            rect.height * scale,
        );
        [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]
            .into_iter()
            .all(f32::is_finite)
            .then_some(rect)
    }
}

fn ink_center(rect: RectF) -> Point {
    Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

fn extend_ink(bounds: &mut Option<RectF>, rect: RectF) -> Result<(), TextRangeEffectError> {
    let Some(previous) = *bounds else {
        *bounds = Some(rect);
        return Ok(());
    };
    let x = previous.x.min(rect.x);
    let y = previous.y.min(rect.y);
    let width = (previous.x + previous.width).max(rect.x + rect.width) - x;
    let height = (previous.y + previous.height).max(rect.y + rect.height) - y;
    if !width.is_finite() || !height.is_finite() {
        return Err(TextRangeEffectError::InvalidGeometry);
    }
    *bounds = Some(RectF::new(x, y, width, height));
    Ok(())
}

struct QualifiedRun<'run> {
    run: &'run GlyphRun,
    binding: &'run TextBinding,
    face: Face<'run>,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    outline_reader: OnceCell<Option<CoreTextOutlineReader>>,
    sources: Vec<Range<usize>>,
    cluster_starts: Vec<usize>,
    grapheme_starts: Vec<usize>,
}

impl<'run> QualifiedRun<'run> {
    fn for_engine(
        engine: &mut dyn TextEngine,
        run: &'run GlyphRun,
        catalog: &FontFaceCatalog,
        scale: f32,
    ) -> Result<Self, TextRangeEffectError> {
        let qualified = Self::new(run, catalog, scale)?;
        let metrics = engine
            .metrics_bound_text(run.font_size, qualified.binding)
            .map_err(|_| TextRangeEffectError::UnqualifiedRun)?;
        if (metrics.ascent, metrics.descent, metrics.line_height)
            != (run.ascent, run.descent, run.height)
        {
            return Err(TextRangeEffectError::UnqualifiedRun);
        }
        Ok(qualified)
    }

    fn new(
        run: &'run GlyphRun,
        catalog: &FontFaceCatalog,
        scale: f32,
    ) -> Result<Self, TextRangeEffectError> {
        let binding = run
            .binding
            .as_ref()
            .ok_or(TextRangeEffectError::UnqualifiedRun)?;
        let admitted = catalog
            .get(binding.face().font_id())
            .filter(|face| &face.binding == binding.face() && binding.source_ink().is_none())
            .ok_or(TextRangeEffectError::UnqualifiedRun)?;
        let sources = verify_run_source_ranges(run, &admitted.shaper, scale)
            .ok_or(TextRangeEffectError::UnqualifiedRun)?;
        if run.glyphs.iter().any(|glyph| glyph.id == 0) {
            return Err(TextRangeEffectError::MissingGlyph);
        }
        let face = Face::parse(binding.face().data(), 0)
            .map_err(|_| TextRangeEffectError::UnqualifiedRun)?;
        let mut cluster_starts: Vec<_> = run
            .clusters
            .iter()
            .map(|cluster| cluster.source.start)
            .collect();
        cluster_starts.sort_unstable();
        cluster_starts.push(run.text.len());
        let mut grapheme_starts: Vec<_> = run.text.grapheme_indices(true).map(|(i, _)| i).collect();
        grapheme_starts.push(run.text.len());
        Ok(Self {
            run,
            binding,
            face,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            outline_reader: OnceCell::new(),
            sources,
            cluster_starts,
            grapheme_starts,
        })
    }

    fn check_range(&self, range: &Range<usize>) -> Result<(), TextRangeEffectError> {
        if range.is_empty() || self.run.text.get(range.clone()).is_none() {
            return Err(TextRangeEffectError::InvalidRange);
        }
        if [range.start, range.end].into_iter().any(|boundary| {
            self.cluster_starts.binary_search(&boundary).is_err()
                || self.grapheme_starts.binary_search(&boundary).is_err()
        }) {
            return Err(TextRangeEffectError::SplitCluster);
        }
        Ok(())
    }

    fn glyph_ink(&self, index: usize) -> Result<Option<RectF>, TextRangeEffectError> {
        let glyph = &self.run.glyphs[index];
        let id =
            GlyphId(u16::try_from(glyph.id).map_err(|_| TextRangeEffectError::UnqualifiedRun)?);
        if self.face.is_color_glyph(id)
            || self.face.glyph_svg_image(id).is_some()
            || self.face.glyph_raster_image(id, u16::MAX).is_some()
        {
            return Err(TextRangeEffectError::UnsupportedInk);
        }
        let rect = if let Some(bounds) = self.face.glyph_bounding_box(id) {
            let unit = self.run.font_size / f32::from(self.face.units_per_em());
            RectF::new(
                glyph.x + f32::from(bounds.x_min) * unit,
                self.run.ascent + glyph.y - f32::from(bounds.y_max) * unit,
                (f32::from(bounds.x_max) - f32::from(bounds.x_min)) * unit,
                (f32::from(bounds.y_max) - f32::from(bounds.y_min)) * unit,
            )
        } else {
            let text = &self.run.text[self.sources[index].clone()];
            if self.face.glyph_index(' ') == Some(id) || text.chars().all(char::is_whitespace) {
                return Ok(None);
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            return Err(TextRangeEffectError::UnsupportedInk);

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                let reader = self
                    .outline_reader
                    .get_or_init(|| {
                        CoreTextOutlineReader::new(self.binding.face().data(), self.run.font_size)
                    })
                    .as_ref()
                    .ok_or(TextRangeEffectError::UnsupportedInk)?;
                let bounds = reader
                    .glyph_bounds(id.0)
                    .ok_or(TextRangeEffectError::UnsupportedInk)?;
                let y_max = bounds.origin.y + bounds.size.height;
                RectF::new(
                    glyph.x + bounds.origin.x as f32,
                    self.run.ascent + glyph.y - y_max as f32,
                    bounds.size.width as f32,
                    bounds.size.height as f32,
                )
            }
        };
        if ![rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]
            .into_iter()
            .all(f32::is_finite)
            || rect.width <= 0.0
            || rect.height <= 0.0
        {
            return Err(TextRangeEffectError::InvalidGeometry);
        }
        Ok(Some(rect))
    }
}
