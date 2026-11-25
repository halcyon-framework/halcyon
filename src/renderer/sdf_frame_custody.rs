//! Private per-surface custody beside the frozen public scene/plan/output types.
//! A reference is issued only by an admitted resource owner before Image emission.

use std::collections::HashMap;
use std::sync::Arc;

use super::compiled_draw_plan::{MultiTargetDrawPlan, MultiTargetRenderOutput};
use super::draw_plan::DrawPlan;
use super::image_binding::ImageBindingIdentity;
use super::sdf_image_generation_custody::{SdfImageCpuUse, SdfImageCustodyError};
use super::sdf_material_status::SdfMaterialKey;
use crate::compositor::{MultiSurfaceContract, SurfaceId};

#[derive(Debug)]
pub(crate) struct SdfFrameCustodyError(SdfImageCustodyError);

impl From<SdfImageCustodyError> for SdfFrameCustodyError {
    fn from(error: SdfImageCustodyError) -> Self {
        Self(error)
    }
}

impl std::fmt::Display for SdfFrameCustodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SDF frame custody: {:?}", self.0)
    }
}

impl std::error::Error for SdfFrameCustodyError {}

struct RetainedImageUse {
    key: SdfMaterialKey,
    cpu: SdfImageCpuUse,
}

/// Clones share the original CPU use until every retained/assembled reference
/// has gone away. They never silently cancel an independently queued frame use.
#[derive(Clone)]
pub(crate) struct SdfImageReference(Arc<RetainedImageUse>);

impl SdfImageReference {
    pub(super) fn new(key: SdfMaterialKey, cpu: SdfImageCpuUse) -> Self {
        Self(Arc::new(RetainedImageUse { key, cpu }))
    }

    fn surface(&self) -> SurfaceId {
        self.0.key.metrics().surface_id
    }

    fn identity(&self) -> ImageBindingIdentity {
        self.0.cpu.binding().identity()
    }
}

/// Engine scene/cache ownership only. SceneGraph itself remains pure render data.
/// Empty ordinary scenes allocate no custody table.
#[derive(Clone, Default)]
pub(crate) struct SdfSceneCustody {
    surfaces: Option<Box<HashMap<SurfaceId, Vec<SdfImageReference>>>>,
}

impl SdfSceneCustody {
    pub(super) fn is_empty(&self) -> bool {
        self.surfaces
            .as_ref()
            .is_none_or(|surfaces| surfaces.is_empty())
    }

    /// The engine must pair this resource-issued reference with the actual Image
    /// emission, and replace/remove it only after old retained E is removed.
    pub(crate) fn retain_reference(&mut self, reference: SdfImageReference) {
        let references = self
            .surfaces
            .get_or_insert_with(Default::default)
            .entry(reference.surface())
            .or_default();
        if !references
            .iter()
            .any(|old| old.0.key == reference.0.key && old.identity() == reference.identity())
        {
            references.push(reference);
        }
    }

    /// Called by the retained owner only after its old Image/E is removed.
    /// Cloned scenes/plans keep their independent shared reference alive.
    pub(crate) fn release_after_removal(
        &mut self,
        key: &SdfMaterialKey,
        identity: ImageBindingIdentity,
    ) {
        let Some(surfaces) = self.surfaces.as_mut() else {
            return;
        };
        surfaces.retain(|_, references| {
            references
                .retain(|reference| &reference.0.key != key || reference.identity() != identity);
            !references.is_empty()
        });
        if surfaces.is_empty() {
            self.surfaces = None;
        }
    }

    pub(crate) fn has_reference(
        &self,
        key: &SdfMaterialKey,
        identity: ImageBindingIdentity,
    ) -> bool {
        self.surfaces
            .as_ref()
            .and_then(|surfaces| surfaces.get(&key.metrics().surface_id))
            .is_some_and(|references| {
                references.iter().any(|reference| {
                    &reference.0.key == key
                        && reference.identity() == identity
                        && reference.0.cpu.validate_retained_reference().is_ok()
                })
            })
    }

    pub(crate) fn references(&self) -> Vec<(SdfMaterialKey, ImageBindingIdentity)> {
        self.surfaces
            .iter()
            .flat_map(|surfaces| surfaces.values())
            .flatten()
            .map(|reference| (reference.0.key.clone(), reference.identity()))
            .collect()
    }

    pub(super) fn contains(&self, surface: SurfaceId, identity: ImageBindingIdentity) -> bool {
        self.surfaces
            .as_ref()
            .and_then(|surfaces| surfaces.get(&surface))
            .is_some_and(|references| {
                references.iter().any(|reference| {
                    reference.identity() == identity
                        && reference.0.cpu.validate_retained_reference().is_ok()
                })
            })
    }

    pub(super) fn retain_compiled(&mut self, compiled: &MultiTargetDrawPlan) {
        let Some(surfaces) = self.surfaces.as_mut() else {
            return;
        };
        surfaces.retain(|surface, references| {
            let Some(data) = compiled.surface_compiled_data(*surface) else {
                return false;
            };
            references.retain(|reference| {
                let identity = reference.identity();
                data.frame_batches.batches.iter().any(|batch| {
                    batch.key.pipeline == super::batcher::PipelineId::Image
                        && batch.key.texture_id == Some(identity.texture_id())
                        && batch.key.image_generation == Some(identity.generation())
                })
            });
            !references.is_empty()
        });
        if surfaces.is_empty() {
            self.surfaces = None;
        }
    }

    pub(super) fn output_uses(
        &self,
        surface: SurfaceId,
    ) -> Result<Vec<SdfImageCpuUse>, SdfImageCustodyError> {
        self.surfaces
            .as_ref()
            .and_then(|surfaces| surfaces.get(&surface))
            .into_iter()
            .flatten()
            .map(|reference| reference.0.cpu.fork_queued_reference())
            .collect()
    }
}

/// Consuming this wrapper is the only SDF-aware Compile ingress. Cloning it
/// preserves the retained reference while each produced output gets its own use.
#[derive(Clone)]
pub(crate) struct SdfFramePlan {
    pub(super) plan: DrawPlan<MultiSurfaceContract>,
    pub(super) custody: SdfSceneCustody,
}

impl SdfFramePlan {
    pub(crate) fn new(plan: DrawPlan<MultiSurfaceContract>, custody: SdfSceneCustody) -> Self {
        Self { plan, custody }
    }
}

pub(crate) struct SdfCompiledFrame {
    pub(super) plan: MultiTargetDrawPlan,
    pub(super) custody: SdfSceneCustody,
    pub(super) companions: Option<super::wgpu_renderer::spatial_hud::CompiledHudFrame>,
}

impl SdfCompiledFrame {
    pub(crate) fn plan(&self) -> &MultiTargetDrawPlan {
        &self.plan
    }

    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn companion_work(
        &self,
    ) -> impl Iterator<
        Item = (
            SurfaceId,
            &Arc<super::wgpu_renderer::spatial_hud::SpatialHudWork>,
        ),
    > {
        self.companions
            .iter()
            .flat_map(|companions| companions.work())
    }

    #[cfg(test)]
    pub(crate) fn companion_refusals_for_test(
        &self,
    ) -> &[(SurfaceId, crate::three_d::PbrSourceError3D)] {
        self.companions
            .as_ref()
            .map_or(&[], |companions| companions.refusals())
    }

    #[cfg(test)]
    pub(crate) fn companion_draw_order_for_test(
        &self,
        surface: SurfaceId,
    ) -> &[(bool, crate::core::paint::EffectivePaintOrder)] {
        self.companions
            .as_ref()
            .map_or(&[], |companions| companions.draw_order_for_test(surface))
    }

    #[cfg(test)]
    pub(crate) fn submit_headless_for_test(
        self,
        renderer: &super::wgpu_renderer::WgpuRenderer,
        surface: SurfaceId,
        view: &wgpu::TextureView,
    ) -> Result<impl Fn() -> bool + use<>, SdfFrameCustodyError> {
        let uses = self.custody.output_uses(surface)?;
        assert!(!uses.is_empty(), "headless proof must sample SDF custody");
        let command = renderer
            .encode_compiled_target_for_test(&self.plan, surface, view)
            .expect("real compiled SDF scene must encode");
        let completions = renderer.submit_frame_command_with_sdf_custody(command, uses)?;
        assert!(!completions.is_empty());
        Ok(move || completions.iter().all(|completion| completion.observed()))
    }
}

/// Dropping an unsubmitted native output cancels exactly its CPU uses. The
/// public SurfaceTexture output shape and all ordinary call sites stay valid.
pub(crate) struct SdfRenderOutput {
    // Separate HAL381 custody, not SDF admission or a different frame owner.
    pub(super) presence: Vec<crate::renderer::wgpu_renderer::PresenceFrameUse>,
    pub(super) companion:
        Option<crate::renderer::wgpu_renderer::spatial_hud::PresentationCompanionUse>,
    pub(super) output: MultiTargetRenderOutput,
    pub(super) custody: Vec<SdfImageCpuUse>,
}

impl SdfRenderOutput {
    pub(crate) fn surface_id(&self) -> SurfaceId {
        self.output.surface_id
    }
}
