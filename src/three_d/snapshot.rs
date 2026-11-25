use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use thiserror::Error;

use super::context::ModelStateLease3D;
use super::{
    AppearanceHandle, AppearanceLease, AppearanceMode, Context3D, Context3DError, GeometryHandle,
    GeometryLease, ModelPlacement3, PolicyBudget3D,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralRevision(u64);

impl StructuralRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, RenderSnapshot3DError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(RenderSnapshot3DError::StructuralRevisionExhausted { revision: self })
    }
}

impl fmt::Display for StructuralRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelRevision(u64);

impl ModelRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, ModelStateError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ModelStateError::RevisionExhausted { revision: self })
    }
}

impl fmt::Display for ModelRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RasterMode {
    #[default]
    SingleSided,
    DoubleSided,
}

impl RasterMode {
    pub const fn culls_back_faces(self) -> bool {
        matches!(self, Self::SingleSided)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawPacketDescriptor {
    geometry: GeometryHandle,
    appearance: AppearanceHandle,
    raster_mode: RasterMode,
    stable_ordinal: u64,
}

/// Private S2 source carrier, separate from the unchanged public legacy packet.
/// Both alternatives retain their actual context-qualified resource leases.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MaterialPacketSource3D {
    #[allow(
        dead_code,
        reason = "Retained legacy appearance packet bridge and existing test coverage"
    )]
    LegacyAppearance(DrawPacketDescriptor),
    PbrMaterial {
        geometry: super::pbr::PbrGeometryHandle3D,
        material: super::pbr::PbrMaterialHandle3D,
        raster: RasterMode,
        stable_ordinal: u64,
    },
}

pub(crate) enum CommittedMaterialPacket3D {
    LegacyAppearance {
        geometry: GeometryLease,
        appearance: AppearanceLease,
        raster: RasterMode,
        #[allow(dead_code, reason = "Retained stable packet ordinal under HAL-355 8.3")]
        ordinal: u64,
    },
    PbrMaterial {
        geometry: super::pbr::PbrGeometryLease3D,
        material: super::pbr::PbrMaterialLease3D,
        raster: RasterMode,
        ordinal: u64,
    },
}

pub(crate) struct MaterialSnapshot3D {
    pub(super) context: Context3D,
    pub(super) revision: StructuralRevision,
    pub(super) source: Arc<[MaterialPacketSource3D]>,
    pub(super) packets: Arc<[CommittedMaterialPacket3D]>,
    pub(super) sampling_families: [bool; 2],
}

impl MaterialSnapshot3D {
    /// New independent-source admission must not reattach retired writable
    /// resource generations. Existing captured readers keep their original
    /// leases and last-good behavior; no successor rule is changed here.
    pub(crate) fn validate_live_resources(
        &self,
        context: &Context3D,
    ) -> Result<(), super::pbr::PbrError3D> {
        if !context.same_resource_family(&self.context) {
            return Err(Context3DError::ContextMismatch.into());
        }
        for packet in self.source.iter() {
            match packet {
                MaterialPacketSource3D::LegacyAppearance(packet) => {
                    context.lease_geometry(packet.geometry())?;
                    context.lease_appearance(packet.appearance())?;
                }
                MaterialPacketSource3D::PbrMaterial {
                    geometry, material, ..
                } => {
                    context.lease_pbr_geometry(geometry)?;
                    context.lease_pbr_material(material)?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn source_packets(&self) -> &[MaterialPacketSource3D] {
        &self.source
    }
    pub(crate) fn validate_successor(
        current: &Arc<Self>,
        next: &Arc<Self>,
    ) -> Result<(), super::pbr::PbrError3D> {
        if !current.context.same_resource_family(&next.context) {
            return Err(Context3DError::ContextMismatch.into());
        }
        if next.revision < current.revision
            || (next.revision == current.revision && !Arc::ptr_eq(current, next))
        {
            return Err(super::pbr::PbrError3D::Association(
                "stale/reused material structural revision",
            ));
        }
        Ok(())
    }
}

pub(crate) struct MaterialModelState3D {
    pub(super) snapshot: Arc<MaterialSnapshot3D>,
    pub(super) revision: ModelRevision,
    pub(super) placements: Arc<[ModelPlacement3]>,
    _lease: super::context::ModelStateLease3D,
}

impl Context3D {
    pub(crate) fn admit_material_snapshot(
        &self,
        revision: StructuralRevision,
        source: Vec<MaterialPacketSource3D>,
    ) -> Result<Arc<MaterialSnapshot3D>, super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        let count = u64::try_from(source.len())
            .map_err(|_| PbrError3D::Association("material packet count"))?;
        super::pbr::budget(
            self,
            super::PolicyBudget3D::DrawPacketsPerSnapshot,
            count,
            u64::from(self.limits().policy.max_draw_packets_per_snapshot),
        )?;
        let mut packets = Vec::new();
        packets.try_reserve_exact(source.len()).map_err(|_| {
            PbrError3D::Association("bounded material packet lease storage unavailable")
        })?;
        for packet in &source {
            packets.push(match packet {
                MaterialPacketSource3D::LegacyAppearance(p) => {
                    let geometry = self.lease_geometry(p.geometry())?;
                    let appearance = self.lease_appearance(p.appearance())?;
                    let required = appearance.descriptor().required_semantics();
                    let available = geometry.descriptor().semantics();
                    if !available.contains(required) {
                        return Err(Context3DError::MissingGeometrySemantics {
                            required,
                            available,
                        }
                        .into());
                    }
                    CommittedMaterialPacket3D::LegacyAppearance {
                        geometry,
                        appearance,
                        raster: p.raster_mode(),
                        ordinal: p.stable_ordinal(),
                    }
                }
                MaterialPacketSource3D::PbrMaterial {
                    geometry,
                    material,
                    raster,
                    stable_ordinal,
                } => {
                    let geometry = self.lease_pbr_geometry(geometry)?;
                    let material = self.lease_pbr_material(material)?;
                    geometry.data.source.validate_roles(&material.data.source)?;
                    if raster.culls_back_faces() == material.data.source.double_sided {
                        return Err(PbrError3D::Association(
                            "PBR sidedness and packet raster class disagree",
                        ));
                    }
                    CommittedMaterialPacket3D::PbrMaterial {
                        geometry,
                        material,
                        raster: *raster,
                        ordinal: *stable_ordinal,
                    }
                }
            });
        }
        let mut sampling_families = [false; 2];
        for p in &packets {
            if let CommittedMaterialPacket3D::PbrMaterial { material, .. } = p {
                sampling_families[usize::from(
                    material.data.source.sampling == super::mip_sampling::SamplingFamily3D::V2,
                )] = true;
            }
        }
        Ok(Arc::new(MaterialSnapshot3D {
            context: self.clone(),
            revision,
            source: source.into(),
            packets: packets.into(),
            sampling_families,
        }))
    }

    pub(crate) fn admit_material_model(
        &self,
        snapshot: Arc<MaterialSnapshot3D>,
        revision: ModelRevision,
        placements: Vec<ModelPlacement3>,
    ) -> Result<Arc<MaterialModelState3D>, super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        if !self.same_resource_family(&snapshot.context) {
            return Err(Context3DError::ContextMismatch.into());
        }
        if placements.len() != snapshot.packets.len() {
            return Err(PbrError3D::Association(
                "complete material model placement count",
            ));
        }
        for (packet, placement) in snapshot.packets.iter().zip(&placements) {
            if matches!(packet, CommittedMaterialPacket3D::PbrMaterial { .. }) {
                let s = placement.scale();
                let lo = s.x().min(s.y()).min(s.z());
                let hi = s.x().max(s.y()).max(s.z());
                if f64::from(hi) / f64::from(lo) > 64.0 {
                    return Err(PbrError3D::Association(
                        "PBR positive-scale ratio exceeds64",
                    ));
                }
            }
        }
        let lease = self.reserve_model_state()?;
        Ok(Arc::new(MaterialModelState3D {
            snapshot,
            revision,
            placements: placements.into(),
            _lease: lease,
        }))
    }
}

impl DrawPacketDescriptor {
    pub fn new(
        geometry: GeometryHandle,
        appearance: AppearanceHandle,
        raster_mode: RasterMode,
        stable_ordinal: u64,
    ) -> Self {
        Self {
            geometry,
            appearance,
            raster_mode,
            stable_ordinal,
        }
    }

    pub const fn geometry(&self) -> &GeometryHandle {
        &self.geometry
    }

    pub const fn appearance(&self) -> &AppearanceHandle {
        &self.appearance
    }

    pub const fn raster_mode(&self) -> RasterMode {
        self.raster_mode
    }

    pub const fn stable_ordinal(&self) -> u64 {
        self.stable_ordinal
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSnapshotDescriptor {
    structural_revision: StructuralRevision,
    draw_packets: Arc<[DrawPacketDescriptor]>,
}

impl RenderSnapshotDescriptor {
    pub fn new(
        structural_revision: StructuralRevision,
        draw_packets: Vec<DrawPacketDescriptor>,
    ) -> Self {
        Self::from_shared(structural_revision, draw_packets.into())
    }

    pub fn from_shared(
        structural_revision: StructuralRevision,
        draw_packets: Arc<[DrawPacketDescriptor]>,
    ) -> Self {
        Self {
            structural_revision,
            draw_packets,
        }
    }

    pub const fn structural_revision(&self) -> StructuralRevision {
        self.structural_revision
    }

    pub fn draw_packets(&self) -> &[DrawPacketDescriptor] {
        &self.draw_packets
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RenderSnapshot3DError {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error("draw packet {packet_index} resource validation failed: {source}")]
    PacketResource {
        packet_index: u32,
        source: Context3DError,
    },
    #[error("structural revision {revision} exhausted its non-wrapping space")]
    StructuralRevisionExhausted { revision: StructuralRevision },
    #[error("structural revision {requested} is stale; current revision is {current}")]
    StaleStructuralRevision {
        current: StructuralRevision,
        requested: StructuralRevision,
    },
    #[error(
        "structural revision {revision} is already bound to another immutable snapshot identity"
    )]
    ReusedStructuralRevision { revision: StructuralRevision },
    #[error("unable to reserve CPU lease storage for {packet_count} bounded draw packets")]
    SnapshotStorageUnavailable { packet_count: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelStateDescriptor {
    revision: ModelRevision,
    placements: Arc<[ModelPlacement3]>,
}

impl ModelStateDescriptor {
    pub fn new(revision: ModelRevision, placements: Vec<ModelPlacement3>) -> Self {
        Self::from_shared(revision, placements.into())
    }

    pub fn from_shared(revision: ModelRevision, placements: Arc<[ModelPlacement3]>) -> Self {
        Self {
            revision,
            placements,
        }
    }

    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }

    pub fn placements(&self) -> &[ModelPlacement3] {
        &self.placements
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModelStateError {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error("model revision {revision} exhausted its non-wrapping space")]
    RevisionExhausted { revision: ModelRevision },
    #[error("model revision {requested} is stale; current revision is {current}")]
    StaleRevision {
        current: ModelRevision,
        requested: ModelRevision,
    },
    #[error("model revision {revision} is already bound to another immutable state identity")]
    ReusedRevision { revision: ModelRevision },
    #[error(
        "model state placement count {actual} does not match structural packet count {expected}"
    )]
    PlacementCountMismatch { expected: u64, actual: u64 },
    #[error("model state belongs to another immutable render snapshot identity")]
    StructuralIdentityMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelStateChange {
    Unchanged,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSnapshotChange {
    Unchanged,
    Advanced,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct CommittedDrawPacket {
    geometry: GeometryLease,
    appearance: AppearanceLease,
}

#[allow(dead_code)]
impl CommittedDrawPacket {
    pub(crate) const fn geometry(&self) -> &GeometryLease {
        &self.geometry
    }

    pub(crate) const fn appearance(&self) -> &AppearanceLease {
        &self.appearance
    }

    pub(crate) fn appearance_mode(&self) -> AppearanceMode {
        self.appearance.descriptor().mode()
    }
}

#[allow(dead_code)]
struct CommittedRenderSnapshot {
    context: Context3D,
    descriptor: RenderSnapshotDescriptor,
    packets: Arc<[CommittedDrawPacket]>,
}

#[derive(Clone)]
pub struct RenderSnapshot {
    committed: Arc<CommittedRenderSnapshot>,
}

impl RenderSnapshot {
    pub fn descriptor(&self) -> &RenderSnapshotDescriptor {
        &self.committed.descriptor
    }

    pub fn structural_revision(&self) -> StructuralRevision {
        self.committed.descriptor.structural_revision()
    }

    pub fn draw_packets(&self) -> &[DrawPacketDescriptor] {
        self.committed.descriptor.draw_packets()
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.committed, &other.committed)
    }

    pub fn validate_successor(
        &self,
        requested: &Self,
    ) -> Result<RenderSnapshotChange, RenderSnapshot3DError> {
        if !self
            .committed
            .context
            .same_resource_family(&requested.committed.context)
        {
            return Err(Context3DError::ContextMismatch.into());
        }

        match requested
            .structural_revision()
            .cmp(&self.structural_revision())
        {
            Ordering::Less => Err(RenderSnapshot3DError::StaleStructuralRevision {
                current: self.structural_revision(),
                requested: requested.structural_revision(),
            }),
            Ordering::Equal if self.same_identity(requested) => Ok(RenderSnapshotChange::Unchanged),
            Ordering::Equal => Err(RenderSnapshot3DError::ReusedStructuralRevision {
                revision: requested.structural_revision(),
            }),
            Ordering::Greater => Ok(RenderSnapshotChange::Advanced),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn committed_packets(&self) -> &[CommittedDrawPacket] {
        &self.committed.packets
    }

    pub(crate) fn context(&self) -> &Context3D {
        &self.committed.context
    }
}

struct CommittedModelState {
    snapshot: RenderSnapshot,
    descriptor: ModelStateDescriptor,
    _retention: ModelStateLease3D,
}

#[derive(Clone)]
pub struct ModelState {
    committed: Arc<CommittedModelState>,
}

impl ModelState {
    pub fn descriptor(&self) -> &ModelStateDescriptor {
        &self.committed.descriptor
    }

    pub fn revision(&self) -> ModelRevision {
        self.committed.descriptor.revision()
    }

    pub fn placements(&self) -> &[ModelPlacement3] {
        self.committed.descriptor.placements()
    }

    pub fn snapshot(&self) -> &RenderSnapshot {
        &self.committed.snapshot
    }

    pub fn matches_snapshot(&self, snapshot: &RenderSnapshot) -> bool {
        self.committed.snapshot.same_identity(snapshot)
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.committed, &other.committed)
    }

    pub fn validate_successor(
        &self,
        requested: &Self,
    ) -> Result<ModelStateChange, ModelStateError> {
        if !self
            .committed
            .snapshot
            .context()
            .same_resource_family(requested.committed.snapshot.context())
        {
            return Err(Context3DError::ContextMismatch.into());
        }

        match requested.revision().cmp(&self.revision()) {
            Ordering::Less => Err(ModelStateError::StaleRevision {
                current: self.revision(),
                requested: requested.revision(),
            }),
            Ordering::Equal if self.same_identity(requested) => Ok(ModelStateChange::Unchanged),
            Ordering::Equal => Err(ModelStateError::ReusedRevision {
                revision: requested.revision(),
            }),
            Ordering::Greater => Ok(ModelStateChange::Advanced),
        }
    }
}

impl fmt::Debug for ModelState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelState")
            .field("revision", &self.revision())
            .field("placement_count", &self.placements().len())
            .field(
                "structural_revision",
                &self.snapshot().structural_revision(),
            )
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RenderSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RenderSnapshot")
            .field("descriptor", &self.committed.descriptor)
            .finish_non_exhaustive()
    }
}

impl Context3D {
    pub fn admit_render_snapshot(
        &self,
        descriptor: RenderSnapshotDescriptor,
    ) -> Result<RenderSnapshot, RenderSnapshot3DError> {
        let packet_count = u64::try_from(descriptor.draw_packets().len()).map_err(|_| {
            Context3DError::ArithmeticOverflow {
                operation: "representing a render snapshot draw packet count",
            }
        })?;
        let maximum = u64::from(self.limits().policy.max_draw_packets_per_snapshot);
        if packet_count > maximum {
            return Err(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::DrawPacketsPerSnapshot,
                requested: packet_count,
                maximum,
            }
            .into());
        }

        let mut committed_packets = Vec::new();
        committed_packets
            .try_reserve_exact(descriptor.draw_packets().len())
            .map_err(|_| RenderSnapshot3DError::SnapshotStorageUnavailable { packet_count })?;

        for (index, packet) in descriptor.draw_packets().iter().enumerate() {
            let packet_index =
                u32::try_from(index).map_err(|_| Context3DError::ArithmeticOverflow {
                    operation: "representing a render snapshot draw packet index",
                })?;
            let geometry = self.lease_geometry(packet.geometry()).map_err(|source| {
                RenderSnapshot3DError::PacketResource {
                    packet_index,
                    source,
                }
            })?;
            let appearance = self
                .lease_appearance(packet.appearance())
                .map_err(|source| RenderSnapshot3DError::PacketResource {
                    packet_index,
                    source,
                })?;

            let required = appearance.descriptor().required_semantics();
            let available = geometry.descriptor().semantics();
            if !available.contains(required) {
                return Err(RenderSnapshot3DError::PacketResource {
                    packet_index,
                    source: Context3DError::MissingGeometrySemantics {
                        required,
                        available,
                    },
                });
            }

            committed_packets.push(CommittedDrawPacket {
                geometry,
                appearance,
            });
        }

        Ok(RenderSnapshot {
            committed: Arc::new(CommittedRenderSnapshot {
                context: self.clone(),
                descriptor,
                packets: committed_packets.into(),
            }),
        })
    }

    pub fn admit_model_state(
        &self,
        snapshot: &RenderSnapshot,
        descriptor: ModelStateDescriptor,
    ) -> Result<ModelState, ModelStateError> {
        if !self.same_resource_family(snapshot.context()) {
            return Err(Context3DError::ContextMismatch.into());
        }
        let expected = u64::try_from(snapshot.draw_packets().len()).map_err(|_| {
            Context3DError::ArithmeticOverflow {
                operation: "representing a render snapshot draw packet count",
            }
        })?;
        let actual = u64::try_from(descriptor.placements().len()).map_err(|_| {
            Context3DError::ArithmeticOverflow {
                operation: "representing a model state placement count",
            }
        })?;
        if actual != expected {
            return Err(ModelStateError::PlacementCountMismatch { expected, actual });
        }
        let retention = self.reserve_model_state()?;

        Ok(ModelState {
            committed: Arc::new(CommittedModelState {
                snapshot: snapshot.clone(),
                descriptor,
                _retention: retention,
            }),
        })
    }
}
