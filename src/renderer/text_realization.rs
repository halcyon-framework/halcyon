//! SPEC-HAL-360: Compile-local text demand and surface realization evidence.

use std::collections::{HashMap, HashSet};

use crate::compositor::SurfaceId;
use crate::core::text_engine::{GlyphRun, TextMetrics};
use crate::platform::backend_text_engine::BackendTextEngine;
use crate::platform::native_font_backend::{GlyphBitmap, GlyphFormat, NativeFontBackend};
use crate::renderer::FrameEpoch;
use crate::renderer::batcher::{FlatPrimitive, PrimitiveData};
use crate::renderer::glyph_atlas::{GlyphAtlas, GlyphKey};
use crate::renderer::gpu_arena::GpuResourceArena;

/// The stage/reason that prevented complete text preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRealizationFailureReason {
    ShapingUnavailable,
    InvalidTypographyInput(crate::core::text_engine::TextInputError),
    MissingGlyph,
    RasterizationUnavailable,
    InvalidBitmap,
    AtlasCapacity,
    AtlasResidency,
}

/// Bounded evidence: indices refer to this surface's frozen primitive/glyph order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRealizationFailure {
    pub primitive_index: usize,
    pub glyph_index: Option<usize>,
    pub reason: TextRealizationFailureReason,
}

/// Preparation evidence only; it does not certify GPU execution or presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRealizationOutcome {
    Complete {
        primitive_count: usize,
        glyph_count: usize,
        no_ink_glyph_count: usize,
    },
    Degraded {
        primitive_count: usize,
        glyph_count: usize,
        no_ink_glyph_count: usize,
        missing_cluster_count: usize,
        first_failure: TextRealizationFailure,
    },
    Unavailable(TextRealizationFailure),
}

impl TextRealizationOutcome {
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    pub fn is_renderable(&self) -> bool {
        matches!(self, Self::Complete { .. } | Self::Degraded { .. })
    }
}

/// Immutable evidence for one surface in one compiled plan's preparation frame.
/// Copying this value does not extend the plan's resource lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceTextRealization {
    surface_id: SurfaceId,
    frame_epoch: FrameEpoch,
    outcome: TextRealizationOutcome,
    markers_prepared: bool,
}

impl SurfaceTextRealization {
    pub fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    pub fn frame_epoch(&self) -> FrameEpoch {
        self.frame_epoch
    }

    pub fn outcome(&self) -> &TextRealizationOutcome {
        &self.outcome
    }

    pub(crate) fn is_prepared_for_output(&self) -> bool {
        self.outcome.is_renderable()
            && (!matches!(self.outcome, TextRealizationOutcome::Degraded { .. })
                || self.markers_prepared)
    }

    pub(crate) fn new(
        surface_id: SurfaceId,
        frame_epoch: FrameEpoch,
        outcome: TextRealizationOutcome,
    ) -> Self {
        Self {
            surface_id,
            frame_epoch,
            outcome,
            markers_prepared: false,
        }
    }
}

pub(crate) struct TextRunDemand {
    pub primitive_index: usize,
    pub glyph_run: GlyphRun,
    pub keys: Vec<GlyphKey>,
    pub metrics: TextMetrics,
    pub ink: Vec<Option<crate::core::text_engine::source_ink::GlyphInk>>,
    /// Compile-local only; never changes keys, source bytes or advances.
    pub suppressed: Vec<bool>,
    pub markers: Vec<(f32, f32)>,
}

#[derive(Default)]
pub(crate) struct SurfaceTextDemand {
    pub runs: Vec<TextRunDemand>,
    pub no_ink: HashSet<GlyphKey>,
    pub failure: Option<TextRealizationFailure>,
    pub first_missing: Option<TextRealizationFailure>,
    pub markers_prepared: bool,
}

enum RasterClassification {
    NoInk,
    Bitmap(GlyphBitmap),
}

/// HAL360 §7.3: six geometry-only quads, with no change to the source advance.
pub(crate) fn missing_cluster_rects(
    origin: [f32; 2],
    interval: (f32, f32),
    metrics: TextMetrics,
    scale: f32,
) -> Option<[crate::core::math::RectF; 6]> {
    use crate::core::math::RectF;
    let (x0, x1) = interval;
    if ![
        origin[0],
        origin[1],
        x0,
        x1,
        metrics.ascent,
        metrics.descent,
        scale,
    ]
    .iter()
    .all(|v| v.is_finite())
        || scale <= 0.0
        || x1 < x0
        || metrics.ascent < 0.0
        || metrics.descent < 0.0
    {
        return None;
    }
    let minimum = 2.0 / scale;
    let width = (x1 - x0).max(minimum);
    let height = (metrics.ascent + metrics.descent).max(minimum);
    let x = origin[0] + x0;
    let y = origin[1] + metrics.ascent + metrics.descent - height;
    let t = (1.0 / scale).min(width * 0.5).min(height * 0.5);
    let rects = [
        RectF::new(x, y, width, t),
        RectF::new(x, y + height - t, width, t),
        RectF::new(x, y + t, t, height - 2.0 * t),
        RectF::new(x + width - t, y + t, t, height - 2.0 * t),
        RectF::new(x + width * 0.5 - t * 0.5, y + t, t, height - 2.0 * t),
        RectF::new(x + t, y + height * 0.5 - t * 0.5, width - 2.0 * t, t),
    ];
    (minimum > 0.0
        && minimum.is_finite()
        && rects.iter().all(|r| {
            [r.x, r.y, r.width, r.height, r.x + r.width, r.y + r.height]
                .iter()
                .all(|v| v.is_finite())
                && r.width >= 0.0
                && r.height >= 0.0
        }))
    .then_some(rects)
}

fn classify_bitmap(
    key: &GlyphKey,
    bitmap: Option<GlyphBitmap>,
) -> Result<RasterClassification, TextRealizationFailureReason> {
    use TextRealizationFailureReason as Reason;
    if key.glyph_id == 0 {
        return Err(Reason::MissingGlyph);
    }
    let bitmap = bitmap.ok_or(Reason::RasterizationUnavailable)?;
    let metrics = bitmap.metrics;
    if bitmap.key != *key
        || bitmap.format != key.format
        || ![
            metrics.advance,
            metrics.bearing_x,
            metrics.bearing_y,
            metrics.ascender,
            metrics.descender,
            metrics.line_height,
        ]
        .iter()
        .all(|value| value.is_finite())
    {
        return Err(Reason::InvalidBitmap);
    }
    if metrics.width == 0 && metrics.height == 0 && bitmap.pixels.is_empty() {
        return Ok(RasterClassification::NoInk);
    }
    let channels = match bitmap.format {
        GlyphFormat::Grayscale => 1,
        GlyphFormat::SubpixelRgb => 4,
    };
    let expected = (metrics.width as usize)
        .checked_mul(metrics.height as usize)
        .and_then(|pixels| pixels.checked_mul(channels));
    if metrics.width == 0 || metrics.height == 0 || expected != Some(bitmap.pixels.len()) {
        return Err(Reason::InvalidBitmap);
    }
    Ok(RasterClassification::Bitmap(bitmap))
}

impl SurfaceTextDemand {
    pub fn gather<B: NativeFontBackend>(
        primitives: &[FlatPrimitive],
        engine: &mut BackendTextEngine<B>,
        scale: f32,
    ) -> Self {
        let mut demand = Self::default();
        let previous_scale = engine.scale();
        engine.set_scale(scale);
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let PrimitiveData::Text {
                text,
                font_size,
                binding,
                position,
                ..
            } = &primitive.data
            else {
                continue;
            };
            let shaping_failure = !font_size.is_finite()
                || *font_size <= 0.0
                || !scale.is_finite()
                || scale <= 0.0
                || (!text.is_empty() && !engine.has_shaper());
            if shaping_failure {
                demand.fail(
                    primitive_index,
                    None,
                    TextRealizationFailureReason::ShapingUnavailable,
                );
                break;
            }
            let (glyph_run, keys, metrics) = if let Some(binding) = binding {
                match engine
                    .shape_bound_with_keys(text, *font_size, binding)
                    .and_then(|(run, keys)| {
                        engine
                            .metrics_bound(*font_size, binding)
                            .map(|metrics| (run, keys, metrics))
                    }) {
                    Ok(shaped) => shaped,
                    Err(error) => {
                        demand.fail(
                            primitive_index,
                            None,
                            TextRealizationFailureReason::InvalidTypographyInput(error),
                        );
                        break;
                    }
                }
            } else {
                let (run, keys) = engine.shape_with_keys(text, *font_size);
                (run, keys, engine.metrics_for_scale(*font_size, scale))
            };
            if glyph_run.glyphs.len() != keys.len() || (!text.is_empty() && keys.is_empty()) {
                demand.fail(
                    primitive_index,
                    None,
                    TextRealizationFailureReason::ShapingUnavailable,
                );
                break;
            }
            let ink = match binding.as_ref().and_then(|binding| binding.source_ink()) {
                Some(paint) => {
                    use crate::core::text_engine::{TextEngine, TextInputError, TextRangeEffectError};
                    match engine
                        .font_face_catalog()
                        .ok_or(TextRangeEffectError::UnqualifiedRun)
                        .and_then(|catalog| paint.realize(&glyph_run, &catalog, scale))
                    {
                        Ok(ink) => ink,
                        Err(reason) => {
                            let reason = match reason {
                                TextRangeEffectError::MissingGlyph => {
                                    TextRealizationFailureReason::MissingGlyph
                                }
                                TextRangeEffectError::InvalidGeometry => {
                                    TextRealizationFailureReason::InvalidTypographyInput(
                                        TextInputError::PositionOverflow,
                                    )
                                }
                                _ => TextRealizationFailureReason::InvalidTypographyInput(
                                    TextInputError::UnsupportedTypography,
                                ),
                            };
                            demand.fail(primitive_index, None, reason);
                            break;
                        }
                    }
                }
                None => Vec::new(),
            };
            let mut suppressed = Vec::new();
            let mut markers = Vec::new();
            if let Some(glyph_index) = keys.iter().position(|key| key.glyph_id == 0) {
                let failure = TextRealizationFailure {
                    primitive_index,
                    glyph_index: Some(glyph_index),
                    reason: TextRealizationFailureReason::MissingGlyph,
                };
                let Some(ranges) = engine.verified_glyph_source_ranges(&glyph_run) else {
                    demand.fail(
                        primitive_index,
                        Some(glyph_index),
                        TextRealizationFailureReason::ShapingUnavailable,
                    );
                    break;
                };
                let missing: HashSet<_> = keys
                    .iter()
                    .zip(&ranges)
                    .filter(|(key, _)| key.glyph_id == 0)
                    .map(|(_, range)| range.clone())
                    .collect();
                suppressed = ranges.iter().map(|range| missing.contains(range)).collect();
                let mut seen = HashSet::new();
                for cluster in &glyph_run.clusters {
                    if missing.contains(&cluster.source) && seen.insert(cluster.source.clone()) {
                        markers.push((
                            cluster.x_start.min(cluster.x_end),
                            cluster.x_start.max(cluster.x_end),
                        ));
                    }
                }
                if markers.len() != missing.len()
                    || ![
                        position.x,
                        position.y,
                        metrics.ascent,
                        metrics.descent,
                        metrics.line_height,
                        metrics.ascent + metrics.descent,
                        2.0 / scale,
                    ]
                    .iter()
                    .all(|v| v.is_finite())
                    || metrics.ascent < 0.0
                    || metrics.descent < 0.0
                {
                    demand.fail(
                        primitive_index,
                        Some(glyph_index),
                        TextRealizationFailureReason::ShapingUnavailable,
                    );
                    break;
                }
                demand.first_missing.get_or_insert(failure);
            }
            demand.runs.push(TextRunDemand {
                primitive_index,
                glyph_run,
                keys,
                metrics,
                ink,
                suppressed,
                markers,
            });
        }
        engine.set_scale(previous_scale);
        demand
    }

    pub fn outcome(&self) -> TextRealizationOutcome {
        if let Some(failure) = self.failure {
            return TextRealizationOutcome::Unavailable(failure);
        }
        let primitive_count = self.runs.len();
        let glyph_count = self.runs.iter().map(|run| run.keys.len()).sum();
        let no_ink_glyph_count = self
            .runs
            .iter()
            .flat_map(|run| &run.keys)
            .filter(|key| self.no_ink.contains(key))
            .count();
        if let Some(first_failure) = self.first_missing {
            if !self.markers_prepared {
                return TextRealizationOutcome::Unavailable(first_failure);
            }
            return TextRealizationOutcome::Degraded {
                primitive_count,
                glyph_count,
                no_ink_glyph_count,
                missing_cluster_count: self.runs.iter().map(|run| run.markers.len()).sum(),
                first_failure,
            };
        }
        TextRealizationOutcome::Complete {
            primitive_count,
            glyph_count,
            no_ink_glyph_count,
        }
    }

    pub(crate) fn realization(
        &self,
        surface: SurfaceId,
        epoch: FrameEpoch,
    ) -> SurfaceTextRealization {
        SurfaceTextRealization {
            surface_id: surface,
            frame_epoch: epoch,
            outcome: self.outcome(),
            markers_prepared: self.markers_prepared,
        }
    }

    pub fn fail(
        &mut self,
        primitive_index: usize,
        glyph_index: Option<usize>,
        reason: TextRealizationFailureReason,
    ) {
        self.failure.get_or_insert(TextRealizationFailure {
            primitive_index,
            glyph_index,
            reason,
        });
    }

    /// Classify the whole candidate before atlas writes, then protect its working
    /// set plus previously admitted surfaces. UVs are intentionally not copied here.
    pub fn admit<B: NativeFontBackend>(
        &mut self,
        backend: &mut B,
        scale: f32,
        atlas: &mut GlyphAtlas,
        protected: &mut HashSet<GlyphKey>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        arena: &mut GpuResourceArena,
        epoch: FrameEpoch,
    ) -> u32 {
        if self.failure.is_some() {
            return 0;
        }
        let mut keys = Vec::new();
        let mut seen = HashSet::new();
        let mut bitmaps = HashMap::new();
        let mut rasterizations = 0;
        for run in &self.runs {
            for (glyph_index, key) in run.keys.iter().enumerate() {
                if run.suppressed.get(glyph_index) == Some(&true) {
                    continue;
                }
                if !seen.insert(*key) {
                    continue;
                }
                if atlas.get(key).is_none() {
                    let classification = classify_bitmap(key, backend.rasterize_glyph(key, scale));
                    rasterizations += 1;
                    match classification {
                        Ok(RasterClassification::NoInk) => {
                            self.no_ink.insert(*key);
                            continue;
                        }
                        Ok(RasterClassification::Bitmap(bitmap)) => {
                            bitmaps.insert(*key, bitmap);
                        }
                        Err(reason) => {
                            self.failure = Some(TextRealizationFailure {
                                primitive_index: run.primitive_index,
                                glyph_index: Some(glyph_index),
                                reason,
                            });
                            return rasterizations;
                        }
                    }
                }
                keys.push((*key, run.primitive_index, glyph_index));
            }
        }
        let added: Vec<_> = keys
            .iter()
            .map(|(key, _, _)| *key)
            .filter(|key| protected.insert(*key))
            .collect();
        for (key, primitive_index, glyph_index) in keys {
            if let Some(bitmap) = bitmaps.remove(&key) {
                if atlas
                    .insert_glyph_protected(
                        key,
                        &bitmap.pixels,
                        bitmap.format,
                        bitmap.metrics,
                        2.0,
                        device,
                        queue,
                        arena,
                        epoch,
                        protected,
                    )
                    .is_none()
                {
                    self.fail(
                        primitive_index,
                        Some(glyph_index),
                        TextRealizationFailureReason::AtlasCapacity,
                    );
                    for key in added {
                        protected.remove(&key);
                    }
                    break;
                }
            }
        }
        rasterizations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text_engine::Glyph;
    use crate::renderer::glyph_atlas::GlyphMetrics;

    #[test]
    fn complete_text_realization_marker_geometry_keeps_fractional_origin_and_minimum() {
        let metrics = TextMetrics {
            ascent: 8.0,
            descent: 2.0,
            line_gap: 0.0,
            line_height: 10.0,
        };
        for scale in [1.0, 2.0] {
            let rects = missing_cluster_rects([4.25, 8.5], (3.125, 3.125), metrics, scale).unwrap();
            assert_eq!(rects[0].x, 7.375);
            assert_eq!(rects[0].width, 2.0 / scale);
            assert_eq!(rects[0].height, 1.0 / scale);
            assert_eq!(rects[0].y, 8.5);
        }
        assert!(missing_cluster_rects([f32::NAN, 0.0], (0.0, 0.0), metrics, 1.0).is_none());
        assert!(
            missing_cluster_rects([f32::MAX, 0.0], (f32::MAX, f32::MAX), metrics, 1.0).is_none()
        );
        assert!(missing_cluster_rects([0.0, 0.0], (0.0, 0.0), metrics, 1.0e-40).is_none());
    }

    fn bitmap(key: GlyphKey, width: u32, height: u32, pixels: Vec<u8>) -> GlyphBitmap {
        GlyphBitmap {
            key,
            pixels,
            format: GlyphFormat::Grayscale,
            metrics: GlyphMetrics {
                advance: 8.0,
                bearing_x: 0.0,
                bearing_y: 8.0,
                width,
                height,
                ascender: 8.0,
                descender: 2.0,
                line_height: 10.0,
            },
        }
    }

    #[test]
    fn complete_text_realization_classifies_no_ink_and_backend_storage_without_silent_loss() {
        let key = GlyphKey::new(1, 16.0, 3, 1.0);
        assert!(matches!(
            classify_bitmap(&key, Some(bitmap(key, 0, 0, Vec::new()))),
            Ok(RasterClassification::NoInk)
        ));
        assert!(matches!(
            classify_bitmap(&key, None),
            Err(TextRealizationFailureReason::RasterizationUnavailable)
        ));
        let missing = GlyphKey { glyph_id: 0, ..key };
        assert!(matches!(
            classify_bitmap(&missing, Some(bitmap(missing, 0, 0, Vec::new()))),
            Err(TextRealizationFailureReason::MissingGlyph)
        ));
        // The old CoreText no-ink shape had positive height but only one byte.
        assert!(matches!(
            classify_bitmap(&key, Some(bitmap(key, 1, 20, vec![0]))),
            Err(TextRealizationFailureReason::InvalidBitmap)
        ));
        assert!(matches!(
            classify_bitmap(&key, Some(bitmap(key, 2, 2, vec![0; 4]))),
            Ok(RasterClassification::Bitmap(_))
        ));
    }

    struct UnavailableBackend;

    impl NativeFontBackend for UnavailableBackend {
        fn font_metrics(&mut self, _: f32, _: f32) -> TextMetrics {
            TextMetrics {
                ascent: 8.0,
                descent: 2.0,
                line_gap: 0.0,
                line_height: 10.0,
            }
        }
        fn rasterize_glyph(&mut self, _: &GlyphKey, _: f32) -> Option<GlyphBitmap> {
            None
        }
    }

    #[test]
    fn complete_text_realization_backend_failure_refuses_surface_before_atlas_mutation() {
        let renderer =
            pollster::block_on(crate::renderer::wgpu_renderer::WgpuRenderer::new_headless())
                .expect("SPEC-HAL-360 GPU required");
        let gpu = renderer.gpu_resources();
        let epoch = renderer.frame_epoch();
        let mut arena = GpuResourceArena::new();
        let mut atlas = GlyphAtlas::new(&gpu.device, 64, 64, &mut arena, epoch);
        let mut backend = UnavailableBackend;
        let key = GlyphKey::new(1, 16.0, 3, 1.0);
        let mut demand = SurfaceTextDemand {
            runs: vec![TextRunDemand {
                primitive_index: 7,
                glyph_run: GlyphRun {
                    binding: None,
                    clusters: Vec::new(),
                    glyphs: vec![Glyph {
                        id: 3,
                        x: 0.0,
                        y: 0.0,
                        advance: 8.0,
                    }],
                    width: 8.0,
                    height: 10.0,
                    text: "A".into(),
                    font_size: 16.0,
                    ascent: 8.0,
                    descent: 2.0,
                },
                keys: vec![key],
                metrics: backend.font_metrics(16.0, 1.0),
                ink: Vec::new(),
                suppressed: Vec::new(),
                markers: Vec::new(),
            }],
            ..Default::default()
        };
        let mut protected = HashSet::new();
        assert_eq!(
            demand.admit(
                &mut backend,
                1.0,
                &mut atlas,
                &mut protected,
                &gpu.device,
                &gpu.queue,
                &mut arena,
                epoch
            ),
            1
        );
        assert_eq!(
            demand.outcome(),
            TextRealizationOutcome::Unavailable(TextRealizationFailure {
                primitive_index: 7,
                glyph_index: Some(0),
                reason: TextRealizationFailureReason::RasterizationUnavailable,
            })
        );
        assert!(protected.is_empty());
        assert!(atlas.get(&key).is_none());
    }
}
