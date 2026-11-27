use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use thiserror::Error;

use crate::compositor::SurfaceId;
use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT};
use crate::core::widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::renderer::wgpu_renderer::GpuDeviceFailure;
use crate::renderer::{
    CustomRenderActivity, CustomRenderAdmission, CustomRenderParticipant, CustomRenderPass,
    CustomRenderPassError, CustomRenderPreparationOutcome, CustomRenderTargetMetrics,
    PhysicalExtent, PhysicalRect,
};
use crate::utils::id::WidgetInternalId;

#[cfg(test)]
use super::audit;
#[cfg(feature = "render-audit")]
use super::cadence_audit;
use super::context::{TargetBudgetLease3D, ViewLease3D};
use super::hdr::{HdrError3D, HdrOutput3D, HdrSource3D};
use super::object_reveal::{ObjectRevealFrame3D, PbrObjectRevealRequest3D};
use super::raster::{
    DEPTH_FORMAT, HeadlessRaster3DError, PreparedRaster3D, Viewport3DRasterTarget,
    prepare_viewport_snapshot_raster, validate_viewport_raster_target,
};
use super::transfer::FlatViewport3DWriterAuthority;
use super::{
    CommittedDomain3, Context3D, Context3DError, DomainAssociation3Error, DomainFrame3,
    DomainFrame3Error, DomainRay3, LocalDomainId3, Math3DError, ModelRevision, ModelState,
    ModelStateError, PolicyBudget3D, Ray3, RenderSnapshot, RenderSnapshot3DError,
    StructuralRevision, ViewDescriptor3, WorldPoint3,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Viewport3DViewRevision(u64);

impl Viewport3DViewRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, Viewport3DError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Viewport3DError::ViewRevisionExhausted { revision: self })
    }
}

impl fmt::Display for Viewport3DViewRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Viewport3DRequestSerial(u64);

impl Viewport3DRequestSerial {
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Viewport3DRequestSerial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleCount3D(u32);

impl SampleCount3D {
    pub const ONE: Self = Self(1);
    pub const FOUR: Self = Self(4);

    pub fn try_new(value: u32) -> Result<Self, Viewport3DError> {
        match value {
            1 => Ok(Self::ONE),
            4 => Ok(Self::FOUR),
            requested => Err(Viewport3DError::UnsupportedSampleCount { requested }),
        }
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport3DTargetCompatibilityKey {
    participant_identity: u64,
    context_identity: usize,
    surface_id: SurfaceId,
    physical_render_extent: PhysicalExtent,
    color_format: wgpu::TextureFormat,
    sample_count: SampleCount3D,
}

impl Viewport3DTargetCompatibilityKey {
    pub const fn surface_id(self) -> SurfaceId {
        self.surface_id
    }

    pub const fn physical_render_extent(self) -> PhysicalExtent {
        self.physical_render_extent
    }

    pub const fn color_format(self) -> wgpu::TextureFormat {
        self.color_format
    }

    pub const fn sample_count(self) -> SampleCount3D {
        self.sample_count
    }
}

#[derive(Debug, Clone)]
pub struct Viewport3DTuple {
    request_serial: Viewport3DRequestSerial,
    target_key: Viewport3DTargetCompatibilityKey,
    view_revision: Viewport3DViewRevision,
    view: ViewDescriptor3,
    snapshot: RenderSnapshot,
    model: ModelState,
    request_kind: Viewport3DRequestKind,
}

impl Viewport3DTuple {
    pub const fn request_serial(&self) -> Viewport3DRequestSerial {
        self.request_serial
    }

    pub const fn target_key(&self) -> Viewport3DTargetCompatibilityKey {
        self.target_key
    }

    pub const fn view_revision(&self) -> Viewport3DViewRevision {
        self.view_revision
    }

    pub const fn view(&self) -> ViewDescriptor3 {
        self.view
    }

    pub fn snapshot(&self) -> &RenderSnapshot {
        &self.snapshot
    }

    pub fn model_revision(&self) -> ModelRevision {
        self.model.revision()
    }

    pub fn structural_revision(&self) -> StructuralRevision {
        self.snapshot.structural_revision()
    }

    pub fn model(&self) -> &ModelState {
        &self.model
    }

    pub const fn sample_count(&self) -> SampleCount3D {
        self.target_key.sample_count
    }

    pub fn domain_frame(&self) -> Option<&DomainFrame3> {
        self.request_kind.domain_frame()
    }
}

/// Internal extended-source record. Its source Arc owns the exact material
/// snapshot/model and every HDR/D/E/G/role generation; no legacy payload exists.
#[derive(Debug, Clone)]
pub(crate) struct MaterialViewport3DTuple {
    request_serial: Viewport3DRequestSerial,
    target_key: Viewport3DTargetCompatibilityKey,
    view_revision: Viewport3DViewRevision,
    view: ViewDescriptor3,
    source: Arc<super::pbr::MaterialSceneSource3D>,
}

impl MaterialViewport3DTuple {
    pub(crate) const fn request_serial(&self) -> Viewport3DRequestSerial {
        self.request_serial
    }
    pub(crate) const fn target_key(&self) -> Viewport3DTargetCompatibilityKey {
        self.target_key
    }
    pub(crate) const fn view_revision(&self) -> Viewport3DViewRevision {
        self.view_revision
    }
    pub(crate) const fn view(&self) -> ViewDescriptor3 {
        self.view
    }
    pub(crate) fn structural_revision(&self) -> StructuralRevision {
        self.source.snapshot.revision
    }
    pub(crate) fn model_revision(&self) -> ModelRevision {
        self.source.model.revision
    }
    pub(crate) fn source(&self) -> &Arc<super::pbr::MaterialSceneSource3D> {
        &self.source
    }
}

#[derive(Debug, Clone)]
enum RetainedSourceTuple3D {
    Legacy(Viewport3DTuple),
    Material(MaterialViewport3DTuple),
}

impl RetainedSourceTuple3D {
    const fn legacy(&self) -> Option<&Viewport3DTuple> {
        match self {
            Self::Legacy(tuple) => Some(tuple),
            Self::Material(_) => None,
        }
    }
    const fn material(&self) -> Option<&MaterialViewport3DTuple> {
        match self {
            Self::Material(tuple) => Some(tuple),
            Self::Legacy(_) => None,
        }
    }
    fn target_key(&self) -> Viewport3DTargetCompatibilityKey {
        match self {
            Self::Legacy(t) => t.target_key,
            Self::Material(t) => t.target_key,
        }
    }
    fn view_revision(&self) -> Viewport3DViewRevision {
        match self {
            Self::Legacy(t) => t.view_revision,
            Self::Material(t) => t.view_revision,
        }
    }
    fn view(&self) -> ViewDescriptor3 {
        match self {
            Self::Legacy(t) => t.view,
            Self::Material(t) => t.view,
        }
    }
    fn matches(
        &self,
        key: Viewport3DTargetCompatibilityKey,
        requested: &RequestedSemanticState,
    ) -> bool {
        if self.target_key() != key
            || self.view_revision() != requested.view_revision
            || self.view() != requested.view
        {
            return false;
        }
        match (self, &requested.source) {
            (
                Self::Legacy(t),
                RequestedSource3D::Legacy {
                    snapshot,
                    model,
                    kind,
                },
            ) => {
                t.snapshot.same_identity(snapshot)
                    && t.model.same_identity(model)
                    && t.request_kind.same_identity(kind)
            }
            (Self::Material(t), RequestedSource3D::Material(source)) => {
                Arc::ptr_eq(&t.source, source)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewport3DLifecycleState {
    Uninitialized,
    Inactive,
    ReadyWithoutCommit,
    Committed,
    FailedWithLastGood,
    IncompatibleTarget,
    Retired,
    TerminalDeviceFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewport3DFailure {
    request_serial: Viewport3DRequestSerial,
    error: Viewport3DError,
}

impl Viewport3DFailure {
    pub const fn request_serial(&self) -> Viewport3DRequestSerial {
        self.request_serial
    }

    pub const fn error(&self) -> &Viewport3DError {
        &self.error
    }
}

#[derive(Debug, Clone)]
pub struct Viewport3DStatus {
    state: Viewport3DLifecycleState,
    requested: Option<RetainedSourceTuple3D>,
    committed: Option<RetainedSourceTuple3D>,
    failure: Option<Viewport3DFailure>,
    device_failure: Option<GpuDeviceFailure>,
}

/// An observation of one existing, current R027 final FRONT. It retains the
/// actual immutable PBR source, never a texture, view, Device or Queue handle.
#[cfg(feature = "test-harness")]
#[derive(Debug, Clone)]
pub struct PbrFrontPixelForTestHarness3D {
    pub source: super::material_api::PbrScene3D,
    pub request_serial: Viewport3DRequestSerial,
    pub target_key: Viewport3DTargetCompatibilityKey,
    pub view_revision: Viewport3DViewRevision,
    pub view: ViewDescriptor3,
    pub image_id: u64,
    pub extent: [u32; 2],
    pub pixel: [u32; 2],
    pub rgba: [f32; 4],
}

#[cfg(feature = "test-harness")]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PbrFrontProofError3D {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error("surface {0:?} has no retained target")]
    MissingTarget(SurfaceId),
    #[error("target does not contain a PBR front")]
    NotPbr,
    #[error("target is not wholly committed to the current PBR request")]
    NotCurrent,
    #[error("PBR front or its renderer custody is unavailable")]
    FrontUnavailable,
    #[error("the existing Device is unavailable")]
    DeviceUnavailable,
    #[error("pixel {pixel:?} is outside the actual front extent {extent:?}")]
    PixelOutOfBounds { pixel: [u32; 2], extent: [u32; 2] },
    #[error("the actual front is not single-sample Rgba16Float at the committed extent")]
    InvalidFinal,
    #[error("the actual FP16 pixel is negative or nonfinite")]
    InvalidPixel,
    #[error(transparent)]
    Image(#[from] crate::renderer::wgpu_renderer::TestHarnessImageProofError),
}

impl Viewport3DStatus {
    fn uninitialized() -> Self {
        Self {
            state: Viewport3DLifecycleState::Uninitialized,
            requested: None,
            committed: None,
            failure: None,
            device_failure: None,
        }
    }

    /// Full target lifecycle for either source family. A Material front can be
    /// Committed while the legacy-only `committed()` projection is None.
    pub const fn state(&self) -> Viewport3DLifecycleState {
        self.state
    }

    /// Only the actual Legacy request. None means no Legacy tuple in this slot,
    /// not absence of a Material request or frame.
    pub const fn requested(&self) -> Option<&Viewport3DTuple> {
        match self.requested.as_ref() {
            Some(tuple) => tuple.legacy(),
            None => None,
        }
    }

    /// Only the actual Legacy front. Material last-good is never projected as
    /// Legacy; lifecycle and family-neutral committed-view queries still apply.
    pub const fn committed(&self) -> Option<&Viewport3DTuple> {
        match self.committed.as_ref() {
            Some(tuple) => tuple.legacy(),
            None => None,
        }
    }

    pub(crate) fn requested_material(&self) -> Option<&MaterialViewport3DTuple> {
        self.requested.as_ref()?.material()
    }

    pub(crate) fn committed_material(&self) -> Option<&MaterialViewport3DTuple> {
        self.committed.as_ref()?.material()
    }

    pub const fn failure(&self) -> Option<&Viewport3DFailure> {
        self.failure.as_ref()
    }

    pub const fn device_failure(&self) -> Option<&GpuDeviceFailure> {
        self.device_failure.as_ref()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport3DTargetStatus<'a> {
    surface_id: SurfaceId,
    metrics: Option<CustomRenderTargetMetrics>,
    status: &'a Viewport3DStatus,
}

impl<'a> Viewport3DTargetStatus<'a> {
    pub const fn surface_id(self) -> SurfaceId {
        self.surface_id
    }

    pub const fn metrics(self) -> Option<CustomRenderTargetMetrics> {
        self.metrics
    }

    pub const fn status(self) -> &'a Viewport3DStatus {
        self.status
    }

    /// Constructs a mechanical ray only from this target's active compatible
    /// committed view and physical render extent. It performs no scene traversal
    /// or semantic picking.
    pub fn committed_ray(self, u: f64, v: f64) -> Result<Option<Ray3>, Math3DError> {
        let Some((committed, physical_render_extent)) = self.active_committed_tuple() else {
            return Ok(None);
        };
        committed
            .view()
            .ray_for_committed_extent(
                physical_render_extent.width,
                physical_render_extent.height,
                u,
                v,
            )
            .map(Some)
    }

    pub fn committed_domain(self, domain_id: LocalDomainId3) -> Option<CommittedDomain3> {
        #[cfg(test)]
        audit::record_domain_association_lookup();
        let (source, _) = self.active_committed_tuple()?;
        let committed = source.legacy()?;
        let frame = committed.domain_frame()?;
        let domain_index = frame
            .states()
            .iter()
            .position(|state| state.domain_id() == domain_id)?;
        Some(CommittedDomain3::new(
            committed.target_key(),
            committed.request_serial(),
            committed.view_revision(),
            frame.clone(),
            domain_index,
        ))
    }

    pub fn committed_domain_ray(
        self,
        domain_id: LocalDomainId3,
        u: f64,
        v: f64,
    ) -> Result<Option<(CommittedDomain3, DomainRay3)>, DomainAssociation3Error> {
        let Some(association) = self.committed_domain(domain_id) else {
            return Ok(None);
        };
        let Some(world_ray) = self.committed_ray(u, v)? else {
            return Ok(None);
        };
        let domain_ray = association.to_domain_ray(world_ray)?;
        Ok(Some((association, domain_ray)))
    }

    pub fn is_current_domain_association(self, association: &CommittedDomain3) -> bool {
        self.committed_domain(association.domain_id())
            .is_some_and(|current| current.same_association(association))
    }

    fn active_committed_tuple(self) -> Option<(&'a RetainedSourceTuple3D, PhysicalExtent)> {
        let metrics = self.metrics?;
        let CustomRenderActivity::Active {
            physical_render_extent,
            ..
        } = metrics.activity
        else {
            return None;
        };
        if !matches!(
            self.status.state(),
            Viewport3DLifecycleState::Committed | Viewport3DLifecycleState::FailedWithLastGood
        ) {
            return None;
        }
        let committed = self.status.committed.as_ref()?;
        if committed.target_key().physical_render_extent != physical_render_extent {
            return None;
        }
        Some((committed, physical_render_extent))
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Viewport3DError {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error(transparent)]
    Snapshot(#[from] RenderSnapshot3DError),
    #[error(transparent)]
    Model(#[from] ModelStateError),
    #[error(transparent)]
    DomainFrame(#[from] DomainFrame3Error),
    #[error(transparent)]
    Math(#[from] Math3DError),
    #[error(transparent)]
    CustomRender(#[from] CustomRenderPassError),
    #[error("view revision {revision} exhausted its non-wrapping space")]
    ViewRevisionExhausted { revision: Viewport3DViewRevision },
    #[error("view revision {requested} is stale; current revision is {current}")]
    StaleViewRevision {
        current: Viewport3DViewRevision,
        requested: Viewport3DViewRevision,
    },
    #[error("view revision {revision} is already bound to another immutable descriptor")]
    ReusedViewRevision { revision: Viewport3DViewRevision },
    #[error("3D viewport request serial exhausted its non-wrapping space")]
    RequestSerialExhausted,
    #[error("unsupported baseline 3D sample count {requested}; expected 1 or 4")]
    UnsupportedSampleCount { requested: u32 },
    #[error("3D target is inactive at {width}x{height}")]
    InactiveTarget { width: u32, height: u32 },
    #[error("target format {format:?} does not support sample count {sample_count}")]
    UnsupportedTargetFormat {
        format: wgpu::TextureFormat,
        sample_count: u32,
    },
    #[error("Depth32Float does not support sample count {sample_count}")]
    UnsupportedDepthFormat { sample_count: u32 },
    #[error("unable to reserve bounded {block} uniform storage for {bytes} bytes")]
    UniformStorageUnavailable { block: &'static str, bytes: u64 },
    #[error("dynamic uniform offset {offset} is not representable as u32")]
    DynamicOffsetOutOfRange { offset: u64 },
    #[error("render snapshot packet and committed-lease arrays are inconsistent")]
    SnapshotInvariant,
    #[error("Viewport3D request kind is fixed by its constructor")]
    RequestKindMismatch,
    /// The linked mount accepts requests only from its engine-qualified Update cutoff.
    #[error("INV-355-G1-EXCLUSIVE-ADMISSION: the linked viewport request writer is exclusive")]
    LinkedWriterExclusive,
}

impl From<HeadlessRaster3DError> for Viewport3DError {
    fn from(error: HeadlessRaster3DError) -> Self {
        match error {
            HeadlessRaster3DError::Context(error) => Self::Context(error),
            HeadlessRaster3DError::Math(error) => Self::Math(error),
            HeadlessRaster3DError::Model(error) => Self::Model(error),
            HeadlessRaster3DError::InactiveExtent { width, height } => {
                Self::InactiveTarget { width, height }
            }
            HeadlessRaster3DError::UnsupportedSampleCount { requested } => {
                Self::UnsupportedSampleCount { requested }
            }
            HeadlessRaster3DError::UnsupportedTargetFormat {
                format,
                sample_count,
            } => Self::UnsupportedTargetFormat {
                format,
                sample_count,
            },
            HeadlessRaster3DError::UnsupportedDepthFormat { sample_count } => {
                Self::UnsupportedDepthFormat { sample_count }
            }
            HeadlessRaster3DError::UniformStorageUnavailable { block, bytes } => {
                Self::UniformStorageUnavailable { block, bytes }
            }
            HeadlessRaster3DError::DynamicOffsetOutOfRange { offset } => {
                Self::DynamicOffsetOutOfRange { offset }
            }
            HeadlessRaster3DError::SnapshotInvariant => Self::SnapshotInvariant,
            #[cfg(test)]
            HeadlessRaster3DError::UnsupportedReadbackFormat { .. }
            | HeadlessRaster3DError::ReadbackFailed => Self::SnapshotInvariant,
        }
    }
}

#[derive(Debug, Clone)]
enum Viewport3DRequestKind {
    Flat,
    DomainAware(DomainFrame3),
    // No public source admission. Only the separately authorized S2 fixture can
    // currently supply scene color; the reusable output owner is production code.
    #[allow(dead_code)] // S2 remains unreachable through public constructors.
    PrivateHdr(HdrSource3D),
}

impl Viewport3DRequestKind {
    fn domain_frame(&self) -> Option<&DomainFrame3> {
        match self {
            Self::Flat | Self::PrivateHdr(_) => None,
            Self::DomainAware(frame) => Some(frame),
        }
    }

    fn same_identity(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Flat, Self::Flat) => true,
            (Self::PrivateHdr(a), Self::PrivateHdr(b)) => a == b,
            (Self::DomainAware(current), Self::DomainAware(requested)) => {
                current.same_identity(requested)
            }
            _ => false,
        }
    }

    fn hdr_source(&self) -> Option<&HdrSource3D> {
        match self {
            Self::PrivateHdr(source) => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum PbrSceneRequestIntent3D {
    Successor,
    IndependentReplacement,
}

#[derive(Clone)]
enum RequestedSource3D {
    // No semantic full source has been accepted for this loading request.
    PresencePending,
    Legacy {
        snapshot: RenderSnapshot,
        model: ModelState,
        kind: Viewport3DRequestKind,
    },
    Material(super::pbr::ValidatedMaterialSceneSource3D),
}

impl RequestedSource3D {
    fn legacy(&self) -> Option<(&RenderSnapshot, &ModelState, &Viewport3DRequestKind)> {
        match self {
            Self::Legacy {
                snapshot,
                model,
                kind,
            } => Some((snapshot, model, kind)),
            Self::Material(_) | Self::PresencePending => None,
        }
    }
    fn is_flat(&self) -> bool {
        matches!(
            self,
            Self::Legacy {
                kind: Viewport3DRequestKind::Flat,
                ..
            }
        )
    }
    fn hdr_source(&self) -> Option<&HdrSource3D> {
        match self {
            Self::Legacy { kind, .. } => kind.hdr_source(),
            Self::Material(source) => Some(&source.hdr),
            Self::PresencePending => None,
        }
    }
}

#[derive(Clone)]
struct RequestedSemanticState {
    view_revision: Viewport3DViewRevision,
    view: ViewDescriptor3,
    sample_count: SampleCount3D,
    source: RequestedSource3D,
}

impl RequestedSemanticState {
    fn matches_committed_material_view(&self, tuple: &MaterialViewport3DTuple) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            // Browser camera preparation may commit an earlier admitted view.
            // An annotation still belongs to that exact front, never the latest
            // camera merely because a newer input has arrived.
            self.view_revision >= tuple.view_revision
                && self.sample_count == tuple.target_key.sample_count
                && matches!(&self.source, RequestedSource3D::Material(source)
                    if Arc::ptr_eq(source.source(), &tuple.source))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.view_revision == tuple.view_revision && self.view == tuple.view
        }
    }
}

struct Viewport3DTargetResources {
    pass: Option<CustomRenderPass>,
    _depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    _multisample_texture: Option<wgpu::Texture>,
    multisample_view: Option<wgpu::TextureView>,
    raster_target: Viewport3DRasterTarget,
    prepared: Option<PreparedRaster3D>,
    _budget: TargetBudgetLease3D,
}

impl Viewport3DTargetResources {
    fn new(context: &Context3D, target: Viewport3DRasterTarget) -> Result<Self, Viewport3DError> {
        let budget = context
            .reserve_target_budget(target.active_target_pixels(), target.attachment_bytes())?;
        let gpu = context.gpu_resources()?;
        let extent = target.extent();
        let pass = CustomRenderPass::new_unpublished_with_filter(
            WidgetInternalId::new().raw(),
            gpu.clone(),
            PhysicalRect::new(0, 0, extent.width, extent.height),
            false,
        )?;
        let depth_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Halcyon 3D Viewport Private Reverse-Z Depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: target.sample_count(),
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let multisample_texture = (target.sample_count() == 4).then(|| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Halcyon 3D Viewport Private MSAA Color"),
                size: extent,
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: target.format(),
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        let multisample_view = multisample_texture
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
        #[cfg(test)]
        audit::record_target(target.sample_count());
        Ok(Self {
            pass: Some(pass),
            _depth_texture: depth_texture,
            depth_view,
            _multisample_texture: multisample_texture,
            multisample_view,
            raster_target: target,
            prepared: None,
            _budget: budget,
        })
    }

    fn texture_id(&self) -> Option<u64> {
        self.pass.as_ref().map(CustomRenderPass::texture_id)
    }

    fn prepare_or_update(
        &mut self,
        context: &Context3D,
        requested: &RequestedSemanticState,
        render_origin: WorldPoint3,
    ) -> Result<Option<PreparedRaster3D>, HeadlessRaster3DError> {
        let (snapshot, model, _) = requested
            .source
            .legacy()
            .ok_or(HeadlessRaster3DError::SnapshotInvariant)?;
        if self
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.matches_structure(snapshot))
        {
            let Some(prepared) = self.prepared.as_mut() else {
                return Err(HeadlessRaster3DError::SnapshotInvariant);
            };
            prepared.update_dynamic(context, model, requested.view, render_origin)?;
            Ok(None)
        } else {
            prepare_viewport_snapshot_raster(
                context,
                snapshot,
                model,
                requested.view,
                render_origin,
                self.raster_target,
            )
            .map(Some)
        }
    }

    fn retire(&mut self) {
        if let Some(prepared) = self.prepared.take() {
            prepared.retire_after_submitted_work();
        }
        if let Some(pass) = self.pass.take() {
            let _ = pass.destroy_deferred();
        }
    }
}

impl Drop for Viewport3DTargetResources {
    fn drop(&mut self) {
        self.retire();
    }
}

#[cfg(target_arch = "wasm32")]
struct BrowserMaterialPreparation3D {
    work: Arc<super::gpu_preparation::GpuPreparation3D>,
    attempt: BrowserMaterialAttempt3D,
    _attempt_metadata: super::context::DynamicStorageLease3D,
    observed_revision: u64,
    origin: WorldPoint3,
    prepared: Option<super::raster::MaterialScenePreparation3D>,
}

#[cfg(target_arch = "wasm32")]
struct BrowserMaterialAttempt3D {
    requested: RequestedSemanticState,
    tuple: RetainedSourceTuple3D,
    intent: PbrSceneRequestIntent3D,
    metrics: CustomRenderTargetMetrics,
    mount: u64,
    mounted: bool,
    presence: Option<(u64, u64)>,
    reveal_revision: u64,
    outline_revision: u64,
    hud_epoch: u64,
}

#[cfg(target_arch = "wasm32")]
impl BrowserMaterialAttempt3D {
    fn new(
        viewport: &Viewport3D,
        tuple: RetainedSourceTuple3D,
        metrics: CustomRenderTargetMetrics,
    ) -> Self {
        Self {
            requested: viewport.requested.clone(),
            tuple,
            intent: viewport.pbr_request_intent,
            metrics,
            mount: viewport.presence_mount,
            mounted: viewport.presence_mounted,
            presence: viewport.presence.as_ref().map(|p| (p.epoch, p.revision)),
            reveal_revision: viewport.reveal_frame_revision,
            outline_revision: viewport.outline_revision,
            hud_epoch: viewport.spatial_hud_epoch,
        }
    }

    fn compatible(
        &self,
        viewport: &Viewport3D,
        key: Viewport3DTargetCompatibilityKey,
        metrics: &CustomRenderTargetMetrics,
    ) -> bool {
        let (RequestedSource3D::Material(active), RequestedSource3D::Material(latest)) =
            (&self.requested.source, &viewport.requested.source)
        else {
            return false;
        };
        self.tuple.target_key() == key
            && Arc::ptr_eq(active.source(), latest.source())
            && active.matches(&viewport.context, active.source())
            && self.requested.sample_count == viewport.requested.sample_count
            && self.requested.view_revision <= viewport.requested.view_revision
            && self.metrics.surface_id == metrics.surface_id
            && self.metrics.final_logical_rect == metrics.final_logical_rect
            && self.metrics.final_logical_viewport == metrics.final_logical_viewport
            && self.metrics.target_logical_extent == metrics.target_logical_extent
            && self.metrics.scale_factor.to_bits() == metrics.scale_factor.to_bits()
            && self.metrics.target_physical_extent == metrics.target_physical_extent
            && self.metrics.activity == metrics.activity
            && self.mount == viewport.presence_mount
            && self.mounted == viewport.presence_mounted
            && self.presence == viewport.presence.as_ref().map(|p| (p.epoch, p.revision))
            && self.reveal_revision == viewport.reveal_frame_revision
            && self.outline_revision == viewport.outline_revision
            && self.hud_epoch == viewport.spatial_hud_epoch
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for BrowserMaterialPreparation3D {
    fn drop(&mut self) {
        self.work.retire();
    }
}

struct Viewport3DTargetState {
    #[cfg(target_arch = "wasm32")]
    browser_material_preparation: Option<BrowserMaterialPreparation3D>,
    spatial_hud: Option<Arc<super::spatial_hud::PreparedHud3D>>,
    spatial_hud_published: bool,
    spatial_hud_failure: Option<super::PbrSourceError3D>,
    last_spatial_hud_attempt: u64,
    outline: Option<OutlineTargetState>,
    outline_failure: Option<super::PbrSourceError3D>,
    last_outline_attempt_revision: u64,
    presence: Option<PresenceTargetState>,
    last_reveal_attempt_revision: u64,
    key: Option<Viewport3DTargetCompatibilityKey>,
    resources: Option<Viewport3DTargetResources>,
    hdr: Option<HdrOutput3D>,
    hdr_failure: Option<(Viewport3DRequestSerial, HdrError3D)>,
    last_hdr_budget_attempt: Option<(u64, u64)>,
    status: Viewport3DStatus,
    render_origin: WorldPoint3,
    last_metrics: Option<CustomRenderTargetMetrics>,
    composition_coverage: Option<PhysicalRect>,
    terminal_publication_withdrawal_pending: bool,
    last_dynamic_budget_attempt_revision: u64,
    material: Option<super::raster::PreparedMaterialScene3D>,
    material_failure: Option<super::pbr::PbrError3D>,
    material_capacity_attempt: Option<(u32, u32, u64, u64)>,
    material_capacity: Option<super::raster::MaterialCapacity3D>,
    material_capacity_budget: Option<Arc<super::raster::MaterialCapacityBudget3D>>,
    last_capacity_ready_attempt: u64,
}

impl Viewport3DTargetState {
    fn new(render_origin: WorldPoint3) -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            browser_material_preparation: None,
            spatial_hud: None,
            spatial_hud_published: false,
            spatial_hud_failure: None,
            last_spatial_hud_attempt: 0,
            outline: None,
            outline_failure: None,
            last_outline_attempt_revision: 0,
            presence: None,
            last_reveal_attempt_revision: 0,
            key: None,
            resources: None,
            hdr: None,
            hdr_failure: None,
            last_hdr_budget_attempt: None,
            status: Viewport3DStatus::uninitialized(),
            render_origin,
            last_metrics: None,
            composition_coverage: None,
            terminal_publication_withdrawal_pending: false,
            last_dynamic_budget_attempt_revision: 0,
            material: None,
            material_failure: None,
            material_capacity_attempt: None,
            material_capacity: None,
            material_capacity_budget: None,
            last_capacity_ready_attempt: 0,
        }
    }

    fn has_sampleable_front(&self) -> bool {
        self.sampleable_texture_id().is_some()
    }

    fn sampleable_texture_id(&self) -> Option<u64> {
        let committed = self.status.committed.as_ref()?;
        let hdr_family = match committed {
            RetainedSourceTuple3D::Material(_) => true,
            RetainedSourceTuple3D::Legacy(tuple) => tuple.request_kind.hdr_source().is_some(),
        };
        if hdr_family {
            return self
                .hdr
                .as_ref()
                .filter(|hdr| hdr.is_live())
                .map(HdrOutput3D::texture_id);
        }
        self.resources
            .as_ref()
            .and_then(Viewport3DTargetResources::texture_id)
    }

    fn requested_matches(
        &self,
        key: Viewport3DTargetCompatibilityKey,
        requested: &RequestedSemanticState,
    ) -> bool {
        self.status
            .requested
            .as_ref()
            .is_some_and(|tuple| tuple.matches(key, requested))
    }

    fn has_retryable_pending_budget_failure(&self) -> bool {
        self.status.failure.as_ref().is_some_and(|failure| {
            matches!(
                failure.error,
                Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
                    budget: PolicyBudget3D::PendingRequests | PolicyBudget3D::PendingPublications,
                    ..
                })
            )
        })
    }

    fn has_released_dynamic_budget_failure(&self, current_release_revision: u64) -> bool {
        self.last_dynamic_budget_attempt_revision != current_release_revision
            && self.status.failure.as_ref().is_some_and(|failure| {
                matches!(
                    failure.error,
                    Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
                        budget: PolicyBudget3D::TotalDynamicStorageBytes
                            | PolicyBudget3D::DynamicStorageBytesPerTarget
                            | PolicyBudget3D::TotalDynamicObjectCapacity
                            | PolicyBudget3D::DynamicStorageSlots
                            | PolicyBudget3D::RetainedDynamicCpuBytes,
                        ..
                    })
                )
            })
    }

    fn retire_incompatible(&mut self) {
        #[cfg(target_arch = "wasm32")]
        drop(self.browser_material_preparation.take());
        self.spatial_hud = None;
        self.spatial_hud_failure = None;
        self.outline = None;
        self.outline_failure = None;
        self.presence = None;
        drop(self.material_capacity.take());
        drop(self.material_capacity_budget.take());
        drop(self.material.take());
        self.material_failure = None;
        drop(self.hdr.take());
        self.hdr_failure = None;
        if let Some(mut resources) = self.resources.take() {
            resources.retire();
        }
        self.key = None;
        self.status.state = Viewport3DLifecycleState::IncompatibleTarget;
        self.status.committed = None;
        self.status.failure = None;
        self.status.device_failure = None;
        self.composition_coverage = None;
        self.terminal_publication_withdrawal_pending = false;
    }

    fn record_failure(&mut self, request_serial: Viewport3DRequestSerial, error: Viewport3DError) {
        #[cfg(feature = "render-audit")]
        cadence_audit::record_failed_preparation();
        self.status.state = if self.has_sampleable_front() {
            Viewport3DLifecycleState::FailedWithLastGood
        } else {
            Viewport3DLifecycleState::ReadyWithoutCommit
        };
        self.status.failure = Some(Viewport3DFailure {
            request_serial,
            error,
        });
        self.status.device_failure = None;
    }

    fn record_hdr_failure(&mut self, serial: Viewport3DRequestSerial, error: HdrError3D) {
        // Keep material/HDR diagnostics private; do not add variants to the public
        // legacy error enum or fabricate an unrelated legacy failure cause.
        self.status.state = if self.has_sampleable_front() {
            Viewport3DLifecycleState::FailedWithLastGood
        } else {
            Viewport3DLifecycleState::ReadyWithoutCommit
        };
        self.status.failure = if let HdrError3D::Context(ref cause) = error {
            Some(Viewport3DFailure {
                request_serial: serial,
                error: cause.clone().into(),
            })
        } else {
            None
        };
        self.hdr_failure = Some((serial, error));
    }

    fn mark_terminal(&mut self, failure: GpuDeviceFailure) {
        #[cfg(target_arch = "wasm32")]
        drop(self.browser_material_preparation.take());
        self.spatial_hud = None;
        self.spatial_hud_failure = None;
        self.terminal_publication_withdrawal_pending |= self.outline.is_some();
        self.outline = None;
        self.outline_failure = None;
        self.terminal_publication_withdrawal_pending |= self.presence.is_some();
        self.presence = None;
        drop(self.material_capacity.take());
        drop(self.material_capacity_budget.take());
        drop(self.material.take());
        self.material_failure = None;
        self.terminal_publication_withdrawal_pending |= self.has_sampleable_front();
        drop(self.hdr.take());
        self.hdr_failure = None;
        if let Some(mut resources) = self.resources.take() {
            resources.retire();
        }
        self.status.state = Viewport3DLifecycleState::TerminalDeviceFailure;
        self.status.failure = None;
        self.status.device_failure = Some(failure);
        self.composition_coverage = None;
    }

    fn take_terminal_publication_withdrawal(&mut self) -> bool {
        std::mem::take(&mut self.terminal_publication_withdrawal_pending)
    }

    fn retire(&mut self, state: Viewport3DLifecycleState) {
        #[cfg(target_arch = "wasm32")]
        drop(self.browser_material_preparation.take());
        self.spatial_hud = None;
        self.spatial_hud_failure = None;
        self.outline = None;
        self.outline_failure = None;
        self.presence = None;
        drop(self.material_capacity.take());
        drop(self.material_capacity_budget.take());
        drop(self.material.take());
        self.material_failure = None;
        drop(self.hdr.take());
        self.hdr_failure = None;
        if let Some(mut resources) = self.resources.take() {
            resources.retire();
        }
        self.key = None;
        self.status.state = state;
        self.status.requested = None;
        self.status.committed = None;
        self.status.failure = None;
        self.status.device_failure = None;
        self.composition_coverage = None;
        self.terminal_publication_withdrawal_pending = false;
    }
}

/// Opt-in, per-viewport CPU wall measurements for one diagnostic request.
///
/// Available only in unit tests or with `test-harness`. This copied receipt
/// contains no renderer handles and does not measure GPU completion or display.
/// Enable/reset it immediately before one request, then read it after the
/// matching current committed tuple. An unchanged preparation retains the prior
/// receipt; callers must match the public source/view/target/request status.
///
/// Durations are nested, not additive. `render_total` encloses
/// `prepare_validation`, `raster.total`, `hdr_allocation`, `hdr_total` and
/// `material_commit`; `active_total` additionally encloses `publication` and
/// unisolated active-preparation work. Their checked differences are unmeasured
/// remainders, not attribution to a specific operation. The public request and
/// preparation call boundaries, caller handoff, GPU waits/readback and Image or
/// native presentation are not an end-to-end timer in this receipt.
#[cfg(any(test, feature = "test-harness"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PbrViewTiming3D {
    /// Inclusive time inside the existing material-source setter.
    pub request_total: std::time::Duration,
    /// Source validation nested inside `request_total`.
    pub request_validation: std::time::Duration,
    /// Source validation nested inside `render_total`.
    pub prepare_validation: std::time::Duration,
    /// Inclusive private material/HDR rendering preparation.
    pub render_total: std::time::Duration,
    pub raster: super::raster::PbrPreparationTiming3D,
    pub hdr_allocation: std::time::Duration,
    pub hdr_reused: bool,
    /// Includes `hdr.encode_cpu` and `hdr.submit_swap_cpu`; remainder unisolated.
    pub hdr_total: std::time::Duration,
    pub hdr: super::hdr::HdrRenderTiming3D,
    pub material_commit: std::time::Duration,
    /// CPU committed-tuple publication after successful submission, not display.
    pub publication: std::time::Duration,
    /// Inclusive active-target preparation; excludes the outer public boundary.
    pub active_total: std::time::Duration,
    /// Successful local commit/publication reached; not proof of GPU completion.
    pub complete: bool,
}

/// Low-intrusion CPU phases for one genuinely reused D-only FOV candidate.
///
/// Available only with tests or `test-harness`, and disabled by default. The
/// removed vertex/UV model phases stay zero; `direct_projection` is real R025
/// work. `raster_total`, `render_total`,
/// `active_total` and `preparation_total` are inclusive owners, not additional
/// work to sum. `hdr_total` encloses the existing encode/submit/publication call,
/// without enabling its detailed clocks. No duration measures GPU completion.
/// A receipt is complete only after the associated current tuple commits.
#[cfg(any(test, feature = "test-harness"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PbrCoarseTiming3D {
    pub request_total: std::time::Duration,
    pub request_validation: std::time::Duration,
    pub prepare_validation: std::time::Duration,
    pub raster_total: std::time::Duration,
    /// Removed proof-only GPU model; always zero in production preparation.
    pub packet_vertex_projection: std::time::Duration,
    pub uv_main: std::time::Duration,
    pub direct_projection: std::time::Duration,
    pub uv_shadow: std::time::Duration,
    /// Enclosing raster call minus actual measured phases; no finer attribution.
    pub raster_unattributed: Option<std::time::Duration>,
    pub hdr_total: std::time::Duration,
    pub material_commit: std::time::Duration,
    pub render_total: std::time::Duration,
    pub publication: std::time::Duration,
    pub active_total: std::time::Duration,
    /// Entire existing CustomRenderParticipant::prepare_custom_render call.
    pub preparation_total: std::time::Duration,
    pub main_uv_packets: usize,
    pub main_uv_qualified: usize,
    pub main_uv_excluded: usize,
    pub main_uv_conditioned: usize,
    pub shadow_uv_triangles: usize,
    pub shadow_faces: usize,
    pub capacity_reused: bool,
    pub target_key: Option<Viewport3DTargetCompatibilityKey>,
    pub view_revision: Option<Viewport3DViewRevision>,
    pub request_serial: Option<Viewport3DRequestSerial>,
    pub source_identity: Option<(u64, u64)>,
    pub structural_revision: Option<StructuralRevision>,
    pub model_revision: Option<ModelRevision>,
    pub direct_identity: Option<(u64, u64)>,
    pub complete: bool,
}

struct RevealOccurrence3D {
    request: PbrObjectRevealRequest3D,
    progress: f32,
    handle: Option<TransitionHandle>,
    pending: bool,
    queued: bool,
    target_ready: bool,
    failure: Option<super::pbr::PbrError3D>,
}

struct PresenceOccurrence {
    proxy: super::LoadingPresenceProxy3D,
    motion: super::PresentationMotionPolicy,
    transition_duration: super::LoadingPresenceTransitionDuration3D,
    transition_duration_frozen: bool,
    epoch: u64,
    backup: Option<RequestedSemanticState>,
    full: Option<Arc<super::pbr::MaterialSceneSource3D>>,
    handle: Option<TransitionHandle>,
    entry_at: Option<f64>,
    phase: f32,
    weight: f32,
    terminal: bool,
    queued: bool,
    revision: u64,
    failure: Option<super::LoadingPresenceError3D>,
}

struct PresenceTargetState {
    gpu: Option<super::loading_presence::PresenceGpuTarget>,
    terminal_factory: super::loading_presence::PresenceTerminalFactory,
    epoch: u64,
    mount: u64,
    view_revision: Viewport3DViewRevision,
    key: Viewport3DTargetCompatibilityKey,
    pin: Option<crate::renderer::custom_render_pass::HdrFrontPin>,
    pin_tuple: Option<MaterialViewport3DTuple>,
    ready: bool,
    ready_at: Option<f64>,
    weight: f32,
    complete: bool,
    rendered: bool,
    terminal: Option<crate::renderer::wgpu_renderer::PresenceOutput>,
    terminal_generation: Option<std::num::NonZeroU64>,
    notified_full: Option<Viewport3DRequestSerial>,
}

fn loading_presence_phase(entry: f64, timestamp: f64) -> f32 {
    (((timestamp - entry).max(0.0) % 3.2) / 3.2 * std::f64::consts::TAU) as f32
}

struct OutlineTargetState {
    gpu: super::outline::OutlineGpuTarget,
    epoch: u64,
    mount: u64,
    key: Viewport3DTargetCompatibilityKey,
    view_revision: Viewport3DViewRevision,
    view: ViewDescriptor3,
    request: Viewport3DRequestSerial,
    pin: Option<crate::renderer::custom_render_pass::HdrFrontPin>,
    notified: bool,
}

pub struct Viewport3D {
    spatial_hud_request: Option<super::PbrSpatialHud3D>,
    spatial_hud_typography: Option<super::spatial_hud::HudTypography>,
    spatial_hud_program: Option<Arc<super::spatial_hud::HudProgram3D>>,
    spatial_hud_epoch: u64,
    spatial_hud_queued: bool,
    outline_request: Option<super::PbrOutlineRequest3D>,
    outline_revision: u64,
    outline_queued: bool,
    presence: Option<PresenceOccurrence>,
    presence_epoch: u64,
    presence_mount: u64,
    presence_mounted: bool,
    retired_presence_handle: Option<TransitionHandle>,
    id: WidgetInternalId,
    context: Arc<Context3D>,
    admission: CustomRenderAdmission,
    _view_lease: ViewLease3D,
    requested: RequestedSemanticState,
    pbr_request_intent: PbrSceneRequestIntent3D,
    next_request_serial: Option<u64>,
    targets: HashMap<SurfaceId, Viewport3DTargetState>,
    current_prepared_surface: Option<SurfaceId>,
    flat_viewport3d_writer: Option<FlatViewport3DWriterAuthority>,
    object_reveal: Option<RevealOccurrence3D>,
    reveal_request_revision: u64,
    reveal_frame_revision: u64,
    retired_reveal_handle: Option<TransitionHandle>,
    #[cfg(test)]
    hdr_writer: Option<Arc<dyn super::hdr::HdrProofSceneWriter3D>>,
    #[cfg(any(test, feature = "test-harness"))]
    pbr_timing: Option<PbrViewTiming3D>,
    #[cfg(any(test, feature = "test-harness"))]
    pbr_coarse_timing: Option<PbrCoarseTiming3D>,
}

impl Viewport3D {
    pub fn new(
        context: Arc<Context3D>,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        model: ModelState,
        sample_count: SampleCount3D,
    ) -> Result<Self, Viewport3DError> {
        Self::new_with_request_kind(
            context,
            view_revision,
            view,
            snapshot,
            model,
            sample_count,
            Viewport3DRequestKind::Flat,
        )
    }

    pub fn new_domain_composition(
        context: Arc<Context3D>,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        frame: DomainFrame3,
        sample_count: SampleCount3D,
    ) -> Result<Self, Viewport3DError> {
        let snapshot = frame.composition().render_snapshot().clone();
        let model = frame.model().clone();
        Self::new_with_request_kind(
            context,
            view_revision,
            view,
            snapshot,
            model,
            sample_count,
            Viewport3DRequestKind::DomainAware(frame),
        )
    }

    fn new_with_request_kind(
        context: Arc<Context3D>,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        model: ModelState,
        sample_count: SampleCount3D,
        request_kind: Viewport3DRequestKind,
    ) -> Result<Self, Viewport3DError> {
        if !context.same_resource_family(snapshot.context()) {
            return Err(Context3DError::ContextMismatch.into());
        }
        if !model.matches_snapshot(&snapshot) {
            return Err(ModelStateError::StructuralIdentityMismatch.into());
        }
        Self::new_with_semantic_source(
            context,
            RequestedSemanticState {
                view_revision,
                view,
                sample_count,
                source: RequestedSource3D::Legacy {
                    snapshot,
                    model,
                    kind: request_kind,
                },
            },
        )
    }

    fn new_with_semantic_source(
        context: Arc<Context3D>,
        requested: RequestedSemanticState,
    ) -> Result<Self, Viewport3DError> {
        let gpu = context.gpu_resources()?;
        let view_lease = context.reserve_view()?;
        #[cfg(test)]
        audit::record_viewport();
        Ok(Self {
            spatial_hud_request: None,
            spatial_hud_typography: None,
            spatial_hud_program: None,
            spatial_hud_epoch: 0,
            spatial_hud_queued: false,
            outline_request: None,
            outline_revision: 0,
            outline_queued: false,
            presence: None,
            presence_epoch: 0,
            presence_mount: 0,
            presence_mounted: false,
            retired_presence_handle: None,
            id: WidgetInternalId::new(),
            context,
            admission: CustomRenderAdmission::from_gpu_resources(&gpu),
            _view_lease: view_lease,
            requested,
            pbr_request_intent: PbrSceneRequestIntent3D::Successor,
            next_request_serial: Some(1),
            targets: HashMap::new(),
            current_prepared_surface: None,
            flat_viewport3d_writer: None,
            object_reveal: None,
            reveal_request_revision: 0,
            reveal_frame_revision: 0,
            retired_reveal_handle: None,
            #[cfg(test)]
            hdr_writer: None,
            #[cfg(any(test, feature = "test-harness"))]
            pbr_timing: None,
            #[cfg(any(test, feature = "test-harness"))]
            pbr_coarse_timing: None,
        })
    }

    /// Request passive HAL375 labels for the exact requested PBR source.
    /// None withdraws the optional presentation without replacing the PBR scene.
    pub fn request_pbr_spatial_hud(
        &mut self,
        hud: Option<super::PbrSpatialHud3D>,
    ) -> super::PbrSourceResult3D<bool> {
        use super::spatial_hud::refused;
        let hud = hud.filter(|hud| !hud.anchors().is_empty());
        if let Some(hud) = &hud {
            hud.validate_context(&self.context)?;
            if !matches!(&self.requested.source, RequestedSource3D::Material(source)
                if hud.matches_source(source.source()))
            {
                return Err(refused("HUD does not belong to the requested PBR scene"));
            }
            if self
                .spatial_hud_request
                .as_ref()
                .is_some_and(|old| old.same_request(hud))
            {
                return Ok(false);
            }
            self.spatial_hud_epoch = self
                .spatial_hud_epoch
                .checked_add(1)
                .ok_or_else(|| refused("HUD epoch exhausted"))?;
        } else if self.spatial_hud_request.is_none() {
            return Ok(false);
        }
        self.spatial_hud_request = hud;
        if self.spatial_hud_request.is_none() {
            self.spatial_hud_program = None;
            self.spatial_hud_typography = None;
        }
        self.spatial_hud_queued = true;
        self.clear_spatial_hud_targets();
        Ok(true)
    }

    fn clear_spatial_hud_targets(&mut self) {
        for target in self.targets.values_mut() {
            target.spatial_hud = None;
            target.spatial_hud_failure = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn spatial_hud_error_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<&super::PbrSourceError3D> {
        self.targets.get(&surface)?.spatial_hud_failure.as_ref()
    }

    fn retire_spatial_hud(&mut self) {
        if self.spatial_hud_request.take().is_some() {
            self.spatial_hud_typography = None;
            self.spatial_hud_program = None;
            self.spatial_hud_queued = false;
            self.clear_spatial_hud_targets();
        }
    }

    fn prepare_spatial_hud_publication(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
        outcome: &mut CustomRenderPreparationOutcome,
    ) {
        let texture_id = outcome
            .presentation_binding()
            .filter(|binding| !binding.canonical_over_neutral())
            .map(|binding| binding.texture_id());
        let previous = self
            .targets
            .get(&metrics.surface_id)
            .and_then(|target| target.spatial_hud.clone());
        let prepared = texture_id.and_then(|id| self.qualified_spatial_hud(metrics, id));
        let Some(target) = self.targets.get_mut(&metrics.surface_id) else {
            return;
        };
        let mut published = false;
        if let Some(prepared) = &prepared {
            match outcome.attach_spatial_hud_companion(Arc::clone(prepared), *metrics) {
                Ok(()) => published = true,
                Err(error) => target.spatial_hud_failure = Some(error),
            }
        }
        let was_published = std::mem::replace(&mut target.spatial_hud_published, published);
        if !published {
            target.spatial_hud = None;
        }
        outcome.front_publication_changed |= was_published != published
            || published
                && prepared
                    .as_ref()
                    .is_some_and(|new| previous.as_ref().is_none_or(|old| !Arc::ptr_eq(old, new)));

        // Submitted work has its own Queue-retired program lease. This cache
        // must not keep a program once the last sampled target is retired.
        if self.spatial_hud_program.is_some()
            && target
                .hdr
                .as_ref()
                .and_then(|hdr| hdr.hud_depth())
                .is_none()
            && !self.targets.values().any(|target| {
                target
                    .hdr
                    .as_ref()
                    .and_then(|hdr| hdr.hud_depth())
                    .is_some()
            })
        {
            self.spatial_hud_program = None;
        }
    }

    fn qualified_spatial_hud(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
        texture_id: u64,
    ) -> Option<Arc<super::spatial_hud::PreparedHud3D>> {
        use super::spatial_hud::{PreparedHud3D, project, refused};
        let source = self.spatial_hud_request.as_ref()?;
        let typography = self.spatial_hud_typography.as_ref()?;
        if self.presence.is_some() || self.outline_request.is_some() {
            return None;
        }
        let target = self.targets.get_mut(&metrics.surface_id)?;
        let tuple = target.status.committed_material()?;
        let request = tuple.request_serial;
        #[cfg(not(target_arch = "wasm32"))]
        let current_request = target.requested_matches(tuple.target_key, &self.requested)
            && target.status.requested_material()?.request_serial == request;
        #[cfg(target_arch = "wasm32")]
        let current_request = target.key == Some(tuple.target_key);
        if !self.presence_mounted
            || target.status.state != Viewport3DLifecycleState::Committed
            || !source.matches_source(&tuple.source)
            || !current_request
            || !self.requested.matches_committed_material_view(tuple)
            || target.last_spatial_hud_attempt != self.spatial_hud_epoch
        {
            target.spatial_hud = None;
            return None;
        }
        let hdr = target.hdr.as_mut()?;
        if hdr.texture_id() != texture_id {
            return None;
        }
        let depth = hdr.hud_depth()?.clone();
        let program = self
            .spatial_hud_program
            .as_ref()
            .filter(|program| program.samples == depth.samples)?
            .clone();
        if let Some(prepared) = target.spatial_hud.as_ref().filter(|p| {
            p.request == request
                && p.epoch == self.spatial_hud_epoch
                && p.mount == self.presence_mount
                && p.typography == *typography
                && Arc::ptr_eq(&p.depth, &depth)
                && p.metrics.final_logical_rect == metrics.final_logical_rect
                && p.metrics.final_logical_viewport == metrics.final_logical_viewport
                && p.metrics.target_logical_extent == metrics.target_logical_extent
                && p.metrics.scale_factor == metrics.scale_factor
                && hdr.matches_pin(&p.front)
        }) {
            return Some(prepared.clone());
        }
        target.spatial_hud = None;
        let points = match project(
            source,
            tuple.view,
            target.render_origin,
            depth.extent,
            metrics,
        ) {
            Ok(points) => points,
            Err(error) => {
                target.spatial_hud_failure = Some(error);
                return None;
            }
        };
        if points.iter().all(Option::is_none) {
            return None;
        }
        let Ok(front) = hdr.pin_front() else {
            target.spatial_hud_failure = Some(refused("current canonical front cannot be pinned"));
            return None;
        };
        let (material_storage, upload) = target
            .material
            .as_ref()?
            .hud_budget_base(target.material_capacity_budget.as_deref());
        let storage = material_storage.checked_add(hdr.tone_storage_bytes())?;
        let cpu_bytes =
            (std::mem::size_of::<PreparedHud3D>() + 2 * std::mem::size_of::<usize>()) as u64;
        let charge = match self.context.reserve_dynamic_storage(0, 0, 0, cpu_bytes) {
            Ok(charge) => charge,
            Err(error) => {
                target.spatial_hud_failure = Some(error.into());
                return None;
            }
        };
        let prepared = Arc::new(PreparedHud3D {
            context: self.context.clone(),
            source: source.clone(),
            metrics: *metrics,
            points,
            typography: typography.clone(),
            depth,
            program,
            front,
            texture_id,
            epoch: self.spatial_hud_epoch,
            mount: self.presence_mount,
            request,
            storage,
            upload,
            _cpu: charge,
        });
        target.spatial_hud = Some(prepared.clone());
        Some(prepared)
    }

    /// Request HAL374's static source-bound selected visible outer Outline.
    /// None restores original F without changing scene, camera or source revision.
    pub fn request_pbr_outline(
        &mut self,
        selection: Option<super::PbrOutlineSelection3D>,
    ) -> super::PbrSourceResult3D<Option<super::PbrOutlineRequest3D>> {
        if let Some(selection) = &selection {
            selection.current(&self.context)?;
            let RequestedSource3D::Material(source) = &self.requested.source else {
                return Err(super::PbrSourceError3D::Association(
                    "HAL374 requires an actual requested PBR source",
                ));
            };
            if !selection.matches_source(source.source()) || self.presence.is_some() {
                return Err(super::PbrSourceError3D::Association(
                    "HAL374 source or exclusive presentation owner mismatch",
                ));
            }
            if self
                .outline_request
                .as_ref()
                .is_some_and(|r| r.selection.matches(selection))
                && self.targets.values().all(|t| t.outline_failure.is_none())
            {
                return Ok(self.outline_request.clone());
            }
        } else if self.outline_request.is_none() {
            return Ok(None);
        }
        let Some(epoch) = self.outline_revision.checked_add(1) else {
            if selection.is_none() {
                self.retire_pbr_outline();
            }
            return Err(super::PbrSourceError3D::Association(
                "HAL374 request epoch exhausted",
            ));
        };
        self.outline_revision = epoch;
        self.outline_request =
            selection.map(|selection| super::PbrOutlineRequest3D { selection, epoch });
        for target in self.targets.values_mut() {
            target.outline = None;
            target.outline_failure = None;
            if self.outline_request.is_none() {
                target.last_outline_attempt_revision = epoch;
            }
        }
        self.outline_queued = true;
        Ok(self.outline_request.clone())
    }

    pub fn pbr_outline_error(&self, surface: SurfaceId) -> Option<&super::PbrSourceError3D> {
        self.targets
            .get(&surface)
            .and_then(|t| t.outline_failure.as_ref())
    }

    fn retire_pbr_outline(&mut self) {
        self.outline_queued |= self.outline_request.take().is_some();
        for target in self.targets.values_mut() {
            self.outline_queued |= target.outline.take().is_some();
            target.outline_failure = None;
            target.last_outline_attempt_revision = self.outline_revision;
        }
    }

    #[cfg(test)]
    pub(crate) fn outline_textures_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<(&wgpu::Texture, &wgpu::Texture, &wgpu::Texture)> {
        let target = &self.targets.get(&surface)?.outline.as_ref()?.gpu;
        if !target.composed() {
            return None;
        }
        Some((target.output_texture(), target.winners(), target.strokes()))
    }

    #[cfg(test)]
    pub(crate) fn hdr_scene_planes_for_test(&self, surface: SurfaceId) -> &[wgpu::Texture; 4] {
        self.targets[&surface]
            .hdr
            .as_ref()
            .expect("actual HDR owner")
            .scene_planes_for_test()
    }

    fn update_pbr_outline(&mut self, cx: &mut UpdateContext) {
        let mut changed = std::mem::take(&mut self.outline_queued);
        let Some(request) = self.outline_request.as_ref() else {
            if changed {
                cx.invalidate_paint();
            }
            return;
        };
        let source_matches = matches!(&self.requested.source, RequestedSource3D::Material(source)
            if request.selection.matches_source(source.source()));
        if !source_matches
            || !self.presence_mounted
            || self.presence.is_some()
            || self.context.device_failure().is_some()
        {
            for target in self.targets.values_mut() {
                changed |= target.outline.take().is_some();
            }
            if changed {
                cx.invalidate_paint();
            }
            return;
        }
        for target in self.targets.values_mut() {
            let Some(outline) = target.outline.as_mut() else {
                continue;
            };
            let valid = outline.epoch == request.epoch
                && outline.mount == self.presence_mount
                && target.status.committed_material().is_some_and(|tuple| {
                    tuple.target_key == outline.key
                        && tuple.request_serial == outline.request
                        && tuple.view_revision == outline.view_revision
                        && tuple.view == outline.view
                        && self.requested.matches_committed_material_view(tuple)
                        && request.selection.matches_source(&tuple.source)
                });
            if !valid {
                target.outline = None;
                changed = true;
                continue;
            }
            let Some(hdr) = target.hdr.as_mut() else {
                target.outline = None;
                changed = true;
                continue;
            };
            if outline
                .pin
                .as_ref()
                .is_some_and(|pin| !hdr.matches_pin(pin))
            {
                target.outline = None;
                changed = true;
                continue;
            }
            if outline.pin.is_none() {
                match hdr.pin_front() {
                    Ok(pin) => outline.pin = Some(pin),
                    Err(super::hdr::HdrError3D::PinnedFront) => continue,
                    Err(error) => {
                        target.outline_failure =
                            Some(super::PbrSourceError3D::Gpu(error.to_string()));
                        target.outline = None;
                        changed = true;
                        continue;
                    }
                }
            }
            let tuple = target
                .status
                .committed_material()
                .expect("qualified complete canonical tuple");
            match outline.gpu.compose(
                &self.context,
                outline.pin.as_ref().expect("actual immutable pin"),
                tuple,
            ) {
                Ok(rendered) => changed |= rendered,
                Err(error) => {
                    if outline
                        .pin
                        .as_ref()
                        .is_some_and(|p| matches!(p.registered(), Ok(None)))
                    {
                        continue;
                    }
                    target.outline_failure = Some(error);
                    target.outline = None;
                    changed = true;
                }
            }
        }
        if changed {
            cx.invalidate_paint();
        }
    }

    /// HAL-370 checked ordinary UNLINKED material writer. No Legacy snapshot or
    /// model is constructed or used as authority for this source family.
    pub fn new_pbr(
        context: Arc<Context3D>,
        source: super::material_api::PbrScene3D,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
    ) -> Result<Self, super::material_api::PbrSourceError3D> {
        let source = super::pbr::ValidatedMaterialSceneSource3D::new(&context, source.0)?;
        let sample_count = SampleCount3D::try_new(source.samples)?;
        Ok(Self::new_with_semantic_source(
            context,
            RequestedSemanticState {
                view_revision,
                view,
                sample_count,
                source: RequestedSource3D::Material(source),
            },
        )?)
    }

    /// HAL381: a source-qualified loading presentation before any full PBR
    /// source is ready. This publishes no PBR committed/current tuple.
    pub fn new_loading_presence(
        context: Arc<Context3D>,
        proxy: super::LoadingPresenceProxy3D,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        samples: SampleCount3D,
        motion: super::PresentationMotionPolicy,
    ) -> Result<Self, super::LoadingPresenceError3D> {
        proxy.current(&context)?;
        let requested = RequestedSemanticState {
            view_revision,
            view,
            sample_count: samples,
            source: RequestedSource3D::PresencePending,
        };
        let mut viewport = Self::new_with_semantic_source(context, requested)
            .map_err(|e| super::LoadingPresenceError3D::Resource(e.to_string()))?;
        viewport.request_loading_presence(proxy, view_revision, view, samples, motion)?;
        Ok(viewport)
    }

    /// Source/transaction owner explicitly selects the proxy association. A
    /// compatible old full front stays a backup, never the new loading source.
    pub fn request_loading_presence(
        &mut self,
        proxy: super::LoadingPresenceProxy3D,
        revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        samples: SampleCount3D,
        motion: super::PresentationMotionPolicy,
    ) -> Result<bool, super::LoadingPresenceError3D> {
        use super::LoadingPresenceError3D as E;
        proxy.current(&self.context)?;
        if self.linked_writer_active() {
            return Err(E::Association(
                "loading presence requires the ordinary unlinked viewport owner",
            ));
        }
        if revision < self.requested.view_revision
            || (revision == self.requested.view_revision && view != self.requested.view)
        {
            return Err(E::Association("stale/reused loading view revision"));
        }
        if self.presence.as_ref().is_some_and(|p| {
            p.proxy.same_resource(&proxy) && self.requested.sample_count == samples
        }) {
            if revision == self.requested.view_revision
                && view == self.requested.view
                && self.presence.as_ref().is_some_and(|p| p.motion == motion)
            {
                return Ok(false);
            }
            let next = self
                .presence
                .as_ref()
                .and_then(|p| p.revision.checked_add(1))
                .ok_or(E::Association("presentation generation exhausted"))?;
            if !motion.allow_nonessential_motion
                && self.retired_presence_handle.is_some()
                && self.presence.as_ref().is_some_and(|p| p.handle.is_some())
            {
                return Err(E::Capacity);
            }
            let mut p = self.presence.take().expect("checked same association");
            if let Some(full) = &p.full {
                if let Err(error) = self.set_material_source(full.clone(), revision, view) {
                    self.presence = Some(p);
                    return Err(E::Resource(error.to_string()));
                }
            } else {
                self.requested.view_revision = revision;
                self.requested.view = view;
            }
            p.motion = motion;
            p.queued = true;
            p.revision = next;
            if !motion.allow_nonessential_motion {
                self.retired_presence_handle = p.handle.take();
                p.phase = 0.0;
            }
            self.presence = Some(p);
            return Ok(true);
        }
        if self.retired_presence_handle.is_some()
            && self.presence.as_ref().is_some_and(|p| p.handle.is_some())
        {
            return Err(E::Capacity);
        }
        let epoch = self
            .presence_epoch
            .checked_add(1)
            .ok_or(E::Association("loading request epoch exhausted"))?;
        let backup = if !matches!(self.requested.source, RequestedSource3D::PresencePending) {
            Some(self.requested.clone())
        } else {
            self.presence.as_ref().and_then(|p| p.backup.clone())
        };
        self.retire_loading_presence();
        self.retire_object_reveal();
        self.retire_pbr_outline();
        self.retire_spatial_hud();
        self.presence_epoch = epoch;
        self.requested = RequestedSemanticState {
            view_revision: revision,
            view,
            sample_count: samples,
            source: RequestedSource3D::PresencePending,
        };
        for target in self.targets.values_mut() {
            target.status.requested = None;
            target.status.state = Viewport3DLifecycleState::ReadyWithoutCommit;
            target.status.failure = None;
            target.material_failure = None;
            target.hdr_failure = None;
        }
        self.presence = Some(PresenceOccurrence {
            proxy,
            motion,
            transition_duration: super::LoadingPresenceTransitionDuration3D::DEFAULT,
            transition_duration_frozen: false,
            epoch,
            backup,
            full: None,
            handle: None,
            entry_at: None,
            phase: 0.0,
            weight: 0.0,
            terminal: false,
            queued: true,
            revision: 1,
            failure: None,
        });
        Ok(true)
    }

    /// Set the duration for the current mounted loading-presence occurrence.
    /// The first registered, pinned readiness receipt freezes the choice for
    /// all of its targets. A new occurrence starts at the default.
    pub fn set_loading_presence_transition_duration(
        &mut self,
        duration: super::LoadingPresenceTransitionDuration3D,
    ) -> Result<bool, super::LoadingPresenceError3D> {
        use super::LoadingPresenceError3D as E;
        let occurrence = self
            .presence
            .as_mut()
            .filter(|p| self.presence_mounted && p.failure.is_none())
            .ok_or(E::Association("no current mounted loading presence"))?;
        occurrence.proxy.current(&self.context)?;
        if occurrence.transition_duration == duration {
            return Ok(false);
        }
        if occurrence.transition_duration_frozen {
            return Err(E::Association("loading presence transition already ready"));
        }
        occurrence.transition_duration = duration;
        Ok(true)
    }

    /// Capture this current request before dispatching asynchronous full work.
    /// Source derivation is the application's claim, while this opaque token
    /// additionally binds the actual viewport, mount, request, view and N.
    pub fn loading_presence_request(
        &self,
        association: &super::LoadingPresenceAssociation3D,
    ) -> Result<super::LoadingPresenceRequest3D, super::LoadingPresenceError3D> {
        use super::LoadingPresenceError3D as E;
        let p = self
            .presence
            .as_ref()
            .ok_or(E::Association("no active loading association"))?;
        if !self.presence_mounted
            || p.failure.is_some()
            || !p.proxy.matches_association(association)
        {
            return Err(E::Association(
                "source association or concrete mount is not current",
            ));
        }
        p.proxy.current(&self.context)?;
        Ok(super::LoadingPresenceRequest3D::new(
            &self.context,
            self.admission.presentation_id(),
            p.epoch,
            self.presence_mount,
            self.requested.view_revision,
            self.requested.sample_count.value(),
            association.clone(),
        ))
    }

    /// The application carries this exact CPU-only request through full work.
    /// It is neither a mathematical certificate nor an HDR identity.
    pub fn request_loading_presence_pbr(
        &mut self,
        scene: super::PbrScene3D,
        request: &super::LoadingPresenceRequest3D,
        revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
    ) -> Result<bool, super::LoadingPresenceError3D> {
        use super::LoadingPresenceError3D as E;
        let presence = self
            .presence
            .as_ref()
            .ok_or(E::Association("no active preview request"))?;
        if !self.presence_mounted
            || presence.failure.is_some()
            || revision != self.requested.view_revision
            || view != self.requested.view
            || !request.matches(
                &self.context,
                self.admission.presentation_id(),
                presence.epoch,
                self.presence_mount,
                self.requested.view_revision,
                self.requested.sample_count.value(),
                &presence.proxy,
            )
            || scene.0.samples != self.requested.sample_count.value()
        {
            return Err(E::Association(
                "full source does not carry the current preview association/sample generation",
            ));
        }
        presence.proxy.current(&self.context)?;
        let mut occurrence = self.presence.take().expect("checked occurrence");
        let result = self
            .set_material_source(scene.0.clone(), revision, view)
            .map_err(|e| E::Resource(e.to_string()));
        if result.is_ok() {
            occurrence.full = Some(scene.0);
            occurrence.queued = true;
        }
        self.presence = Some(occurrence);
        result
    }

    pub fn loading_presence_error(&self) -> Option<&super::LoadingPresenceError3D> {
        self.presence.as_ref().and_then(|p| p.failure.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn loading_presence_texture_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<&wgpu::Texture> {
        self.targets
            .get(&surface)?
            .presence
            .as_ref()?
            .gpu
            .as_ref()
            .map(|gpu| gpu.output_texture())
    }

    #[cfg(test)]
    pub(crate) fn loading_presence_weight_for_test(&self) -> Option<f32> {
        self.presence.as_ref().map(|p| p.weight)
    }

    #[cfg(test)]
    pub(crate) fn loading_presence_terminal_for_test(&self, surface: SurfaceId) -> bool {
        self.targets
            .get(&surface)
            .and_then(|t| t.presence.as_ref())
            .is_some_and(|p| p.terminal.is_some() && p.gpu.is_none())
    }

    #[cfg(test)]
    pub(crate) fn loading_presence_target_progress_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<(f32, bool)> {
        self.targets
            .get(&surface)?
            .presence
            .as_ref()
            .map(|p| (p.weight, p.complete))
    }

    /// CPU progress for the existing native test harness, not GPU telemetry or
    /// physical-display evidence. Terminal requires the actual registered pin.
    #[cfg(feature = "test-harness")]
    pub fn loading_presence_progress_for_test_harness(
        &self,
        surface: SurfaceId,
    ) -> Option<(f32, bool)> {
        let occurrence = self
            .presence
            .as_ref()
            .filter(|p| p.failure.is_none() && self.presence_mounted)?;
        let target = self.targets.get(&surface)?;
        let state = target.presence.as_ref().filter(|p| {
            p.epoch == occurrence.epoch
                && p.mount == self.presence_mount
                && Some(p.key) == target.key
                && p.view_revision == self.requested.view_revision
        })?;
        Some((
            state.weight,
            state.terminal.is_some()
                && state.gpu.is_none()
                && state.pin_tuple.as_ref().is_some_and(|tuple| {
                    occurrence
                        .full
                        .as_ref()
                        .is_some_and(|full| Arc::ptr_eq(full, &tuple.source))
                        && tuple.view_revision == self.requested.view_revision
                        && tuple.view == self.requested.view
                }),
        ))
    }

    /// Cancellation restores only the still-live original requested source. Its
    /// target/currentness checks are performed by the ordinary preparation owner.
    pub fn cancel_loading_presence(&mut self) -> bool {
        let backup = self
            .presence
            .as_ref()
            .filter(|p| p.full.is_none())
            .and_then(|p| p.backup.clone());
        let existed = self.presence.is_some();
        self.retire_loading_presence();
        if let Some(backup) = backup {
            self.requested = backup;
        }
        existed
    }

    fn retire_loading_presence(&mut self) {
        if let Some(p) = self.presence.take() {
            if let Some(handle) = p.handle {
                self.retired_presence_handle = Some(handle);
            }
        }
        for target in self.targets.values_mut() {
            target.presence = None;
        }
    }

    fn prepare_loading_presence(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
        extent: PhysicalExtent,
    ) -> bool {
        use super::LoadingPresenceError3D as E;
        let Some(occurrence) = self.presence.as_mut() else {
            return false;
        };
        if occurrence.failure.is_some() || !self.presence_mounted {
            return false;
        }
        if let Err(error) = occurrence.proxy.current(&self.context) {
            occurrence.failure = Some(error);
            return true;
        }
        let key = Viewport3DTargetCompatibilityKey {
            participant_identity: self.id.raw(),
            context_identity: self.context.identity_token(),
            surface_id: metrics.surface_id,
            physical_render_extent: extent,
            color_format: super::hdr::FINAL_FORMAT,
            sample_count: self.requested.sample_count,
        };
        let target = self.targets.entry(metrics.surface_id).or_insert_with(|| {
            Viewport3DTargetState::new(self.requested.view.pose().translation())
        });
        target.last_metrics = Some(*metrics);
        if target.presence.as_ref().is_some_and(|p| {
            p.epoch != occurrence.epoch || p.mount != self.presence_mount || p.key != key
        }) {
            target.presence = None;
            occurrence.queued = true;
            occurrence.weight = 0.0;
            occurrence.terminal = false;
        }
        if target.key.is_some_and(|old| old != key) {
            target.retire_incompatible();
        }
        target.key = Some(key);
        if target.presence.is_none() {
            let active = target.hdr.as_ref().and_then(|h| h.active_credit());
            match super::loading_presence::PresenceGpuTarget::new(
                &self.context,
                &occurrence.proxy,
                extent.width,
                extent.height,
                self.requested.sample_count.value(),
                active,
            ) {
                Ok(gpu) => {
                    target.presence = Some(PresenceTargetState {
                        terminal_factory: gpu.terminal_factory(),
                        gpu: Some(gpu),
                        epoch: occurrence.epoch,
                        mount: self.presence_mount,
                        view_revision: self.requested.view_revision,
                        key,
                        pin: None,
                        pin_tuple: None,
                        ready: false,
                        ready_at: None,
                        weight: 0.0,
                        complete: false,
                        rendered: false,
                        terminal: None,
                        terminal_generation: None,
                        notified_full: None,
                    })
                }
                Err(error) => {
                    occurrence.failure = Some(error);
                    return true;
                }
            }
        }
        let p = target.presence.as_mut().expect("admitted private target");
        // View changes update uniforms and require a matching new full front;
        // they do not rebuild immutable source geometry or restart t_entry.
        if p.view_revision != self.requested.view_revision {
            p.view_revision = self.requested.view_revision;
            p.ready = false;
        }
        let mut publication_changed = false;
        // A new exposure/view may still be preparing while the previous
        // complete F remains lawful last-good. Do not relabel its pin or write
        // another transition sample using the new tuple's source custody.
        if p.pin_tuple.as_ref().is_some_and(|tuple| {
            occurrence
                .full
                .as_ref()
                .is_none_or(|full| !Arc::ptr_eq(full, &tuple.source))
                || tuple.view_revision != self.requested.view_revision
                || tuple.view != self.requested.view
        }) {
            return false;
        }
        if let Some(gpu) = p.gpu.as_mut() {
            let phase = match (p.ready, p.ready_at, occurrence.entry_at) {
                (true, Some(ready), Some(entry)) if occurrence.motion.allow_nonessential_motion => {
                    loading_presence_phase(entry, ready)
                }
                _ => occurrence.phase,
            };
            match gpu.render(
                self.requested.view,
                phase,
                p.weight,
                occurrence.revision,
                p.pin.as_ref().filter(|_| p.ready),
                p.pin_tuple.clone().filter(|_| p.ready),
            ) {
                Ok(changed) => {
                    p.rendered |= changed;
                    publication_changed = changed;
                }
                Err(error) => {
                    occurrence.failure = Some(error);
                    target.presence = None;
                    publication_changed = true;
                }
            }
        }
        if self.context.device_failure().is_some() {
            occurrence.failure = Some(E::Device("Context Device retired".into()));
            target.presence = None;
        }
        publication_changed
    }

    fn update_loading_presence(&mut self, cx: &mut UpdateContext) {
        use super::LoadingPresenceError3D as E;
        use crate::core::animation::{Playback, RepeatCount};
        if let Some(h) = self.retired_presence_handle.take() {
            cx.cancel_animation(h);
        }
        let Some(p) = self.presence.as_mut() else {
            return;
        };
        if p.failure.is_some() {
            let had_handle = p.handle.take();
            if let Some(h) = had_handle {
                cx.cancel_animation(h);
            }
            let mut withdrew = had_handle.is_some();
            for target in self.targets.values_mut() {
                withdrew |= target.presence.take().is_some();
            }
            if withdrew {
                cx.invalidate_paint();
            }
            return;
        }
        if !self.presence_mounted
            || p.proxy.current(&self.context).is_err()
            || self.context.device_failure().is_some()
        {
            p.failure = Some(E::Association("loading occurrence owner/source retired"));
            if let Some(h) = p.handle.take() {
                cx.cancel_animation(h);
            }
            for target in self.targets.values_mut() {
                target.presence = None;
            }
            cx.invalidate_paint();
            return;
        }
        let mut changed = std::mem::take(&mut p.queued);
        let old_phase = p.phase;
        let old_weight = p.weight;
        if let Some(handle) = p.handle {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::Active(t)) | Some(AnimationSample::Completed(t))
                    if t.is_finite() => {}
                _ => {
                    p.failure = Some(E::Resource("HAL021 sample unavailable".into()));
                    p.handle = None;
                }
            }
        }
        // Only actual matching canonical commits can issue a pin. Registration
        // is resolved after the existing renderer drain, at a later legal Update.
        let now = cx.animation_frame_timestamp_secs();
        if p.entry_at.is_none() {
            p.entry_at = now;
        }
        let active = self.targets.values().any(|target| {
            target.presence.is_some()
                && target
                    .last_metrics
                    .is_some_and(|m| matches!(m.activity, CustomRenderActivity::Active { .. }))
        });
        if active && p.motion.allow_nonessential_motion {
            if let Some(now) = now {
                let entry = *p.entry_at.get_or_insert(now);
                p.phase = loading_presence_phase(entry, now);
            }
        } else if !p.motion.allow_nonessential_motion {
            p.phase = 0.0;
        }
        let duration = f64::from(p.transition_duration.as_millis()) / 1_000.0;
        for target in self.targets.values_mut() {
            let Some(t) = target
                .presence
                .as_mut()
                .filter(|t| t.epoch == p.epoch && t.mount == self.presence_mount)
            else {
                continue;
            };
            let canonical = p.full.as_ref().is_some_and(|source| {
                target.status.committed_material().is_some_and(|tuple| {
                    Arc::ptr_eq(&tuple.source, source)
                        && tuple.view_revision == self.requested.view_revision
                        && tuple.view == self.requested.view
                        && tuple.target_key == t.key
                })
            });
            if !canonical {
                continue;
            }
            let Some(hdr) = target.hdr.as_mut() else {
                continue;
            };
            if t.pin.as_ref().is_some_and(|pin| !hdr.matches_pin(pin)) {
                t.pin = None;
                t.pin_tuple = None;
                t.ready = false;
                changed = true;
            }
            if t.pin.is_none() {
                match hdr.pin_front() {
                    Ok(pin) => {
                        t.pin = Some(pin);
                        t.pin_tuple = target.status.committed_material().cloned();
                        changed = true;
                    }
                    Err(error) => {
                        p.failure = Some(E::Resource(error.to_string()));
                        continue;
                    }
                }
            }
            match t.pin.as_ref().expect("issued immutable pin").registered() {
                Ok(Some(_)) => {
                    p.transition_duration_frozen = true;
                    t.ready = true;
                    if !p.motion.allow_nonessential_motion {
                        t.weight = 1.0;
                        t.complete = true;
                    } else if let Some(now) = now {
                        let start = *t.ready_at.get_or_insert(now);
                        let u = ((now - start) / duration).clamp(0.0, 1.0) as f32;
                        let weight = u * u * (3.0 - 2.0 * u);
                        changed |= weight != t.weight;
                        t.weight = weight;
                        t.complete = now - start >= duration;
                    } else {
                        p.failure = Some(E::Resource("HAL021 frame timestamp unavailable".into()));
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    p.failure = Some(E::Resource(error.to_string()));
                }
            }
        }
        if p.failure.is_some() {
            if let Some(h) = p.handle.take() {
                cx.cancel_animation(h);
            }
            for target in self.targets.values_mut() {
                target.presence = None;
            }
            cx.invalidate_paint();
            return;
        }
        if p.handle.is_some() && !active {
            cx.cancel_animation(p.handle.take().expect("active animation"));
        }
        if p.handle.is_none() && !p.terminal && active && p.motion.allow_nonessential_motion {
            match cx.start_animation_descriptor(
                Animation::new(0.0, std::f32::consts::TAU, 3.2, Easing::Linear)
                    .with_playback(Playback::Loop(RepeatCount::Infinite)),
            ) {
                Ok(Some(handle)) => p.handle = Some(handle),
                _ => {
                    p.failure = Some(E::Resource(
                        "HAL021 waiting presentation admission refused".into(),
                    ))
                }
            }
        }
        {
            for target in self.targets.values_mut() {
                let Some(t) = target.presence.as_mut().filter(|t| t.ready && t.complete) else {
                    continue;
                };
                let Some(pin) = t.pin.as_ref() else {
                    continue;
                };
                let Some(_hdr) = target.hdr.as_ref().filter(|h| h.matches_pin(pin)) else {
                    continue;
                };
                let Some(binding) = pin.registered().ok().flatten() else {
                    continue;
                };
                if t.terminal_generation == Some(binding.identity().generation()) {
                    continue;
                }
                let Some(source) = t.pin_tuple.clone().filter(|tuple| {
                    p.full
                        .as_ref()
                        .is_some_and(|full| Arc::ptr_eq(full, &tuple.source))
                        && tuple.view_revision == self.requested.view_revision
                        && tuple.view == self.requested.view
                }) else {
                    continue;
                };
                match t.terminal_factory.output(&self.context, pin, source) {
                    Ok(output) => {
                        t.terminal = Some(output);
                        t.terminal_generation = Some(binding.identity().generation());
                        t.gpu = None;
                        changed = true;
                    }
                    Err(error) => p.failure = Some(error),
                }
            }
        }
        let mut any_target = false;
        let mut terminal = true;
        for target in self.targets.values() {
            if let Some(t) = target.presence.as_ref().filter(|t| t.epoch == p.epoch) {
                any_target = true;
                terminal &= t.terminal.is_some();
            }
        }
        p.terminal = any_target && terminal;
        if p.terminal {
            if let Some(h) = p.handle.take() {
                cx.cancel_animation(h);
            }
            p.backup = None;
        }
        p.weight = self
            .current_prepared_surface
            .and_then(|s| self.targets.get(&s))
            .and_then(|t| t.presence.as_ref())
            .map_or(0.0, |t| t.weight);
        if p.phase != old_phase || p.weight != old_weight || changed {
            if let Some(revision) = p.revision.checked_add(1) {
                p.revision = revision;
            } else {
                p.failure = Some(E::Association("presentation generation exhausted"));
            }
            cx.invalidate_paint();
        }
        // Refusals can also occur while admitting the Timeline or the terminal
        // reader. They are terminal now, without relying on a future frame
        // which a failed occurrence is deliberately no longer demanding.
        if p.failure.is_some() {
            if let Some(handle) = p.handle.take() {
                cx.cancel_animation(handle);
            }
            for target in self.targets.values_mut() {
                target.presence = None;
            }
            cx.invalidate_paint();
        }
    }

    /// The original exclusive-writer guard executes before any request or
    /// target mutation, including on equal-content linked/domain requests.
    pub fn request_pbr_scene(
        &mut self,
        source: super::material_api::PbrScene3D,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
    ) -> Result<bool, super::material_api::PbrSourceError3D> {
        Ok(self.set_material_source(source.0, view_revision, view)?)
    }

    /// Explicitly select an independently admitted whole source with a different
    /// producer HDR identity. Revisions remain scoped to their original lineage;
    /// this does not relax the successor contract of `request_pbr_scene`.
    pub fn replace_pbr_scene(
        &mut self,
        scene: super::material_api::PbrScene3D,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
    ) -> super::material_api::PbrSourceResult3D<bool> {
        Ok(self.set_material_source_with_intent(
            scene.0,
            view_revision,
            view,
            PbrSceneRequestIntent3D::IndependentReplacement,
        )?)
    }

    /// Queue an optional whole-current-source appearance. Timeline mutation is
    /// performed only by this mounted widget's ordinary Update capability.
    pub fn request_pbr_object_reveal(
        &mut self,
        mut request: PbrObjectRevealRequest3D,
    ) -> Result<bool, super::material_api::PbrSourceError3D> {
        use super::material_api::PbrSourceError3D;
        if self.linked_writer_active() {
            return Err(PbrSourceError3D::Association(
                "linked viewport has no reveal writer grant",
            ));
        }
        let RequestedSource3D::Material(current) = &self.requested.source else {
            return Err(PbrSourceError3D::Association(
                "object reveal requires the current PBR source",
            ));
        };
        if !Arc::ptr_eq(current.source(), &request.source.0)
            || !current.matches(&self.context, &request.source.0)
        {
            return Err(PbrSourceError3D::Association(
                "object reveal source/context is not current",
            ));
        }
        if request.request_revision == 0 || request.request_revision < self.reveal_request_revision
        {
            return Err(PbrSourceError3D::Association(
                "stale/nonpositive object reveal request revision",
            ));
        }
        if self.reveal_frame_revision == u64::MAX {
            return Err(PbrSourceError3D::Association(
                "object reveal presentation generation exhausted",
            ));
        }
        if request.enabled
            && (request.duration.is_zero()
                || !request
                    .extent
                    .as_ref()
                    .is_some_and(|e| e.matches(current.source())))
        {
            return Err(PbrSourceError3D::Association(
                "enabled object reveal requires its source extent and positive duration",
            ));
        }
        if !request.enabled {
            request.extent = None;
        }
        if let Some(old) = &mut self.object_reveal {
            if old.request.source.same_identity(&request.source)
                && old.request.enabled == request.enabled
                && old.request.duration == request.duration
            {
                self.reveal_request_revision = request.request_revision;
                old.request.request_revision = request.request_revision;
                return Ok(false);
            }
        }
        if request.request_revision == self.reveal_request_revision {
            return Err(PbrSourceError3D::Association(
                "reused object reveal request revision",
            ));
        }
        self.reveal_request_revision = request.request_revision;
        let old = self.object_reveal.take();
        let progress = old
            .as_ref()
            .map_or(if request.enabled { 0.0 } else { 1.0 }, |o| o.progress);
        let target_ready = old.as_ref().is_some_and(|o| o.target_ready);
        let handle = old.and_then(|o| o.handle);
        self.object_reveal = Some(RevealOccurrence3D {
            request,
            progress,
            handle,
            pending: true,
            queued: true,
            target_ready,
            failure: None,
        });
        Ok(true)
    }

    fn retire_object_reveal(&mut self) {
        if let Some(old) = self.object_reveal.take() {
            if old.handle.is_some() {
                self.retired_reveal_handle = old.handle;
            }
            if let Some(next) = self.reveal_frame_revision.checked_add(1) {
                self.reveal_frame_revision = next;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn reveal_progress_for_test(&self) -> Option<f32> {
        self.object_reveal.as_ref().map(|r| r.progress)
    }

    pub fn pbr_failure(
        &self,
        surface_id: SurfaceId,
    ) -> Option<super::material_api::PbrSourceError3D> {
        let target = self.targets.get(&surface_id)?;
        if let Some(error) = &target.material_failure {
            return Some(error.clone().into());
        }
        target.hdr_failure.as_ref().map(|(_, error)| match error {
            HdrError3D::Context(e) => super::material_api::PbrSourceError3D::Context(e.clone()),
            HdrError3D::Source(e) | HdrError3D::Target(e) => {
                super::material_api::PbrSourceError3D::Output(e)
            }
            HdrError3D::Shader(e) => super::material_api::PbrSourceError3D::Gpu(e.clone()),
            HdrError3D::PinnedFront => super::material_api::PbrSourceError3D::Output(
                "canonical final is pinned by a queued/active presentation reader",
            ),
        })
    }

    /// Copies exactly one existing linear-FP16 final-front pixel. Pending,
    /// failed-last-good, inactive and stale source requests are not observations
    /// of the current HEAD. Drain the renderer's pending Image registrations
    /// after preparation; that same current image ID can then be sampled by the
    /// renderer's test-harness Image diagnostic.
    #[cfg(feature = "test-harness")]
    pub fn pbr_front_pixel_for_test_harness(
        &self,
        surface: SurfaceId,
        pixel: [u32; 2],
    ) -> Result<PbrFrontPixelForTestHarness3D, PbrFrontProofError3D> {
        self.pbr_presentation_pixel_for_test_harness(surface, pixel, false)
    }

    /// Read the actual registered private HAL374 output, retaining its genuine
    /// canonical tuple attribution. None means no eligible composed Outline.
    /// This is GPU evidence only, not a claim about physical display pixels.
    #[cfg(feature = "test-harness")]
    pub fn pbr_outline_pixel_for_test_harness(
        &self,
        surface: SurfaceId,
        pixel: [u32; 2],
    ) -> Result<Option<PbrFrontPixelForTestHarness3D>, PbrFrontProofError3D> {
        let target = self
            .targets
            .get(&surface)
            .ok_or(PbrFrontProofError3D::MissingTarget(surface))?;
        let Some(outline) = target.outline.as_ref().filter(|o| o.gpu.composed()) else {
            return Ok(None);
        };
        let valid = self.presence_mounted
            && self.outline_request.as_ref().is_some_and(|r| {
                r.epoch == outline.epoch
                    && outline.mount == self.presence_mount
                    && target
                        .status
                        .committed_material()
                        .is_some_and(|t| r.selection.matches_source(&t.source))
            })
            && outline
                .pin
                .as_ref()
                .is_some_and(|pin| target.hdr.as_ref().is_some_and(|h| h.matches_pin(pin)));
        if !valid {
            return Err(PbrFrontProofError3D::NotCurrent);
        }
        self.pbr_presentation_pixel_for_test_harness(surface, pixel, true)
            .map(Some)
    }

    #[cfg(feature = "test-harness")]
    fn pbr_presentation_pixel_for_test_harness(
        &self,
        surface: SurfaceId,
        pixel: [u32; 2],
        outline: bool,
    ) -> Result<PbrFrontPixelForTestHarness3D, PbrFrontProofError3D> {
        use crate::renderer::wgpu_renderer::{TestHarnessImageProofError, mapped_test_pixel_bytes};
        if self.context.device_failure().is_some() {
            return Err(PbrFrontProofError3D::DeviceUnavailable);
        }
        let target = self
            .targets
            .get(&surface)
            .ok_or(PbrFrontProofError3D::MissingTarget(surface))?;
        let committed = target
            .status
            .committed
            .as_ref()
            .ok_or(PbrFrontProofError3D::NotCurrent)?;
        let tuple = committed.material().ok_or(PbrFrontProofError3D::NotPbr)?;
        let hdr = target
            .hdr
            .as_ref()
            .filter(|hdr| hdr.is_live())
            .ok_or(PbrFrontProofError3D::FrontUnavailable)?;
        let extent = [hdr.extent.0, hdr.extent.1];
        let key = self.key_for(surface, PhysicalExtent::new(extent[0], extent[1]));
        if target.status.state != Viewport3DLifecycleState::Committed
            || target.key != Some(key)
            || !target.requested_matches(key, &self.requested)
            || !committed.matches(key, &self.requested)
            || target.status.requested_material().map(|r| r.request_serial)
                != Some(tuple.request_serial)
        {
            return Err(PbrFrontProofError3D::NotCurrent);
        }
        if pixel[0] >= extent[0] || pixel[1] >= extent[1] {
            return Err(PbrFrontProofError3D::PixelOutOfBounds { pixel, extent });
        }
        let texture = if outline {
            target
                .outline
                .as_ref()
                .ok_or(PbrFrontProofError3D::FrontUnavailable)?
                .gpu
                .output_texture()
        } else {
            hdr.front_for_test_harness()
                .map_err(|_| PbrFrontProofError3D::FrontUnavailable)?
        };
        if texture.format() != wgpu::TextureFormat::Rgba16Float
            || texture.sample_count() != 1
            || texture.width() != extent[0]
            || texture.height() != extent[1]
            || texture.depth_or_array_layers() != 1
            || texture.mip_level_count() != 1
            || !texture.usage().contains(wgpu::TextureUsages::COPY_SRC)
        {
            return Err(PbrFrontProofError3D::InvalidFinal);
        }
        let gpu = self.context.gpu_resources()?;
        let image_id = if outline {
            target
                .outline
                .as_ref()
                .ok_or(PbrFrontProofError3D::FrontUnavailable)?
                .gpu
                .output
                .id()
        } else {
            target
                .sampleable_texture_id()
                .ok_or(PbrFrontProofError3D::FrontUnavailable)?
        };
        let registration_pending = gpu
            .pending_registrations
            .lock()
            .map_err(|_| TestHarnessImageProofError::RegistrationLock)?
            .iter()
            .any(|r| r.texture_id == image_id);
        let removal_pending = gpu
            .pending_unregistrations
            .lock()
            .map_err(|_| TestHarnessImageProofError::RegistrationLock)?
            .iter()
            .any(|r| r.texture_id == image_id);
        if registration_pending || removal_pending {
            return Err(TestHarnessImageProofError::RegistrationPending(image_id).into());
        }
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PBR test-harness committed FP16 pixel"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("PBR test-harness existing front copy"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: pixel[0],
                    y: pixel[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let bytes = mapped_test_pixel_bytes(&gpu.device, &buffer)?;
        if self.context.device_failure().is_some() {
            return Err(PbrFrontProofError3D::DeviceUnavailable);
        }
        let mut rgba = [0.0; 4];
        for (channel, value) in rgba.iter_mut().enumerate() {
            let bits = u16::from_le_bytes([bytes[2 * channel], bytes[2 * channel + 1]]);
            let exponent = (bits >> 10) & 31;
            let fraction = bits & 1023;
            if (bits >> 15 != 0 && bits & 0x7fff != 0) || exponent == 31 {
                return Err(PbrFrontProofError3D::InvalidPixel);
            }
            *value = if exponent == 0 {
                f32::from(fraction) * 2.0_f32.powi(-24)
            } else {
                (1.0 + f32::from(fraction) / 1024.0) * 2.0_f32.powi(i32::from(exponent) - 15)
            };
            if bits >> 15 != 0 {
                *value = -*value;
            }
        }
        Ok(PbrFrontPixelForTestHarness3D {
            source: super::material_api::PbrScene3D(tuple.source.clone()),
            request_serial: tuple.request_serial,
            target_key: tuple.target_key,
            view_revision: tuple.view_revision,
            view: tuple.view,
            image_id,
            extent,
            pixel,
            rgba,
        })
    }

    pub fn context(&self) -> &Arc<Context3D> {
        &self.context
    }

    #[cfg(test)]
    pub(crate) fn requested_model_for_test(&self) -> &ModelState {
        self.requested
            .source
            .legacy()
            .expect("legacy-only test model")
            .1
    }

    /// HAL-354/HAL-355: direct requests are refused while this mount has a linked writer.
    pub fn set_request(
        &mut self,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        model: ModelState,
        sample_count: SampleCount3D,
    ) -> Result<(), Viewport3DError> {
        // INV-355-G1-EXCLUSIVE-ADMISSION: refuse even equal requests before validation.
        if self.linked_writer_active() {
            return Err(Viewport3DError::LinkedWriterExclusive);
        }
        if !self.requested.source.is_flat() {
            return Err(Viewport3DError::RequestKindMismatch);
        }
        self.set_request_with_kind(
            view_revision,
            view,
            snapshot,
            model,
            sample_count,
            Viewport3DRequestKind::Flat,
        )
        .map(|_| ())
    }

    pub fn set_domain_composition_request(
        &mut self,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        frame: DomainFrame3,
        sample_count: SampleCount3D,
    ) -> Result<(), Viewport3DError> {
        if self.linked_writer_active() {
            return Err(Viewport3DError::LinkedWriterExclusive);
        }
        let Some((_, _, Viewport3DRequestKind::DomainAware(current))) =
            self.requested.source.legacy()
        else {
            return Err(Viewport3DError::RequestKindMismatch);
        };
        current.validate_successor(&frame)?;
        let snapshot = frame.composition().render_snapshot().clone();
        let model = frame.model().clone();
        self.set_request_with_kind(
            view_revision,
            view,
            snapshot,
            model,
            sample_count,
            Viewport3DRequestKind::DomainAware(frame),
        )
        .map(|_| ())
    }

    fn linked_writer_active(&self) -> bool {
        self.flat_viewport3d_writer
            .as_ref()
            .is_some_and(FlatViewport3DWriterAuthority::is_active)
    }

    pub(crate) fn claim_flat_viewport3d_writer(
        &mut self,
        authority: FlatViewport3DWriterAuthority,
    ) -> Result<(), Viewport3DError> {
        if !self.requested.source.is_flat() {
            return Err(Viewport3DError::RequestKindMismatch);
        }
        if !authority.is_active() || self.linked_writer_active() {
            return Err(Viewport3DError::LinkedWriterExclusive);
        }
        self.flat_viewport3d_writer = Some(authority);
        Ok(())
    }

    #[allow(
        dead_code,
        reason = "Retained exact linked-writer retirement operation"
    )]
    pub(crate) fn release_flat_viewport3d_writer(
        &mut self,
        authority: &FlatViewport3DWriterAuthority,
    ) -> bool {
        if self.flat_viewport3d_writer.as_ref() != Some(authority) {
            return false;
        }
        authority.retire();
        self.flat_viewport3d_writer = None;
        true
    }

    pub(crate) fn transfer_flat_viewport3d_request(
        &mut self,
        authority: &FlatViewport3DWriterAuthority,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        model: ModelState,
        sample_count: SampleCount3D,
    ) -> Result<bool, Viewport3DError> {
        if !authority.is_active() || self.flat_viewport3d_writer.as_ref() != Some(authority) {
            return Err(Viewport3DError::LinkedWriterExclusive);
        }
        if !self.requested.source.is_flat() {
            return Err(Viewport3DError::RequestKindMismatch);
        }
        self.set_request_with_kind(
            view_revision,
            view,
            snapshot,
            model,
            sample_count,
            Viewport3DRequestKind::Flat,
        )
    }

    fn set_request_with_kind(
        &mut self,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        model: ModelState,
        sample_count: SampleCount3D,
        request_kind: Viewport3DRequestKind,
    ) -> Result<bool, Viewport3DError> {
        if !self.context.same_resource_family(snapshot.context()) {
            return Err(Context3DError::ContextMismatch.into());
        }
        if !model.matches_snapshot(&snapshot) {
            return Err(ModelStateError::StructuralIdentityMismatch.into());
        }
        if view_revision < self.requested.view_revision {
            return Err(Viewport3DError::StaleViewRevision {
                current: self.requested.view_revision,
                requested: view_revision,
            });
        }
        if view_revision == self.requested.view_revision && view != self.requested.view {
            return Err(Viewport3DError::ReusedViewRevision {
                revision: view_revision,
            });
        }
        let (current_snapshot, current_model, current_kind) = self
            .requested
            .source
            .legacy()
            .ok_or(Viewport3DError::RequestKindMismatch)?;
        current_snapshot.validate_successor(&snapshot)?;
        current_model.validate_successor(&model)?;
        if view_revision == self.requested.view_revision
            && current_snapshot.same_identity(&snapshot)
            && current_model.same_identity(&model)
            && sample_count == self.requested.sample_count
            && current_kind.same_identity(&request_kind)
        {
            return Ok(false);
        }

        self.retire_object_reveal();
        self.retire_pbr_outline();
        self.requested = RequestedSemanticState {
            view_revision,
            view,
            sample_count,
            source: RequestedSource3D::Legacy {
                snapshot,
                model,
                kind: request_kind,
            },
        };
        Ok(true)
    }

    pub fn status_for(&self, surface_id: SurfaceId) -> Option<&Viewport3DStatus> {
        self.targets.get(&surface_id).map(|target| &target.status)
    }

    pub fn target_statuses(
        &self,
    ) -> impl ExactSizeIterator<Item = Viewport3DTargetStatus<'_>> + '_ {
        self.targets
            .iter()
            .map(|(&surface_id, target)| Viewport3DTargetStatus {
                surface_id,
                metrics: target.last_metrics,
                status: &target.status,
            })
    }

    pub fn current_prepared_target(&self) -> Option<Viewport3DTargetStatus<'_>> {
        let surface_id = self.current_prepared_surface?;
        let target = self.targets.get(&surface_id)?;
        Some(Viewport3DTargetStatus {
            surface_id,
            metrics: target.last_metrics,
            status: &target.status,
        })
    }

    fn allocate_request_serial(&mut self) -> Result<Viewport3DRequestSerial, Viewport3DError> {
        let value = self
            .next_request_serial
            .ok_or(Viewport3DError::RequestSerialExhausted)?;
        self.next_request_serial = value.checked_add(1);
        Ok(Viewport3DRequestSerial(value))
    }

    fn key_for(
        &self,
        surface_id: SurfaceId,
        physical_render_extent: PhysicalExtent,
    ) -> Viewport3DTargetCompatibilityKey {
        Viewport3DTargetCompatibilityKey {
            participant_identity: self.id.raw(),
            context_identity: self.context.identity_token(),
            surface_id,
            physical_render_extent,
            color_format: if self.requested.source.hdr_source().is_some()
                || matches!(self.requested.source, RequestedSource3D::PresencePending)
            {
                super::hdr::FINAL_FORMAT
            } else {
                self.context.surface_format()
            },
            sample_count: self.requested.sample_count,
        }
    }

    fn requested_tuple(
        &self,
        request_serial: Viewport3DRequestSerial,
        target_key: Viewport3DTargetCompatibilityKey,
    ) -> Option<RetainedSourceTuple3D> {
        Some(match &self.requested.source {
            RequestedSource3D::PresencePending => return None,
            RequestedSource3D::Legacy {
                snapshot,
                model,
                kind,
            } => RetainedSourceTuple3D::Legacy(Viewport3DTuple {
                request_serial,
                target_key,
                view_revision: self.requested.view_revision,
                view: self.requested.view,
                snapshot: snapshot.clone(),
                model: model.clone(),
                request_kind: kind.clone(),
            }),
            RequestedSource3D::Material(source) => {
                RetainedSourceTuple3D::Material(MaterialViewport3DTuple {
                    request_serial,
                    target_key,
                    view_revision: self.requested.view_revision,
                    view: self.requested.view,
                    source: source.source().clone(),
                })
            }
        })
    }

    fn preparation_outcome_for(
        &self,
        surface_id: SurfaceId,
        front_publication_changed: bool,
    ) -> CustomRenderPreparationOutcome {
        let binding = self.targets.get(&surface_id).and_then(|target| {
            if let Some(p) = target.presence.as_ref().filter(|p| {
                self.presence.as_ref().is_some_and(|r| {
                    r.failure.is_none()
                        && r.epoch == p.epoch
                        && self.presence_mounted
                        && p.mount == self.presence_mount
                })
            }) {
                if let Some(terminal) = &p.terminal {
                    return Some(self.admission.loading_presence_binding(terminal.id(), true));
                }
                if let Some(gpu) = p.gpu.as_ref().filter(|_| p.rendered) {
                    return Some(
                        self.admission
                            .loading_presence_binding(gpu.output.id(), false),
                    );
                }
            }
            if let Some(outline) = target.outline.as_ref().filter(|o| {
                self.outline_request.as_ref().is_some_and(|r| {
                    r.epoch == o.epoch && self.presence_mounted && o.mount == self.presence_mount
                        && matches!(&self.requested.source, RequestedSource3D::Material(s) if r.selection.matches_source(s.source()))
                        && target.status.committed_material().is_some_and(|t| t.request_serial == o.request
                            && t.target_key == o.key && t.view_revision == o.view_revision && t.view == o.view
                            && self.requested.matches_committed_material_view(t) && r.selection.matches_source(&t.source))
                }) && o.gpu.composed() && o.pin.as_ref().is_some_and(|p| target.hdr.as_ref().is_some_and(|h| h.matches_pin(p)))
            }) {
                return Some(self.admission.loading_presence_binding(outline.gpu.output.id(), false));
            }
            target
                .sampleable_texture_id()
                .map(|id| self.admission.presentation_binding(id))
        });
        match binding {
            Some(binding) => CustomRenderPreparationOutcome::with_presentation_binding(
                front_publication_changed,
                binding,
            ),
            None => CustomRenderPreparationOutcome::new(front_publication_changed),
        }
    }

    fn prepare_terminal_device(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
        failure: GpuDeviceFailure,
    ) -> CustomRenderPreparationOutcome {
        let initial_origin = self.requested.view.pose().translation();
        self.targets
            .entry(metrics.surface_id)
            .or_insert_with(|| Viewport3DTargetState::new(initial_origin))
            .last_metrics = Some(*metrics);
        for target in self.targets.values_mut() {
            target.mark_terminal(failure.clone());
        }
        let front_publication_changed = self
            .targets
            .get_mut(&metrics.surface_id)
            .is_some_and(Viewport3DTargetState::take_terminal_publication_withdrawal);
        CustomRenderPreparationOutcome::new(front_publication_changed)
    }

    fn prepare_inactive_target(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome {
        #[cfg(feature = "render-audit")]
        cadence_audit::record_inactive_preparation();
        let surface_id = metrics.surface_id;
        let key = self.key_for(surface_id, PhysicalExtent::new(0, 0));
        let initial_origin = self.requested.view.pose().translation();
        self.targets
            .entry(surface_id)
            .or_insert_with(|| Viewport3DTargetState::new(initial_origin));
        let stable = self.targets.get(&surface_id).is_some_and(|target| {
            target.status.state == Viewport3DLifecycleState::Inactive
                && target.requested_matches(key, &self.requested)
        });
        if stable {
            #[cfg(feature = "render-audit")]
            cadence_audit::record_unchanged_preparation();
            return CustomRenderPreparationOutcome::default();
        }

        let front_publication_changed = self
            .targets
            .get(&surface_id)
            .and_then(Viewport3DTargetState::sampleable_texture_id)
            .is_some();
        let request_serial = match self.allocate_request_serial() {
            Ok(serial) => serial,
            Err(_) => {
                #[cfg(feature = "render-audit")]
                cadence_audit::record_failed_preparation();
                if let Some(target) = self.targets.get_mut(&surface_id) {
                    target.last_metrics = Some(*metrics);
                    target.retire(Viewport3DLifecycleState::Inactive);
                    target.key = Some(key);
                }
                return CustomRenderPreparationOutcome::new(front_publication_changed);
            }
        };
        let requested_tuple = self.requested_tuple(request_serial, key);
        if let Some(target) = self.targets.get_mut(&surface_id) {
            target.last_metrics = Some(*metrics);
            target.retire(Viewport3DLifecycleState::Inactive);
            target.key = Some(key);
            target.status.requested = requested_tuple;
        }
        CustomRenderPreparationOutcome::new(front_publication_changed)
    }

    #[cfg(target_arch = "wasm32")]
    fn wake_browser_material_successor(
        &self,
        surface: SurfaceId,
        key: Viewport3DTargetCompatibilityKey,
    ) {
        if self.targets.get(&surface).is_some_and(|target| {
            matches!(self.requested.source, RequestedSource3D::Material(_))
                && !target.requested_matches(key, &self.requested)
        }) {
            if let Ok(gpu) = self.context.gpu_resources() {
                // Completion has settled the sole active attempt. The existing
                // App preparation wake now admits the one coalesced successor,
                // including when the pointer stopped before completion.
                gpu.wake_custom_render_preparation();
            }
        }
    }

    fn prepare_active_target(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
        physical_render_extent: PhysicalExtent,
        physical_composition_coverage: PhysicalRect,
    ) -> CustomRenderPreparationOutcome {
        #[cfg(any(test, feature = "test-harness"))]
        let active_start = self.pbr_timing.as_ref().map(|_| std::time::Instant::now());
        #[cfg(any(test, feature = "test-harness"))]
        let coarse_active_start = self
            .pbr_coarse_timing
            .as_ref()
            .map(|_| std::time::Instant::now());
        let surface_id = metrics.surface_id;
        let key = self.key_for(surface_id, physical_render_extent);
        let initial_origin = self.requested.view.pose().translation();
        self.targets
            .entry(surface_id)
            .or_insert_with(|| Viewport3DTargetState::new(initial_origin));

        let mut publication_changed = false;
        #[cfg(target_arch = "wasm32")]
        let retire_browser_attempt = self.targets.get(&surface_id).is_some_and(|target| {
            target
                .browser_material_preparation
                .as_ref()
                .is_some_and(|pending| !pending.attempt.compatible(self, key, metrics))
        });
        {
            let Some(target) = self.targets.get_mut(&surface_id) else {
                return CustomRenderPreparationOutcome::default();
            };
            #[cfg(target_arch = "wasm32")]
            if retire_browser_attempt {
                drop(target.browser_material_preparation.take());
                target.status.requested = None;
            }
            target.last_metrics = Some(*metrics);
            if target.key.is_some_and(|current| current != key) {
                publication_changed = target.sampleable_texture_id().is_some();
                target.retire_incompatible();
            }
            target.composition_coverage = Some(physical_composition_coverage);
        }

        let mut capacity_progress_error = None;
        if let Some(target) = self.targets.get_mut(&surface_id) {
            #[cfg(target_arch = "wasm32")]
            let capacity_origin = target
                .browser_material_preparation
                .as_ref()
                .map_or(target.render_origin, |pending| pending.origin);
            #[cfg(not(target_arch = "wasm32"))]
            let capacity_origin = target.render_origin;
            let compatible = match &self.requested.source {
                RequestedSource3D::Material(source) => {
                    target.material_capacity.as_ref().is_none_or(|pool| {
                        pool.matches(
                            source,
                            capacity_origin,
                            (physical_render_extent.width, physical_render_extent.height),
                        )
                    })
                }
                _ => false,
            };
            if !compatible {
                drop(target.material_capacity.take());
            }
            // Progress belongs to this already requested preparation, not an
            // idle maintenance cadence or a new semantic render request.
            if !target.requested_matches(key, &self.requested) || target.material_failure.is_some()
            {
                if let Some(pool) = target.material_capacity.as_mut() {
                    capacity_progress_error = pool.progress(&self.context).err();
                } else if let Some(budget) = target.material_capacity_budget.as_ref() {
                    capacity_progress_error = budget.progress_retired(&self.context).err();
                }
            }
        }
        let dynamic_release_revision = self.context.dynamic_storage_release_revision();
        let material_capacity = if matches!(self.requested.source, RequestedSource3D::Material(_)) {
            self.context.resource_usage().ok().map(|u| {
                (
                    u.admitted_shader_modules,
                    u.resident_pipeline_variants,
                    u.resident_geometry_bytes,
                    u.resident_texture_bytes,
                )
            })
        } else {
            None
        };
        let hdr_budget = if self.requested.source.hdr_source().is_some() {
            self.context.target_budget_usage().ok()
        } else {
            None
        };
        let needs_attempt = self
            .targets
            .get(&surface_id)
            .map(|target| {
                #[cfg(target_arch = "wasm32")]
                if let Some(pending) = &target.browser_material_preparation {
                    return pending.work.revision() != pending.observed_revision;
                }
                !target.requested_matches(key, &self.requested)
                    || (self.spatial_hud_typography.is_some()
                        && self.spatial_hud_request.is_some()
                        && target.last_spatial_hud_attempt != self.spatial_hud_epoch)
                    || target.last_outline_attempt_revision != self.outline_revision
                    || target.last_reveal_attempt_revision != self.reveal_frame_revision
                    || capacity_progress_error.is_some()
                    || (target
                        .material_capacity
                        .as_ref()
                        .map_or(0, |pool| pool.ready_revision())
                        != target.last_capacity_ready_attempt
                        && target.material_failure.as_ref().is_some_and(|e| {
                            matches!(
                                e,
                                super::pbr::PbrError3D::Context(
                                    Context3DError::PolicyBudgetExceeded { .. }
                                ) | super::pbr::PbrError3D::Resource(
                                    super::context::ControlledResourceError3D::Context(
                                        Context3DError::PolicyBudgetExceeded { .. }
                                    )
                                )
                            )
                        }))
                    || target.has_retryable_pending_budget_failure()
                    || target.has_released_dynamic_budget_failure(dynamic_release_revision)
                    || (target
                        .hdr_failure
                        .as_ref()
                        .is_some_and(|(_, e)| matches!(e, HdrError3D::PinnedFront))
                        && target.hdr.as_ref().is_some_and(|h| h.back_writable()))
                    || (material_capacity != target.material_capacity_attempt
                        && target.material_failure.as_ref().is_some_and(|e| {
                            matches!(
                                e,
                                super::pbr::PbrError3D::Context(
                                    Context3DError::PolicyBudgetExceeded { .. }
                                ) | super::pbr::PbrError3D::Resource(
                                    super::context::ControlledResourceError3D::Context(
                                        Context3DError::PolicyBudgetExceeded { .. }
                                    )
                                )
                            )
                        }))
                    || (hdr_budget != target.last_hdr_budget_attempt
                        && target.hdr_failure.as_ref().is_some_and(|(_, error)| {
                            matches!(
                                error,
                                HdrError3D::Context(Context3DError::PolicyBudgetExceeded {
                                    budget: PolicyBudget3D::TotalAttachmentBytes
                                        | PolicyBudget3D::ActiveTargetPixels,
                                    ..
                                })
                            )
                        }))
                    || (target.status.committed.is_some()
                        && target.sampleable_texture_id().is_none())
            })
            .unwrap_or(true);
        if !needs_attempt {
            #[cfg(feature = "render-audit")]
            cadence_audit::record_unchanged_preparation();
            return self.preparation_outcome_for(surface_id, publication_changed);
        }
        if let Some(target) = self.targets.get_mut(&surface_id) {
            if self.spatial_hud_typography.is_some() && self.spatial_hud_request.is_some() {
                target.last_spatial_hud_attempt = self.spatial_hud_epoch;
            }
            target.last_reveal_attempt_revision = self.reveal_frame_revision;
            target.last_outline_attempt_revision = self.outline_revision;
            target.last_dynamic_budget_attempt_revision = dynamic_release_revision;
            target.last_hdr_budget_attempt = hdr_budget;
            target.last_capacity_ready_attempt = target
                .material_capacity
                .as_ref()
                .map_or(0, |pool| pool.ready_revision());
        }

        #[cfg(target_arch = "wasm32")]
        let resumed_serial = self
            .targets
            .get(&surface_id)
            .and_then(|t| t.browser_material_preparation.as_ref())
            .map(|p| {
                p.attempt
                    .tuple
                    .material()
                    .expect("captured material attempt")
                    .request_serial
            });
        #[cfg(not(target_arch = "wasm32"))]
        let resumed_serial = None;
        let request_serial = match resumed_serial.map_or_else(|| self.allocate_request_serial(), Ok)
        {
            Ok(serial) => serial,
            Err(_) => {
                #[cfg(feature = "render-audit")]
                cadence_audit::record_failed_preparation();
                return self.preparation_outcome_for(surface_id, publication_changed);
            }
        };

        #[cfg(target_arch = "wasm32")]
        let resumed = self
            .targets
            .get(&surface_id)
            .and_then(|target| target.browser_material_preparation.as_ref());
        #[cfg(target_arch = "wasm32")]
        let requested_tuple = resumed
            .map(|pending| pending.attempt.tuple.clone())
            .or_else(|| self.requested_tuple(request_serial, key));
        #[cfg(not(target_arch = "wasm32"))]
        let requested_tuple = self.requested_tuple(request_serial, key);
        let Some(requested_tuple) = requested_tuple else {
            return self.preparation_outcome_for(surface_id, publication_changed);
        };
        #[cfg(target_arch = "wasm32")]
        let (render_requested, render_intent) = resumed
            .map(|pending| (pending.attempt.requested.clone(), pending.attempt.intent))
            .unwrap_or_else(|| (self.requested.clone(), self.pbr_request_intent));
        #[cfg(not(target_arch = "wasm32"))]
        let (render_requested, render_intent) = (self.requested.clone(), self.pbr_request_intent);
        #[cfg(target_arch = "wasm32")]
        let new_browser_attempt = (resumed_serial.is_none()
            && matches!(render_requested.source, RequestedSource3D::Material(_)))
        .then(|| BrowserMaterialAttempt3D::new(self, requested_tuple.clone(), *metrics));
        let mut _pending_request = match resumed_serial.map_or_else(
            || self.context.reserve_pending_request().map(Some),
            |_| Ok(None),
        ) {
            Ok(lease) => lease,
            Err(error) => {
                if let Some(target) = self.targets.get_mut(&surface_id) {
                    target.key = Some(key);
                    target.status.requested = Some(requested_tuple);
                    target.record_failure(request_serial, error.into());
                }
                return self.preparation_outcome_for(surface_id, publication_changed);
            }
        };
        let committed_tuple = requested_tuple.clone();
        {
            let Some(target) = self.targets.get_mut(&surface_id) else {
                return CustomRenderPreparationOutcome::default();
            };
            target.key = Some(key);
            target.status.requested = Some(requested_tuple);
            target.status.failure = None;
            target.status.state = Viewport3DLifecycleState::ReadyWithoutCommit;
            #[cfg(target_arch = "wasm32")]
            if matches!(self.requested.source, RequestedSource3D::Material(_))
                && target.has_sampleable_front()
            {
                // This state still describes the old committed tuple. The new
                // request remains distinct and has no fabricated failure/front.
                target.status.state = Viewport3DLifecycleState::Committed;
            }
        }
        let _pending_publication = match self.context.reserve_pending_publication() {
            Ok(lease) => lease,
            Err(error) => {
                if let Some(target) = self.targets.get_mut(&surface_id) {
                    #[cfg(target_arch = "wasm32")]
                    drop(target.browser_material_preparation.take());
                    target.record_failure(request_serial, error.into());
                }
                #[cfg(target_arch = "wasm32")]
                self.wake_browser_material_successor(surface_id, key);
                return self.preparation_outcome_for(surface_id, publication_changed);
            }
        };
        #[cfg(target_arch = "wasm32")]
        if matches!(self.requested.source, RequestedSource3D::Material(_)) {
            let target = self
                .targets
                .get_mut(&surface_id)
                .expect("requested material target");
            if target.browser_material_preparation.is_none() {
                let attempt_metadata = match self.context.reserve_dynamic_storage(
                    0,
                    0,
                    0,
                    std::mem::size_of::<BrowserMaterialAttempt3D>() as u64,
                ) {
                    Ok(charge) => charge,
                    Err(error) => {
                        target.record_hdr_failure(request_serial, error.into());
                        return self.preparation_outcome_for(surface_id, publication_changed);
                    }
                };
                let work = match super::gpu_preparation::GpuPreparation3D::new(
                    &self.context,
                    _pending_request.take().expect("new request reservation"),
                ) {
                    Ok(work) => work,
                    Err(error) => {
                        target.record_hdr_failure(
                            request_serial,
                            super::gpu_preparation::hdr_refusal(error),
                        );
                        return self.preparation_outcome_for(surface_id, publication_changed);
                    }
                };
                target.browser_material_preparation = Some(BrowserMaterialPreparation3D {
                    work,
                    attempt: new_browser_attempt.expect("new immutable material attempt"),
                    _attempt_metadata: attempt_metadata,
                    observed_revision: 0,
                    origin: target.render_origin,
                    prepared: None,
                });
            }
            let pending = target.browser_material_preparation.as_mut().unwrap();
            pending.observed_revision = pending.work.revision();
        }
        if let Some(source) = render_requested.source.hdr_source().cloned() {
            #[cfg(any(test, feature = "test-harness"))]
            let render_start = self.pbr_timing.as_ref().map(|_| std::time::Instant::now());
            #[cfg(any(test, feature = "test-harness"))]
            let coarse_render_start = self
                .pbr_coarse_timing
                .as_ref()
                .map(|_| std::time::Instant::now());
            let result = if let Some(error) = capacity_progress_error {
                self.targets
                    .get_mut(&surface_id)
                    .expect("requested target")
                    .material_failure = Some(error.clone());
                Err((match error {
                    super::pbr::PbrError3D::Context(error) => HdrError3D::Context(error),
                    error => HdrError3D::Shader(error.to_string()),
                })
                .into())
            } else {
                self.render_private_hdr_target(
                    surface_id,
                    physical_render_extent,
                    &source,
                    &render_requested,
                    render_intent,
                )
            };
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (self.pbr_timing.as_mut(), render_start) {
                t.render_total += start.elapsed();
            }
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (self.pbr_coarse_timing.as_mut(), coarse_render_start) {
                t.render_total += start.elapsed();
            }
            match result {
                Ok(()) => {
                    #[cfg(any(test, feature = "test-harness"))]
                    let publication_start =
                        self.pbr_timing.as_ref().map(|_| std::time::Instant::now());
                    #[cfg(any(test, feature = "test-harness"))]
                    let coarse_publication_start = self
                        .pbr_coarse_timing
                        .as_ref()
                        .map(|_| std::time::Instant::now());
                    let target = self
                        .targets
                        .get_mut(&surface_id)
                        .expect("prepared HDR target");
                    target.status.committed = Some(committed_tuple);
                    target.status.failure = None;
                    target.hdr_failure = None;
                    target.status.state = Viewport3DLifecycleState::Committed;
                    #[cfg(target_arch = "wasm32")]
                    drop(target.browser_material_preparation.take());
                    if let Some(reveal) = self.object_reveal.as_mut() {
                        reveal.target_ready = true;
                    }
                    if matches!(
                        target.status.committed,
                        Some(RetainedSourceTuple3D::Material(_))
                    ) {
                        if let Some(mut old) = target.resources.take() {
                            old.retire();
                        }
                    }
                    #[cfg(test)]
                    audit::record_publication();
                    #[cfg(target_arch = "wasm32")]
                    self.wake_browser_material_successor(surface_id, key);
                    let outcome = self.preparation_outcome_for(surface_id, true);
                    #[cfg(any(test, feature = "test-harness"))]
                    if let (Some(t), Some(start), Some(active)) = (
                        self.pbr_coarse_timing.as_mut(),
                        coarse_publication_start,
                        coarse_active_start,
                    ) {
                        t.publication = start.elapsed();
                        t.active_total = active.elapsed();
                        if t.capacity_reused {
                            let tuple = self
                                .targets
                                .get(&surface_id)
                                .and_then(|target| target.status.committed_material())
                                .expect("committed material receipt");
                            t.target_key = Some(tuple.target_key());
                            t.view_revision = Some(tuple.view_revision());
                            t.request_serial = Some(tuple.request_serial());
                            t.source_identity =
                                Some((tuple.source.hdr.identity, tuple.source.hdr.revision));
                            t.structural_revision = Some(tuple.structural_revision());
                            t.model_revision = Some(tuple.model_revision());
                            t.direct_identity = tuple
                                .source
                                .direct
                                .as_ref()
                                .map(|d| (d.identity, d.revision));
                            t.complete = true;
                        }
                    }
                    #[cfg(any(test, feature = "test-harness"))]
                    if let (Some(t), Some(start), Some(active)) =
                        (self.pbr_timing.as_mut(), publication_start, active_start)
                    {
                        t.publication += start.elapsed();
                        t.active_total = active.elapsed();
                        t.complete = true;
                        if let RequestedSource3D::Material(source) = &render_requested.source {
                            let r = t.raster;
                            let capacity = self
                                .targets
                                .get(&surface_id)
                                .and_then(|target| target.material_capacity.as_ref())
                                .map(|pool| pool.diagnostic());
                            let limits = self.context.limits().policy;
                            let _ = std::io::Write::write_fmt(
                                &mut std::io::stderr(),
                                format_args!(
                                    "PBR_CAPACITY_REQUEST samples={} hdr_identity={} hdr_revision={} source_direct={:?} view_revision={} staged={} reused={} creates={:?} slots={} ready={} shadow_faces={} direct_ns={} validation_ns={} main_uv_ns={} hdr_ns={} active_ns={} capacity_slots_owned_free_inflight_awaiting_ready_buffer_bytes_shadow_bytes_checkouts_observed_maps_ready_target_reserved_bytes={capacity:?} target_usage={:?} resident_texture_bytes={:?} slot_limit={} dynamic_byte_limit={} attachment_limit={} texture_limit={} view={:?}\n",
                                    source.samples,
                                    source.hdr.identity,
                                    source.hdr.revision,
                                    source.direct.as_ref().map(|d| (d.identity, d.revision)),
                                    render_requested.view_revision.value(),
                                    self.targets
                                        .get(&surface_id)
                                        .and_then(|target| target.material.as_ref())
                                        .and_then(|p| p.encoder_completion())
                                        .is_some_and(|c| c.submitted()),
                                    r.capacity_reused,
                                    r.gpu_creations,
                                    r.capacity_slots,
                                    r.capacity_ready_revision,
                                    r.shadow_faces,
                                    r.direct_projection.as_nanos(),
                                    r.direct_source_validation.as_nanos(),
                                    r.uv_main_domain.as_nanos(),
                                    t.hdr_total.as_nanos(),
                                    t.active_total.as_nanos(),
                                    self.context.target_budget_usage(),
                                    self.context
                                        .resource_usage()
                                        .ok()
                                        .map(|u| u.resident_texture_bytes),
                                    limits.max_dynamic_storage_slots,
                                    limits.max_total_dynamic_storage_bytes,
                                    limits.max_total_attachment_bytes,
                                    limits.max_resident_texture_bytes,
                                    render_requested.view
                                ),
                            );
                        }
                    }
                    return outcome;
                }
                Err(error) => {
                    let Some(error) = super::gpu_preparation::hdr_failure(error) else {
                        return self.preparation_outcome_for(surface_id, publication_changed);
                    };
                    let target = self
                        .targets
                        .get_mut(&surface_id)
                        .expect("requested HDR target");
                    #[cfg(target_arch = "wasm32")]
                    drop(target.browser_material_preparation.take());
                    target.record_hdr_failure(request_serial, error);
                    if target.material_failure.is_some() {
                        // Observe capacity after candidate rollback so its own
                        // temporary release cannot create a per-frame retry loop.
                        target.material_capacity_attempt =
                            self.context.resource_usage().ok().map(|u| {
                                (
                                    u.admitted_shader_modules,
                                    u.resident_pipeline_variants,
                                    u.resident_geometry_bytes,
                                    u.resident_texture_bytes,
                                )
                            });
                        target.last_dynamic_budget_attempt_revision =
                            self.context.dynamic_storage_release_revision();
                    }
                    #[cfg(target_arch = "wasm32")]
                    self.wake_browser_material_successor(surface_id, key);
                    return self.preparation_outcome_for(surface_id, publication_changed);
                }
            }
        }
        {
            let Some(target) = self.targets.get_mut(&surface_id) else {
                return CustomRenderPreparationOutcome::default();
            };
            if target.resources.is_none() {
                let raster_target = match validate_viewport_raster_target(
                    &self.context,
                    physical_render_extent.width,
                    physical_render_extent.height,
                    self.requested.sample_count.value(),
                ) {
                    Ok(target) => target,
                    Err(error) => {
                        target.record_failure(request_serial, error.into());
                        return self.preparation_outcome_for(surface_id, publication_changed);
                    }
                };
                match Viewport3DTargetResources::new(&self.context, raster_target) {
                    Ok(resources) => target.resources = Some(resources),
                    Err(error) => {
                        target.record_failure(request_serial, error);
                        return self.preparation_outcome_for(surface_id, publication_changed);
                    }
                }
            }
        }

        let Some(render_origin) = self
            .targets
            .get(&surface_id)
            .and_then(|target| target.resources.as_ref().map(|_| target.render_origin))
        else {
            if let Some(target) = self.targets.get_mut(&surface_id) {
                target.record_failure(request_serial, Viewport3DError::SnapshotInvariant);
            }
            return self.preparation_outcome_for(surface_id, publication_changed);
        };
        let camera_origin = self.requested.view.pose().translation();
        let mut frame = match self
            .targets
            .get(&surface_id)
            .and_then(|target| target.resources.as_ref())
            .and_then(|resources| resources.pass.as_ref())
            .ok_or(CustomRenderPassError::Destroyed)
            .and_then(|pass| pass.begin_frame(Some("Halcyon 3D Viewport Raster Encoder")))
        {
            Ok(frame) => frame,
            Err(error) => {
                if let Some(target) = self.targets.get_mut(&surface_id) {
                    target.record_failure(request_serial, error.into());
                }
                return self.preparation_outcome_for(surface_id, publication_changed);
            }
        };
        let first_preparation = self
            .targets
            .get_mut(&surface_id)
            .and_then(|target| target.resources.as_mut())
            .ok_or(HeadlessRaster3DError::SnapshotInvariant)
            .and_then(|resources| {
                resources.prepare_or_update(&self.context, &self.requested, render_origin)
            });
        let (mut prepared, committed_origin) = match first_preparation {
            Ok(prepared) => (prepared, render_origin),
            Err(HeadlessRaster3DError::Math(
                Math3DError::NonRepresentableRenderRelativeTranslation
                | Math3DError::NonRepresentableViewTransform,
            )) if render_origin != camera_origin => {
                let retry = self
                    .targets
                    .get_mut(&surface_id)
                    .and_then(|target| target.resources.as_mut())
                    .ok_or(HeadlessRaster3DError::SnapshotInvariant)
                    .and_then(|resources| {
                        resources.prepare_or_update(&self.context, &self.requested, camera_origin)
                    });
                match retry {
                    Ok(prepared) => (prepared, camera_origin),
                    Err(error) => {
                        if let Some(target) = self.targets.get_mut(&surface_id) {
                            target.record_failure(request_serial, error.into());
                        }
                        return self.preparation_outcome_for(surface_id, publication_changed);
                    }
                }
            }
            Err(error) => {
                if let Some(target) = self.targets.get_mut(&surface_id) {
                    target.record_failure(request_serial, error.into());
                }
                return self.preparation_outcome_for(surface_id, publication_changed);
            }
        };

        {
            let Some(target) = self.targets.get_mut(&surface_id) else {
                return CustomRenderPreparationOutcome::default();
            };
            let Some(resources) = target.resources.as_mut() else {
                target.record_failure(request_serial, Viewport3DError::SnapshotInvariant);
                return self.preparation_outcome_for(surface_id, publication_changed);
            };
            let prepared_raster = match prepared.as_ref().or(resources.prepared.as_ref()) {
                Some(prepared) => prepared,
                None => {
                    target.record_failure(request_serial, Viewport3DError::SnapshotInvariant);
                    return self.preparation_outcome_for(surface_id, publication_changed);
                }
            };
            let Some(pass) = resources.pass.as_mut() else {
                target.record_failure(
                    request_serial,
                    Viewport3DError::CustomRender(CustomRenderPassError::Destroyed),
                );
                return self.preparation_outcome_for(surface_id, publication_changed);
            };
            {
                let (encoder, back_view) = frame.encoder_and_view();
                prepared_raster.encode(
                    encoder,
                    back_view,
                    &resources.depth_view,
                    resources.multisample_view.as_ref(),
                );
            }
            #[cfg(test)]
            audit::record_encode();
            #[cfg(feature = "render-audit")]
            cadence_audit::record_encode();
            pass.submit_and_swap(frame)
                .expect("a retained pass that began this frame must submit and swap");
            #[cfg(feature = "render-audit")]
            cadence_audit::record_submission_and_swap();
        }

        let target = self
            .targets
            .get_mut(&surface_id)
            .expect("submitted viewport target must remain retained through local commit");
        if let Some(prepared) = prepared.take() {
            let retired = target
                .resources
                .as_mut()
                .expect("submitted viewport resources must remain retained through local commit")
                .prepared
                .replace(prepared);
            if let Some(retired) = retired {
                retired.retire_after_submitted_work();
            }
        }
        target.render_origin = committed_origin;
        target.status.committed = Some(committed_tuple);
        target.status.failure = None;
        target.status.state = Viewport3DLifecycleState::Committed;
        #[cfg(test)]
        audit::record_publication();
        #[cfg(feature = "render-audit")]
        cadence_audit::record_publication();
        self.preparation_outcome_for(surface_id, true)
    }

    fn render_private_hdr_target(
        &mut self,
        surface: SurfaceId,
        extent: PhysicalExtent,
        source: &HdrSource3D,
        requested: &RequestedSemanticState,
        intent: PbrSceneRequestIntent3D,
    ) -> super::gpu_preparation::HdrGpuResult3D<()> {
        source.exposure()?;
        // The internally admitted material writer is real production code. The
        // standalone radiance fixture remains test-only and is never a fallback.
        if let RequestedSource3D::Material(material) = requested.source.clone() {
            return self
                .render_private_material_target(surface, extent, material, requested, intent);
        }
        #[cfg(not(test))]
        {
            let _ = (surface, extent);
            Err(HdrError3D::Source("standalone HDR fixture has no production writer").into())
        }
        #[cfg(test)]
        {
            let writer = self
                .hdr_writer
                .clone()
                .ok_or(HdrError3D::Source("missing S2 scene fixture"))?;
            if writer.source() != source {
                return Err(HdrError3D::Source("scene/certificate provenance mismatch").into());
            }
            if writer.samples() != requested.sample_count.value()
                || !self.context.matches_gpu_resources(writer.gpu_resources())
            {
                return Err(
                    HdrError3D::Source("scene producer sample/Device/Queue mismatch").into(),
                );
            }
            let target = self
                .targets
                .get_mut(&surface)
                .expect("requested HDR target");
            if target.hdr.is_none() {
                target.hdr = Some(HdrOutput3D::new(
                    &self.context,
                    extent.width,
                    extent.height,
                    requested.sample_count.value(),
                    source,
                )?);
            }
            target
                .hdr
                .as_mut()
                .expect("qualified HDR target")
                .render(source, |encoder, scene, depth| {
                    writer.encode(encoder, scene, depth)
                })?;
            audit::record_encode();
            Ok(())
        }
    }

    fn render_private_material_target(
        &mut self,
        surface: SurfaceId,
        extent: PhysicalExtent,
        source: super::pbr::ValidatedMaterialSceneSource3D,
        requested: &RequestedSemanticState,
        intent: PbrSceneRequestIntent3D,
    ) -> super::gpu_preparation::HdrGpuResult3D<()> {
        use super::pbr::PbrError3D;
        #[cfg(not(target_arch = "wasm32"))]
        let context = &self.context;
        #[cfg(target_arch = "wasm32")]
        let context = {
            let mut context = self.context.as_ref().clone();
            context.gpu_preparation = self
                .targets
                .get(&surface)
                .and_then(|target| target.browser_material_preparation.as_ref())
                .map(|pending| pending.work.clone());
            context
        };
        let wants_spatial_hud = self.spatial_hud_typography.is_some()
            && self.presence_mounted
            && self.presence.is_none()
            && self.outline_request.is_none()
            && self
                .spatial_hud_request
                .as_ref()
                .is_some_and(|hud| hud.matches_source(source.source()));
        let outline_request = self
            .outline_request
            .as_ref()
            .filter(|r| {
                self.presence_mounted
                    && self.presence.is_none()
                    && r.selection.matches_source(source.source())
            })
            .cloned();
        let outline_key = outline_request
            .as_ref()
            .map(|_| self.key_for(surface, extent));
        let slot_target = (source.direct.is_some() && !self.linked_writer_active())
            .then(|| self.key_for(surface, extent));
        #[cfg(any(test, feature = "test-harness"))]
        let mut timing = self.pbr_timing.as_mut();
        #[cfg(any(test, feature = "test-harness"))]
        let mut coarse = self.pbr_coarse_timing.as_mut();
        #[cfg(any(test, feature = "test-harness"))]
        debug_assert!(timing.is_none() || coarse.is_none());
        let target = self
            .targets
            .get_mut(&surface)
            .expect("requested material target");
        target.spatial_hud = None;
        let mut previous_outline = target.outline.take();
        if let Some(previous) = previous_outline.as_mut() {
            previous.pin = None;
        }
        if target.hdr.as_ref().is_some_and(|hdr| !hdr.back_writable()) {
            return Err((HdrError3D::PinnedFront).into());
        }
        let mut presence_refusal = None;
        let result = (|| -> super::gpu_preparation::PbrGpuResult3D<()> {
            let mut presence_active = None;
            if let Some(private) = target.presence.as_ref().and_then(|p| p.gpu.as_ref()) {
                let p = u64::from(extent.width) * u64::from(extent.height);
                let bytes = p
                    .checked_mul(20 * u64::from(source.samples) + 16)
                    .and_then(|n| n.checked_add(private.attachment_bytes))
                    .and_then(|n| {
                        n.checked_add(if source.direct.is_some() {
                            super::direct_shadow::POOL_BYTES
                        } else {
                            0
                        })
                    })
                    .ok_or(PbrError3D::Association(
                        "combined HAL381 complete target byte overflow",
                    ))?;
                let maximum = context.limits().policy.max_single_target_attachment_bytes;
                presence_active = Some(private.active_credit());
                if bytes > maximum {
                    // Refuse the optional transition before preparing the original PBR
                    // target. Reuse the same active-pixel credit; the retiring preview's
                    // attachments remain charged through the renderer's retained reads.
                    presence_refusal = Some(Context3DError::PolicyBudgetExceeded {
                        budget: PolicyBudget3D::SingleTargetAttachmentBytes,
                        requested: bytes,
                        maximum,
                    });
                    target.presence = None;
                }
            }
            #[cfg(any(test, feature = "test-harness"))]
            let validation_start = timing.as_ref().map(|_| std::time::Instant::now());
            #[cfg(any(test, feature = "test-harness"))]
            let coarse_validation_start = coarse.as_ref().map(|_| std::time::Instant::now());
            let validation = super::pbr::validate_material_scene_source(
                &context,
                source.source(),
                Some(&source),
            );
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (timing.as_deref_mut(), validation_start) {
                t.prepare_validation += start.elapsed();
            }
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (coarse.as_deref_mut(), coarse_validation_start) {
                t.prepare_validation += start.elapsed();
            }
            validation?;
            if let Some(error) = self.object_reveal.as_ref().and_then(|r| r.failure.as_ref()) {
                return Err((error.clone()).into());
            }
            let reveal_frame = self
                .object_reveal
                .as_ref()
                .filter(|r| r.request.enabled)
                .and_then(|r| {
                    r.request.extent.as_ref().map(|extent| ObjectRevealFrame3D {
                        extent: extent.clone(),
                        progress: r.progress,
                    })
                });
            if source.samples != requested.sample_count.value() {
                return Err((PbrError3D::Association("source/target sample mismatch")).into());
            }
            let committed_predecessor = slot_target
                .filter(|_| matches!(intent, PbrSceneRequestIntent3D::Successor))
                .and_then(|key| {
                    target.status.committed_material().filter(|old| {
                        old.target_key() == key && old.view_revision() == requested.view_revision
                    })
                })
                .map(MaterialViewport3DTuple::source);
            #[cfg(not(target_arch = "wasm32"))]
            let mut origin = target.render_origin;
            #[cfg(target_arch = "wasm32")]
            let mut origin = target
                .browser_material_preparation
                .as_ref()
                .expect("current browser preparation")
                .origin;
            #[cfg_attr(not(any(test, feature = "test-harness")), allow(unused_mut))]
            let mut prepare = |origin| {
                #[cfg(any(test, feature = "test-harness"))]
                let raster_start = timing.as_ref().map(|_| std::time::Instant::now());
                #[cfg(any(test, feature = "test-harness"))]
                let coarse_raster_start = coarse.as_ref().map(|_| std::time::Instant::now());
                let result = super::raster::prepare_material_scene_with_capacity(
                    &context,
                    source.source().clone(),
                    requested.view,
                    origin,
                    (extent.width, extent.height),
                    &mut target.material,
                    &mut target.material_capacity,
                    &mut target.material_capacity_budget,
                    Some(&source),
                    reveal_frame.as_ref(),
                    committed_predecessor,
                    #[cfg(any(test, feature = "test-harness"))]
                    timing.as_deref_mut().map(|t| &mut t.raster),
                    #[cfg(any(test, feature = "test-harness"))]
                    coarse.as_deref_mut(),
                );
                #[cfg(any(test, feature = "test-harness"))]
                if let (Some(t), Some(start)) = (timing.as_deref_mut(), raster_start) {
                    t.raster.total += start.elapsed();
                }
                #[cfg(any(test, feature = "test-harness"))]
                if let (Some(t), Some(start)) = (coarse.as_deref_mut(), coarse_raster_start) {
                    t.raster_total += start.elapsed();
                    t.raster_unattributed = t
                        .packet_vertex_projection
                        .checked_add(t.uv_main)
                        .and_then(|v| v.checked_add(t.direct_projection))
                        .and_then(|v| v.checked_add(t.uv_shadow))
                        .and_then(|measured| t.raster_total.checked_sub(measured));
                }
                result
            };
            #[cfg(target_arch = "wasm32")]
            let saved = target
                .browser_material_preparation
                .as_mut()
                .and_then(|pending| pending.prepared.take());
            #[cfg(not(target_arch = "wasm32"))]
            let saved = None;
            let mut prepared = if let Some(prepared) = saved {
                prepared
            } else {
                let first = prepare(origin);
                let retry = first
                    .as_ref()
                    .err()
                    .and_then(super::gpu_preparation::pbr_failure_ref)
                    .is_some_and(|e| {
                        matches!(
                            e,
                            PbrError3D::Math(
                                super::Math3DError::NonRepresentableRenderRelativeTranslation
                                    | super::Math3DError::NonRepresentableViewTransform,
                            )
                        )
                    });
                if retry && origin != requested.view.pose().translation() {
                    origin = requested.view.pose().translation();
                    #[cfg(target_arch = "wasm32")]
                    {
                        target
                            .browser_material_preparation
                            .as_mut()
                            .expect("current browser preparation")
                            .origin = origin;
                    }
                    prepare(origin)?
                } else {
                    first?
                }
            };
            // Source/geometry/material validation and all of their reservations
            // precede writes. The existing HDR owner performs its own complete
            // attachment/tone reservation before invoking this scene writer.
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(t) = timing.as_deref_mut() {
                t.hdr_reused = target.hdr.is_some();
            }
            if target.hdr.is_none() {
                #[cfg(any(test, feature = "test-harness"))]
                let allocation_start = timing.as_ref().map(|_| std::time::Instant::now());
                let hdr = HdrOutput3D::new_with_presence(
                    &context,
                    extent.width,
                    extent.height,
                    source.samples,
                    &source.hdr,
                    presence_active,
                    target
                        .presence
                        .as_ref()
                        .and_then(|p| p.gpu.as_ref())
                        .map_or(0, |gpu| gpu.attachment_bytes),
                );
                #[cfg(target_arch = "wasm32")]
                if matches!(
                    &hdr,
                    Err(super::gpu_preparation::PreparationError3D::Pending)
                ) {
                    target
                        .browser_material_preparation
                        .as_mut()
                        .expect("current browser preparation")
                        .prepared = Some(prepared);
                    return Err(super::gpu_preparation::PreparationError3D::Pending);
                }
                target.hdr = Some(hdr.map_err(super::gpu_preparation::hdr_to_pbr)?);
                #[cfg(any(test, feature = "test-harness"))]
                if let (Some(t), Some(start)) = (timing.as_deref_mut(), allocation_start) {
                    t.hdr_allocation += start.elapsed();
                }
            }
            // HAL374 is subordinate to lawful original PBR. Reserve canonical
            // HDR first, so private aggregate denial cannot consume its quota.
            let mut outline_gpu = None;
            if let Some(request) = outline_request.as_ref() {
                match prepared.outline_target(
                    target.material.as_ref(),
                    &context,
                    &request.selection,
                    (extent.width, extent.height),
                    previous_outline.as_ref().map(|p| &p.gpu),
                ) {
                    Ok(gpu) => {
                        prepared.force_scene_write();
                        outline_gpu = Some(gpu);
                        target.outline_failure = None;
                    }
                    Err(error) => target.outline_failure = Some(error),
                }
            }
            #[cfg(any(test, feature = "test-harness"))]
            if timing.is_some() {
                target
                    .hdr
                    .as_mut()
                    .expect("qualified HDR owner")
                    .time_next_render_for_test();
            }
            #[cfg(any(test, feature = "test-harness"))]
            if coarse.is_some() {
                // An unsuccessful detailed render may leave its record armed.
                target
                    .hdr
                    .as_mut()
                    .expect("qualified HDR owner")
                    .take_render_timing_for_test();
            }
            let hdr = target.hdr.as_mut().expect("qualified HDR owner");
            if wants_spatial_hud {
                let depth_replaced = hdr.hud_depth().is_none();
                let depth = (|| {
                    if self
                        .spatial_hud_program
                        .as_ref()
                        .is_none_or(|p| p.samples != source.samples)
                    {
                        self.spatial_hud_program = Some(Arc::new(
                            super::spatial_hud::HudProgram3D::new(&context, source.samples)?,
                        ));
                    }
                    hdr.qualify_hud_depth(&context, source.direct.is_some())
                })();
                match depth {
                    Ok(()) => {
                        target.spatial_hud_failure = None;
                        if depth_replaced {
                            prepared.force_scene_write();
                        }
                    }
                    Err(error) => target.spatial_hud_failure = Some(error),
                }
            }
            if slot_target.is_some() && reveal_frame.is_none() {
                prepared.seed_direct_slots(
                    &context,
                    #[cfg(any(test, feature = "test-harness"))]
                    timing.as_deref_mut().map(|t| &mut t.raster),
                );
            }
            #[cfg(any(test, feature = "test-harness"))]
            let hdr_start = timing.as_ref().map(|_| std::time::Instant::now());
            #[cfg(any(test, feature = "test-harness"))]
            let coarse_hdr_start = coarse.as_ref().map(|_| std::time::Instant::now());
            let rendered = if let Some(completion) = prepared.encoder_completion() {
                hdr.render_encoder_bound(
                    &source.hdr,
                    completion,
                    |encoder, scene, depth, tone, bytes| {
                        prepared.encode_encoder_bound(encoder, scene, depth, tone, bytes);
                        if let Some(outline) = &outline_gpu {
                            outline.encode(encoder);
                        }
                    },
                )
                .map(|_| ())
                .map_err(|(error, _)| error)
            } else {
                hdr.render(&source.hdr, |encoder, scene, depth| {
                    prepared.encode(target.material.as_ref(), encoder, scene, depth);
                    if let Some(outline) = &outline_gpu {
                        outline.encode(encoder);
                    }
                })
            };
            rendered.map_err(super::gpu_preparation::hdr_to_pbr)?;
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (timing.as_deref_mut(), hdr_start) {
                t.hdr_total += start.elapsed();
                t.hdr = target
                    .hdr
                    .as_mut()
                    .expect("qualified HDR owner")
                    .take_render_timing_for_test()
                    .expect("enabled HDR timing");
            }
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (coarse.as_deref_mut(), coarse_hdr_start) {
                t.hdr_total += start.elapsed();
            }
            #[cfg(any(test, feature = "test-harness"))]
            let commit_start = timing.as_ref().map(|_| std::time::Instant::now());
            #[cfg(any(test, feature = "test-harness"))]
            let coarse_commit_start = coarse.as_ref().map(|_| std::time::Instant::now());
            prepared.commit(&mut target.material);
            target.render_origin = origin;
            if let (Some(gpu), Some(request), Some(key), Some(tuple)) = (
                outline_gpu,
                outline_request.as_ref(),
                outline_key,
                target.status.requested_material(),
            ) {
                target.outline = Some(OutlineTargetState {
                    gpu,
                    epoch: request.epoch,
                    mount: self.presence_mount,
                    key,
                    view_revision: requested.view_revision,
                    view: requested.view,
                    request: tuple.request_serial,
                    pin: None,
                    notified: false,
                });
            }
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (timing.as_deref_mut(), commit_start) {
                t.material_commit += start.elapsed();
            }
            #[cfg(any(test, feature = "test-harness"))]
            if let (Some(t), Some(start)) = (coarse.as_deref_mut(), coarse_commit_start) {
                t.material_commit += start.elapsed();
            }
            Ok(())
        })();
        let outcome = match result {
            Ok(()) => {
                target.material_failure = None;
                #[cfg(test)]
                audit::record_encode();
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            Err(super::gpu_preparation::PreparationError3D::Pending) => {
                target.material_failure = None;
                target.hdr_failure = None;
                Err(super::gpu_preparation::PreparationError3D::Pending)
            }
            Err(e) => {
                #[cfg(target_arch = "wasm32")]
                let e = match e {
                    super::gpu_preparation::PreparationError3D::Failed(e) => e,
                    super::gpu_preparation::PreparationError3D::Pending => {
                        return Err(super::gpu_preparation::PreparationError3D::Pending);
                    }
                };
                if target.hdr.as_ref().is_some_and(|hdr| !hdr.is_live()) {
                    drop(target.material_capacity.take());
                }
                target.material_failure = Some(e.clone());
                Err(super::gpu_preparation::hdr_refusal(e).into())
            }
        };
        if let Some(error) = presence_refusal {
            if let Some(occurrence) = self.presence.as_mut() {
                occurrence.failure = Some(error.into());
                if let Some(handle) = occurrence.handle.take() {
                    self.retired_presence_handle = Some(handle);
                }
            }
            for target in self.targets.values_mut() {
                target.presence = None;
            }
        }
        outcome
    }

    /// Internal complete-source admission only. Public material admission awaits
    /// S3; linked G1 and domain-owned viewports have no extended writer grant.
    pub(crate) fn set_material_source(
        &mut self,
        source: Arc<super::pbr::MaterialSceneSource3D>,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
    ) -> Result<bool, super::pbr::PbrError3D> {
        self.set_material_source_with_intent(
            source,
            view_revision,
            view,
            PbrSceneRequestIntent3D::Successor,
        )
    }

    fn set_material_source_with_intent(
        &mut self,
        source: Arc<super::pbr::MaterialSceneSource3D>,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        intent: PbrSceneRequestIntent3D,
    ) -> Result<bool, super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(t) = self.pbr_coarse_timing.as_mut() {
            *t = PbrCoarseTiming3D::default();
        }
        #[cfg(any(test, feature = "test-harness"))]
        let request_start = self.pbr_timing.as_ref().map(|_| std::time::Instant::now());
        #[cfg(any(test, feature = "test-harness"))]
        let coarse_request_start = self
            .pbr_coarse_timing
            .as_ref()
            .map(|_| std::time::Instant::now());
        if self.linked_writer_active()
            || !matches!(
                &self.requested.source,
                RequestedSource3D::Legacy {
                    kind: Viewport3DRequestKind::Flat,
                    ..
                } | RequestedSource3D::Material(_)
                    | RequestedSource3D::PresencePending
            )
        {
            return Err(PbrError3D::Association(
                "private material source requires an unlinked ordinary flat viewport",
            ));
        }
        if matches!(intent, PbrSceneRequestIntent3D::IndependentReplacement) {
            let RequestedSource3D::Material(current) = &self.requested.source else {
                return Err(PbrError3D::Association(
                    "independent PBR replacement requires a current PBR source",
                ));
            };
            if Arc::ptr_eq(current.source(), &source) || current.hdr.identity == source.hdr.identity
            {
                return Err(PbrError3D::Association(
                    "independent PBR replacement requires a different source Arc and HDR identity",
                ));
            }
        }
        #[cfg(any(test, feature = "test-harness"))]
        let validation_start = self.pbr_timing.as_ref().map(|_| std::time::Instant::now());
        #[cfg(any(test, feature = "test-harness"))]
        let coarse_validation_start = self
            .pbr_coarse_timing
            .as_ref()
            .map(|_| std::time::Instant::now());
        let validation = match &self.requested.source {
            RequestedSource3D::Material(current) if Arc::ptr_eq(current.source(), &source) => {
                super::pbr::validate_material_scene_source(&self.context, &source, Some(current))
                    .map(|()| current.clone())
            }
            _ => super::pbr::ValidatedMaterialSceneSource3D::new(&self.context, source.clone()),
        };
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (self.pbr_timing.as_mut(), validation_start) {
            t.request_validation += start.elapsed();
        }
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (self.pbr_coarse_timing.as_mut(), coarse_validation_start) {
            t.request_validation += start.elapsed();
        }
        let captured = validation?;
        if matches!(intent, PbrSceneRequestIntent3D::IndependentReplacement) {
            if !captured.matches(&self.context, &source) {
                return Err(PbrError3D::Association(
                    "independent PBR replacement Context/Device is not live",
                ));
            }
            if source.model.placements.len() != source.snapshot.packets.len() {
                return Err(PbrError3D::Association(
                    "independent PBR replacement model/packet count mismatch",
                ));
            }
            source.snapshot.validate_live_resources(&self.context)?;
        }
        if view_revision < self.requested.view_revision {
            return Err(PbrError3D::Shader(
                Viewport3DError::StaleViewRevision {
                    current: self.requested.view_revision,
                    requested: view_revision,
                }
                .to_string(),
            ));
        }
        if view_revision == self.requested.view_revision && view != self.requested.view {
            return Err(PbrError3D::Shader(
                Viewport3DError::ReusedViewRevision {
                    revision: view_revision,
                }
                .to_string(),
            ));
        }
        if let (PbrSceneRequestIntent3D::Successor, RequestedSource3D::Material(current)) =
            (intent, &self.requested.source)
        {
            super::pbr::MaterialSceneSource3D::validate_successor(current, &source)?;
            if source.hdr.identity != current.hdr.identity
                || source.hdr.revision < current.hdr.revision
                || (source.hdr.revision == current.hdr.revision && !Arc::ptr_eq(current, &source))
            {
                return Err(PbrError3D::Association(
                    "stale/reused complete extension source revision",
                ));
            }
        }
        let samples = SampleCount3D::try_new(source.samples)
            .map_err(|_| PbrError3D::Association("unsupported sample count"))?;
        if view_revision == self.requested.view_revision
            && samples == self.requested.sample_count
            && matches!(&self.requested.source, RequestedSource3D::Material(current) if Arc::ptr_eq(current, &source))
        {
            return Ok(false);
        }
        if !matches!(&self.requested.source, RequestedSource3D::Material(old) if Arc::ptr_eq(old.source(), &source))
        {
            self.retire_object_reveal();
            self.retire_pbr_outline();
            self.retire_spatial_hud();
        }
        if self.presence.is_some() {
            self.retire_loading_presence();
        }
        self.requested = RequestedSemanticState {
            view_revision,
            view,
            sample_count: samples,
            source: RequestedSource3D::Material(captured),
        };
        self.pbr_request_intent = intent;
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (self.pbr_timing.as_mut(), request_start) {
            t.request_total = start.elapsed();
        }
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (self.pbr_coarse_timing.as_mut(), coarse_request_start) {
            t.request_total = start.elapsed();
        }
        Ok(true)
    }

    /// Enable/reset or disable owner-local PBR CPU timing for a test request.
    ///
    /// Disabled by default, including when `test-harness` is enabled. Enabling
    /// starts an empty receipt; disabling removes it. This changes diagnostics
    /// only and does not request, prepare or publish a frame.
    ///
    /// ```no_run
    /// use halcyon::three_d::{
    ///     HdrRenderTiming3D, PbrPreparationTiming3D, PbrViewTiming3D, Viewport3D,
    /// };
    ///
    /// fn begin(viewport: &mut Viewport3D) {
    ///     viewport.set_pbr_timing_for_test(true);
    /// }
    ///
    /// // Call after the separately submitted request reaches its matching commit.
    /// fn receipt(viewport: &Viewport3D) -> Option<PbrViewTiming3D> {
    ///     let timing = viewport.pbr_timing_for_test()?;
    ///     let _: PbrPreparationTiming3D = timing.raster;
    ///     let _: HdrRenderTiming3D = timing.hdr;
    ///     timing.complete.then_some(timing)
    /// }
    /// ```
    #[cfg(any(test, feature = "test-harness"))]
    pub fn set_pbr_timing_for_test(&mut self, enabled: bool) {
        self.pbr_timing = enabled.then(PbrViewTiming3D::default);
        if enabled {
            self.pbr_coarse_timing = None;
        }
    }

    /// Copy the current opt-in CPU receipt without consuming or mutating it.
    ///
    /// See [`PbrViewTiming3D`] for inclusive stages, unmeasured remainders and
    /// the required association with the current committed request.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn pbr_timing_for_test(&self) -> Option<PbrViewTiming3D> {
        self.pbr_timing
    }

    /// Arm/reset coarse timing without detailed UV clocks, replay or packet output.
    /// Enabling clears detailed timing; disabling affects only coarse timing.
    /// Requests and preparations reset the receipt, including unchanged/failing
    /// attempts. This diagnostic never requests work or implies GPU completion.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn set_pbr_coarse_timing_for_test(&mut self, enabled: bool) {
        self.pbr_coarse_timing = enabled.then(PbrCoarseTiming3D::default);
        if enabled {
            self.pbr_timing = None;
        }
    }

    /// Copy the coarse receipt; use it only when `complete` and its committed
    /// target, source, view revision and request serial match current status.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn pbr_coarse_timing_for_test(&self) -> Option<PbrCoarseTiming3D> {
        self.pbr_coarse_timing
    }

    #[cfg(test)]
    pub(crate) fn set_material_source_for_test(
        &mut self,
        source: Arc<super::pbr::MaterialSceneSource3D>,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
    ) -> Result<bool, super::pbr::PbrError3D> {
        self.set_material_source(source, view_revision, view)
    }

    #[cfg(test)]
    pub(crate) fn material_failure_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<&super::pbr::PbrError3D> {
        self.targets.get(&surface)?.material_failure.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn assert_direct_full_parity_for_test(&self, surface: SurfaceId) -> (usize, usize) {
        self.targets[&surface]
            .material
            .as_ref()
            .unwrap()
            .assert_direct_full_parity_for_test()
    }

    #[cfg(test)]
    pub(crate) fn direct_proof_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<super::raster::DirectProof3D> {
        self.targets
            .get(&surface)?
            .material
            .as_ref()?
            .direct_proof_for_test()
    }

    #[cfg(test)]
    pub(crate) fn capacity_objects_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<super::raster::MaterialCapacityObjects3D> {
        self.targets
            .get(&surface)?
            .material
            .as_ref()?
            .capacity_objects_for_test()
    }

    #[cfg(test)]
    pub(crate) fn capacity_counts_for_test(&self, surface: SurfaceId) -> (usize, usize, u64) {
        self.targets
            .get(&surface)
            .and_then(|t| t.material_capacity.as_ref())
            .map_or((0, 0, 0), |p| p.counts_for_test())
    }

    #[cfg(test)]
    pub(crate) fn material_observation_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<&super::raster::MaterialObservation3D> {
        self.targets.get(&surface)?.material.as_ref()?.observation()
    }

    #[cfg(test)]
    pub(crate) fn committed_material_source_for_test(
        &self,
        surface: SurfaceId,
    ) -> Option<&Arc<super::pbr::MaterialSceneSource3D>> {
        Some(
            self.targets
                .get(&surface)?
                .status
                .committed_material()?
                .source(),
        )
    }

    #[cfg(test)]
    pub(crate) fn set_hdr_fixture_for_test(
        &mut self,
        writer: Arc<dyn super::hdr::HdrProofSceneWriter3D>,
        samples: SampleCount3D,
    ) -> Result<(), HdrError3D> {
        if self.linked_writer_active() {
            return Err(HdrError3D::Source(
                "linked G1 writer excludes the private proof request",
            ));
        }
        let Some((_, _, kind)) = self.requested.source.legacy() else {
            return Err(HdrError3D::Source(
                "standalone fixture cannot replace a Material family",
            ));
        };
        if matches!(kind, Viewport3DRequestKind::DomainAware(_)) {
            return Err(HdrError3D::Source(
                "private proof requires the ordinary flat source",
            ));
        }
        let source = writer.source();
        source.exposure()?;
        if writer.samples() != samples.value()
            || !self.context.matches_gpu_resources(writer.gpu_resources())
        {
            return Err(HdrError3D::Source("scene producer sample/Device/Queue mismatch").into());
        }
        if let Viewport3DRequestKind::PrivateHdr(current) = kind {
            if source.revision < current.revision
                || (source.revision == current.revision
                    && (source != current
                        || self
                            .hdr_writer
                            .as_ref()
                            .is_some_and(|held| !Arc::ptr_eq(held, &writer))))
            {
                return Err(HdrError3D::Source("stale/reused extension source revision").into());
            }
        }
        let RequestedSource3D::Legacy { kind, .. } = &mut self.requested.source else {
            unreachable!()
        };
        *kind = Viewport3DRequestKind::PrivateHdr(source.clone());
        self.requested.sample_count = samples;
        self.hdr_writer = Some(writer);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn hdr_front_for_test(&self, surface: SurfaceId) -> &wgpu::Texture {
        self.targets[&surface]
            .hdr
            .as_ref()
            .unwrap()
            .front_for_test()
    }

    #[cfg(test)]
    pub(crate) fn hdr_swaps_for_test(&self, surface: SurfaceId) -> u64 {
        self.targets[&surface]
            .hdr
            .as_ref()
            .unwrap()
            .swaps_for_test()
    }

    #[cfg(test)]
    pub(crate) fn target_texture_id_for_test(&self, surface_id: SurfaceId) -> Option<u64> {
        self.targets
            .get(&surface_id)
            .and_then(Viewport3DTargetState::sampleable_texture_id)
    }

    #[cfg(test)]
    pub(crate) fn target_metrics_for_test(
        &self,
        surface_id: SurfaceId,
    ) -> Option<CustomRenderTargetMetrics> {
        self.targets
            .get(&surface_id)
            .and_then(|target| target.last_metrics)
    }

    #[cfg(test)]
    pub(crate) fn composition_coverage_for_test(
        &self,
        surface_id: SurfaceId,
    ) -> Option<PhysicalRect> {
        self.targets
            .get(&surface_id)
            .and_then(|target| target.composition_coverage)
    }

    #[cfg(test)]
    pub(crate) fn target_dynamic_storage_bytes_for_test(
        &self,
        surface_id: SurfaceId,
    ) -> Option<u64> {
        self.targets
            .get(&surface_id)?
            .resources
            .as_ref()?
            .prepared
            .as_ref()
            .map(PreparedRaster3D::dynamic_storage_bytes_for_test)
    }

    #[cfg(test)]
    pub(crate) fn target_dynamic_bytes_for_test(
        &self,
        surface_id: SurfaceId,
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        self.targets
            .get(&surface_id)?
            .resources
            .as_ref()?
            .prepared
            .as_ref()
            .map(PreparedRaster3D::dynamic_bytes_for_test)
    }
}

impl WidgetDowncast for Viewport3D {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl CustomRenderParticipant for Viewport3D {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome {
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(t) = self.pbr_coarse_timing.as_mut() {
            *t = PbrCoarseTiming3D {
                request_total: t.request_total,
                request_validation: t.request_validation,
                ..PbrCoarseTiming3D::default()
            };
        }
        #[cfg(any(test, feature = "test-harness"))]
        let coarse_start = self
            .pbr_coarse_timing
            .as_ref()
            .map(|_| std::time::Instant::now());
        let mut result = (|| {
            #[cfg(feature = "render-audit")]
            cadence_audit::record_custom_preparation();
            self.current_prepared_surface = Some(metrics.surface_id);
            if let Some(failure) = self.context.device_failure() {
                #[cfg(feature = "render-audit")]
                cadence_audit::record_failed_preparation();
                return self.prepare_terminal_device(metrics, failure);
            }
            if self.requested.source.hdr_source().is_some()
                && self
                    .context
                    .gpu_resources()
                    .is_ok_and(|gpu| !gpu.private_hdr_owner_live())
            {
                let mut withdrew = false;
                for target in self.targets.values_mut() {
                    withdrew |= target.status.committed.is_some();
                    target.retire(Viewport3DLifecycleState::Retired);
                }
                return CustomRenderPreparationOutcome::new(withdrew);
            }
            let mut presence_changed = false;
            let outcome = match metrics.activity {
                CustomRenderActivity::Active {
                    physical_render_extent,
                    physical_composition_coverage,
                } => {
                    let key = self.key_for(metrics.surface_id, physical_render_extent);
                    // A target that never showed this proxy has no spectral
                    // endpoint to transition from. Try the admitted full source first.
                    let full_first = self.presence.as_ref().is_some_and(|occurrence| {
                        self.presence_mounted
                            && occurrence.full.is_some()
                            && occurrence.failure.is_none()
                            && !self.targets.get(&metrics.surface_id).is_some_and(|target| {
                                target.presence.as_ref().is_some_and(|p| {
                                    p.epoch == occurrence.epoch
                                        && p.mount == self.presence_mount
                                        && p.key == key
                                        && (p.rendered || p.terminal.is_some())
                                })
                            })
                    });
                    let full_outcome = full_first.then(|| {
                        self.prepare_active_target(
                            metrics,
                            physical_render_extent,
                            physical_composition_coverage,
                        )
                    });
                    let matching_full = full_first
                        && self.targets.get(&metrics.surface_id).is_some_and(|target| {
                            target.status.state == Viewport3DLifecycleState::Committed
                                && target.sampleable_texture_id().is_some()
                                && target.status.committed_material().is_some_and(|tuple| {
                                    self.presence
                                        .as_ref()
                                        .and_then(|p| p.full.as_ref())
                                        .is_some_and(|full| Arc::ptr_eq(full, &tuple.source))
                                        && tuple.view_revision == self.requested.view_revision
                                        && tuple.view == self.requested.view
                                        && tuple.target_key == key
                                        && target.status.requested_material().is_some_and(|r| {
                                            r.request_serial == tuple.request_serial
                                        })
                                })
                        });
                    if matching_full {
                        if let Some(target) = self.targets.get_mut(&metrics.surface_id) {
                            target.presence = None;
                        }
                    } else {
                        presence_changed =
                            self.prepare_loading_presence(metrics, physical_render_extent);
                        if matches!(self.requested.source, RequestedSource3D::PresencePending) {
                            return self
                                .preparation_outcome_for(metrics.surface_id, presence_changed);
                        }
                    }
                    full_outcome.unwrap_or_else(|| {
                        self.prepare_active_target(
                            metrics,
                            physical_render_extent,
                            physical_composition_coverage,
                        )
                    })
                }
                CustomRenderActivity::Inactive => {
                    if let Some(target) = self.targets.get_mut(&metrics.surface_id) {
                        target.presence = None;
                    }
                    self.prepare_inactive_target(metrics)
                }
            };
            match self.context.device_failure() {
                Some(failure) => {
                    #[cfg(feature = "render-audit")]
                    if !self
                        .targets
                        .get(&metrics.surface_id)
                        .is_some_and(|target| target.status.failure.is_some())
                    {
                        cadence_audit::record_failed_preparation();
                    }
                    self.prepare_terminal_device(metrics, failure)
                }
                None => {
                    let full_failure = self.targets.get(&metrics.surface_id).and_then(|target| {
                        target
                            .material_failure
                            .as_ref()
                            .map(|e| e.to_string())
                            .or_else(|| {
                                target
                                    .hdr_failure
                                    .as_ref()
                                    .filter(|(_, e)| !matches!(e, HdrError3D::PinnedFront))
                                    .map(|(_, e)| e.to_string())
                            })
                    });
                    if let (Some(occurrence), Some(error)) = (
                        self.presence.as_mut().filter(|p| p.full.is_some()),
                        full_failure,
                    ) {
                        occurrence.failure = Some(super::LoadingPresenceError3D::Resource(error));
                        if let Some(target) = self.targets.get_mut(&metrics.surface_id) {
                            target.presence = None;
                        }
                        presence_changed = true;
                    }
                    if let Some(occurrence) = self.presence.as_mut().filter(|p| p.full.is_some()) {
                        // Canonical image completion is a lawful image-owner wake,
                        // not a timer or a consumer cross-thread callback.
                        if let Some(target) = self.targets.get_mut(&metrics.surface_id) {
                            if let (Some(p), Some(commit)) =
                                (target.presence.as_mut(), target.status.committed_material())
                            {
                                if occurrence
                                    .full
                                    .as_ref()
                                    .is_some_and(|full| Arc::ptr_eq(full, &commit.source))
                                    && p.notified_full != Some(commit.request_serial)
                                {
                                    p.notified_full = Some(commit.request_serial);
                                    occurrence.queued = true;
                                    if let Ok(gpu) = self.context.gpu_resources() {
                                        gpu.image_admission.notify();
                                    }
                                }
                            }
                        }
                    }
                    if let Some(outline) = self
                        .targets
                        .get_mut(&metrics.surface_id)
                        .and_then(|t| t.outline.as_mut())
                    {
                        if !outline.notified {
                            outline.notified = true;
                            self.outline_queued = true;
                            if let Ok(gpu) = self.context.gpu_resources() {
                                gpu.image_admission.notify();
                            }
                        }
                    }
                    if self.presence.is_some() || self.outline_request.is_some() {
                        self.preparation_outcome_for(
                            metrics.surface_id,
                            presence_changed || outcome.front_publication_changed,
                        )
                    } else {
                        outcome
                    }
                }
            }
        })();
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (self.pbr_coarse_timing.as_mut(), coarse_start) {
            t.preparation_total = start.elapsed();
            t.complete &= self.targets.get(&metrics.surface_id).is_some_and(|target| {
                target.status.state == Viewport3DLifecycleState::Committed
                    && target.status.committed_material().is_some_and(|tuple| {
                        Some(tuple.request_serial()) == t.request_serial
                            && Some(tuple.view_revision()) == t.view_revision
                            && match &self.requested.source {
                                RequestedSource3D::Material(source) => {
                                    Arc::ptr_eq(&tuple.source, source)
                                }
                                _ => false,
                            }
                    })
            });
        }
        self.prepare_spatial_hud_publication(metrics, &mut result);
        result
    }
}

impl Widget for Viewport3D {
    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        if std::mem::take(&mut self.spatial_hud_queued) {
            if self.spatial_hud_request.is_some() && self.spatial_hud_typography.is_none() {
                cx.invalidate_layout();
            } else {
                cx.invalidate_paint();
            }
        }
        self.update_pbr_outline(cx);
        self.update_loading_presence(cx);
        if let Some(handle) = self.retired_reveal_handle.take() {
            cx.cancel_animation(handle);
        }
        if self.context.device_failure().is_some() {
            if let Some(reveal) = self.object_reveal.take() {
                if let Some(handle) = reveal.handle {
                    cx.cancel_animation(handle);
                }
            }
            return;
        }
        let Some(reveal) = self.object_reveal.as_mut() else {
            return;
        };
        let previous = reveal.progress;
        let mut changed = std::mem::take(&mut reveal.queued);
        if let Some(handle) = reveal.handle {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::Active(p)) | Some(AnimationSample::Completed(p))
                    if p.is_finite() && (0.0..=1.0).contains(&p) =>
                {
                    reveal.progress = p;
                    if p == 1.0 {
                        reveal.handle = None;
                    }
                }
                _ => {
                    reveal.failure = Some(super::pbr::PbrError3D::Association(
                        "object reveal Timeline sample unavailable",
                    ));
                    reveal.handle = None;
                    reveal.pending = false;
                    changed = true;
                }
            }
        }
        if reveal.pending && (!reveal.request.enabled || reveal.target_ready) {
            if let Some(handle) = reveal.handle.take() {
                cx.cancel_animation(handle);
            }
            reveal.pending = false;
            if !reveal.request.enabled {
                reveal.progress = 1.0;
            } else if reveal.progress < 1.0 {
                match cx.try_start_animation(Animation::new(
                    reveal.progress,
                    1.0,
                    reveal.request.duration.as_secs_f32(),
                    Easing::Linear,
                )) {
                    Ok(Some(handle)) => {
                        reveal.handle = Some(handle);
                    }
                    _ => {
                        reveal.failure = Some(super::pbr::PbrError3D::Association(
                            "object reveal Timeline admission failed",
                        ));
                        changed = true;
                    }
                }
            }
        }
        if reveal.progress != previous || changed {
            if let Some(next) = self.reveal_frame_revision.checked_add(1) {
                self.reveal_frame_revision = next;
            } else {
                reveal.failure = Some(super::pbr::PbrError3D::Association(
                    "object reveal presentation generation exhausted",
                ));
            }
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.spatial_hud_queued
            || self.outline_queued
            || self.retired_presence_handle.is_some()
            || self
                .presence
                .as_ref()
                .is_some_and(|p| p.failure.is_none() && (p.queued || p.handle.is_some()))
            || self.retired_reveal_handle.is_some()
            || self
                .object_reveal
                .as_ref()
                .is_some_and(|r| r.queued || (r.pending && r.target_ready) || r.handle.is_some())
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        if self.spatial_hud_request.is_some() {
            self.spatial_hud_typography =
                super::spatial_hud::HudTypography::measure(cx.text_engine, cx.theme).ok();
        }
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            constraints.min_width
        };
        let height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            constraints.min_height
        };
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_target_bound_image(
            rect,
            self.admission.presentation_id(),
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            LAYER_CONTENT,
            cx.scene_key,
        );
    }

    fn custom_render_admission(&self) -> Option<&CustomRenderAdmission> {
        Some(&self.admission)
    }

    fn custom_render_participant_mut(&mut self) -> Option<&mut dyn CustomRenderParticipant> {
        Some(self)
    }

    fn on_unmount(&mut self) {
        self.retire_spatial_hud();
        self.outline_request = None;
        self.outline_queued = false;
        self.presence_mounted = false;
        self.retire_loading_presence();
        self.retired_presence_handle = None;
        self.object_reveal = None;
        self.retired_reveal_handle = None;
        if let Some(authority) = self.flat_viewport3d_writer.take() {
            authority.retire();
        }
        self.current_prepared_surface = None;
        for target in self.targets.values_mut() {
            target.retire(Viewport3DLifecycleState::Retired);
        }
    }

    fn on_mount(&mut self) {
        if let Some(next) = self.presence_mount.checked_add(1) {
            self.presence_mount = next;
            self.presence_mounted = true;
        } else if let Some(p) = self.presence.as_mut() {
            p.failure = Some(super::LoadingPresenceError3D::Association(
                "mount generation exhausted",
            ));
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "Viewport3D"
    }
}
