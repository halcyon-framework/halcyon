mod appearance;
mod content;
mod context;
#[allow(dead_code)]
pub(crate) mod direct_shadow;
mod domain;
mod geometry;
// Numerical/output owners remain private; checked consumer input is adapted by
// material_api. Backend/source qualification and S3 acceptance stay separate.
#[allow(dead_code)]
pub(crate) mod hdr;
#[allow(dead_code)]
pub(crate) mod indirect;
#[allow(dead_code)]
pub(crate) mod mip_sampling;
// Source math is not a public programmable-shader surface.
mod geometry_cpu;
mod loading_presence;
mod material_api;
#[cfg(test)]
pub(crate) use geometry::cpu_geometry_visits_for_test;
pub use geometry_cpu::{
    PbrGeometryCpuCost3D, PbrGeometryCpuPending3D, PbrGeometryCpuPrepared3D,
    PbrGeometryCpuResult3D, PbrGeometryCpuSuccessorJob3D,
};
pub use loading_presence::{
    LoadingPresenceAssociation3D, LoadingPresenceError3D, LoadingPresenceProxy3D,
    LoadingPresenceProxyPrepared3D, LoadingPresenceRequest3D, LoadingPresenceSource3D,
    LoadingPresenceTransitionDuration3D, PresentationMotionPolicy,
};
pub(crate) mod math;
mod navigation;
mod normal_cpu;
pub(crate) mod object_reveal;
mod outline;
pub use object_reveal::{
    PbrObjectRevealExtent3D, PbrObjectRevealExtentCpuError3D, PbrObjectRevealExtentCpuJob3D,
    PbrObjectRevealExtentCpuResult3D, PbrObjectRevealExtentPending3D, PbrObjectRevealRequest3D,
};
#[cfg(test)]
pub(crate) use outline::validate_programs_for_test as validate_outline_programs_for_test;
pub use outline::{PbrOutlineRequest3D, PbrOutlineSelection3D};
#[allow(dead_code)]
pub(crate) mod pbr;
mod raster;
mod snapshot;
pub(crate) mod spatial_hud;
pub use spatial_hud::{PbrSpatialHud3D, PbrSpatialHudAnchor3D};
pub(crate) mod transfer;
mod viewport;

#[cfg(test)]
pub(crate) mod audit {
    use std::cell::Cell;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub(crate) struct AuditSnapshot3D {
        pub contexts: u64,
        pub resource_generations: u64,
        pub gpu_buffers: u64,
        pub gpu_textures: u64,
        pub gpu_samplers: u64,
        pub raster_states: u64,
        pub shader_modules: u64,
        pub pipelines: u64,
        pub viewports: u64,
        pub targets: u64,
        pub depth_targets: u64,
        pub multisample_targets: u64,
        pub encodes: u64,
        pub publications: u64,
        pub structural_preparations: u64,
        pub dynamic_buffers: u64,
        pub dynamic_bind_groups: u64,
        pub dynamic_writes: u64,
        pub dynamic_bytes_written: u64,
        pub domain_composition_admissions: u64,
        pub domain_state_admissions: u64,
        pub domain_content_lowerings: u64,
        pub domain_frame_admissions: u64,
        pub domain_association_lookups: u64,
    }

    thread_local! {
        static SNAPSHOT: Cell<AuditSnapshot3D> = const {
            Cell::new(AuditSnapshot3D {
                contexts: 0,
                resource_generations: 0,
                gpu_buffers: 0,
                gpu_textures: 0,
                gpu_samplers: 0,
                raster_states: 0,
                shader_modules: 0,
                pipelines: 0,
                viewports: 0,
                targets: 0,
                depth_targets: 0,
                multisample_targets: 0,
                encodes: 0,
                publications: 0,
                structural_preparations: 0,
                dynamic_buffers: 0,
                dynamic_bind_groups: 0,
                dynamic_writes: 0,
                dynamic_bytes_written: 0,
                domain_composition_admissions: 0,
                domain_state_admissions: 0,
                domain_content_lowerings: 0,
                domain_frame_admissions: 0,
                domain_association_lookups: 0,
            })
        };
    }

    fn update(update: impl FnOnce(&mut AuditSnapshot3D)) {
        SNAPSHOT.with(|snapshot| {
            let mut next = snapshot.get();
            update(&mut next);
            snapshot.set(next);
        });
    }

    pub(crate) fn snapshot() -> AuditSnapshot3D {
        SNAPSHOT.get()
    }

    pub(crate) fn record_context() {
        update(|snapshot| snapshot.contexts += 1);
    }

    pub(crate) fn record_resource_generation() {
        update(|snapshot| snapshot.resource_generations += 1);
    }

    pub(crate) fn record_gpu_buffers(count: u64) {
        update(|snapshot| snapshot.gpu_buffers += count);
    }

    pub(crate) fn record_gpu_texture() {
        update(|snapshot| snapshot.gpu_textures += 1);
    }

    pub(crate) fn record_gpu_sampler() {
        update(|snapshot| snapshot.gpu_samplers += 1);
    }

    pub(crate) fn record_raster_state() {
        update(|snapshot| snapshot.raster_states += 1);
    }

    pub(crate) fn record_shader_module() {
        update(|snapshot| snapshot.shader_modules += 1);
    }

    pub(crate) fn record_pipeline() {
        update(|snapshot| snapshot.pipelines += 1);
    }

    pub(crate) fn record_viewport() {
        update(|snapshot| snapshot.viewports += 1);
    }

    pub(crate) fn record_target(sample_count: u32) {
        update(|snapshot| {
            snapshot.targets += 1;
            snapshot.depth_targets += 1;
            if sample_count == 4 {
                snapshot.multisample_targets += 1;
            }
        });
    }

    pub(crate) fn record_encode() {
        update(|snapshot| snapshot.encodes += 1);
    }

    pub(crate) fn record_publication() {
        update(|snapshot| snapshot.publications += 1);
    }

    pub(crate) fn record_structural_preparation() {
        update(|snapshot| snapshot.structural_preparations += 1);
    }

    pub(crate) fn record_dynamic_resources(buffers: u64, bind_groups: u64) {
        update(|snapshot| {
            snapshot.dynamic_buffers += buffers;
            snapshot.dynamic_bind_groups += bind_groups;
        });
    }

    pub(crate) fn record_dynamic_write(bytes: u64) {
        update(|snapshot| {
            snapshot.dynamic_writes += 1;
            snapshot.dynamic_bytes_written += bytes;
        });
    }

    pub(crate) fn record_domain_composition_admission() {
        update(|snapshot| snapshot.domain_composition_admissions += 1);
    }

    pub(crate) fn record_domain_state_admission(content_count: u64) {
        update(|snapshot| {
            snapshot.domain_state_admissions += 1;
            snapshot.domain_content_lowerings += content_count;
        });
    }

    pub(crate) fn record_domain_frame_admission() {
        update(|snapshot| snapshot.domain_frame_admissions += 1);
    }

    pub(crate) fn record_domain_association_lookup() {
        update(|snapshot| snapshot.domain_association_lookups += 1);
    }
}

#[cfg(feature = "render-audit")]
mod cadence_audit {
    use std::cell::Cell;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct Viewport3DCadenceSnapshot {
        pub custom_preparations: u64,
        pub unchanged_preparations: u64,
        pub inactive_preparations: u64,
        pub failed_preparations: u64,
        pub encodes: u64,
        pub submissions: u64,
        pub swaps: u64,
        pub publications: u64,
    }

    thread_local! {
        static SNAPSHOT: Cell<Viewport3DCadenceSnapshot> = const {
            Cell::new(Viewport3DCadenceSnapshot {
                custom_preparations: 0,
                unchanged_preparations: 0,
                inactive_preparations: 0,
                failed_preparations: 0,
                encodes: 0,
                submissions: 0,
                swaps: 0,
                publications: 0,
            })
        };
    }

    fn update(update: impl FnOnce(&mut Viewport3DCadenceSnapshot)) {
        SNAPSHOT.with(|snapshot| {
            let mut next = snapshot.get();
            update(&mut next);
            snapshot.set(next);
        });
    }

    pub fn snapshot() -> Viewport3DCadenceSnapshot {
        SNAPSHOT.get()
    }

    pub(crate) fn record_custom_preparation() {
        update(|snapshot| snapshot.custom_preparations += 1);
    }

    pub(crate) fn record_unchanged_preparation() {
        update(|snapshot| snapshot.unchanged_preparations += 1);
    }

    pub(crate) fn record_inactive_preparation() {
        update(|snapshot| snapshot.inactive_preparations += 1);
    }

    pub(crate) fn record_failed_preparation() {
        update(|snapshot| snapshot.failed_preparations += 1);
    }

    pub(crate) fn record_encode() {
        update(|snapshot| snapshot.encodes += 1);
    }

    pub(crate) fn record_submission_and_swap() {
        update(|snapshot| {
            snapshot.submissions += 1;
            snapshot.swaps += 1;
        });
    }

    pub(crate) fn record_publication() {
        update(|snapshot| snapshot.publications += 1);
    }
}

#[cfg(feature = "render-audit")]
pub use cadence_audit::{Viewport3DCadenceSnapshot, snapshot as viewport_cadence_snapshot};

pub use appearance::{
    APPEARANCE_BLOCK_SIZE, AbiLayout3D, Appearance3DAdmissionError, AppearanceAdmissionBudget,
    AppearanceFallbackBindingContract, AppearanceSourceDiagnostic, AppearanceSourceDiagnosticLabel,
    AppearanceSourceLocation, AppearanceSourceRule, NAGA_ADAPTER_VERSION, OBJECT_BLOCK_SIZE,
    VIEW_BLOCK_SIZE, WGPU_ADAPTER_VERSION,
};
#[cfg(test)]
pub(crate) use audit as viewport_audit;
pub use content::{
    Appearance3DError, AppearanceMode, AppearanceSourceDescriptor, AppearanceTextureBinding,
    CANONICAL_COLOR_OFFSET, CANONICAL_NORMAL_OFFSET, CANONICAL_POSITION_OFFSET,
    CANONICAL_UV_OFFSET, CANONICAL_VERTEX_STRIDE, DEFAULT_LINEAR_VERTEX_COLOR, Geometry3DError,
    GeometryIndices, GeometrySemantics, GeometrySourceDescriptor, LinearVertexColorRgba8,
    Sampler3D, SamplerAddressMode, SamplerFilterMode, Texture3D, Texture3DError,
    TextureCoordinate2, TextureFormat3D,
};
pub use context::{
    AppearanceHandle, AppearanceLease, Context3D, Context3DError, DeviceLimits3D,
    DomainRetentionBudget3D, DomainRetentionError3D, DomainRetentionLimits3D,
    DomainRetentionUsage3D, EffectiveLimits3D, GeometryHandle, GeometryLease, HardLimit3D,
    LimitProfile3D, PolicyBudget3D, PolicyLimits3D, ResourceKind3D, ResourceUsage3D, SamplerHandle,
    SamplerLease, TextureHandle, TextureLease,
};
pub use domain::{
    CommittedDomain3, DomainAssociation3Error, DomainComposition3, DomainComposition3Descriptor,
    DomainComposition3Error, DomainContent3Descriptor, DomainContentId3, DomainContentPlacement3,
    DomainEmbedding3, DomainFrame3, DomainFrame3Descriptor, DomainFrame3Error, DomainPoint2,
    DomainPoint3, DomainRay3, DomainState3, DomainState3Descriptor, DomainState3Error,
    DomainStateRevision3, DomainVector3, LocalDomain3Descriptor, LocalDomainId3, LocalDomainKind3,
    PlanarDomainHit3, PlanarRayAccess3, PlanarRaySide3,
};
pub(crate) use geometry::{CanonicalGeometryBytes, CanonicalIndexFormat};
#[cfg(any(test, feature = "test-harness"))]
pub use hdr::HdrRenderTiming3D;
pub use material_api::{
    PbrAlphaMode3D, PbrDirectDescriptor3D, PbrDirectSource3D, PbrDirectionalLight3D,
    PbrDrawPacket3D, PbrEnvironment3D, PbrEnvironmentAsset3D, PbrEnvironmentInput3D,
    PbrEnvironmentRole3D, PbrFrameCertificate3D, PbrGeometry3D, PbrGeometryDescriptor3D, PbrGi3D,
    PbrGiDependencies3D, PbrGiDescriptor3D, PbrHdrSource3D, PbrImageOrigin3D,
    PbrIndirectDescriptor3D, PbrIndirectSource3D, PbrMaterial3D, PbrMaterialDescriptor3D,
    PbrMaterialRoles3D, PbrMipChain3D, PbrMipChainDescriptor3D, PbrMipGenerator3D, PbrMipLevel3D,
    PbrMipMin3D, PbrMipProvenance3D, PbrMipRole3D, PbrMipSampler3D, PbrMipSamplerDescriptor3D,
    PbrModelState3D, PbrNormalBox3D, PbrNormalCell3D, PbrNormalCertificate3D, PbrNormalException3D,
    PbrNormalWitness3D, PbrOriginalGeometry3D, PbrOriginalHandedness3D, PbrOriginalIdentity3D,
    PbrOriginalVertex3D, PbrPointLight3D, PbrProducerEvidence3D, PbrRequiredRoles3D,
    PbrRoleBinding3D, PbrSamplingFamily3D, PbrScene3D, PbrSceneDescriptor3D,
    PbrSmoothSingularity3D, PbrSnapshot3D, PbrSourceError3D, PbrSourceReflectionAxis3D,
    PbrSourceResult3D, PbrSpotLight3D, PbrTangentProvenance3D, PbrTextureRole3D,
    PbrTransportInput3D, PbrTriangleCertificate3D, PbrVertex3D, PbrViewportTuple3D,
};
pub use math::{
    LocalPoint3, LocalRay3, LocalVector3, Math3DError, ModelPlacement3, Orientation3,
    OrthographicProjection, PerspectiveFar, PerspectiveProjection, PositiveScale3, Projection3,
    Ray3, RayHit3, RayIntervalHit3, RenderRelativeTranslation3, RigidPose3, TriangleHit3,
    ViewDescriptor3, WorldPoint3, WorldVector3,
};
pub use navigation::{
    CameraNavigationController3, CameraNavigationError, CameraNavigationMode3,
    CameraNavigationPolicy3, CameraNavigationPolicyDescriptor3, CameraPointerBindings3,
    CameraPointerOperation3, CameraState3, CameraWheelOperation3, FlyKeyBindings3,
    FlyMotionOperation3,
};
pub use normal_cpu::{
    PbrNormalCpuProofError3D, PbrNormalCpuProofErrorKind3D, PbrNormalCpuProofJob3D,
    PbrNormalCpuProofPending3D, PbrNormalCpuProofPrepared3D, PbrNormalCpuProofResult3D,
    PbrNormalCpuProofSource3D,
};
#[cfg(any(test, feature = "test-harness"))]
pub use raster::PbrPreparationTiming3D;
#[cfg(test)]
pub(crate) use raster::{HeadlessRaster3DError, HeadlessRasterOutput};
pub use snapshot::{
    DrawPacketDescriptor, ModelRevision, ModelState, ModelStateChange, ModelStateDescriptor,
    ModelStateError, RasterMode, RenderSnapshot, RenderSnapshot3DError, RenderSnapshotChange,
    RenderSnapshotDescriptor, StructuralRevision,
};
pub use transfer::{
    FlatViewport3DIndeterminateCause, FlatViewport3DLinkId, FlatViewport3DLinkResult,
    FlatViewport3DPublicationStatus, FlatViewport3DRetirementReason, FlatViewport3DStaleReason,
    FlatViewport3DTransferError, FlatViewport3DTransferOutcome, FlatViewport3DTransferStage,
};
#[cfg(any(test, feature = "test-harness"))]
pub use viewport::PbrViewTiming3D;
#[cfg(feature = "test-harness")]
pub use viewport::{PbrFrontPixelForTestHarness3D, PbrFrontProofError3D};
pub use viewport::{
    SampleCount3D, Viewport3D, Viewport3DError, Viewport3DFailure, Viewport3DLifecycleState,
    Viewport3DRequestSerial, Viewport3DStatus, Viewport3DTargetCompatibilityKey,
    Viewport3DTargetStatus, Viewport3DTuple, Viewport3DViewRevision,
};
