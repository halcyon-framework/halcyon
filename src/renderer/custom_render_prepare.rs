use crate::compositor::SurfaceId;
use crate::core::frame_realization::AppFrameAttemptId;
use crate::core::math::{RectF, Size};
use crate::renderer::batcher::{ClipState, DrawBatchKey, FlatPrimitive, PrimitiveData};
use crate::renderer::custom_render_pass::{
    CustomRenderPresentationBinding, CustomRenderPresentationId,
};
use crate::renderer::pixel_coverage::{
    PhysicalExtent, PhysicalRect, PhysicalTargetMetrics, PixelCoverageError,
    physical_composition_coverage, physical_render_extent,
};
use crate::three_d::PbrSourceResult3D;
use crate::three_d::spatial_hud::{HudCompileCharge, PreparedHud3D};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Target-dependent custom-render activity for one participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomRenderActivity {
    Active {
        physical_render_extent: PhysicalExtent,
        physical_composition_coverage: PhysicalRect,
    },
    Inactive,
}

/// Immutable post-layout metrics supplied to an admitted custom-render participant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomRenderTargetMetrics {
    pub surface_id: SurfaceId,
    /// Canonical App attempt association; None for noncanonical preparation.
    pub app_frame_attempt: Option<crate::core::frame_realization::AppFrameAttemptId>,
    pub final_logical_rect: RectF,
    pub final_logical_viewport: RectF,
    pub target_logical_extent: Size,
    pub scale_factor: f32,
    pub target_physical_extent: PhysicalExtent,
    pub activity: CustomRenderActivity,
}

/// Participant-local result consumed by the retained-scene presentation gate.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CustomRenderPreparationOutcome {
    pub front_publication_changed: bool,
    publication: Option<CustomRenderPublication>,
    pub(crate) plot_evidence: Vec<crate::plot::PlotPreparationEvidence>,
}

impl std::fmt::Debug for CustomRenderPreparationOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomRenderPreparationOutcome")
            .field("front_publication_changed", &self.front_publication_changed)
            .field("presentation_binding", &self.presentation_binding())
            .finish()
    }
}

impl CustomRenderPreparationOutcome {
    pub const fn new(front_publication_changed: bool) -> Self {
        Self {
            front_publication_changed,
            publication: None,
            plot_evidence: Vec::new(),
        }
    }

    pub const fn front_publication_changed() -> Self {
        Self::new(true)
    }

    pub const fn with_presentation_binding(
        front_publication_changed: bool,
        presentation_binding: CustomRenderPresentationBinding,
    ) -> Self {
        Self {
            front_publication_changed,
            publication: Some(CustomRenderPublication {
                binding: presentation_binding,
                companion: None,
            }),
            plot_evidence: Vec::new(),
        }
    }

    pub const fn presentation_binding(&self) -> Option<CustomRenderPresentationBinding> {
        match &self.publication {
            Some(publication) => Some(publication.binding),
            None => None,
        }
    }

    pub(crate) fn into_publication(self) -> Option<CustomRenderPublication> {
        self.publication
    }

    pub(crate) fn attach_spatial_hud_companion(
        &mut self,
        prepared: Arc<PreparedHud3D>,
        metrics: CustomRenderTargetMetrics,
    ) -> PbrSourceResult3D<()> {
        let publication = self.publication.as_mut().expect("qualified Image binding");
        debug_assert_eq!(publication.binding.texture_id(), prepared.texture_id);
        debug_assert!(!publication.binding.canonical_over_neutral());
        debug_assert_eq!(metrics.surface_id, prepared.metrics.surface_id);
        let bytes = std::mem::size_of::<PreparedCompanion>()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of::<CompanionRecord>();
        let charge = prepared.reserve_cpu(bytes as u64)?;
        publication.companion = Some(Arc::new(PreparedCompanion {
            prepared,
            metrics,
            _charge: charge,
        }));
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct CustomRenderPublication {
    binding: CustomRenderPresentationBinding,
    companion: Option<Arc<PreparedCompanion>>,
}

impl PartialEq for CustomRenderPublication {
    fn eq(&self, other: &Self) -> bool {
        self.binding == other.binding
            && match (&self.companion, &other.companion) {
                (Some(a), Some(b)) => Arc::ptr_eq(&a.prepared, &b.prepared),
                (None, None) => true,
                _ => false,
            }
    }
}

impl Eq for CustomRenderPublication {}

struct PreparedCompanion {
    prepared: Arc<PreparedHud3D>,
    metrics: CustomRenderTargetMetrics,
    _charge: HudCompileCharge,
}

type PresentationKey = (CustomRenderPresentationId, SurfaceId);

struct PresentationRecord {
    binding: CustomRenderPresentationBinding,
    companion: Option<Box<CompanionRecord>>,
}

struct CompanionRecord {
    payload: Arc<PreparedCompanion>,
    occurrence: ImageOccurrence,
    next_recorded: Option<PresentationKey>,
}

enum ImageOccurrence {
    Unseen,
    Unique(ImageOccurrenceReceipt),
    Ambiguous,
}

pub(crate) struct ImageOccurrenceReceipt {
    pub(crate) order_key: u32,
    pub(crate) batch_key: DrawBatchKey,
    pub(crate) clip: ClipState,
    pub(crate) rect: RectF,
    pub(crate) scale_factor: f32,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CustomRenderPreparationError {
    #[error(transparent)]
    PixelCoverage(#[from] PixelCoverageError),
    #[error(
        "custom-render participant returned presentation identity {actual:?}, expected {expected:?}"
    )]
    PresentationIdentityMismatch {
        expected: CustomRenderPresentationId,
        actual: CustomRenderPresentationId,
    },
    #[error(
        "custom-render presentation {presentation_id:?} published more than once for target {surface_id:?}"
    )]
    DuplicatePresentationBinding {
        presentation_id: CustomRenderPresentationId,
        surface_id: SurfaceId,
    },
}

/// Renderer-owned per-frame bindings populated by authenticated participants.
#[derive(Default)]
pub(crate) struct CustomRenderPresentationBindings {
    entries: HashMap<PresentationKey, PresentationRecord>,
    recorded_head: Option<PresentationKey>,
}

impl CustomRenderPresentationBindings {
    pub(crate) fn begin_frame(&mut self) -> PresentationFrame<'_> {
        debug_assert!(self.entries.is_empty());
        PresentationFrame { bindings: self }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.recorded_head = None;
    }

    pub(crate) fn publish_for_target(
        &mut self,
        surface_id: SurfaceId,
        expected: CustomRenderPresentationId,
        publication: impl Into<CustomRenderPublication>,
    ) -> Result<(), CustomRenderPreparationError> {
        let publication = publication.into();
        let actual = publication.binding.presentation_id();
        if actual != expected {
            return Err(CustomRenderPreparationError::PresentationIdentityMismatch {
                expected,
                actual,
            });
        }
        match self.entries.entry((expected, surface_id)) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(PresentationRecord {
                    binding: publication.binding,
                    companion: publication.companion.map(|payload| {
                        Box::new(CompanionRecord {
                            payload,
                            occurrence: ImageOccurrence::Unseen,
                            next_recorded: None,
                        })
                    }),
                });
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(CustomRenderPreparationError::DuplicatePresentationBinding {
                    presentation_id: expected,
                    surface_id,
                })
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn resolve(
        &self,
        presentation_id: CustomRenderPresentationId,
        surface_id: SurfaceId,
    ) -> Option<u64> {
        audit::record_presentation_lookup();
        self.entries
            .get(&(presentation_id, surface_id))
            .map(|record| record.binding.texture_id())
    }

    #[cfg(test)]
    pub(crate) fn reset_for_test(&mut self) {
        self.clear();
    }

    #[cfg(test)]
    pub(crate) fn prepared_companion_for_test(
        &self,
        id: CustomRenderPresentationId,
        surface: SurfaceId,
    ) -> Option<&Arc<PreparedHud3D>> {
        Some(
            &self
                .entries
                .get(&(id, surface))?
                .companion
                .as_ref()?
                .payload
                .prepared,
        )
    }

    #[cfg(test)]
    pub(crate) fn begin_materialization_for_test(
        &mut self,
        build_frame: u64,
    ) -> ImageOccurrenceRecorder<'_> {
        PresentationFrame { bindings: self }.begin_materialization(build_frame, None)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.entries.capacity()
    }
}

#[cfg(test)]
impl From<CustomRenderPresentationBinding> for CustomRenderPublication {
    fn from(binding: CustomRenderPresentationBinding) -> Self {
        Self {
            binding,
            companion: None,
        }
    }
}

pub(crate) struct PresentationFrame<'frame> {
    bindings: &'frame mut CustomRenderPresentationBindings,
}

impl<'frame> PresentationFrame<'frame> {
    pub(crate) fn preparation(&mut self) -> &mut CustomRenderPresentationBindings {
        self.bindings
    }

    pub(crate) fn begin_materialization(
        self,
        build_frame: u64,
        attempt: Option<AppFrameAttemptId>,
    ) -> ImageOccurrenceRecorder<'frame> {
        ImageOccurrenceRecorder {
            frame: self,
            build_frame,
            attempt,
        }
    }
}

impl Drop for PresentationFrame<'_> {
    fn drop(&mut self) {
        self.bindings.clear();
    }
}

pub(crate) struct ImageOccurrenceRecorder<'frame> {
    frame: PresentationFrame<'frame>,
    build_frame: u64,
    attempt: Option<AppFrameAttemptId>,
}

impl<'frame> ImageOccurrenceRecorder<'frame> {
    pub(crate) fn resolve_target_bound(
        &mut self,
        id: CustomRenderPresentationId,
        surface: SurfaceId,
    ) -> Option<ResolvedTargetImage<'_>> {
        #[cfg(test)]
        audit::record_presentation_lookup();
        let key = (id, surface);
        let bindings = &mut *self.frame.bindings;
        Some(ResolvedTargetImage {
            key,
            record: bindings.entries.get_mut(&key)?,
            recorded_head: &mut bindings.recorded_head,
        })
    }

    pub(crate) fn freeze(self) -> FrozenPresentationBindings<'frame> {
        let bindings = &mut *self.frame.bindings;
        let mut pending = bindings.recorded_head.take();
        while let Some(key) = pending {
            let record = bindings
                .entries
                .get_mut(&key)
                .unwrap()
                .companion
                .as_mut()
                .unwrap();
            pending = std::mem::replace(&mut record.next_recorded, bindings.recorded_head);
            bindings.recorded_head = Some(key);
        }
        FrozenPresentationBindings {
            frame: self.frame,
            build_frame: self.build_frame,
            attempt: self.attempt,
        }
    }
}

pub(crate) struct ResolvedTargetImage<'a> {
    key: PresentationKey,
    record: &'a mut PresentationRecord,
    recorded_head: &'a mut Option<PresentationKey>,
}

impl ResolvedTargetImage<'_> {
    pub(crate) fn binding(&self) -> CustomRenderPresentationBinding {
        self.record.binding
    }

    pub(crate) fn record_inserted(self, scale_factor: f32, image: &FlatPrimitive) {
        let Some(record) = self.record.companion.as_mut() else {
            return;
        };
        let PrimitiveData::Image { rect, .. } = image.data else {
            unreachable!("only an inserted target-bound Image can have an occurrence receipt");
        };
        if matches!(record.occurrence, ImageOccurrence::Unseen) {
            record.occurrence = ImageOccurrence::Unique(ImageOccurrenceReceipt {
                order_key: image.order_key,
                batch_key: image.key.clone(),
                clip: image.clip_state,
                rect,
                scale_factor,
            });
            record.next_recorded = self.recorded_head.replace(self.key);
        } else {
            record.occurrence = ImageOccurrence::Ambiguous;
        }
    }
}

pub(crate) struct FrozenPresentationBindings<'frame> {
    frame: PresentationFrame<'frame>,
    build_frame: u64,
    attempt: Option<AppFrameAttemptId>,
}

pub(crate) struct RecordedCompanion<'a> {
    pub(crate) surface: SurfaceId,
    pub(crate) binding: CustomRenderPresentationBinding,
    pub(crate) metrics: CustomRenderTargetMetrics,
    pub(crate) prepared: &'a Arc<PreparedHud3D>,
    pub(crate) occurrence: &'a ImageOccurrenceReceipt,
}

impl FrozenPresentationBindings<'_> {
    pub(crate) fn has_recorded_companions(&self) -> bool {
        self.frame.bindings.recorded_head.is_some()
    }

    pub(crate) fn recorded(&self, build_frame: u64) -> impl Iterator<Item = RecordedCompanion<'_>> {
        let bindings = &*self.frame.bindings;
        let mut next = bindings
            .recorded_head
            .filter(|_| self.build_frame == build_frame);
        std::iter::from_fn(move || {
            let key = next?;
            let record = &bindings.entries[&key];
            let companion = record.companion.as_ref().unwrap();
            next = companion.next_recorded;
            Some((key.1, record.binding, companion))
        })
        .filter_map(move |(surface, binding, companion)| {
            let ImageOccurrence::Unique(occurrence) = &companion.occurrence else {
                return None;
            };
            let payload = &companion.payload;
            (payload.metrics.app_frame_attempt == self.attempt).then_some(RecordedCompanion {
                surface,
                binding,
                metrics: payload.metrics,
                prepared: &payload.prepared,
                occurrence,
            })
        })
    }
}

/// Opt-in GPU-cache/lifecycle callback invoked after final layout and before
/// paint. Implementing this trait does not admit a WidgetTree participant;
/// registration also requires an opaque SPEC-HAL-295A `CustomRenderAdmission`.
pub trait CustomRenderParticipant {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome;
}

/// Renderer-owned immutable target authority captured once for an application frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CustomRenderTargetSnapshot {
    surface_id: SurfaceId,
    target: PhysicalTargetMetrics,
    app_frame_attempt: Option<crate::core::frame_realization::AppFrameAttemptId>,
}

impl CustomRenderTargetSnapshot {
    pub(crate) fn new(
        surface_id: SurfaceId,
        physical_extent: PhysicalExtent,
        scale_factor: f32,
    ) -> Result<Self, PixelCoverageError> {
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(PixelCoverageError::InvalidScaleFactor);
        }
        if physical_extent.is_empty() {
            return Err(PixelCoverageError::InvalidPhysicalTargetExtent);
        }
        let logical_extent = Size::new(
            physical_extent.width as f32 / scale_factor,
            physical_extent.height as f32 / scale_factor,
        );
        let target = PhysicalTargetMetrics::new(logical_extent, physical_extent, scale_factor)?;
        Ok(Self {
            surface_id,
            target,
            app_frame_attempt: None,
        })
    }

    pub(crate) fn with_app_frame_attempt(
        mut self,
        attempt: crate::core::frame_realization::AppFrameAttemptId,
    ) -> Self {
        self.app_frame_attempt = Some(attempt);
        self
    }

    pub(crate) const fn scale_factor(self) -> f32 {
        self.target.scale_factor()
    }

    pub(crate) fn metrics_for(
        self,
        final_logical_rect: RectF,
        final_logical_viewport: RectF,
    ) -> Result<CustomRenderTargetMetrics, PixelCoverageError> {
        self.metrics_for_visible_rect(
            final_logical_rect,
            final_logical_viewport,
            final_logical_rect.intersection(&final_logical_viewport),
        )
    }

    /// Convert an engine-qualified composition footprint using this target.
    /// The authored rect still determines the complete offscreen render extent.
    pub(crate) fn metrics_for_visible_rect(
        self,
        final_logical_rect: RectF,
        final_logical_viewport: RectF,
        visible: Option<RectF>,
    ) -> Result<CustomRenderTargetMetrics, PixelCoverageError> {
        #[cfg(test)]
        audit::record_metrics_snapshot();
        let render_extent = physical_render_extent(final_logical_rect.size(), self.target)?;
        let target_bounds = RectF::new(
            0.0,
            0.0,
            self.target.logical_extent().width,
            self.target.logical_extent().height,
        );
        let visible = visible.and_then(|rect| rect.intersection(&target_bounds));
        let coverage = match visible {
            Some(rect) => physical_composition_coverage(rect, self.target)?,
            None => None,
        };
        let activity = match (render_extent, coverage) {
            (Some(physical_render_extent), Some(physical_composition_coverage)) => {
                CustomRenderActivity::Active {
                    physical_render_extent,
                    physical_composition_coverage,
                }
            }
            _ => CustomRenderActivity::Inactive,
        };

        Ok(CustomRenderTargetMetrics {
            surface_id: self.surface_id,
            app_frame_attempt: self.app_frame_attempt,
            final_logical_rect,
            final_logical_viewport,
            target_logical_extent: self.target.logical_extent(),
            scale_factor: self.target.scale_factor(),
            target_physical_extent: self.target.physical_extent(),
            activity,
        })
    }
}

#[cfg(test)]
pub(crate) mod audit {
    use std::cell::Cell;

    thread_local! {
        static METRICS_SNAPSHOTS: Cell<usize> = const { Cell::new(0) };
        static PRESENTATION_LOOKUPS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn reset() {
        METRICS_SNAPSHOTS.set(0);
        PRESENTATION_LOOKUPS.set(0);
    }

    pub(crate) fn record_metrics_snapshot() {
        METRICS_SNAPSHOTS.set(METRICS_SNAPSHOTS.get() + 1);
    }

    pub(crate) fn metrics_snapshots() -> usize {
        METRICS_SNAPSHOTS.get()
    }

    pub(crate) fn record_presentation_lookup() {
        PRESENTATION_LOOKUPS.set(PRESENTATION_LOOKUPS.get() + 1);
    }

    pub(crate) fn presentation_lookups() -> usize {
        PRESENTATION_LOOKUPS.get()
    }
}
