//! App-owned staged resources and authenticated Frame B CPU publication evidence.

use super::sdf_image_reservation::{
    SdfAuthenticatedAdmission, SdfTargetEligibility, SdfTargetFailure, SdfTextureReservation,
};
use super::{SdfReservationError, WgpuRenderer};
use crate::core::math::{RectF, Size};
use crate::renderer::custom_render_pass::{
    CustomRenderPass, CustomRenderPassError, UnadmittedPreparationError,
};
use crate::renderer::image_binding::ImageBindingIdentity;
use crate::renderer::pixel_coverage::{
    PhysicalRect, PhysicalTargetMetrics, PixelCoverageError, physical_render_extent,
};
use crate::renderer::sdf_champagne_material::{ChampagneMaterial, ChampagnePipeline};
use crate::renderer::sdf_material_status::{
    ChampagneV4Basis, SdfMaterialKey, SdfMaterialPublication, SdfMaterialSource, SdfMaterialStatus,
    SdfMaterialUnavailableCause,
};
use crate::renderer::sdf_static_material::{
    OUTSETS_1X, OUTSETS_2X, StaticSdfMaterial, StaticSdfPipelines, v3_outsets,
};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[derive(Clone)]
pub(crate) struct SdfMaterialFrameInput {
    pub(crate) key: SdfMaterialKey,
    pub(crate) request_revision: crate::renderer::sdf_material_status::SdfMaterialRevision,
    pub(crate) source: SdfMaterialSource,
    pub(crate) pressed: bool,
    pub(crate) hovered: bool,
}

impl PartialEq for SdfMaterialFrameInput {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.same_material(other)
    }
}

impl SdfMaterialFrameInput {
    fn same_material(&self, other: &Self) -> bool {
        self.source == other.source
            && (self.source.uses_basis_revision()
                || (self.pressed == other.pressed
                    && self.request_revision == other.request_revision))
    }
}

enum MaterialParameters {
    Classic(StaticSdfMaterial),
    Champagne(ChampagneMaterial),
}

impl MaterialParameters {
    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Classic(material) => material.as_bytes(),
            Self::Champagne(material) => material.as_bytes(),
        }
    }
}

struct MaterialCell {
    rect: PhysicalRect,
    parameters: MaterialParameters,
}

/// Exact logical E and its source subrect; texture cell rounding never scales F.
struct MaterialMapping {
    coverage: RectF,
    uv: RectF,
    extent: PhysicalRect,
    cells: Vec<MaterialCell>,
}

#[derive(Debug, thiserror::Error)]
enum PreparationError {
    #[error("{0}")]
    UnsupportedScale(&'static str),
    #[error("invalid derived SDF coverage")]
    InvalidCoverage,
    #[error(transparent)]
    PixelCoverage(PixelCoverageError),
    #[error("invalid static material: {0}")]
    Material(&'static str),
    #[error(transparent)]
    Producer(CustomRenderPassError),
}

impl MaterialMapping {
    fn new(input: &SdfMaterialFrameInput) -> Result<Self, PreparationError> {
        let metrics = input.key.metrics();
        let scale = metrics.scale_factor;
        let outsets = match input.source {
            SdfMaterialSource::ChampagneV4(_) if ChampagneMaterial::supports_scale(scale) => {
                [1.0 / scale; 4]
            }
            SdfMaterialSource::ChampagneV4(_) => {
                return Err(PreparationError::UnsupportedScale(
                    "ChampagneV4 requires a finite target scale from 0.5 through 4",
                ));
            }
            SdfMaterialSource::V3 { .. } => {
                v3_outsets(scale).ok_or(PreparationError::UnsupportedScale(
                    "V3 requires a finite target scale from 0.5 through 4",
                ))?
            }
            _ => match scale {
                1.0 => OUTSETS_1X,
                2.0 => OUTSETS_2X,
                _ => {
                    return Err(PreparationError::UnsupportedScale(
                        "only the admitted 1x and 2x V1/V2 profiles are supported",
                    ));
                }
            },
        };
        let [left, right, top, bottom] = outsets;
        let face = metrics.final_logical_rect;
        let coverage = RectF::new(
            face.x - left,
            face.y - top,
            face.width + left + right,
            face.height + top + bottom,
        );
        if ![coverage.x, coverage.y, coverage.width, coverage.height]
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(PreparationError::InvalidCoverage);
        }
        let target = PhysicalTargetMetrics::new(
            metrics.target_logical_extent,
            metrics.target_physical_extent,
            scale,
        )
        .map_err(PreparationError::PixelCoverage)?;
        let extent = physical_render_extent(Size::new(coverage.width, coverage.height), target)
            .map_err(PreparationError::PixelCoverage)?
            .ok_or(PreparationError::InvalidCoverage)?;
        // Enclose fractional E in whole source cells, then use matching UVs.
        // This padding is storage only, not a changed visual or layout outset.
        let contain = |extent: u32, logical: f32| {
            if (extent as f32) < logical * scale {
                extent.checked_add(1)
            } else {
                Some(extent)
            }
        };
        let columns = if input.source.has_pressed_cell() {
            2
        } else {
            1
        };
        let rows = if input.source == SdfMaterialSource::ChampagneV4(ChampagneV4Basis::Enabled) {
            2
        } else {
            1
        };
        // Align neighboring cells to whole derivative quads. Storage padding
        // stays outside the sampled UV and leaves logical E unchanged.
        let axis = |extent, logical, cells| {
            let contained = contain(extent, logical)?;
            let cell = if cells > 1 {
                contained.checked_add(contained & 1)?
            } else {
                contained
            };
            Some((cell, cell.checked_mul(cells)?))
        };
        let (width, texture_width) =
            axis(extent.width, coverage.width, columns).ok_or(PreparationError::InvalidCoverage)?;
        let (height, texture_height) =
            axis(extent.height, coverage.height, rows).ok_or(PreparationError::InvalidCoverage)?;
        let uv = RectF::new(
            0.0,
            0.0,
            coverage.width * scale / texture_width as f32,
            coverage.height * scale / texture_height as f32,
        );
        let half_size = [face.width * 0.5, face.height * 0.5];
        let center = [left + half_size[0], top + half_size[1]];
        let parameters = |center, hovered, pressed| {
            match input.source {
                SdfMaterialSource::V1 { dark } => {
                    StaticSdfMaterial::resting(dark, scale, half_size, center)
                        .map(MaterialParameters::Classic)
                }
                SdfMaterialSource::V2 { dark } => {
                    StaticSdfMaterial::v2(dark, scale, half_size, center, input.pressed)
                        .map(MaterialParameters::Classic)
                }
                SdfMaterialSource::V3 { dark } => {
                    StaticSdfMaterial::v3(dark, scale, half_size, center, pressed)
                        .map(MaterialParameters::Classic)
                }
                SdfMaterialSource::ChampagneV4(basis) => {
                    ChampagneMaterial::new(basis, scale, half_size, center, hovered, pressed)
                        .map(MaterialParameters::Champagne)
                }
            }
            .map_err(PreparationError::Material)
        };
        let cells = (0..rows * columns)
            .map(|index| {
                let column = index % columns;
                let row = index / columns;
                let (x, y) = (column * width, row * height);
                Ok(MaterialCell {
                    rect: PhysicalRect::new(x, y, width, height),
                    parameters: parameters(
                        [center[0] + x as f32 / scale, center[1] + y as f32 / scale],
                        row != 0,
                        column != 0,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, PreparationError>>()?;
        Ok(Self {
            coverage,
            uv,
            extent: PhysicalRect::new(0, 0, texture_width, texture_height),
            cells,
        })
    }

    fn uv_for_cell(&self, index: usize) -> Option<RectF> {
        let cell = self.cells.get(index)?.rect;
        Some(self.uv.translate(
            cell.x as f32 / self.extent.width as f32,
            cell.y as f32 / self.extent.height as f32,
        ))
    }

    fn same_content(&self, other: &Self) -> bool {
        self.coverage.size() == other.coverage.size()
            && self.uv == other.uv
            && self.extent == other.extent
            && self.cells.len() == other.cells.len()
            && self.cells.iter().zip(&other.cells).all(|(left, right)| {
                left.rect == right.rect && left.parameters.as_bytes() == right.parameters.as_bytes()
            })
    }
}

#[derive(Default)]
struct UnadmittedFront {
    reservation: Option<SdfTextureReservation>,
    producer: Option<CustomRenderPass>,
    prepared: bool,
}

impl Drop for UnadmittedFront {
    fn drop(&mut self) {
        if let Some(producer) = self.producer.take() {
            // Drop has no registrar or completion certificate. Keep the actual
            // front alive on shutdown/unwind instead of asserting safe disposal.
            std::mem::forget(producer);
            tracing::error!("Uncertified unadmitted SDF front retained at owner drop");
        }
    }
}

enum StagedFront {
    Unadmitted(UnadmittedFront),
    Admitted(SdfAuthenticatedAdmission),
}

#[derive(Debug, thiserror::Error)]
enum StagingFailure {
    #[error("preparation: {0}")]
    Preparation(PreparationError),
    #[error("reservation: {0:?}")]
    Reservation(SdfReservationError),
    #[error("preparation completion: {0:?}")]
    Completion(UnadmittedPreparationError),
    #[error("authenticated admission: {0:?}")]
    Admission(SdfReservationError),
}

impl StagingFailure {
    fn unavailable_cause(&self, source: SdfMaterialSource) -> Option<SdfMaterialUnavailableCause> {
        match self {
            Self::Preparation(PreparationError::UnsupportedScale(_))
                if matches!(
                    source,
                    SdfMaterialSource::V3 { .. } | SdfMaterialSource::ChampagneV4(_)
                ) =>
            {
                Some(SdfMaterialUnavailableCause::UnsupportedScale)
            }
            Self::Preparation(
                PreparationError::InvalidCoverage | PreparationError::Material(_),
            ) if matches!(source, SdfMaterialSource::ChampagneV4(_)) => {
                Some(SdfMaterialUnavailableCause::UnsupportedGeometry)
            }
            Self::Preparation(PreparationError::Producer(_))
            | Self::Completion(
                UnadmittedPreparationError::Uncertified | UnadmittedPreparationError::PollFailed,
            )
            | Self::Admission(SdfReservationError::Front(_)) => {
                Some(SdfMaterialUnavailableCause::ProducerPreparationFailed)
            }
            Self::Admission(SdfReservationError::Admission(cause)) => Some(
                SdfMaterialUnavailableCause::BindingAdmissionFailed(cause.clone()),
            ),
            // Missing evidence, unsupported metrics and rejected owner keys are
            // not fabricated resource failures or a third material outcome.
            _ => None,
        }
    }
}

struct StagedMaterial {
    input: SdfMaterialFrameInput,
    producer_preparation: Arc<()>,
    mapping: Option<MaterialMapping>,
    front: StagedFront,
    failure: Option<StagingFailure>,
    retirement_pending: bool,
    // Publish the current destination atomically with its exact eligibility.
    // The original mapping/input/admission remain immutable production evidence.
    eligibility: Option<(SdfTargetEligibility, MaterialMapping)>,
    window_failure: Option<SdfTargetFailure>,
    requires_eligibility: bool,
    retirement_started: bool,
}

impl StagedMaterial {
    fn matches_input(&self, input: &SdfMaterialFrameInput, preparation: &Arc<()>) -> bool {
        !self.retirement_started
            && ((!self.requires_eligibility && self.input == *input)
                || self
                    .eligibility
                    .as_ref()
                    .is_some_and(|(e, _)| e.matches(&input.key, preparation))
                || self
                    .window_failure
                    .as_ref()
                    .is_some_and(|f| f.matches(&input.key, preparation)))
    }

    fn matches_key(&self, key: &SdfMaterialKey, preparation: &Arc<()>) -> bool {
        !self.retirement_started
            && ((!self.requires_eligibility && self.input.key == *key)
                || self
                    .eligibility
                    .as_ref()
                    .is_some_and(|(e, _)| e.matches(key, preparation)))
    }

    fn qualify_window_extent(
        &mut self,
        renderer: &mut WgpuRenderer,
        current: &[SdfMaterialFrameInput],
        eligible_keys: &[SdfMaterialKey],
        preparation: &Arc<()>,
    ) {
        // A new publication expires BOTH eligibility and its destination mapping.
        self.eligibility = None;
        self.window_failure = None;
        if self.failure.is_some() || self.retirement_started {
            return;
        }
        let StagedFront::Admitted(admission) = &self.front else {
            return;
        };
        let Some(mapping) = &self.mapping else { return };
        let Some(input) = current.iter().find(|input| {
            self.input.key.same_content_geometry_basis(&input.key)
                && self.input.same_material(input)
        }) else {
            return;
        };
        // A replacement admitted before a superseded Compile can still owe
        // its first output. Certify its current attempt even when K repeats.
        if !self.requires_eligibility
            && self.input.key == input.key
            && !SdfMaterialStaging::current_eligible_key(input, eligible_keys)
        {
            return;
        }
        self.requires_eligibility = true;
        let metrics = input.key.metrics();
        let limit = renderer
            .gpu_resources()
            .device
            .limits()
            .max_texture_dimension_2d;
        if metrics.target_physical_extent.width > limit
            || metrics.target_physical_extent.height > limit
        {
            return;
        }
        let Ok(current_mapping) = MaterialMapping::new(input) else {
            return;
        };
        if !mapping.same_content(&current_mapping) {
            return;
        }
        match admission.certify_window_extent(renderer, &input.key, preparation) {
            Ok(eligibility) => self.eligibility = Some((eligibility, current_mapping)),
            Err(
                crate::renderer::sdf_material_status::SdfMaterialRejection::BindingUnavailable(_),
            ) => {
                self.window_failure = admission
                    .window_failure(renderer, &input.key, preparation)
                    .ok();
            }
            Err(_) => {}
        }
    }

    fn retire_if_ready(&mut self, renderer: &mut WgpuRenderer) -> bool {
        self.retirement_started = true;
        self.eligibility = None;
        self.window_failure = None;
        self.retirement_pending = false;
        match &mut self.front {
            StagedFront::Unadmitted(front) => {
                let Some(reservation) = front.reservation.as_ref() else {
                    return true;
                };
                match renderer.abort_unadmitted_sdf_reservation(
                    reservation,
                    &self.input.key,
                    &mut front.producer,
                ) {
                    Ok(retired) => {
                        self.retirement_pending = !retired;
                        retired
                    }
                    Err(error) => {
                        tracing::error!(?error, "SDF unadmitted abort retained its resources");
                        false
                    }
                }
            }
            StagedFront::Admitted(admission) => match admission.retire_if_ready(renderer) {
                Ok(retired) => {
                    self.retirement_pending = !retired;
                    retired
                }
                Err(error) => {
                    tracing::error!(?error, "SDF generation retirement remains uncertified");
                    false
                }
            },
        }
    }
}

/// Bounded by the qualified requests; unresolved obsolete resources block new
/// allocation rather than accumulating another generation on every frame.
#[derive(Default)]
pub(crate) struct SdfMaterialStaging {
    pipelines: Option<StaticSdfPipelines>,
    v2_pipelines: Option<StaticSdfPipelines>,
    v3_pipelines: Option<StaticSdfPipelines>,
    champagne_pipeline: Option<ChampagnePipeline>,
    materials: Vec<StagedMaterial>,
    preparation: Arc<()>,
    #[cfg(test)]
    preparation_totals: (u64, u128),
}

impl SdfMaterialStaging {
    /// Continuation within one still-selected stateful profile, never a new K.
    pub(crate) fn can_continue_selection(
        &self,
        prospective: &SdfMaterialFrameInput,
        selected: &SdfMaterialKey,
    ) -> bool {
        self.materials.iter().any(|material| {
            !material.retirement_started
                && !material.requires_eligibility
                && &material.input.key == selected
                && prospective.source == material.input.source
                && !matches!(prospective.source, SdfMaterialSource::V1 { .. })
                && (!prospective.source.uses_basis_revision() || prospective.key == *selected)
                && prospective.key.same_owner_target(selected)
        })
    }

    pub(crate) fn continuing_publication(
        &self,
        renderer: &mut WgpuRenderer,
        prospective: &SdfMaterialFrameInput,
        selected: &SdfMaterialKey,
    ) -> Option<(SdfMaterialPublication, Option<ImageBindingIdentity>)> {
        if !self.can_continue_selection(prospective, selected) {
            return None;
        }
        let material = self
            .materials
            .iter()
            .find(|material| &material.input.key == selected)?;
        let StagedFront::Admitted(admission) = &material.front else {
            return None;
        };
        // Recheck the OLD committed K and its actual resource. Revocation yields
        // typed Unavailable, never an old Image passed off as the prospective K.
        admission.publication_for_current(renderer, selected).ok()
    }

    #[cfg(test)]
    pub(crate) fn retained_generation_count(&self) -> usize {
        self.materials.len()
    }

    #[cfg(test)]
    pub(crate) fn preparation_totals_for_test(&self) -> (u64, u128) {
        self.preparation_totals
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }

    /// Retire after the native App has discarded every plan and scene that could submit.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn retire_at_native_exit(&mut self, renderer: &mut WgpuRenderer) {
        if self.is_empty() {
            return;
        }
        let lifetime = super::DeviceLifetimeObservation::for_device(&renderer.device);
        self.materials.retain_mut(|material| {
            lifetime.failure().is_some() || !material.retire_if_ready(renderer)
        });
        if lifetime.failure().is_some()
            || !self.materials.iter().any(|material| material.retirement_pending)
        {
            return;
        }

        // Drive the existing observations once. The fence result supplies no
        // generation certificate, including when the bounded wait times out.
        match renderer.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_millis(250)),
        }) {
            Ok(_) | Err(wgpu::PollError::Timeout) => {}
            Err(error) => {
                tracing::error!(?error, "Native SDF retirement queue wait failed");
                return;
            }
        }
        self.materials.retain_mut(|material| {
            !material.retirement_pending
                || lifetime.failure().is_some()
                || !material.retire_if_ready(renderer)
        });
    }

    /// Resource-owned acquisition only; the engine must call before the actual
    /// first retained Image reference, after its current Ready/live-mount gate.
    pub(crate) fn retain_image_reference(
        &self,
        current: &SdfMaterialKey,
        identity: ImageBindingIdentity,
    ) -> Result<
        crate::renderer::sdf_frame_custody::SdfImageReference,
        crate::renderer::sdf_frame_custody::SdfFrameCustodyError,
    > {
        use crate::renderer::sdf_image_generation_custody::SdfImageCustodyError;
        let material = self
            .materials
            .iter()
            .find(|material| material.matches_key(current, &self.preparation))
            .ok_or(SdfImageCustodyError::IdentityMismatch)?;
        let StagedFront::Admitted(admission) = &material.front else {
            return Err(SdfImageCustodyError::Retired.into());
        };
        if admission.binding().identity() != identity {
            return Err(SdfImageCustodyError::IdentityMismatch.into());
        }
        if !material.requires_eligibility && &material.input.key == current {
            admission
                .retain_scene_reference(current)
                .map_err(Into::into)
        } else {
            let (eligibility, _) = material
                .eligibility
                .as_ref()
                .ok_or(SdfImageCustodyError::IdentityMismatch)?;
            admission
                .retain_eligible_reference(current, eligibility, &self.preparation)
                .map_err(Into::into)
        }
    }

    pub(crate) fn paint_mapping(
        &self,
        current: &SdfMaterialKey,
        identity: ImageBindingIdentity,
        pressed: bool,
        hovered: bool,
    ) -> Option<(RectF, RectF)> {
        let material = self
            .materials
            .iter()
            .find(|material| material.matches_key(current, &self.preparation))?;
        let StagedFront::Admitted(admission) = &material.front else {
            return None;
        };
        if admission.binding().identity() != identity {
            return None;
        }
        let mapping = if !material.requires_eligibility && &material.input.key == current {
            material.mapping.as_ref()?
        } else {
            // The matching certificate and this finite current mapping were
            // published together. Never translate/reuse the original coverage.
            &material.eligibility.as_ref()?.1
        };
        let index = match material.input.source {
            SdfMaterialSource::ChampagneV4(ChampagneV4Basis::Enabled) => {
                2 * usize::from(hovered) + usize::from(pressed)
            }
            SdfMaterialSource::V3 { .. } => usize::from(pressed),
            _ => 0,
        };
        Some((mapping.coverage, mapping.uv_for_cell(index)?))
    }

    pub(crate) fn needs_followup(&self) -> bool {
        self.materials.iter().any(|material| {
            material.retirement_pending
                || material.failure.is_none()
                    && matches!(&material.front, StagedFront::Unadmitted(front) if front.prepared)
        })
    }

    /// Only previously admitted fronts can supply Ready. Pending Frame A fronts
    /// supply no status; actual failures retain their precise resource evidence.
    pub(crate) fn publications_before_paint(
        &self,
        renderer: &mut WgpuRenderer,
        current: &[SdfMaterialFrameInput],
    ) -> Vec<(SdfMaterialPublication, Option<ImageBindingIdentity>)> {
        let mut publications = Vec::new();
        for material in &self.materials {
            let Some(input) = current
                .iter()
                .find(|input| material.matches_input(input, &self.preparation))
            else {
                continue;
            };
            let publication = if let StagedFront::Admitted(admission) = &material.front {
                if !material.requires_eligibility && material.input == *input {
                    admission.publication_for_current(renderer, &input.key)
                } else if let Some(failure) = material
                    .window_failure
                    .as_ref()
                    .filter(|failure| failure.matches(&input.key, &self.preparation))
                {
                    Ok((failure.publication(), Some(admission.binding().identity())))
                } else {
                    admission.publication_for_eligibility(
                        renderer,
                        &input.key,
                        material
                            .eligibility
                            .as_ref()
                            .map(|(eligibility, _)| eligibility)
                            .expect("matched current eligibility"),
                        &self.preparation,
                    )
                }
            } else if let Some(cause) = material
                .failure
                .as_ref()
                .and_then(|failure| failure.unavailable_cause(input.source))
            {
                SdfMaterialPublication::new(
                    &input.key,
                    material.input.key.clone(),
                    None,
                    SdfMaterialStatus::Unavailable { cause },
                )
                .map(|publication| (publication, None))
            } else {
                continue;
            };
            match publication {
                Ok(publication) => publications.push(publication),
                Err(error) => {
                    tracing::error!(?error, owner = ?input.key.owner(), "SDF Frame B evidence rejected");
                }
            }
        }
        publications
    }

    /// Called only at the canonical post-layout, pre-retained-Paint hook.
    pub(crate) fn prepare_before_paint(
        &mut self,
        renderer: &mut WgpuRenderer,
        current: &[SdfMaterialFrameInput],
        selected: &[SdfMaterialKey],
    ) {
        self.prepare_with_active_size_replacements(renderer, current, selected, &[]);
    }

    fn current_eligible_key(input: &SdfMaterialFrameInput, eligible: &[SdfMaterialKey]) -> bool {
        eligible
            .iter()
            .any(|key| key == &input.key && key.metrics() == input.key.metrics())
    }

    pub(crate) fn prepare_with_active_size_replacements(
        &mut self,
        renderer: &mut WgpuRenderer,
        current: &[SdfMaterialFrameInput],
        selected: &[SdfMaterialKey],
        eligible_keys: &[SdfMaterialKey],
    ) {
        // A fresh bounded publication rejects replay even when K or App attempt
        // values repeat. It is resource evidence, never widget/paint state.
        self.preparation = Arc::new(());
        for material in &mut self.materials {
            material.qualify_window_extent(renderer, current, eligible_keys, &self.preparation);
        }
        self.materials.retain_mut(|material| {
            // A prior attempt's unadmitted replacement cannot become this
            // attempt's fresh admission merely because resource K repeats.
            let obsolete_replacement = matches!(material.front, StagedFront::Unadmitted(_))
                && current.iter().any(|input| {
                    input.source.has_pressed_cell()
                        && Self::current_eligible_key(input, eligible_keys)
                        && material.matches_input(input, &self.preparation)
                });
            if obsolete_replacement {
                return !material.retire_if_ready(renderer);
            }
            if !material.retirement_started
                && (current
                    .iter()
                    .any(|input| material.matches_input(input, &self.preparation))
                    || selected.contains(&material.input.key))
            {
                material.retirement_pending = false;
                true
            } else {
                !material.retire_if_ready(renderer)
            }
        });
        let obsolete = self.materials.iter().any(|material| {
            !current
                .iter()
                .any(|input| material.matches_input(input, &self.preparation))
        });
        for input in current {
            if self
                .materials
                .iter()
                .any(|material| material.matches_input(input, &self.preparation))
            {
                continue;
            }
            if matches!(input.source, SdfMaterialSource::V1 { .. }) && obsolete {
                continue;
            }
            // One selected front plus one preparing/retiring generation. Rapid
            // supersession waits for real custody completion, never allocates a tail.
            let retained = self
                .materials
                .iter()
                .filter(|material| {
                    material.input.key.owner() == input.key.owner()
                        && material.input.key.tree_lifetime() == input.key.tree_lifetime()
                })
                .count();
            if retained >= 2 {
                continue;
            }
            #[cfg(test)]
            let preparation_started = web_time::Instant::now();
            let mut material = StagedMaterial {
                input: input.clone(),
                producer_preparation: self.preparation.clone(),
                mapping: None,
                front: StagedFront::Unadmitted(UnadmittedFront::default()),
                failure: None,
                window_failure: None,
                retirement_pending: false,
                eligibility: None,
                requires_eligibility: false,
                retirement_started: false,
            };
            let preparation = (|| {
                let mapping = MaterialMapping::new(input)?;
                let limit = renderer
                    .gpu_resources()
                    .device
                    .limits()
                    .max_texture_dimension_2d;
                if mapping.extent.width > limit || mapping.extent.height > limit {
                    return Err(PreparationError::InvalidCoverage);
                }
                material.mapping = Some(mapping);
                Ok(())
            })();
            if let Err(error) = preparation {
                tracing::error!(?error, owner = ?input.key.owner(), "SDF preparation input rejected");
                material.failure = Some(StagingFailure::Preparation(error));
            } else if let StagedFront::Unadmitted(front) = &mut material.front {
                match renderer.reserve_sdf_texture(input.key.clone()) {
                    Ok(reservation) => {
                        front.reservation = Some(reservation);
                        let gpu = renderer.gpu_resources();
                        let legacy = match input.source {
                            SdfMaterialSource::V1 { .. } => Some(
                                self.pipelines
                                    .get_or_insert_with(|| StaticSdfPipelines::new(&gpu)),
                            ),
                            SdfMaterialSource::V2 { .. } => Some(
                                self.v2_pipelines
                                    .get_or_insert_with(|| StaticSdfPipelines::new_v2(&gpu)),
                            ),
                            SdfMaterialSource::V3 { .. } => Some(
                                self.v3_pipelines
                                    .get_or_insert_with(|| StaticSdfPipelines::new_v3(&gpu)),
                            ),
                            SdfMaterialSource::ChampagneV4(_) => None,
                        };
                        let (bindings, face, shadow) = if let Some(pipelines) = legacy {
                            (
                                &pipelines.bindings,
                                &pipelines.face,
                                Some(&pipelines.shadow),
                            )
                        } else {
                            let pipeline = self
                                .champagne_pipeline
                                .get_or_insert_with(|| ChampagnePipeline::new(&gpu));
                            (&pipeline.bindings, &pipeline.face, None)
                        };
                        let mapping = material.mapping.as_ref().expect("validated mapping");
                        let prepared = (|| {
                            front.producer = Some(
                                CustomRenderPass::new_unpublished(
                                    front
                                        .reservation
                                        .as_ref()
                                        .expect("owned reservation")
                                        .texture_id(),
                                    gpu.clone(),
                                    mapping.extent,
                                )
                                .map_err(PreparationError::Producer)?,
                            );
                            let producer = front.producer.as_mut().expect("retained producer");
                            let groups: Vec<_> = mapping
                                .cells
                                .iter()
                                .map(|cell| {
                                    let uniform = gpu.device.create_buffer_init(
                                        &wgpu::util::BufferInitDescriptor {
                                            label: Some("4E immutable cell parameters"),
                                            contents: cell.parameters.as_bytes(),
                                            usage: wgpu::BufferUsages::UNIFORM,
                                        },
                                    );
                                    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                        label: Some("4E cell binding"),
                                        layout: bindings,
                                        entries: &[wgpu::BindGroupEntry {
                                            binding: 0,
                                            resource: uniform.as_entire_binding(),
                                        }],
                                    })
                                })
                                .collect();
                            let mut frame = producer
                                .begin_frame(Some("4E canonical pre-Paint preparation"))
                                .map_err(PreparationError::Producer)?;
                            {
                                let (encoder, view) = frame.encoder_and_view();
                                let mut pass =
                                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("4E shadow below face"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                depth_slice: None,
                                                view,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Clear(
                                                        wgpu::Color::TRANSPARENT,
                                                    ),
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        multiview_mask: None,
                                        occlusion_query_set: None,
                                        timestamp_writes: None,
                                    });
                                for (cell, group) in mapping.cells.iter().zip(&groups) {
                                    if groups.len() > 1 {
                                        let PhysicalRect {
                                            x,
                                            y,
                                            width,
                                            height,
                                        } = cell.rect;
                                        pass.set_viewport(
                                            x as f32,
                                            y as f32,
                                            width as f32,
                                            height as f32,
                                            0.0,
                                            1.0,
                                        );
                                        pass.set_scissor_rect(x, y, width, height);
                                    }
                                    pass.set_bind_group(0, group, &[]);
                                    if let Some(shadow) = shadow {
                                        pass.set_pipeline(shadow);
                                        pass.draw(0..3, 0..1);
                                    }
                                    pass.set_pipeline(face);
                                    pass.draw(0..3, 0..1);
                                }
                            }
                            producer
                                .submit_and_swap_deferred_registration(frame)
                                .map_err(PreparationError::Producer)
                        })();
                        match prepared {
                            Ok(()) => front.prepared = true,
                            Err(error) => {
                                tracing::error!(?error, owner = ?input.key.owner(), "SDF producer preparation failed; front retained");
                                material.failure = Some(StagingFailure::Preparation(error));
                            }
                        }
                    }
                    Err(error) => {
                        tracing::error!(?error, owner = ?input.key.owner(), "SDF reservation rejected");
                        material.failure = Some(StagingFailure::Reservation(error));
                    }
                }
            }
            #[cfg(test)]
            if matches!(&material.front, StagedFront::Unadmitted(front) if front.prepared) {
                self.preparation_totals.0 += 1;
                self.preparation_totals.1 += preparation_started.elapsed().as_nanos();
            }
            self.materials.push(material);
        }
    }

    /// App supplies only already-active, first-output-qualified size replacements.
    /// The initial optional Frame A/B admission below is deliberately unchanged.
    pub(crate) fn admit_active_size_replacements_before_paint(
        &mut self,
        renderer: &mut WgpuRenderer,
        current: &[SdfMaterialFrameInput],
        eligible_keys: &[SdfMaterialKey],
    ) -> bool {
        let mut changed = false;
        for material in &mut self.materials {
            let Some(input) = current.iter().find(|input| {
                input.source.has_pressed_cell()
                    && Self::current_eligible_key(input, eligible_keys)
                    && material.input == **input
                    && material.input.key.metrics() == input.key.metrics()
                    && Arc::ptr_eq(&material.producer_preparation, &self.preparation)
            }) else {
                continue;
            };
            if material.failure.is_some()
                || material.retirement_started
                || material
                    .mapping
                    .as_ref()
                    .is_none_or(|mapping| mapping.cells.len() < 2)
            {
                continue;
            }
            let StagedFront::Unadmitted(front) = &mut material.front else {
                continue;
            };
            if !front.prepared {
                continue;
            }
            match renderer.register_reserved_sdf_replacement_front(
                front.reservation.as_ref().expect("prepared reservation"),
                &input.key,
                &material.input.key,
                &mut front.producer,
            ) {
                Ok(Some(admission)) => {
                    material.front = StagedFront::Admitted(admission);
                    changed = true;
                }
                Ok(None) => {}
                Err(SdfReservationError::Preparation(error)) => {
                    material.failure = Some(StagingFailure::Completion(error));
                    changed = true;
                }
                Err(error) => {
                    material.failure = Some(StagingFailure::Admission(error));
                    changed = true;
                }
            }
        }
        changed
    }

    /// Missing or unpublished failure evidence must not escape the scoped
    /// dependent-output gate. Only authenticated current Ready/Unavailable ends it.
    pub(crate) fn active_size_replacement_pending(
        &self,
        renderer: &mut WgpuRenderer,
        current: &SdfMaterialFrameInput,
        eligible_keys: &[SdfMaterialKey],
    ) -> bool {
        current.source.has_pressed_cell()
            && Self::current_eligible_key(current, eligible_keys)
            && !self
                .publications_before_paint(renderer, std::slice::from_ref(current))
                .iter()
                .any(|(publication, _)| {
                    publication.key() == &current.key
                        && publication.key().metrics() == current.key.metrics()
                })
    }

    #[cfg(test)]
    pub(crate) fn hold_writer_completion_observation_for_test(
        &self,
        key: &SdfMaterialKey,
    ) -> Option<Arc<std::sync::atomic::AtomicBool>> {
        let material = self
            .materials
            .iter()
            .find(|material| material.matches_key(key, &self.preparation))?;
        match &material.front {
            StagedFront::Unadmitted(front) => front
                .producer
                .as_ref()?
                .hold_preparation_observation_for_test(),
            StagedFront::Admitted(admission) => admission.hold_writer_observation_for_test(),
        }
    }

    #[cfg(test)]
    pub(crate) fn hold_sampling_completion_observation_for_test(
        &self,
        key: &SdfMaterialKey,
    ) -> Option<Arc<std::sync::atomic::AtomicBool>> {
        let material = self
            .materials
            .iter()
            .find(|material| material.matches_key(key, &self.preparation))?;
        let StagedFront::Admitted(admission) = &material.front else {
            return None;
        };
        Some(admission.hold_sampling_observation_for_test())
    }

    /// Registrar work only: the frozen front was already rendered before Paint.
    /// Retain genuine evidence for Frame B; do not construct/publish Ready here.
    pub(crate) fn admit_after_paint(
        &mut self,
        renderer: &mut WgpuRenderer,
        current: &[SdfMaterialFrameInput],
    ) -> bool {
        let mut changed = false;
        for material in &mut self.materials {
            if material.failure.is_some()
                || material.retirement_started
                || !current.contains(&material.input)
            {
                continue;
            }
            let StagedFront::Unadmitted(front) = &mut material.front else {
                continue;
            };
            if !front.prepared {
                continue;
            }
            // Registered custody accounts for sampling. Settle the separately
            // tracked producer writes first, including a front never sampled.
            match front
                .producer
                .as_ref()
                .expect("prepared producer")
                .unadmitted_preparation_complete()
            {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    tracing::error!(?error, owner = ?material.input.key.owner(), "SDF preparation completion remains uncertified");
                    material.failure = Some(StagingFailure::Completion(error));
                    changed = true;
                    continue;
                }
            }
            let result = renderer.register_reserved_sdf_front(
                front.reservation.as_ref().expect("prepared reservation"),
                &material.input.key,
                &material.input.key,
                &mut front.producer,
            );
            match result {
                Ok(admission) => {
                    material.front = StagedFront::Admitted(admission);
                    changed = true;
                }
                Err(error) => {
                    tracing::error!(?error, owner = ?material.input.key.owner(), "SDF checked admission failed; resources retained");
                    material.failure = Some(StagingFailure::Admission(error));
                    changed = true;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod window_eligibility_tests {
    use super::*;
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::widget_tree::WidgetTree;
    use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
    use crate::renderer::pixel_coverage::PhysicalExtent;
    use crate::renderer::sdf_material_status::SdfMaterialRevision;
    use crate::widgets::label::Label;

    fn inputs() -> (WidgetTree, SdfMaterialFrameInput, SdfMaterialFrameInput) {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("material")), None, 0);
        let owner = tree.node(root).unwrap().id;
        let viewport = RectF::new(0.0, 0.0, 200.0, 120.0);
        let face = RectF::new(30.0, 30.0, 84.0, 30.0);
        let mut compositor = Compositor::new(viewport);
        let surface = compositor.create_surface(0, viewport, SceneKey::BASE);
        let input = |width| {
            let metrics =
                CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(width, 180), 1.0)
                    .unwrap()
                    .metrics_for(face, viewport)
                    .unwrap();
            SdfMaterialFrameInput {
                key: SdfMaterialKey::new(
                    tree.snapshot_tree_lifetime(),
                    owner,
                    surface,
                    SdfMaterialRevision::initial(),
                    metrics,
                )
                .unwrap(),
                request_revision: SdfMaterialRevision::initial(),
                source: SdfMaterialSource::V3 { dark: false },
                pressed: false,
                hovered: false,
            }
        };
        let a = input(320);
        let b = input(360);
        (tree, a, b)
    }

    #[test]
    fn champagne_v4_fractional_mapping_contains_each_cell_at_the_exact_scale() {
        let (_tree, base, _) = inputs();
        let face = RectF::new(13.375, 17.25, 290.375, 38.25);
        let viewport = RectF::new(0.0, 0.0, 320.0, 100.0);
        let original = base.key.metrics();
        let with_metrics = |metrics, source| SdfMaterialFrameInput {
            key: SdfMaterialKey::new(
                base.key.tree_lifetime().clone(),
                base.key.owner(),
                original.surface_id,
                base.key.revision(),
                metrics,
            )
            .unwrap(),
            request_revision: base.request_revision,
            source,
            pressed: false,
            hovered: false,
        };
        assert_eq!(v3_outsets(1.0), Some(OUTSETS_1X));
        assert_eq!(v3_outsets(2.0), Some(OUTSETS_2X));
        for scale in [0.5_f32, 0.73, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0] {
            let metrics = CustomRenderTargetSnapshot::new(
                original.surface_id,
                PhysicalExtent::new(
                    (320.0 * scale).round() as u32,
                    (100.0 * scale).round() as u32,
                ),
                scale,
            )
            .unwrap()
            .metrics_for(face, viewport)
            .unwrap();
            for basis in [ChampagneV4Basis::Enabled, ChampagneV4Basis::Disabled] {
                let input = with_metrics(metrics, SdfMaterialSource::ChampagneV4(basis));
                let m = MaterialMapping::new(&input).unwrap();
                let e = m.coverage;
                let side = if basis == ChampagneV4Basis::Enabled {
                    2
                } else {
                    1
                };
                for gap in [
                    face.x - e.x,
                    face.y - e.y,
                    e.right() - face.right(),
                    e.bottom() - face.bottom(),
                ] {
                    assert!((gap * scale - 1.0).abs() < 0.0002, "{scale}x gap={gap}");
                }
                assert_eq!(m.cells.len(), (side * side) as usize);
                for (index, cell) in m.cells.iter().enumerate() {
                    let (column, row) = (index as u32 % side, index as u32 / side);
                    let PhysicalRect {
                        x,
                        y,
                        width,
                        height,
                    } = cell.rect;
                    assert_eq!((x, y), (column * width, row * height));
                    assert_eq!(
                        (m.extent.width, m.extent.height),
                        (width * side, height * side)
                    );
                    assert!(width as f32 >= e.width * scale);
                    assert!(height as f32 >= e.height * scale);
                    let uv = m.uv_for_cell(index).unwrap();
                    assert_eq!(
                        (uv.x, uv.y),
                        (column as f32 / side as f32, row as f32 / side as f32)
                    );
                    assert!(uv.right() <= (column + 1) as f32 / side as f32);
                    assert!(uv.bottom() <= (row + 1) as f32 / side as f32);
                    assert!((uv.width * m.extent.width as f32 - e.width * scale).abs() < 0.0002);
                    assert!((uv.height * m.extent.height as f32 - e.height * scale).abs() < 0.0002);
                    if side == 2 {
                        assert_eq!((width % 2, height % 2), (0, 0));
                    }
                }
                if basis == ChampagneV4Basis::Enabled {
                    let mut interacting = input.clone();
                    interacting.pressed = true;
                    interacting.hovered = true;
                    interacting.request_revision =
                        input.request_revision.checked_increment().unwrap();
                    assert!(input == interacting);
                    assert!(m.same_content(&MaterialMapping::new(&interacting).unwrap()));
                }
            }
            for source in [
                SdfMaterialSource::V1 { dark: false },
                SdfMaterialSource::V2 { dark: false },
                SdfMaterialSource::V3 { dark: false },
            ] {
                let input = with_metrics(metrics, source);
                let mapping = MaterialMapping::new(&input);
                assert_eq!(
                    mapping.is_ok(),
                    matches!(source, SdfMaterialSource::V3 { .. }) || matches!(scale, 1.0 | 2.0),
                    "{source:?} at {scale}x",
                );
                if matches!(source, SdfMaterialSource::V3 { .. }) {
                    let m = mapping.unwrap();
                    let coverage = m.coverage;
                    let expected = v3_outsets(scale).unwrap();
                    for (actual, expected) in [
                        (face.x - coverage.x, expected[0]),
                        (coverage.right() - face.right(), expected[1]),
                        (face.y - coverage.y, expected[2]),
                        (coverage.bottom() - face.bottom(), expected[3]),
                    ] {
                        assert!(
                            (actual - expected).abs() < 0.0002,
                            "{scale}x: {actual} != {expected}"
                        );
                    }
                    assert_eq!(m.cells.len(), 2, "V3 retains resting/pressed cells");
                } else if !matches!(scale, 1.0 | 2.0) {
                    let Err(error @ PreparationError::UnsupportedScale(_)) = mapping else {
                        panic!("V1/V2 must preserve the existing scale refusal");
                    };
                    assert_eq!(
                        StagingFailure::Preparation(error).unavailable_cause(source),
                        None,
                        "V1/V2 failure projection must not change",
                    );
                }
            }
        }
        for scale in [0.49, 4.01, f32::NAN, f32::INFINITY, -0.5] {
            // Exercise the material boundary too, independently of earlier
            // physical target validation for malformed scale values.
            let mut metrics = original;
            metrics.scale_factor = scale;
            let source = SdfMaterialSource::ChampagneV4(ChampagneV4Basis::Disabled);
            let input = with_metrics(metrics, source);
            let Err(error @ PreparationError::UnsupportedScale(_)) = MaterialMapping::new(&input)
            else {
                panic!("unsupported {scale}x must stop before a material mapping");
            };
            assert_eq!(
                StagingFailure::Preparation(error).unavailable_cause(source),
                Some(SdfMaterialUnavailableCause::UnsupportedScale)
            );
        }
        for scale in [0.49, 4.01, f32::NAN, f32::INFINITY, -0.5] {
            let mut metrics = original;
            metrics.scale_factor = scale;
            let source = SdfMaterialSource::V3 { dark: false };
            let input = with_metrics(metrics, source);
            let Err(error @ PreparationError::UnsupportedScale(_)) = MaterialMapping::new(&input)
            else {
                panic!("unsupported V3 {scale}x must not allocate a material");
            };
            assert_eq!(
                StagingFailure::Preparation(error).unavailable_cause(source),
                Some(SdfMaterialUnavailableCause::UnsupportedScale),
            );
        }
    }

    #[test]
    fn sdf_window_eligibility_keeps_full_key_and_content_boundaries() {
        let (_tree, a, b) = inputs();
        assert_ne!(a.key, b.key);
        assert!(a.key.same_content_geometry_basis(&b.key));
        let mapping = MaterialMapping::new(&a).unwrap();
        assert!(mapping.same_content(&MaterialMapping::new(&b).unwrap()));
        let mut changed_material = b.clone();
        changed_material.source = SdfMaterialSource::V3 { dark: true };
        assert!(!mapping.same_content(&MaterialMapping::new(&changed_material).unwrap()));
        let metrics = b.key.metrics();
        for (face, viewport) in [
            (
                RectF::new(30.25, 31.5, 84.0, 30.0),
                metrics.final_logical_viewport,
            ),
            (
                metrics.final_logical_rect,
                RectF::new(0.0, 0.0, 200.0, 44.0),
            ),
        ] {
            let target = CustomRenderTargetSnapshot::new(
                metrics.surface_id,
                metrics.target_physical_extent,
                metrics.scale_factor,
            )
            .unwrap()
            .metrics_for(face, viewport)
            .unwrap();
            let changed = SdfMaterialKey::new(
                b.key.tree_lifetime().clone(),
                b.key.owner(),
                target.surface_id,
                b.key.revision(),
                target,
            )
            .unwrap();
            assert_ne!(a.key, changed, "placement and clip remain part of full K");
            assert!(a.key.same_content_geometry_basis(&changed));
            let mut input = b.clone();
            input.key = changed;
            let current_mapping = MaterialMapping::new(&input).unwrap();
            assert!(mapping.same_content(&current_mapping));
            assert_eq!(
                current_mapping.coverage.x - mapping.coverage.x,
                face.x - 30.0
            );
            assert_eq!(
                current_mapping.coverage.y - mapping.coverage.y,
                face.y - 30.0
            );
        }
        for (face, scale) in [
            (RectF::new(30.0, 30.0, 85.0, 30.0), 1.0),
            (metrics.final_logical_rect, 2.0),
        ] {
            let target = CustomRenderTargetSnapshot::new(
                metrics.surface_id,
                metrics.target_physical_extent,
                scale,
            )
            .unwrap()
            .metrics_for(face, metrics.final_logical_viewport)
            .unwrap();
            let changed = SdfMaterialKey::new(
                b.key.tree_lifetime().clone(),
                b.key.owner(),
                target.surface_id,
                b.key.revision(),
                target,
            )
            .unwrap();
            assert!(!a.key.same_content_geometry_basis(&changed));
            let mut input = b.clone();
            input.key = changed;
            assert!(!mapping.same_content(&MaterialMapping::new(&input).unwrap()));
        }
    }

    #[test]
    fn sdf_window_eligibility_never_admits_cancelled_cold_aba() {
        let (_tree, a, b) = inputs();
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real renderer required");
        let mut staging = SdfMaterialStaging::default();
        staging.prepare_before_paint(&mut renderer, std::slice::from_ref(&a), &[]);
        let StagedFront::Unadmitted(front) = &staging.materials[0].front else {
            panic!("cold preparation must remain unadmitted");
        };
        let obsolete_id = front.reservation.as_ref().unwrap().texture_id();
        assert!(
            staging
                .publications_before_paint(&mut renderer, std::slice::from_ref(&a))
                .is_empty()
        );
        staging.prepare_before_paint(&mut renderer, std::slice::from_ref(&b), &[]);
        for _ in 0..4 {
            renderer
                .gpu_resources()
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            staging.prepare_before_paint(&mut renderer, std::slice::from_ref(&a), &[]);
            staging.admit_after_paint(&mut renderer, std::slice::from_ref(&a));
            assert!(!renderer.image_textures.contains_key(&obsolete_id));
            if !staging
                .publications_before_paint(&mut renderer, std::slice::from_ref(&a))
                .is_empty()
            {
                break;
            }
        }
        let publications =
            staging.publications_before_paint(&mut renderer, std::slice::from_ref(&a));
        let Some((publication, _)) = publications.first() else {
            panic!("a fresh cold generation must eventually complete");
        };
        let SdfMaterialStatus::Ready { binding } = publication.status() else {
            panic!("fresh generation must be Ready");
        };
        assert_ne!(binding.identity().texture_id(), obsolete_id);
        staging.prepare_before_paint(&mut renderer, &[], &[]);
        assert!(
            staging.is_empty(),
            "unused generations must retire by actual completion"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sdf_native_exit_retires_current_and_preserves_pending_writer() {
        use crate::renderer::image_binding::ImageBindingFailure;
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        let (_tree, current, obsolete) = inputs();
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let mut staging = SdfMaterialStaging::default();
        let current_only = std::slice::from_ref(&current);
        staging.prepare_before_paint(&mut renderer, current_only, &[]);
        let finish_writes = wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(5)),
        };
        renderer.device.poll(finish_writes.clone()).unwrap();
        assert!(staging.admit_after_paint(&mut renderer, current_only));
        let StagedFront::Admitted(admission) = &staging.materials[0].front else {
            panic!("current generation must be admitted");
        };
        let binding = admission.binding().clone();
        let current_id = binding.identity().texture_id();
        assert_eq!(
            renderer.admit_image(current_id).unwrap().identity(),
            binding.identity()
        );

        staging.prepare_before_paint(&mut renderer, &[current.clone(), obsolete.clone()], &[]);
        let writer_hold = staging
            .hold_writer_completion_observation_for_test(&obsolete.key)
            .unwrap();
        let StagedFront::Unadmitted(front) = &staging.materials[1].front else {
            panic!("obsolete generation must still be unadmitted");
        };
        let obsolete_id = front.reservation.as_ref().unwrap().texture_id();
        staging.prepare_before_paint(&mut renderer, current_only, &[]);
        assert_eq!(staging.materials.len(), 2);
        assert!(!staging.materials[0].retirement_started);
        assert!(staging.materials[1].retirement_pending);

        staging.retire_at_native_exit(&mut renderer);

        assert_eq!(
            binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert!(!renderer.image_textures.contains_key(&current_id));
        assert!(!renderer.sdf_texture_reservations.contains_key(&current_id));
        assert!(!renderer.protected_sdf_texture_ids.contains(&current_id));
        assert_eq!(staging.materials.len(), 1);
        assert_eq!(staging.materials[0].input.key, obsolete.key);
        assert!(staging.materials[0].retirement_pending);
        assert!(renderer.sdf_texture_reservations.contains_key(&obsolete_id));
        assert!(renderer.protected_sdf_texture_ids.contains(&obsolete_id));
        assert!(!renderer.image_textures.contains_key(&obsolete_id));

        // Release the held observation and clean up through the existing owner.
        writer_hold.store(false, Ordering::Release);
        renderer.device.poll(finish_writes).unwrap();
        staging.prepare_before_paint(&mut renderer, &[], &[]);
        assert!(staging.is_empty());
        assert!(!renderer.sdf_texture_reservations.contains_key(&obsolete_id));
        assert!(!renderer.protected_sdf_texture_ids.contains(&obsolete_id));
    }

    #[test]
    fn sdf_active_size_replacement_keeps_cold_gate_and_pending_writer_custody() {
        use std::sync::atomic::Ordering;
        use web_time::{Duration, Instant};

        let (_tree, a, _) = inputs();
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let metrics = a.key.metrics();
        renderer
            .register_headless_render_target(
                metrics.surface_id,
                (
                    metrics.target_physical_extent.width,
                    metrics.target_physical_extent.height,
                ),
                metrics.scale_factor,
            )
            .unwrap();
        let mut staging = SdfMaterialStaging::default();
        staging.prepare_before_paint(&mut renderer, std::slice::from_ref(&a), &[]);
        let cold_hold = staging
            .hold_writer_completion_observation_for_test(&a.key)
            .unwrap();
        assert!(!staging.admit_active_size_replacements_before_paint(
            &mut renderer,
            std::slice::from_ref(&a),
            &[]
        ));
        assert!(
            staging
                .publications_before_paint(&mut renderer, std::slice::from_ref(&a))
                .is_empty()
        );
        assert!(
            !staging.admit_after_paint(&mut renderer, std::slice::from_ref(&a)),
            "initial optional admission must still observe actual writer completion"
        );
        cold_hold.store(false, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !staging.admit_after_paint(&mut renderer, std::slice::from_ref(&a)) {
            assert!(
                Instant::now() < deadline,
                "cold completion was not observed"
            );
            std::thread::yield_now();
        }
        let initial = staging.publications_before_paint(&mut renderer, std::slice::from_ref(&a));
        let SdfMaterialStatus::Ready {
            binding: old_binding,
        } = initial[0].0.status()
        else {
            panic!("initial genuine admission must be Ready");
        };
        let old_binding = old_binding.clone();
        let face = metrics.final_logical_rect;
        let changed_metrics = CustomRenderTargetSnapshot::new(
            metrics.surface_id,
            metrics.target_physical_extent,
            metrics.scale_factor,
        )
        .unwrap()
        .metrics_for(
            RectF::new(face.x, face.y, 76.0, face.height),
            metrics.final_logical_viewport,
        )
        .unwrap();
        let mut b = a.clone();
        b.key = SdfMaterialKey::new(
            a.key.tree_lifetime().clone(),
            a.key.owner(),
            metrics.surface_id,
            a.key.revision(),
            changed_metrics,
        )
        .unwrap();
        staging.prepare_before_paint(
            &mut renderer,
            std::slice::from_ref(&b),
            std::slice::from_ref(&a.key),
        );
        assert_eq!(staging.retained_generation_count(), 2);
        let eligible = std::slice::from_ref(&b.key);
        assert!(staging.active_size_replacement_pending(&mut renderer, &b, eligible));
        assert!(
            !staging.admit_active_size_replacements_before_paint(
                &mut renderer,
                std::slice::from_ref(&b),
                std::slice::from_ref(&a.key)
            ),
            "an unrelated eligible key must not authorize admission"
        );
        let writer_hold = staging
            .hold_writer_completion_observation_for_test(&b.key)
            .unwrap();
        let unsubmitted = {
            let material = staging
                .materials
                .iter()
                .find(|material| material.input == b)
                .unwrap();
            let StagedFront::Unadmitted(front) = &material.front else {
                panic!("new pair must begin unadmitted");
            };
            front
                .producer
                .as_ref()
                .unwrap()
                .begin_frame(Some("outstanding replacement writer"))
                .unwrap()
        };
        assert!(!staging.admit_active_size_replacements_before_paint(
            &mut renderer,
            std::slice::from_ref(&b),
            eligible
        ));
        assert!(staging.active_size_replacement_pending(&mut renderer, &b, eligible));
        drop(unsubmitted); // Exact terminal no-submit cancellation, not writer completion.
        assert!(
            staging.admit_active_size_replacements_before_paint(
                &mut renderer,
                std::slice::from_ref(&b),
                eligible
            ),
            "submitted immutable pair may admit while writer observation remains held"
        );
        assert!(!staging.active_size_replacement_pending(&mut renderer, &b, eligible));
        let replacement =
            staging.publications_before_paint(&mut renderer, std::slice::from_ref(&b));
        let SdfMaterialStatus::Ready {
            binding: new_binding,
        } = replacement[0].0.status()
        else {
            panic!("replacement needs genuine complete current admission");
        };
        let new_binding = new_binding.clone();
        assert_ne!(new_binding.identity(), old_binding.identity());
        assert_eq!(replacement[0].0.key(), &b.key);
        // Neither pair has a sampling use. The replacement's actual pending writer
        // obligation must nevertheless survive its authenticated admission.
        staging.prepare_before_paint(&mut renderer, &[], &[]);
        assert!(!staging.is_empty());
        assert_eq!(new_binding.availability(), Ok(()));
        writer_hold.store(false, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !staging.is_empty() {
            assert!(
                Instant::now() < deadline,
                "writer-certified retirement did not finish"
            );
            staging.prepare_before_paint(&mut renderer, &[], &[]);
            std::thread::yield_now();
        }
        assert_eq!(
            new_binding.availability(),
            Err(crate::renderer::image_binding::ImageBindingFailure::Unregistered)
        );
        assert_eq!(
            old_binding.availability(),
            Err(crate::renderer::image_binding::ImageBindingFailure::Unregistered)
        );
    }

    #[test]
    fn sdf_active_size_replacement_rejects_obsolete_attempt_and_preparation() {
        use crate::core::frame_realization::AppFrameAttempts;
        use std::sync::atomic::Ordering;
        use web_time::{Duration, Instant};

        let (_tree, base, _) = inputs();
        let metrics = base.key.metrics();
        let mut attempts = AppFrameAttempts::new();
        let mut with_attempt = || {
            let target = CustomRenderTargetSnapshot::new(
                metrics.surface_id,
                metrics.target_physical_extent,
                metrics.scale_factor,
            )
            .unwrap()
            .with_app_frame_attempt(attempts.next())
            .metrics_for(metrics.final_logical_rect, metrics.final_logical_viewport)
            .unwrap();
            let mut input = base.clone();
            input.key = SdfMaterialKey::new(
                base.key.tree_lifetime().clone(),
                base.key.owner(),
                metrics.surface_id,
                base.key.revision(),
                target,
            )
            .unwrap();
            input
        };
        let a = with_attempt();
        let b = with_attempt();
        assert_eq!(
            a.key, b.key,
            "resource equality intentionally excludes attempt provenance"
        );
        assert_ne!(a.key.metrics(), b.key.metrics());
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        renderer
            .register_headless_render_target(
                metrics.surface_id,
                (
                    metrics.target_physical_extent.width,
                    metrics.target_physical_extent.height,
                ),
                metrics.scale_factor,
            )
            .unwrap();
        let mut staging = SdfMaterialStaging::default();
        staging.prepare_with_active_size_replacements(
            &mut renderer,
            std::slice::from_ref(&a),
            &[],
            std::slice::from_ref(&a.key),
        );
        let old_nonce = staging.preparation.clone();
        let old_id = match &staging.materials[0].front {
            StagedFront::Unadmitted(front) => front.reservation.as_ref().unwrap().texture_id(),
            _ => panic!("prepared pair has not been admitted"),
        };
        let hold = staging
            .hold_writer_completion_observation_for_test(&a.key)
            .unwrap();
        // The cold entrypoint preserves an old unadmitted candidate, but its
        // earlier preparation nonce cannot authorize the new scoped ingress.
        staging.prepare_before_paint(&mut renderer, std::slice::from_ref(&a), &[]);
        assert!(!staging.materials[0].retirement_started);
        assert!(!Arc::ptr_eq(
            &staging.materials[0].producer_preparation,
            &staging.preparation
        ));
        assert!(
            !staging.admit_active_size_replacements_before_paint(
                &mut renderer,
                std::slice::from_ref(&a),
                std::slice::from_ref(&a.key)
            ),
            "an exact repeated attempt ID/resource K still needs this preparation's nonce"
        );
        assert!(
            !staging.admit_active_size_replacements_before_paint(
                &mut renderer,
                std::slice::from_ref(&b),
                std::slice::from_ref(&b.key)
            ),
            "same resource K from another attempt is not current preparation evidence"
        );
        assert!(
            !staging.admit_active_size_replacements_before_paint(
                &mut renderer,
                std::slice::from_ref(&a),
                std::slice::from_ref(&b.key)
            ),
            "eligible observation provenance must also match"
        );
        assert!(staging.active_size_replacement_pending(
            &mut renderer,
            &b,
            std::slice::from_ref(&b.key)
        ));
        // Resource equality cannot turn an obsolete attempt's finite failure
        // publication into current-target permission to end the Pending gate.
        staging.materials[0].failure = Some(StagingFailure::Completion(
            UnadmittedPreparationError::Uncertified,
        ));
        let stale_failure =
            staging.publications_before_paint(&mut renderer, std::slice::from_ref(&b));
        assert_eq!(stale_failure.len(), 1);
        assert!(matches!(
            stale_failure[0].0.status(),
            SdfMaterialStatus::Unavailable { .. }
        ));
        assert_eq!(stale_failure[0].0.key().metrics(), a.key.metrics());
        assert!(staging.active_size_replacement_pending(
            &mut renderer,
            &b,
            std::slice::from_ref(&b.key)
        ));
        staging.materials[0].failure = None;
        {
            let material = &mut staging.materials[0];
            let StagedFront::Unadmitted(front) = &mut material.front else {
                unreachable!()
            };
            assert!(matches!(
                renderer.register_reserved_sdf_replacement_front(
                    front.reservation.as_ref().unwrap(),
                    &b.key,
                    &material.input.key,
                    &mut front.producer
                ),
                Err(SdfReservationError::KeyMismatch)
            ));
        }
        // Even repeating the same observation ID/resource K starts a distinct
        // preparation. The old unadmitted writer cannot be relabelled.
        staging.prepare_with_active_size_replacements(
            &mut renderer,
            std::slice::from_ref(&a),
            &[],
            std::slice::from_ref(&a.key),
        );
        assert!(!Arc::ptr_eq(&old_nonce, &staging.preparation));
        assert_eq!(staging.retained_generation_count(), 2);
        assert!(staging.materials[0].retirement_started);
        assert!(!Arc::ptr_eq(
            &staging.materials[0].producer_preparation,
            &staging.preparation
        ));
        assert!(Arc::ptr_eq(
            &staging.materials[1].producer_preparation,
            &staging.preparation
        ));
        assert!(staging.admit_active_size_replacements_before_paint(
            &mut renderer,
            std::slice::from_ref(&a),
            std::slice::from_ref(&a.key)
        ));
        assert!(
            !renderer.image_textures.contains_key(&old_id),
            "obsolete front was never admitted"
        );
        let publications =
            staging.publications_before_paint(&mut renderer, std::slice::from_ref(&a));
        let SdfMaterialStatus::Ready { binding } = publications[0].0.status() else {
            panic!("fresh exact preparation must genuinely admit");
        };
        let binding = binding.clone();
        assert_ne!(binding.identity().texture_id(), old_id);

        // Compile can be superseded after genuine admission and before output.
        // The successor still owes its first output for this replacement.
        let produced = staging.preparation_totals.0;
        let successor = std::slice::from_ref(&b);
        let eligible = std::slice::from_ref(&b.key);
        staging.prepare_with_active_size_replacements(&mut renderer, successor, &[], eligible);
        assert_eq!(staging.preparation_totals.0, produced);
        assert!(!staging.admit_active_size_replacements_before_paint(
            &mut renderer,
            successor,
            eligible
        ));
        assert!(!staging.active_size_replacement_pending(&mut renderer, &b, eligible));
        let renewed = staging.publications_before_paint(&mut renderer, successor);
        assert_eq!(renewed.len(), 1);
        let (publication, identity) = &renewed[0];
        assert_eq!(publication.key().metrics(), b.key.metrics());
        assert_eq!(*identity, Some(binding.identity()));
        assert!(matches!(
            publication.status(),
            SdfMaterialStatus::Ready { binding: ready } if ready.identity() == binding.identity()
        ));
        assert!(
            staging
                .publications_before_paint(&mut renderer, std::slice::from_ref(&a))
                .is_empty(),
            "the cancelled attempt's eligibility stays expired"
        );

        hold.store(false, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !staging.is_empty() {
            assert!(
                Instant::now() < deadline,
                "freshness test certified retirement did not finish"
            );
            staging.prepare_before_paint(&mut renderer, &[], &[]);
            std::thread::yield_now();
        }
        assert!(binding.availability().is_err());
    }
}
