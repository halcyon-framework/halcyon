use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::mem::size_of;
use std::ops::Range;
use std::sync::Arc;

use glam::{DQuat, DVec3};
use thiserror::Error;

use super::context::{
    Context3D, Context3DError, DomainMetadataLease3D, DomainRetentionError3D, DomainRetentionKind3,
};
use super::math::{
    Math3DError, ModelPlacement3, Orientation3, PositiveScale3, Ray3, WorldPoint3, WorldVector3,
};
use super::snapshot::{
    DrawPacketDescriptor, ModelRevision, ModelState, ModelStateDescriptor, ModelStateError,
    RenderSnapshot, RenderSnapshot3DError, RenderSnapshotDescriptor, StructuralRevision,
};
use super::viewport::{
    Viewport3DRequestSerial, Viewport3DTargetCompatibilityKey, Viewport3DViewRevision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDomainId3(u64);

impl LocalDomainId3 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for LocalDomainId3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainContentId3(u64);

impl DomainContentId3 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DomainContentId3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainStateRevision3(u64);

impl DomainStateRevision3 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, DomainState3Error> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(DomainState3Error::RevisionExhausted { revision: self })
    }
}

impl fmt::Display for DomainStateRevision3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalDomainKind3 {
    Planar2,
    Spatial3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainPoint2 {
    u: f64,
    v: f64,
}

impl DomainPoint2 {
    pub fn try_new(u: f64, v: f64) -> Result<Self, Math3DError> {
        if !(u.is_finite() && v.is_finite()) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(Self { u, v })
    }

    pub const fn u(self) -> f64 {
        self.u
    }

    pub const fn v(self) -> f64 {
        self.v
    }

    fn expanded(self) -> DomainPoint3 {
        DomainPoint3 {
            x: self.u,
            y: self.v,
            z: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainPoint3 {
    x: f64,
    y: f64,
    z: f64,
}

impl DomainPoint3 {
    pub fn try_new(x: f64, y: f64, z: f64) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(Self { x, y, z })
    }

    pub const fn origin() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub const fn x(self) -> f64 {
        self.x
    }

    pub const fn y(self) -> f64 {
        self.y
    }

    pub const fn z(self) -> f64 {
        self.z
    }

    fn to_dvec3(self) -> DVec3 {
        DVec3::new(self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainVector3 {
    x: f64,
    y: f64,
    z: f64,
}

impl DomainVector3 {
    pub fn try_new(x: f64, y: f64, z: f64) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(Self { x, y, z })
    }

    pub const fn x(self) -> f64 {
        self.x
    }

    pub const fn y(self) -> f64 {
        self.y
    }

    pub const fn z(self) -> f64 {
        self.z
    }

    fn to_dvec3(self) -> DVec3 {
        DVec3::new(self.x, self.y, self.z)
    }

    fn normalized(self) -> Result<Self, Math3DError> {
        let normalized = WorldVector3::try_new(self.x, self.y, self.z)?.normalized()?;
        Self::try_new(normalized.x(), normalized.y(), normalized.z())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainEmbedding3 {
    origin_world: WorldPoint3,
    orientation_world_from_domain: Orientation3,
    uniform_scale_world_per_domain: f32,
}

impl DomainEmbedding3 {
    pub fn try_new(
        origin_world: WorldPoint3,
        orientation_world_from_domain: Orientation3,
        uniform_scale_world_per_domain: f32,
    ) -> Result<Self, Math3DError> {
        PositiveScale3::try_new(
            uniform_scale_world_per_domain,
            uniform_scale_world_per_domain,
            uniform_scale_world_per_domain,
        )?;
        Ok(Self {
            origin_world,
            orientation_world_from_domain,
            uniform_scale_world_per_domain,
        })
    }

    pub fn identity() -> Self {
        Self {
            origin_world: WorldPoint3::origin(),
            orientation_world_from_domain: Orientation3::identity(),
            uniform_scale_world_per_domain: 1.0,
        }
    }

    pub const fn origin_world(self) -> WorldPoint3 {
        self.origin_world
    }

    pub const fn orientation_world_from_domain(self) -> Orientation3 {
        self.orientation_world_from_domain
    }

    pub const fn uniform_scale_world_per_domain(self) -> f32 {
        self.uniform_scale_world_per_domain
    }

    fn rotation(self) -> DQuat {
        DQuat::from_xyzw(
            f64::from(self.orientation_world_from_domain.x()),
            f64::from(self.orientation_world_from_domain.y()),
            f64::from(self.orientation_world_from_domain.z()),
            f64::from(self.orientation_world_from_domain.w()),
        )
    }

    fn point_to_world(self, point: DomainPoint3) -> Result<WorldPoint3, Math3DError> {
        let rotated =
            self.rotation() * (point.to_dvec3() * f64::from(self.uniform_scale_world_per_domain));
        let world = DVec3::new(
            self.origin_world.x(),
            self.origin_world.y(),
            self.origin_world.z(),
        ) + rotated;
        if !world.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        WorldPoint3::try_new(world.x, world.y, world.z)
    }

    fn point_from_world(self, point: WorldPoint3) -> Result<DomainPoint3, Math3DError> {
        let translated = DVec3::new(
            point.x() - self.origin_world.x(),
            point.y() - self.origin_world.y(),
            point.z() - self.origin_world.z(),
        );
        if !translated.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let local = (self.rotation().conjugate() * translated)
            / f64::from(self.uniform_scale_world_per_domain);
        if !local.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        DomainPoint3::try_new(local.x, local.y, local.z)
    }

    fn vector_to_world(self, vector: DomainVector3) -> Result<WorldVector3, Math3DError> {
        let world =
            self.rotation() * (vector.to_dvec3() * f64::from(self.uniform_scale_world_per_domain));
        if !world.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        WorldVector3::try_new(world.x, world.y, world.z)
    }

    fn vector_from_world(self, vector: WorldVector3) -> Result<DomainVector3, Math3DError> {
        let local = (self.rotation().conjugate() * DVec3::new(vector.x(), vector.y(), vector.z()))
            / f64::from(self.uniform_scale_world_per_domain);
        if !local.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        DomainVector3::try_new(local.x, local.y, local.z)
    }
}

impl Default for DomainEmbedding3 {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainContentPlacement3 {
    translation_domain: DomainPoint3,
    orientation_domain_from_content: Orientation3,
    scale_content: PositiveScale3,
}

impl DomainContentPlacement3 {
    pub const fn new(
        translation_domain: DomainPoint3,
        orientation_domain_from_content: Orientation3,
        scale_content: PositiveScale3,
    ) -> Self {
        Self {
            translation_domain,
            orientation_domain_from_content,
            scale_content,
        }
    }

    pub const fn identity() -> Self {
        Self {
            translation_domain: DomainPoint3::origin(),
            orientation_domain_from_content: Orientation3::identity(),
            scale_content: PositiveScale3::one(),
        }
    }

    pub const fn translation_domain(self) -> DomainPoint3 {
        self.translation_domain
    }

    pub const fn orientation_domain_from_content(self) -> Orientation3 {
        self.orientation_domain_from_content
    }

    pub const fn scale_content(self) -> PositiveScale3 {
        self.scale_content
    }

    fn lower(self, embedding: DomainEmbedding3) -> Result<ModelPlacement3, Math3DError> {
        let translation = embedding.point_to_world(self.translation_domain)?;
        let orientation = embedding
            .orientation_world_from_domain
            .compose_local(self.orientation_domain_from_content)?;
        let uniform = f64::from(embedding.uniform_scale_world_per_domain);
        let checked_component = |component: f32| {
            let result = uniform * f64::from(component);
            if !result.is_finite() || result <= 0.0 || result > f64::from(f32::MAX) {
                return Err(Math3DError::NonRepresentableModelTransform);
            }
            let result = result as f32;
            if !result.is_finite() || result <= 0.0 {
                return Err(Math3DError::NonRepresentableModelTransform);
            }
            Ok(result)
        };
        let scale = PositiveScale3::try_new(
            checked_component(self.scale_content.x())?,
            checked_component(self.scale_content.y())?,
            checked_component(self.scale_content.z())?,
        )?;
        ModelPlacement3::try_new(translation, orientation, scale)
    }
}

impl Default for DomainContentPlacement3 {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainContent3Descriptor {
    id: DomainContentId3,
    draw_packets: Arc<[DrawPacketDescriptor]>,
}

impl DomainContent3Descriptor {
    pub fn new(id: DomainContentId3, draw_packets: Vec<DrawPacketDescriptor>) -> Self {
        Self::from_shared(id, draw_packets.into())
    }

    pub fn from_shared(id: DomainContentId3, draw_packets: Arc<[DrawPacketDescriptor]>) -> Self {
        Self { id, draw_packets }
    }

    pub const fn id(&self) -> DomainContentId3 {
        self.id
    }

    pub fn draw_packets(&self) -> &[DrawPacketDescriptor] {
        &self.draw_packets
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalDomain3Descriptor {
    id: LocalDomainId3,
    kind: LocalDomainKind3,
    contents: Arc<[DomainContent3Descriptor]>,
}

impl LocalDomain3Descriptor {
    pub fn new(
        id: LocalDomainId3,
        kind: LocalDomainKind3,
        contents: Vec<DomainContent3Descriptor>,
    ) -> Self {
        Self::from_shared(id, kind, contents.into())
    }

    pub fn from_shared(
        id: LocalDomainId3,
        kind: LocalDomainKind3,
        contents: Arc<[DomainContent3Descriptor]>,
    ) -> Self {
        Self { id, kind, contents }
    }

    pub const fn id(&self) -> LocalDomainId3 {
        self.id
    }

    pub const fn kind(&self) -> LocalDomainKind3 {
        self.kind
    }

    pub fn contents(&self) -> &[DomainContent3Descriptor] {
        &self.contents
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainComposition3Descriptor {
    structural_revision: StructuralRevision,
    domains: Arc<[LocalDomain3Descriptor]>,
}

impl DomainComposition3Descriptor {
    pub fn new(
        structural_revision: StructuralRevision,
        domains: Vec<LocalDomain3Descriptor>,
    ) -> Self {
        Self::from_shared(structural_revision, domains.into())
    }

    pub fn from_shared(
        structural_revision: StructuralRevision,
        domains: Arc<[LocalDomain3Descriptor]>,
    ) -> Self {
        Self {
            structural_revision,
            domains,
        }
    }

    pub const fn structural_revision(&self) -> StructuralRevision {
        self.structural_revision
    }

    pub fn domains(&self) -> &[LocalDomain3Descriptor] {
        &self.domains
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainState3Descriptor {
    domain_id: LocalDomainId3,
    revision: DomainStateRevision3,
    embedding: DomainEmbedding3,
    content_placements: Arc<[(DomainContentId3, DomainContentPlacement3)]>,
}

impl DomainState3Descriptor {
    pub fn new(
        domain_id: LocalDomainId3,
        revision: DomainStateRevision3,
        embedding: DomainEmbedding3,
        content_placements: Vec<(DomainContentId3, DomainContentPlacement3)>,
    ) -> Self {
        Self::from_shared(domain_id, revision, embedding, content_placements.into())
    }

    pub fn from_shared(
        domain_id: LocalDomainId3,
        revision: DomainStateRevision3,
        embedding: DomainEmbedding3,
        content_placements: Arc<[(DomainContentId3, DomainContentPlacement3)]>,
    ) -> Self {
        Self {
            domain_id,
            revision,
            embedding,
            content_placements,
        }
    }

    pub const fn domain_id(&self) -> LocalDomainId3 {
        self.domain_id
    }

    pub const fn revision(&self) -> DomainStateRevision3 {
        self.revision
    }

    pub const fn embedding(&self) -> DomainEmbedding3 {
        self.embedding
    }

    pub fn content_placements(&self) -> &[(DomainContentId3, DomainContentPlacement3)] {
        &self.content_placements
    }
}

#[derive(Debug, Clone)]
pub struct DomainFrame3Descriptor {
    model_revision: ModelRevision,
    states: Arc<[DomainState3]>,
}

impl DomainFrame3Descriptor {
    pub fn new(model_revision: ModelRevision, states: Vec<DomainState3>) -> Self {
        Self::from_shared(model_revision, states.into())
    }

    pub fn from_shared(model_revision: ModelRevision, states: Arc<[DomainState3]>) -> Self {
        Self {
            model_revision,
            states,
        }
    }

    pub const fn model_revision(&self) -> ModelRevision {
        self.model_revision
    }

    pub fn states(&self) -> &[DomainState3] {
        &self.states
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainComposition3Error {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error(transparent)]
    Retention(#[from] DomainRetentionError3D),
    #[error(transparent)]
    Snapshot(#[from] RenderSnapshot3DError),
    #[error("domain composition must contain at least one domain")]
    EmptyComposition,
    #[error("duplicate local domain ID {domain_id}")]
    DuplicateDomainId { domain_id: LocalDomainId3 },
    #[error("domain {domain_id} must contain at least one content descriptor")]
    EmptyDomain { domain_id: LocalDomainId3 },
    #[error("duplicate content ID {content_id} in domain {domain_id}")]
    DuplicateContentId {
        domain_id: LocalDomainId3,
        content_id: DomainContentId3,
    },
    #[error("content {content_id} in domain {domain_id} must contain at least one draw packet")]
    EmptyContent {
        domain_id: LocalDomainId3,
        content_id: DomainContentId3,
    },
    #[error("unable to reserve bounded domain-composition storage")]
    StorageUnavailable,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainState3Error {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error(transparent)]
    Retention(#[from] DomainRetentionError3D),
    #[error(transparent)]
    Math(#[from] Math3DError),
    #[error("domain {domain_id} is absent from the admitted composition")]
    MissingDomain { domain_id: LocalDomainId3 },
    #[error("domain state belongs to another immutable composition identity")]
    CompositionIdentityMismatch,
    #[error("domain state targets {requested}, but current domain is {current}")]
    DomainIdentityMismatch {
        current: LocalDomainId3,
        requested: LocalDomainId3,
    },
    #[error("domain {domain_id} has kind {actual:?}; operation requires {required:?}")]
    WrongDomainKind {
        domain_id: LocalDomainId3,
        required: LocalDomainKind3,
        actual: LocalDomainKind3,
    },
    #[error("duplicate content placement {content_id} for domain {domain_id}")]
    DuplicateContentPlacement {
        domain_id: LocalDomainId3,
        content_id: DomainContentId3,
    },
    #[error("content placement {content_id} is not admitted in domain {domain_id}")]
    ExtraContentPlacement {
        domain_id: LocalDomainId3,
        content_id: DomainContentId3,
    },
    #[error("domain {domain_id} is missing content placement {content_id}")]
    MissingContentPlacement {
        domain_id: LocalDomainId3,
        content_id: DomainContentId3,
    },
    #[error("domain-state revision {revision} exhausted its non-wrapping space")]
    RevisionExhausted { revision: DomainStateRevision3 },
    #[error("domain-state revision {requested} is stale; current revision is {current}")]
    StaleRevision {
        current: DomainStateRevision3,
        requested: DomainStateRevision3,
    },
    #[error("domain-state revision {revision} is bound to another immutable state identity")]
    ReusedRevision { revision: DomainStateRevision3 },
    #[error("unable to reserve bounded domain-state storage")]
    StorageUnavailable,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainFrame3Error {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error(transparent)]
    Retention(#[from] DomainRetentionError3D),
    #[error(transparent)]
    Model(#[from] ModelStateError),
    #[error(transparent)]
    Snapshot(#[from] RenderSnapshot3DError),
    #[error("duplicate state for domain {domain_id}")]
    DuplicateDomainState { domain_id: LocalDomainId3 },
    #[error("state for domain {domain_id} belongs to another composition")]
    ForeignDomainState { domain_id: LocalDomainId3 },
    #[error("state targets domain {domain_id}, which is absent from this composition")]
    ExtraDomainState { domain_id: LocalDomainId3 },
    #[error("complete frame is missing state for domain {domain_id}")]
    MissingDomainState { domain_id: LocalDomainId3 },
    #[error("domain {domain_id} state transition is invalid: {source}")]
    StateTransition {
        domain_id: LocalDomainId3,
        #[source]
        source: DomainState3Error,
    },
    #[error("unable to reserve bounded domain-frame storage")]
    StorageUnavailable,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainAssociation3Error {
    #[error(transparent)]
    Math(#[from] Math3DError),
    #[error("domain {domain_id} has kind {actual:?}; operation requires Planar2")]
    PlanarDomainRequired {
        domain_id: LocalDomainId3,
        actual: LocalDomainKind3,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DomainContentRecord3 {
    id: DomainContentId3,
    packet_range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalDomainRecord3 {
    id: LocalDomainId3,
    kind: LocalDomainKind3,
    content_range: Range<usize>,
    packet_range: Range<usize>,
}

struct DomainComposition3Core {
    context: Context3D,
    structural_revision: StructuralRevision,
    domains: Arc<[LocalDomainRecord3]>,
    contents: Arc<[DomainContentRecord3]>,
    snapshot: RenderSnapshot,
    retained_metadata_bytes: u64,
    _retention: DomainMetadataLease3D,
}

#[derive(Clone)]
pub struct DomainComposition3 {
    committed: Arc<DomainComposition3Core>,
}

impl DomainComposition3 {
    pub fn structural_revision(&self) -> StructuralRevision {
        self.committed.structural_revision
    }

    pub fn render_snapshot(&self) -> &RenderSnapshot {
        &self.committed.snapshot
    }

    pub fn domain_count(&self) -> usize {
        self.committed.domains.len()
    }

    pub fn content_count(&self) -> usize {
        self.committed.contents.len()
    }

    pub fn domains(
        &self,
    ) -> impl ExactSizeIterator<Item = (LocalDomainId3, LocalDomainKind3)> + '_ {
        self.committed
            .domains
            .iter()
            .map(|domain| (domain.id, domain.kind))
    }

    pub fn domain_kind(&self, domain_id: LocalDomainId3) -> Option<LocalDomainKind3> {
        self.domain_index(domain_id)
            .map(|index| self.committed.domains[index].kind)
    }

    pub fn contents(
        &self,
        domain_id: LocalDomainId3,
    ) -> Option<impl ExactSizeIterator<Item = DomainContentId3> + '_> {
        let domain = &self.committed.domains[self.domain_index(domain_id)?];
        Some(
            self.committed.contents[domain.content_range.clone()]
                .iter()
                .map(|content| content.id),
        )
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.committed, &other.committed)
    }

    pub fn retained_metadata_bytes(&self) -> u64 {
        self.committed.retained_metadata_bytes
    }

    pub fn validate_successor(&self, requested: &Self) -> Result<(), DomainFrame3Error> {
        self.committed
            .snapshot
            .validate_successor(&requested.committed.snapshot)?;
        Ok(())
    }

    fn context(&self) -> &Context3D {
        &self.committed.context
    }

    fn domain_index(&self, domain_id: LocalDomainId3) -> Option<usize> {
        self.committed
            .domains
            .iter()
            .position(|domain| domain.id == domain_id)
    }

    fn domain_record(&self, domain_index: usize) -> &LocalDomainRecord3 {
        &self.committed.domains[domain_index]
    }

    fn content_records(&self, domain_index: usize) -> &[DomainContentRecord3] {
        let domain = self.domain_record(domain_index);
        &self.committed.contents[domain.content_range.clone()]
    }
}

impl fmt::Debug for DomainComposition3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainComposition3")
            .field("structural_revision", &self.structural_revision())
            .field("domain_count", &self.domain_count())
            .field("content_count", &self.content_count())
            .finish_non_exhaustive()
    }
}

struct DomainState3Core {
    composition: DomainComposition3,
    domain_index: usize,
    revision: DomainStateRevision3,
    embedding: DomainEmbedding3,
    content_placements: Arc<[DomainContentPlacement3]>,
    lowered_content_placements: Arc<[ModelPlacement3]>,
    retained_metadata_bytes: u64,
    _retention: DomainMetadataLease3D,
}

#[derive(Clone)]
pub struct DomainState3 {
    committed: Arc<DomainState3Core>,
}

impl DomainState3 {
    pub fn composition(&self) -> &DomainComposition3 {
        &self.committed.composition
    }

    pub fn domain_id(&self) -> LocalDomainId3 {
        self.committed
            .composition
            .domain_record(self.committed.domain_index)
            .id
    }

    pub fn kind(&self) -> LocalDomainKind3 {
        self.committed
            .composition
            .domain_record(self.committed.domain_index)
            .kind
    }

    pub fn revision(&self) -> DomainStateRevision3 {
        self.committed.revision
    }

    pub fn embedding(&self) -> DomainEmbedding3 {
        self.committed.embedding
    }

    pub fn content_placement(
        &self,
        content_id: DomainContentId3,
    ) -> Option<DomainContentPlacement3> {
        self.committed
            .composition
            .content_records(self.committed.domain_index)
            .iter()
            .position(|content| content.id == content_id)
            .map(|index| self.committed.content_placements[index])
    }

    pub fn content_placements(
        &self,
    ) -> impl ExactSizeIterator<Item = (DomainContentId3, DomainContentPlacement3)> + '_ {
        self.committed
            .composition
            .content_records(self.committed.domain_index)
            .iter()
            .zip(self.committed.content_placements.iter())
            .map(|(content, placement)| (content.id, *placement))
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.committed, &other.committed)
    }

    pub fn retained_metadata_bytes(&self) -> u64 {
        self.committed.retained_metadata_bytes
    }

    pub fn validate_successor(&self, requested: &Self) -> Result<(), DomainState3Error> {
        if !self
            .committed
            .composition
            .same_identity(&requested.committed.composition)
        {
            return Err(DomainState3Error::CompositionIdentityMismatch);
        }
        if self.domain_id() != requested.domain_id() {
            return Err(DomainState3Error::DomainIdentityMismatch {
                current: self.domain_id(),
                requested: requested.domain_id(),
            });
        }
        match requested.revision().cmp(&self.revision()) {
            Ordering::Less => Err(DomainState3Error::StaleRevision {
                current: self.revision(),
                requested: requested.revision(),
            }),
            Ordering::Equal if self.same_identity(requested) => Ok(()),
            Ordering::Equal => Err(DomainState3Error::ReusedRevision {
                revision: requested.revision(),
            }),
            Ordering::Greater => Ok(()),
        }
    }

    pub fn planar_point_to_world(
        &self,
        point: DomainPoint2,
    ) -> Result<WorldPoint3, DomainState3Error> {
        self.require_kind(LocalDomainKind3::Planar2)?;
        Ok(self.embedding().point_to_world(point.expanded())?)
    }

    pub fn spatial_point_to_world(
        &self,
        point: DomainPoint3,
    ) -> Result<WorldPoint3, DomainState3Error> {
        self.require_kind(LocalDomainKind3::Spatial3)?;
        Ok(self.embedding().point_to_world(point)?)
    }

    pub fn world_point_to_domain(&self, point: WorldPoint3) -> Result<DomainPoint3, Math3DError> {
        self.embedding().point_from_world(point)
    }

    pub fn domain_vector_to_world(
        &self,
        vector: DomainVector3,
    ) -> Result<WorldVector3, Math3DError> {
        self.embedding().vector_to_world(vector)
    }

    pub fn world_vector_to_domain(
        &self,
        vector: WorldVector3,
    ) -> Result<DomainVector3, Math3DError> {
        self.embedding().vector_from_world(vector)
    }

    pub fn world_ray_to_domain(&self, ray: Ray3) -> Result<DomainRay3, Math3DError> {
        DomainRay3::from_world(self.domain_id(), self.kind(), self.embedding(), ray)
    }

    fn require_kind(&self, required: LocalDomainKind3) -> Result<(), DomainState3Error> {
        let actual = self.kind();
        if actual != required {
            return Err(DomainState3Error::WrongDomainKind {
                domain_id: self.domain_id(),
                required,
                actual,
            });
        }
        Ok(())
    }
}

impl fmt::Debug for DomainState3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainState3")
            .field("domain_id", &self.domain_id())
            .field("kind", &self.kind())
            .field("revision", &self.revision())
            .finish_non_exhaustive()
    }
}

struct DomainFrame3Core {
    composition: DomainComposition3,
    states: Arc<[DomainState3]>,
    model: ModelState,
    retained_metadata_bytes: u64,
    _retention: DomainMetadataLease3D,
}

#[derive(Clone)]
pub struct DomainFrame3 {
    committed: Arc<DomainFrame3Core>,
}

impl DomainFrame3 {
    pub fn composition(&self) -> &DomainComposition3 {
        &self.committed.composition
    }

    pub fn model(&self) -> &ModelState {
        &self.committed.model
    }

    pub fn model_revision(&self) -> ModelRevision {
        self.committed.model.revision()
    }

    pub fn states(&self) -> &[DomainState3] {
        &self.committed.states
    }

    pub fn state(&self, domain_id: LocalDomainId3) -> Option<&DomainState3> {
        let index = self.committed.composition.domain_index(domain_id)?;
        self.committed.states.get(index)
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.committed, &other.committed)
    }

    pub fn retained_metadata_bytes(&self) -> u64 {
        self.committed.retained_metadata_bytes
    }

    pub fn validate_successor(&self, requested: &Self) -> Result<(), DomainFrame3Error> {
        self.committed
            .composition
            .validate_successor(&requested.committed.composition)?;
        self.committed
            .model
            .validate_successor(&requested.committed.model)?;
        if self
            .committed
            .composition
            .same_identity(&requested.committed.composition)
        {
            for (current, successor) in self.states().iter().zip(requested.states()) {
                current.validate_successor(successor).map_err(|source| {
                    DomainFrame3Error::StateTransition {
                        domain_id: current.domain_id(),
                        source,
                    }
                })?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for DomainFrame3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainFrame3")
            .field(
                "structural_revision",
                &self.composition().structural_revision(),
            )
            .field("model_revision", &self.model_revision())
            .field("domain_count", &self.states().len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanarRayAccess3 {
    FrontOnly,
    BackOnly,
    TwoSided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanarRaySide3 {
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanarDomainHit3 {
    point: DomainPoint2,
    approached_side: PlanarRaySide3,
    domain_parameter: f64,
    world_distance: f64,
}

impl PlanarDomainHit3 {
    pub const fn point(self) -> DomainPoint2 {
        self.point
    }

    pub const fn approached_side(self) -> PlanarRaySide3 {
        self.approached_side
    }

    pub const fn domain_parameter(self) -> f64 {
        self.domain_parameter
    }

    pub const fn world_distance(self) -> f64 {
        self.world_distance
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainRay3 {
    domain_id: LocalDomainId3,
    kind: LocalDomainKind3,
    origin: DomainPoint3,
    direction: DomainVector3,
    world_units_per_local_parameter: f64,
}

impl DomainRay3 {
    fn from_world(
        domain_id: LocalDomainId3,
        kind: LocalDomainKind3,
        embedding: DomainEmbedding3,
        ray: Ray3,
    ) -> Result<Self, Math3DError> {
        let origin = embedding.point_from_world(ray.origin())?;
        let inverse_rotation = embedding.rotation().conjugate();
        let direction = inverse_rotation
            * DVec3::new(
                ray.direction().x(),
                ray.direction().y(),
                ray.direction().z(),
            );
        if !direction.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let direction =
            DomainVector3::try_new(direction.x, direction.y, direction.z)?.normalized()?;
        let world_units_per_local_parameter = f64::from(embedding.uniform_scale_world_per_domain());
        if !world_units_per_local_parameter.is_finite() || world_units_per_local_parameter <= 0.0 {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(Self {
            domain_id,
            kind,
            origin,
            direction,
            world_units_per_local_parameter,
        })
    }

    pub const fn domain_id(self) -> LocalDomainId3 {
        self.domain_id
    }

    pub const fn kind(self) -> LocalDomainKind3 {
        self.kind
    }

    pub const fn origin(self) -> DomainPoint3 {
        self.origin
    }

    pub const fn direction(self) -> DomainVector3 {
        self.direction
    }

    pub const fn world_units_per_local_parameter(self) -> f64 {
        self.world_units_per_local_parameter
    }

    pub fn point_at(self, domain_parameter: f64) -> Result<DomainPoint3, Math3DError> {
        if !domain_parameter.is_finite() || domain_parameter < 0.0 {
            return Err(Math3DError::InvalidRayDistance);
        }
        DomainPoint3::try_new(
            self.origin.x + self.direction.x * domain_parameter,
            self.origin.y + self.direction.y * domain_parameter,
            self.origin.z + self.direction.z * domain_parameter,
        )
    }

    pub fn intersect_planar(
        self,
        access: PlanarRayAccess3,
    ) -> Result<Option<PlanarDomainHit3>, DomainAssociation3Error> {
        if self.kind != LocalDomainKind3::Planar2 {
            return Err(DomainAssociation3Error::PlanarDomainRequired {
                domain_id: self.domain_id,
                actual: self.kind,
            });
        }
        let normal_component = self.direction.z;
        if normal_component == 0.0 {
            return Ok(None);
        }
        let approached_side = if normal_component < 0.0 {
            PlanarRaySide3::Front
        } else {
            PlanarRaySide3::Back
        };
        if matches!(
            (access, approached_side),
            (PlanarRayAccess3::FrontOnly, PlanarRaySide3::Back)
                | (PlanarRayAccess3::BackOnly, PlanarRaySide3::Front)
        ) {
            return Ok(None);
        }
        let domain_parameter = -self.origin.z / normal_component;
        if !domain_parameter.is_finite() || domain_parameter < 0.0 {
            return Ok(None);
        }
        let point = self.point_at(domain_parameter)?;
        let world_distance = domain_parameter * self.world_units_per_local_parameter;
        if !world_distance.is_finite() || world_distance < 0.0 {
            return Err(Math3DError::ArithmeticOverflow.into());
        }
        Ok(Some(PlanarDomainHit3 {
            point: DomainPoint2::try_new(point.x, point.y)?,
            approached_side,
            domain_parameter,
            world_distance,
        }))
    }
}

#[derive(Clone)]
pub struct CommittedDomain3 {
    target_key: Viewport3DTargetCompatibilityKey,
    request_serial: Viewport3DRequestSerial,
    view_revision: Viewport3DViewRevision,
    frame: DomainFrame3,
    domain_index: usize,
}

impl CommittedDomain3 {
    pub(crate) fn new(
        target_key: Viewport3DTargetCompatibilityKey,
        request_serial: Viewport3DRequestSerial,
        view_revision: Viewport3DViewRevision,
        frame: DomainFrame3,
        domain_index: usize,
    ) -> Self {
        Self {
            target_key,
            request_serial,
            view_revision,
            frame,
            domain_index,
        }
    }

    pub const fn target_key(&self) -> Viewport3DTargetCompatibilityKey {
        self.target_key
    }

    pub const fn request_serial(&self) -> Viewport3DRequestSerial {
        self.request_serial
    }

    pub const fn view_revision(&self) -> Viewport3DViewRevision {
        self.view_revision
    }

    pub fn structural_revision(&self) -> StructuralRevision {
        self.frame.composition().structural_revision()
    }

    pub fn model_revision(&self) -> ModelRevision {
        self.frame.model_revision()
    }

    pub fn domain_id(&self) -> LocalDomainId3 {
        self.state().domain_id()
    }

    pub fn kind(&self) -> LocalDomainKind3 {
        self.state().kind()
    }

    pub fn domain_state_revision(&self) -> DomainStateRevision3 {
        self.state().revision()
    }

    pub fn embedding(&self) -> DomainEmbedding3 {
        self.state().embedding()
    }

    pub fn to_domain_ray(&self, ray: Ray3) -> Result<DomainRay3, Math3DError> {
        self.state().world_ray_to_domain(ray)
    }

    pub(crate) fn same_association(&self, other: &Self) -> bool {
        self.target_key == other.target_key
            && self.request_serial == other.request_serial
            && self.view_revision == other.view_revision
            && self.domain_index == other.domain_index
            && self.frame.same_identity(&other.frame)
    }

    fn state(&self) -> &DomainState3 {
        &self.frame.states()[self.domain_index]
    }
}

impl fmt::Debug for CommittedDomain3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommittedDomain3")
            .field("target_key", &self.target_key)
            .field("request_serial", &self.request_serial)
            .field("view_revision", &self.view_revision)
            .field("structural_revision", &self.structural_revision())
            .field("model_revision", &self.model_revision())
            .field("domain_id", &self.domain_id())
            .field("kind", &self.kind())
            .field("domain_state_revision", &self.domain_state_revision())
            .finish_non_exhaustive()
    }
}

fn checked_metadata_bytes(
    base: usize,
    arrays: &[(usize, usize)],
) -> Result<u64, DomainRetentionError3D> {
    let mut bytes =
        u64::try_from(base).map_err(|_| DomainRetentionError3D::ArithmeticOverflow {
            operation: "representing retained 3D domain metadata bytes",
        })?;
    for &(count, element_size) in arrays {
        let count =
            u64::try_from(count).map_err(|_| DomainRetentionError3D::ArithmeticOverflow {
                operation: "representing retained 3D domain metadata count",
            })?;
        let element_size = u64::try_from(element_size).map_err(|_| {
            DomainRetentionError3D::ArithmeticOverflow {
                operation: "representing retained 3D domain metadata element size",
            }
        })?;
        let array_bytes =
            count
                .checked_mul(element_size)
                .ok_or(DomainRetentionError3D::ArithmeticOverflow {
                    operation: "multiplying retained 3D domain metadata bytes",
                })?;
        bytes =
            bytes
                .checked_add(array_bytes)
                .ok_or(DomainRetentionError3D::ArithmeticOverflow {
                    operation: "summing retained 3D domain metadata bytes",
                })?;
    }
    Ok(bytes)
}

impl Context3D {
    pub fn admit_domain_composition(
        &self,
        descriptor: DomainComposition3Descriptor,
    ) -> Result<DomainComposition3, DomainComposition3Error> {
        if descriptor.domains().is_empty() {
            return Err(DomainComposition3Error::EmptyComposition);
        }

        let mut domain_ids = HashSet::new();
        domain_ids
            .try_reserve(descriptor.domains().len())
            .map_err(|_| DomainComposition3Error::StorageUnavailable)?;
        let mut domain_records = Vec::new();
        domain_records
            .try_reserve_exact(descriptor.domains().len())
            .map_err(|_| DomainComposition3Error::StorageUnavailable)?;
        let mut content_records = Vec::new();
        let mut draw_packets = Vec::new();

        for domain in descriptor.domains() {
            if !domain_ids.insert(domain.id()) {
                return Err(DomainComposition3Error::DuplicateDomainId {
                    domain_id: domain.id(),
                });
            }
            if domain.contents().is_empty() {
                return Err(DomainComposition3Error::EmptyDomain {
                    domain_id: domain.id(),
                });
            }
            content_records
                .try_reserve(domain.contents().len())
                .map_err(|_| DomainComposition3Error::StorageUnavailable)?;
            let content_start = content_records.len();
            let packet_start = draw_packets.len();
            let mut content_ids = HashSet::new();
            content_ids
                .try_reserve(domain.contents().len())
                .map_err(|_| DomainComposition3Error::StorageUnavailable)?;
            for content in domain.contents() {
                if !content_ids.insert(content.id()) {
                    return Err(DomainComposition3Error::DuplicateContentId {
                        domain_id: domain.id(),
                        content_id: content.id(),
                    });
                }
                if content.draw_packets().is_empty() {
                    return Err(DomainComposition3Error::EmptyContent {
                        domain_id: domain.id(),
                        content_id: content.id(),
                    });
                }
                draw_packets
                    .try_reserve(content.draw_packets().len())
                    .map_err(|_| DomainComposition3Error::StorageUnavailable)?;
                let content_packet_start = draw_packets.len();
                draw_packets.extend(content.draw_packets().iter().cloned());
                content_records.push(DomainContentRecord3 {
                    id: content.id(),
                    packet_range: content_packet_start..draw_packets.len(),
                });
            }
            domain_records.push(LocalDomainRecord3 {
                id: domain.id(),
                kind: domain.kind(),
                content_range: content_start..content_records.len(),
                packet_range: packet_start..draw_packets.len(),
            });
        }

        let retained_metadata_bytes = checked_metadata_bytes(
            size_of::<DomainComposition3Core>(),
            &[
                (domain_records.len(), size_of::<LocalDomainRecord3>()),
                (content_records.len(), size_of::<DomainContentRecord3>()),
            ],
        )?;
        let reservation = self
            .reserve_domain_metadata(DomainRetentionKind3::Composition, retained_metadata_bytes)?;
        let snapshot = self.admit_render_snapshot(RenderSnapshotDescriptor::new(
            descriptor.structural_revision(),
            draw_packets,
        ))?;
        let retention = reservation.commit();
        #[cfg(test)]
        super::audit::record_domain_composition_admission();
        Ok(DomainComposition3 {
            committed: Arc::new(DomainComposition3Core {
                context: self.clone(),
                structural_revision: descriptor.structural_revision(),
                domains: domain_records.into(),
                contents: content_records.into(),
                snapshot,
                retained_metadata_bytes,
                _retention: retention,
            }),
        })
    }

    pub fn admit_domain_state(
        &self,
        composition: &DomainComposition3,
        descriptor: DomainState3Descriptor,
    ) -> Result<DomainState3, DomainState3Error> {
        if !self.same_resource_family(composition.context()) {
            return Err(Context3DError::ContextMismatch.into());
        }
        let domain_index = composition.domain_index(descriptor.domain_id()).ok_or(
            DomainState3Error::MissingDomain {
                domain_id: descriptor.domain_id(),
            },
        )?;
        let content_records = composition.content_records(domain_index);
        let mut supplied = HashMap::new();
        supplied
            .try_reserve(descriptor.content_placements().len())
            .map_err(|_| DomainState3Error::StorageUnavailable)?;
        for &(content_id, placement) in descriptor.content_placements() {
            if !content_records
                .iter()
                .any(|content| content.id == content_id)
            {
                return Err(DomainState3Error::ExtraContentPlacement {
                    domain_id: descriptor.domain_id(),
                    content_id,
                });
            }
            if supplied.insert(content_id, placement).is_some() {
                return Err(DomainState3Error::DuplicateContentPlacement {
                    domain_id: descriptor.domain_id(),
                    content_id,
                });
            }
        }

        let mut placements = Vec::new();
        placements
            .try_reserve_exact(content_records.len())
            .map_err(|_| DomainState3Error::StorageUnavailable)?;
        let mut lowered = Vec::new();
        lowered
            .try_reserve_exact(content_records.len())
            .map_err(|_| DomainState3Error::StorageUnavailable)?;
        for content in content_records {
            let placement =
                supplied
                    .remove(&content.id)
                    .ok_or(DomainState3Error::MissingContentPlacement {
                        domain_id: descriptor.domain_id(),
                        content_id: content.id,
                    })?;
            lowered.push(placement.lower(descriptor.embedding())?);
            placements.push(placement);
        }

        let retained_metadata_bytes = checked_metadata_bytes(
            size_of::<DomainState3Core>(),
            &[
                (placements.len(), size_of::<DomainContentPlacement3>()),
                (lowered.len(), size_of::<ModelPlacement3>()),
            ],
        )?;
        let reservation =
            self.reserve_domain_metadata(DomainRetentionKind3::State, retained_metadata_bytes)?;
        let retention = reservation.commit();
        #[cfg(test)]
        super::audit::record_domain_state_admission(placements.len() as u64);
        Ok(DomainState3 {
            committed: Arc::new(DomainState3Core {
                composition: composition.clone(),
                domain_index,
                revision: descriptor.revision(),
                embedding: descriptor.embedding(),
                content_placements: placements.into(),
                lowered_content_placements: lowered.into(),
                retained_metadata_bytes,
                _retention: retention,
            }),
        })
    }

    pub fn admit_domain_frame(
        &self,
        composition: &DomainComposition3,
        descriptor: DomainFrame3Descriptor,
    ) -> Result<DomainFrame3, DomainFrame3Error> {
        if !self.same_resource_family(composition.context()) {
            return Err(Context3DError::ContextMismatch.into());
        }
        let mut supplied = HashMap::new();
        supplied
            .try_reserve(descriptor.states().len())
            .map_err(|_| DomainFrame3Error::StorageUnavailable)?;
        for state in descriptor.states() {
            if composition.domain_index(state.domain_id()).is_none() {
                return Err(DomainFrame3Error::ExtraDomainState {
                    domain_id: state.domain_id(),
                });
            }
            if !state.composition().same_identity(composition) {
                return Err(DomainFrame3Error::ForeignDomainState {
                    domain_id: state.domain_id(),
                });
            }
            if supplied.insert(state.domain_id(), state.clone()).is_some() {
                return Err(DomainFrame3Error::DuplicateDomainState {
                    domain_id: state.domain_id(),
                });
            }
        }

        let mut states = Vec::new();
        states
            .try_reserve_exact(composition.domain_count())
            .map_err(|_| DomainFrame3Error::StorageUnavailable)?;
        let mut placements = Vec::new();
        placements
            .try_reserve_exact(composition.render_snapshot().draw_packets().len())
            .map_err(|_| DomainFrame3Error::StorageUnavailable)?;
        for (domain_index, domain) in composition.committed.domains.iter().enumerate() {
            let state =
                supplied
                    .remove(&domain.id)
                    .ok_or(DomainFrame3Error::MissingDomainState {
                        domain_id: domain.id,
                    })?;
            for (content, placement) in composition
                .content_records(domain_index)
                .iter()
                .zip(state.committed.lowered_content_placements.iter())
            {
                placements.extend(std::iter::repeat_n(*placement, content.packet_range.len()));
            }
            states.push(state);
        }

        let retained_metadata_bytes = checked_metadata_bytes(
            size_of::<DomainFrame3Core>(),
            &[(states.len(), size_of::<DomainState3>())],
        )?;
        let reservation =
            self.reserve_domain_metadata(DomainRetentionKind3::Frame, retained_metadata_bytes)?;
        let model = self.admit_model_state(
            composition.render_snapshot(),
            ModelStateDescriptor::new(descriptor.model_revision(), placements),
        )?;
        let retention = reservation.commit();
        #[cfg(test)]
        super::audit::record_domain_frame_admission();
        Ok(DomainFrame3 {
            committed: Arc::new(DomainFrame3Core {
                composition: composition.clone(),
                states: states.into(),
                model,
                retained_metadata_bytes,
                _retention: retention,
            }),
        })
    }
}
