use std::any::Any;
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::renderer::wgpu_renderer::{DeviceLifetimeObservation, GpuDeviceFailure, GpuResources};

use super::appearance::{
    AppearanceCompilerLimits, compile_canonical_vertex, compile_fragment_appearance,
};
use super::raster::GpuState3D;
use super::{
    AbiLayout3D, Appearance3DAdmissionError, AppearanceFallbackBindingContract, AppearanceMode,
    AppearanceSourceDescriptor, CanonicalGeometryBytes, CanonicalIndexFormat, Geometry3DError,
    GeometrySemantics, GeometrySourceDescriptor, Sampler3D, Texture3D,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitProfile3D {
    Conservative,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyLimits3D {
    pub max_live_views: u32,
    pub max_active_target_pixels: u64,
    pub max_target_dimension_2d: u32,
    pub max_single_target_attachment_bytes: u64,
    pub max_total_attachment_bytes: u64,
    pub max_mesh_bytes: u64,
    pub max_vertices_per_mesh: u32,
    pub max_indices_per_mesh: u32,
    /// Actual vertex/index buffer allocation bytes retained by the GPU geometry cache.
    pub max_resident_geometry_bytes: u64,
    pub max_texture_dimension_2d: u32,
    pub max_texture_bytes: u64,
    pub max_resident_texture_bytes: u64,
    pub max_draw_packets_per_snapshot: u32,
    /// Canonical source bytes newly materialized during one preparation/frame budget scope.
    pub max_upload_bytes_per_frame: u64,
    /// Retained view/object uniform bytes for one live viewport target.
    pub max_dynamic_storage_bytes_per_target: u64,
    /// Aggregate retained view/object uniform bytes across this context.
    pub max_total_dynamic_storage_bytes: u64,
    /// Aggregate object slots retained by live dynamic-storage allocations.
    pub max_total_dynamic_object_capacity: u64,
    /// Live or transactionally overlapping target-local dynamic-storage slots.
    pub max_dynamic_storage_slots: u32,
    /// Aggregate logical CPU packing/staging bytes retained by dynamic-storage slots.
    pub max_retained_dynamic_cpu_bytes: u64,
    /// View/object uniform bytes written for one target preparation.
    pub max_dynamic_upload_bytes_per_frame: u64,
    /// Distinct immutable model-state values simultaneously retained by the context.
    pub max_retained_model_states: u32,
    pub max_resource_slots: u32,
    pub max_retained_generations: u32,
    pub max_appearance_source_bytes: u64,
    pub max_appearance_functions: u32,
    pub max_appearance_nesting: u32,
    pub max_appearance_ir_nodes: u64,
    pub max_registered_appearances: u32,
    pub max_shader_modules: u32,
    pub max_pipeline_variants: u32,
    pub max_pending_requests: u32,
    pub max_pending_publications: u32,
    /// Retained CPU descriptor/source bytes; GPU geometry has its distinct residency budget.
    pub max_cache_residency_bytes: u64,
}

impl PolicyLimits3D {
    pub const fn conservative() -> Self {
        Self {
            max_live_views: 16,
            max_active_target_pixels: 67_108_864,
            max_target_dimension_2d: 8_192,
            max_single_target_attachment_bytes: 536_870_912,
            max_total_attachment_bytes: 1_073_741_824,
            max_mesh_bytes: 67_108_864,
            max_vertices_per_mesh: 1_000_000,
            max_indices_per_mesh: 3_000_000,
            max_resident_geometry_bytes: 536_870_912,
            max_texture_dimension_2d: 8_192,
            max_texture_bytes: 268_435_456,
            max_resident_texture_bytes: 536_870_912,
            max_draw_packets_per_snapshot: 100_000,
            max_upload_bytes_per_frame: 67_108_864,
            max_dynamic_storage_bytes_per_target: 67_108_864,
            max_total_dynamic_storage_bytes: 536_870_912,
            max_total_dynamic_object_capacity: 1_000_000,
            max_dynamic_storage_slots: 64,
            max_retained_dynamic_cpu_bytes: 1_073_741_824,
            max_dynamic_upload_bytes_per_frame: 67_108_864,
            max_retained_model_states: 256,
            max_resource_slots: 65_536,
            max_retained_generations: 131_072,
            max_appearance_source_bytes: 65_536,
            max_appearance_functions: 128,
            max_appearance_nesting: 32,
            max_appearance_ir_nodes: 262_144,
            max_registered_appearances: 256,
            max_shader_modules: 256,
            max_pipeline_variants: 2_048,
            max_pending_requests: 64,
            max_pending_publications: 16,
            max_cache_residency_bytes: 536_870_912,
        }
    }

    fn validate(self) -> Result<(), Context3DError> {
        let values = [
            ("max_live_views", u64::from(self.max_live_views)),
            ("max_active_target_pixels", self.max_active_target_pixels),
            (
                "max_target_dimension_2d",
                u64::from(self.max_target_dimension_2d),
            ),
            (
                "max_single_target_attachment_bytes",
                self.max_single_target_attachment_bytes,
            ),
            (
                "max_total_attachment_bytes",
                self.max_total_attachment_bytes,
            ),
            ("max_mesh_bytes", self.max_mesh_bytes),
            (
                "max_vertices_per_mesh",
                u64::from(self.max_vertices_per_mesh),
            ),
            ("max_indices_per_mesh", u64::from(self.max_indices_per_mesh)),
            (
                "max_resident_geometry_bytes",
                self.max_resident_geometry_bytes,
            ),
            (
                "max_texture_dimension_2d",
                u64::from(self.max_texture_dimension_2d),
            ),
            ("max_texture_bytes", self.max_texture_bytes),
            (
                "max_resident_texture_bytes",
                self.max_resident_texture_bytes,
            ),
            (
                "max_draw_packets_per_snapshot",
                u64::from(self.max_draw_packets_per_snapshot),
            ),
            (
                "max_upload_bytes_per_frame",
                self.max_upload_bytes_per_frame,
            ),
            (
                "max_dynamic_storage_bytes_per_target",
                self.max_dynamic_storage_bytes_per_target,
            ),
            (
                "max_total_dynamic_storage_bytes",
                self.max_total_dynamic_storage_bytes,
            ),
            (
                "max_total_dynamic_object_capacity",
                self.max_total_dynamic_object_capacity,
            ),
            (
                "max_dynamic_storage_slots",
                u64::from(self.max_dynamic_storage_slots),
            ),
            (
                "max_retained_dynamic_cpu_bytes",
                self.max_retained_dynamic_cpu_bytes,
            ),
            (
                "max_dynamic_upload_bytes_per_frame",
                self.max_dynamic_upload_bytes_per_frame,
            ),
            (
                "max_retained_model_states",
                u64::from(self.max_retained_model_states),
            ),
            ("max_resource_slots", u64::from(self.max_resource_slots)),
            (
                "max_retained_generations",
                u64::from(self.max_retained_generations),
            ),
            (
                "max_appearance_source_bytes",
                self.max_appearance_source_bytes,
            ),
            (
                "max_appearance_functions",
                u64::from(self.max_appearance_functions),
            ),
            (
                "max_appearance_nesting",
                u64::from(self.max_appearance_nesting),
            ),
            ("max_appearance_ir_nodes", self.max_appearance_ir_nodes),
            (
                "max_registered_appearances",
                u64::from(self.max_registered_appearances),
            ),
            ("max_shader_modules", u64::from(self.max_shader_modules)),
            (
                "max_pipeline_variants",
                u64::from(self.max_pipeline_variants),
            ),
            ("max_pending_requests", u64::from(self.max_pending_requests)),
            (
                "max_pending_publications",
                u64::from(self.max_pending_publications),
            ),
            ("max_cache_residency_bytes", self.max_cache_residency_bytes),
        ];
        if let Some((field, _)) = values.into_iter().find(|(_, value)| *value == 0) {
            return Err(Context3DError::InvalidConfiguration { field });
        }
        Ok(())
    }

    fn clamped_to(self, hard: DeviceLimits3D) -> Self {
        Self {
            max_target_dimension_2d: self
                .max_target_dimension_2d
                .min(hard.max_texture_dimension_2d),
            max_mesh_bytes: self.max_mesh_bytes.min(hard.max_buffer_size),
            max_vertices_per_mesh: self
                .max_vertices_per_mesh
                .min(u32::try_from(hard.max_buffer_size / 36).unwrap_or(u32::MAX)),
            max_indices_per_mesh: self
                .max_indices_per_mesh
                .min(u32::try_from(hard.max_buffer_size / 4).unwrap_or(u32::MAX)),
            max_texture_dimension_2d: self
                .max_texture_dimension_2d
                .min(hard.max_texture_dimension_2d),
            ..self
        }
    }
}

impl Default for PolicyLimits3D {
    fn default() -> Self {
        Self::conservative()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceLimits3D {
    pub max_texture_dimension_2d: u32,
    pub max_bind_groups: u32,
    pub max_sampled_textures_per_shader_stage: u32,
    pub max_samplers_per_shader_stage: u32,
    pub max_uniform_buffer_binding_size: u64,
    pub max_vertex_buffers: u32,
    pub max_buffer_size: u64,
    pub max_vertex_attributes: u32,
    pub max_vertex_buffer_array_stride: u32,
    pub min_uniform_buffer_offset_alignment: u32,
    pub max_inter_stage_shader_variables: u32,
}

impl DeviceLimits3D {
    fn from_device(device: &wgpu::Device) -> Self {
        let limits = device.limits();
        Self {
            max_texture_dimension_2d: limits.max_texture_dimension_2d,
            max_bind_groups: limits.max_bind_groups,
            max_sampled_textures_per_shader_stage: limits.max_sampled_textures_per_shader_stage,
            max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage,
            max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size,
            max_vertex_buffers: limits.max_vertex_buffers,
            max_buffer_size: limits.max_buffer_size,
            max_vertex_attributes: limits.max_vertex_attributes,
            max_vertex_buffer_array_stride: limits.max_vertex_buffer_array_stride,
            min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment,
            max_inter_stage_shader_variables: limits.max_inter_stage_shader_variables,
        }
    }

    #[cfg(test)]
    pub(crate) const fn baseline_for_test() -> Self {
        Self {
            max_texture_dimension_2d: 8_192,
            max_bind_groups: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_samplers_per_shader_stage: 16,
            max_uniform_buffer_binding_size: 65_536,
            max_vertex_buffers: 8,
            max_buffer_size: 268_435_456,
            max_vertex_attributes: 16,
            max_vertex_buffer_array_stride: 2_048,
            min_uniform_buffer_offset_alignment: 256,
            max_inter_stage_shader_variables: 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectiveLimits3D {
    pub profile: LimitProfile3D,
    pub hard: DeviceLimits3D,
    pub policy: PolicyLimits3D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardLimit3D {
    TextureDimension2D,
    BindGroups,
    SampledTexturesPerShaderStage,
    SamplersPerShaderStage,
    UniformBufferBindingSize,
    VertexBuffers,
    BufferSize,
    VertexAttributes,
    VertexBufferArrayStride,
    UniformBufferOffsetAlignment,
    InterStageShaderVariables,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyBudget3D {
    LiveViews,
    TargetDimension2D,
    ActiveTargetPixels,
    SingleTargetAttachmentBytes,
    TotalAttachmentBytes,
    MeshBytes,
    MeshVertices,
    MeshIndices,
    ResidentGeometryBytes,
    DrawPacketsPerSnapshot,
    UploadBytesPerFrame,
    DynamicStorageBytesPerTarget,
    TotalDynamicStorageBytes,
    TotalDynamicObjectCapacity,
    DynamicStorageSlots,
    RetainedDynamicCpuBytes,
    DynamicUploadBytesPerFrame,
    RetainedModelStates,
    ResourceSlots,
    RetainedGenerations,
    AppearanceSourceBytes,
    RegisteredAppearances,
    ShaderModules,
    PipelineVariants,
    PendingRequests,
    PendingPublications,
    TextureDimension2D,
    TextureBytes,
    ResidentTextureBytes,
    CacheResidencyBytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind3D {
    Geometry,
    Appearance,
    Texture,
    Sampler,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Context3DError {
    #[error("finite 3D context limit {field} must be non-zero")]
    InvalidConfiguration { field: &'static str },
    #[error("hard device limit {limit:?} exceeded: requested {requested}, maximum {maximum}")]
    HardDeviceLimitExceeded {
        limit: HardLimit3D,
        requested: u64,
        maximum: u64,
    },
    #[error("3D policy budget {budget:?} exceeded: requested {requested}, maximum {maximum}")]
    PolicyBudgetExceeded {
        budget: PolicyBudget3D,
        requested: u64,
        maximum: u64,
    },
    #[error("resource handle belongs to another Context3D")]
    ContextMismatch,
    #[error("wrong resource kind: expected {expected:?}, got {actual:?}")]
    WrongResourceKind {
        expected: ResourceKind3D,
        actual: ResourceKind3D,
    },
    #[error("resource slot {slot} does not exist")]
    SlotOutOfRange { slot: u32 },
    #[error("resource slot {slot} generation {generation} has been freed")]
    FreedSlot { slot: u32, generation: u64 },
    #[error("resource slot {slot} generation {requested} is stale; live generation is {live:?}")]
    StaleGeneration {
        slot: u32,
        requested: u64,
        live: Option<u64>,
    },
    #[error("resource slot {slot} exhausted its non-wrapping generation space")]
    GenerationExhausted { slot: u32 },
    #[error("checked arithmetic overflow while {operation}")]
    ArithmeticOverflow { operation: &'static str },
    #[error("canonical geometry packing failed: {0}")]
    GeometryPacking(#[from] Geometry3DError),
    #[error("appearance admission failed: {0}")]
    AppearanceAdmission(#[from] Appearance3DAdmissionError),
    #[error(
        "appearance requires unavailable geometry semantics: required {required:?}, available {available:?}"
    )]
    MissingGeometrySemantics {
        required: GeometrySemantics,
        available: GeometrySemantics,
    },
    #[error("GPU geometry materialization requires a live GpuResources binding")]
    GpuMaterializationUnavailable,
    #[error("3D context resource registry is unavailable")]
    RegistryUnavailable,
    #[error("3D geometry cache is unavailable")]
    GeometryCacheUnavailable,
    #[error("3D appearance cache is unavailable")]
    AppearanceCacheUnavailable,
    #[error("3D texture cache is unavailable")]
    TextureCacheUnavailable,
    #[error("3D sampler cache is unavailable")]
    SamplerCacheUnavailable,
    #[error("3D raster cache is unavailable")]
    RasterCacheUnavailable,
    #[error("3D pipeline cache is unavailable")]
    PipelineCacheUnavailable,
    #[error("3D viewport accounting is unavailable")]
    ViewportAccountingUnavailable,
    #[error("3D dynamic-state accounting is unavailable")]
    DynamicStateAccountingUnavailable,
    #[error("3D pending-work accounting is unavailable")]
    PendingWorkAccountingUnavailable,
}

#[derive(Clone)]
pub(super) struct ContextId(Arc<ContextIdentity>);

#[derive(Debug)]
struct ContextIdentity;

impl ContextId {
    fn new() -> Self {
        Self(Arc::new(ContextIdentity))
    }

    pub(super) fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn token(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

#[derive(Default)]
struct ViewportAccountingState {
    live_views: u32,
    active_target_pixels: u64,
    attachment_bytes: u64,
}

#[derive(Default)]
struct ViewportAccounting {
    state: Mutex<ViewportAccountingState>,
}

#[derive(Default)]
struct DynamicStateAccountingState {
    dynamic_storage_bytes: u64,
    dynamic_storage_bytes_high_water: u64,
    dynamic_object_capacity: u64,
    dynamic_object_capacity_high_water: u64,
    dynamic_storage_slots: u32,
    dynamic_storage_slots_high_water: u32,
    retained_cpu_bytes: u64,
    retained_cpu_bytes_high_water: u64,
}

#[derive(Default)]
struct DynamicStateAccounting {
    state: Mutex<DynamicStateAccountingState>,
    retained_model_states: AtomicU32,
    retained_model_states_high_water: AtomicU32,
    storage_release_revision: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DomainRetentionKind3 {
    Composition,
    State,
    Frame,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainRetentionBudget3D {
    LiveDomainCompositions,
    LiveDomainStates,
    LiveDomainFrames,
    RetainedDomainMetadataBytes,
}

#[non_exhaustive]
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainRetentionError3D {
    #[error("finite 3D domain-retention limit {field} must be non-zero")]
    InvalidConfiguration { field: &'static str },
    #[error(
        "3D domain-retention budget {budget:?} exceeded: requested {requested}, maximum {maximum}"
    )]
    BudgetExceeded {
        budget: DomainRetentionBudget3D,
        requested: u64,
        maximum: u64,
    },
    #[error("checked arithmetic overflow while {operation}")]
    ArithmeticOverflow { operation: &'static str },
    #[error("3D domain-metadata accounting is unavailable")]
    AccountingUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainRetentionLimits3D {
    max_live_domain_compositions: u32,
    max_live_domain_states: u32,
    max_live_domain_frames: u32,
    max_retained_domain_metadata_bytes: u64,
}

impl DomainRetentionLimits3D {
    pub const fn conservative() -> Self {
        Self {
            max_live_domain_compositions: 256,
            max_live_domain_states: 1_024,
            max_live_domain_frames: 256,
            max_retained_domain_metadata_bytes: 67_108_864,
        }
    }

    pub fn try_new(
        max_live_domain_compositions: u32,
        max_live_domain_states: u32,
        max_live_domain_frames: u32,
        max_retained_domain_metadata_bytes: u64,
    ) -> Result<Self, DomainRetentionError3D> {
        let values = [
            (
                "max_live_domain_compositions",
                u64::from(max_live_domain_compositions),
            ),
            ("max_live_domain_states", u64::from(max_live_domain_states)),
            ("max_live_domain_frames", u64::from(max_live_domain_frames)),
            (
                "max_retained_domain_metadata_bytes",
                max_retained_domain_metadata_bytes,
            ),
        ];
        if let Some((field, _)) = values.into_iter().find(|(_, value)| *value == 0) {
            return Err(DomainRetentionError3D::InvalidConfiguration { field });
        }
        Ok(Self {
            max_live_domain_compositions,
            max_live_domain_states,
            max_live_domain_frames,
            max_retained_domain_metadata_bytes,
        })
    }

    pub const fn max_live_domain_compositions(self) -> u32 {
        self.max_live_domain_compositions
    }

    pub const fn max_live_domain_states(self) -> u32 {
        self.max_live_domain_states
    }

    pub const fn max_live_domain_frames(self) -> u32 {
        self.max_live_domain_frames
    }

    pub const fn max_retained_domain_metadata_bytes(self) -> u64 {
        self.max_retained_domain_metadata_bytes
    }
}

impl Default for DomainRetentionLimits3D {
    fn default() -> Self {
        Self::conservative()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainRetentionUsage3D {
    live_domain_compositions: u32,
    live_domain_compositions_high_water: u32,
    live_domain_states: u32,
    live_domain_states_high_water: u32,
    live_domain_frames: u32,
    live_domain_frames_high_water: u32,
    retained_domain_metadata_bytes: u64,
    retained_domain_metadata_bytes_high_water: u64,
}

impl DomainRetentionUsage3D {
    pub const fn live_domain_compositions(self) -> u32 {
        self.live_domain_compositions
    }

    pub const fn live_domain_compositions_high_water(self) -> u32 {
        self.live_domain_compositions_high_water
    }

    pub const fn live_domain_states(self) -> u32 {
        self.live_domain_states
    }

    pub const fn live_domain_states_high_water(self) -> u32 {
        self.live_domain_states_high_water
    }

    pub const fn live_domain_frames(self) -> u32 {
        self.live_domain_frames
    }

    pub const fn live_domain_frames_high_water(self) -> u32 {
        self.live_domain_frames_high_water
    }

    pub const fn retained_domain_metadata_bytes(self) -> u64 {
        self.retained_domain_metadata_bytes
    }

    pub const fn retained_domain_metadata_bytes_high_water(self) -> u64 {
        self.retained_domain_metadata_bytes_high_water
    }
}

#[derive(Default)]
struct DomainMetadataAccountingState {
    live_compositions: u32,
    live_compositions_high_water: u32,
    live_states: u32,
    live_states_high_water: u32,
    live_frames: u32,
    live_frames_high_water: u32,
    retained_bytes: u64,
    retained_bytes_high_water: u64,
}

#[derive(Default)]
struct DomainMetadataAccounting {
    state: Mutex<DomainMetadataAccountingState>,
}

pub(crate) struct DomainMetadataReservation3D<'a> {
    accounting: &'a Arc<DomainMetadataAccounting>,
    state: MutexGuard<'a, DomainMetadataAccountingState>,
    kind: DomainRetentionKind3,
    requested_count: u32,
    requested_bytes: u64,
    retained_bytes: u64,
}

impl DomainMetadataReservation3D<'_> {
    pub(crate) fn commit(mut self) -> DomainMetadataLease3D {
        match self.kind {
            DomainRetentionKind3::Composition => {
                self.state.live_compositions = self.requested_count;
                self.state.live_compositions_high_water = self
                    .state
                    .live_compositions_high_water
                    .max(self.requested_count);
            }
            DomainRetentionKind3::State => {
                self.state.live_states = self.requested_count;
                self.state.live_states_high_water =
                    self.state.live_states_high_water.max(self.requested_count);
            }
            DomainRetentionKind3::Frame => {
                self.state.live_frames = self.requested_count;
                self.state.live_frames_high_water =
                    self.state.live_frames_high_water.max(self.requested_count);
            }
        }
        self.state.retained_bytes = self.requested_bytes;
        self.state.retained_bytes_high_water = self
            .state
            .retained_bytes_high_water
            .max(self.requested_bytes);
        DomainMetadataLease3D {
            accounting: Arc::clone(self.accounting),
            kind: self.kind,
            retained_bytes: self.retained_bytes,
        }
    }
}

pub(crate) struct DomainMetadataLease3D {
    accounting: Arc<DomainMetadataAccounting>,
    kind: DomainRetentionKind3,
    retained_bytes: u64,
}

impl Drop for DomainMetadataLease3D {
    fn drop(&mut self) {
        let mut state = match self.accounting.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        match self.kind {
            DomainRetentionKind3::Composition => {
                debug_assert!(state.live_compositions > 0);
                state.live_compositions = state.live_compositions.saturating_sub(1);
            }
            DomainRetentionKind3::State => {
                debug_assert!(state.live_states > 0);
                state.live_states = state.live_states.saturating_sub(1);
            }
            DomainRetentionKind3::Frame => {
                debug_assert!(state.live_frames > 0);
                state.live_frames = state.live_frames.saturating_sub(1);
            }
        }
        debug_assert!(state.retained_bytes >= self.retained_bytes);
        state.retained_bytes = state.retained_bytes.saturating_sub(self.retained_bytes);
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DynamicStateAccountingSnapshot3D {
    pub dynamic_storage_bytes: u64,
    pub dynamic_storage_bytes_high_water: u64,
    pub dynamic_object_capacity: u64,
    pub dynamic_object_capacity_high_water: u64,
    pub dynamic_storage_slots: u32,
    pub dynamic_storage_slots_high_water: u32,
    pub retained_cpu_bytes: u64,
    pub retained_cpu_bytes_high_water: u64,
    pub retained_model_states: u32,
    pub retained_model_states_high_water: u32,
}

#[derive(Default)]
struct PendingWorkAccountingState {
    requests: u32,
    publications: u32,
}

#[derive(Default)]
struct PendingWorkAccounting {
    state: Mutex<PendingWorkAccountingState>,
}

pub(crate) struct PendingRequestLease3D {
    accounting: Arc<PendingWorkAccounting>,
}

impl Drop for PendingRequestLease3D {
    fn drop(&mut self) {
        let mut state = match self.accounting.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.requests = state.requests.saturating_sub(1);
    }
}

pub(crate) struct PendingPublicationLease3D {
    accounting: Arc<PendingWorkAccounting>,
}

impl Drop for PendingPublicationLease3D {
    fn drop(&mut self) {
        let mut state = match self.accounting.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.publications = state.publications.saturating_sub(1);
    }
}

pub(super) struct ViewLease3D {
    accounting: Arc<ViewportAccounting>,
}

impl Drop for ViewLease3D {
    fn drop(&mut self) {
        let mut state = match self.accounting.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.live_views = state.live_views.saturating_sub(1);
    }
}

pub(super) struct TargetBudgetLease3D {
    accounting: Arc<ViewportAccounting>,
    active_target_pixels: u64,
    attachment_bytes: u64,
}

impl TargetBudgetLease3D {
    pub(super) fn split_attachment(&mut self, bytes: u64) -> Self {
        self.attachment_bytes = self
            .attachment_bytes
            .checked_sub(bytes)
            .expect("partition of admitted attachment bytes");
        Self {
            accounting: self.accounting.clone(),
            active_target_pixels: 0,
            attachment_bytes: bytes,
        }
    }

    pub(super) fn active_for(&self, context: &Context3D, pixels: u64) -> bool {
        Arc::ptr_eq(&self.accounting, &context.core.viewport_accounting)
            && self.active_target_pixels == pixels
            && self.attachment_bytes == 0
    }
    // Partition one already-admitted complete target; never admit its parts separately.
    pub(super) fn split_hdr(mut self, scene_bytes: u64, final_bytes: u64) -> (Self, Self, Self) {
        assert_eq!(
            scene_bytes.checked_add(final_bytes),
            Some(self.attachment_bytes)
        );
        let active = Self {
            accounting: self.accounting.clone(),
            active_target_pixels: self.active_target_pixels,
            attachment_bytes: 0,
        };
        let scene = Self {
            accounting: self.accounting.clone(),
            active_target_pixels: 0,
            attachment_bytes: scene_bytes,
        };
        let finals = Self {
            accounting: self.accounting.clone(),
            active_target_pixels: 0,
            attachment_bytes: final_bytes,
        };
        self.active_target_pixels = 0;
        self.attachment_bytes = 0;
        (active, scene, finals)
    }
}

impl Drop for TargetBudgetLease3D {
    fn drop(&mut self) {
        let mut state = match self.accounting.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.active_target_pixels = state
            .active_target_pixels
            .checked_sub(self.active_target_pixels)
            .expect("target pixel lease released exactly once");
        state.attachment_bytes = state
            .attachment_bytes
            .checked_sub(self.attachment_bytes)
            .expect("target attachment lease released exactly once");
    }
}

pub(super) struct DynamicStorageLease3D {
    accounting: Arc<DynamicStateAccounting>,
    dynamic_storage_bytes: u64,
    dynamic_object_capacity: u64,
    dynamic_storage_slots: u32,
    retained_cpu_bytes: u64,
}

impl DynamicStorageLease3D {
    /// Divide an existing reservation between independently retained allocations.
    pub(super) fn split_retained_cpu(&mut self, bytes: u64) -> Self {
        self.retained_cpu_bytes = self
            .retained_cpu_bytes
            .checked_sub(bytes)
            .expect("retained CPU allocation was preflighted");
        Self {
            accounting: self.accounting.clone(),
            dynamic_storage_bytes: 0,
            dynamic_object_capacity: 0,
            dynamic_storage_slots: 0,
            retained_cpu_bytes: bytes,
        }
    }
}

// UI-owned retirement of a CPU proof charge. The worker carries only the inert
// ticket, never this node, lease, Context or accounting owner. One node is charged
// and allocated at begin; retirement itself performs no allocation.
pub(super) struct NormalCpuPendingCharge3D {
    ticket: Weak<()>,
    // UI-only escrow preserves the original generation/CPU charge while a
    // worker's plain Arc capture still references the registered source arrays.
    geometry: Vec<ControlledResourceLease3D>,
    next: Option<Box<NormalCpuPendingCharge3D>>,
    _charge: DynamicStorageLease3D,
}

#[derive(Default)]
struct NormalCpuRetirements3D {
    head: Option<Box<NormalCpuPendingCharge3D>>,
}

impl NormalCpuRetirements3D {
    fn collect(&mut self) {
        let mut at = self.head.take();
        while let Some(mut node) = at {
            at = node.next.take();
            if node.ticket.strong_count() != 0 {
                node.next = self.head.take();
                self.head = Some(node);
            }
        }
    }
}
impl Drop for NormalCpuRetirements3D {
    fn drop(&mut self) {
        while let Some(mut node) = self.head.take() {
            self.head = node.next.take();
        }
    }
}

impl Drop for DynamicStorageLease3D {
    fn drop(&mut self) {
        let mut state = match self.accounting.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.dynamic_storage_bytes = state
            .dynamic_storage_bytes
            .saturating_sub(self.dynamic_storage_bytes);
        state.dynamic_object_capacity = state
            .dynamic_object_capacity
            .saturating_sub(self.dynamic_object_capacity);
        state.dynamic_storage_slots = state
            .dynamic_storage_slots
            .saturating_sub(self.dynamic_storage_slots);
        state.retained_cpu_bytes = state
            .retained_cpu_bytes
            .saturating_sub(self.retained_cpu_bytes);
        let _ = self.accounting.storage_release_revision.try_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |revision| revision.checked_add(1),
        );
    }
}

pub(crate) struct ModelStateLease3D {
    accounting: Arc<DynamicStateAccounting>,
}

impl Drop for ModelStateLease3D {
    fn drop(&mut self) {
        let previous = self
            .accounting
            .retained_model_states
            .fetch_sub(1, Ordering::AcqRel);
        debug_assert!(
            previous > 0,
            "a model-state lease must balance one reservation"
        );
    }
}

pub(crate) struct PreparationUploadBudget3D {
    context: ContextId,
    remaining_bytes: u64,
}

impl PreparationUploadBudget3D {
    fn remaining_after(&self, upload_bytes: u64) -> Result<u64, Context3DError> {
        self.remaining_bytes
            .checked_sub(upload_bytes)
            .ok_or(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::UploadBytesPerFrame,
                requested: upload_bytes,
                maximum: self.remaining_bytes,
            })
    }

    pub(crate) fn consume(&mut self, upload_bytes: u64) -> Result<(), Context3DError> {
        self.remaining_bytes = self.remaining_after(upload_bytes)?;
        Ok(())
    }

    pub(crate) const fn remaining_bytes(&self) -> u64 {
        self.remaining_bytes
    }
}

#[derive(Clone)]
struct ResourceKey {
    context: ContextId,
    kind: ResourceKind3D,
    slot: u32,
    generation: u64,
}

impl ResourceKey {
    fn same_as(&self, other: &Self) -> bool {
        self.context.same_as(&other.context)
            && self.kind == other.kind
            && self.slot == other.slot
            && self.generation == other.generation
    }
}

macro_rules! typed_handle {
    ($name:ident, $kind:expr) => {
        #[derive(Clone)]
        pub struct $name {
            key: ResourceKey,
        }

        impl $name {
            pub const fn slot(&self) -> u32 {
                self.key.slot
            }

            pub const fn generation(&self) -> u64 {
                self.key.generation
            }

            pub const fn kind(&self) -> ResourceKind3D {
                $kind
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("kind", &$kind)
                    .field("slot", &self.key.slot)
                    .field("generation", &self.key.generation)
                    .finish_non_exhaustive()
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.key.same_as(&other.key)
            }
        }

        impl Eq for $name {}
    };
}

typed_handle!(GeometryHandle, ResourceKind3D::Geometry);
typed_handle!(AppearanceHandle, ResourceKind3D::Appearance);
typed_handle!(TextureHandle, ResourceKind3D::Texture);
typed_handle!(SamplerHandle, ResourceKind3D::Sampler);

#[derive(Clone)]
pub struct GeometryLease {
    handle: GeometryHandle,
    descriptor: GeometrySourceDescriptor,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl GeometryLease {
    fn try_new(
        handle: GeometryHandle,
        ownership: Arc<ResourceLeaseOwnership>,
    ) -> Result<Self, Context3DError> {
        let descriptor = match &ownership.record().payload {
            ResourcePayload::Geometry(descriptor) => descriptor.clone(),
            payload => {
                return Err(Context3DError::WrongResourceKind {
                    expected: ResourceKind3D::Geometry,
                    actual: payload.kind(),
                });
            }
        };
        Ok(Self {
            handle,
            descriptor,
            _ownership: ownership,
        })
    }

    pub const fn handle(&self) -> &GeometryHandle {
        &self.handle
    }

    pub fn descriptor(&self) -> &GeometrySourceDescriptor {
        &self.descriptor
    }
}

impl fmt::Debug for GeometryLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeometryLease")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct TextureLease {
    handle: TextureHandle,
    descriptor: Texture3D,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl TextureLease {
    fn try_new(
        handle: TextureHandle,
        ownership: Arc<ResourceLeaseOwnership>,
    ) -> Result<Self, Context3DError> {
        let descriptor = match &ownership.record().payload {
            ResourcePayload::Texture(descriptor) => descriptor.clone(),
            payload => {
                return Err(Context3DError::WrongResourceKind {
                    expected: ResourceKind3D::Texture,
                    actual: payload.kind(),
                });
            }
        };
        Ok(Self {
            handle,
            descriptor,
            _ownership: ownership,
        })
    }

    pub const fn handle(&self) -> &TextureHandle {
        &self.handle
    }

    pub fn descriptor(&self) -> &Texture3D {
        &self.descriptor
    }
}

impl fmt::Debug for TextureLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextureLease")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct SamplerLease {
    handle: SamplerHandle,
    descriptor: Sampler3D,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl SamplerLease {
    fn try_new(
        handle: SamplerHandle,
        ownership: Arc<ResourceLeaseOwnership>,
    ) -> Result<Self, Context3DError> {
        let descriptor = match &ownership.record().payload {
            ResourcePayload::Sampler(descriptor) => *descriptor,
            payload => {
                return Err(Context3DError::WrongResourceKind {
                    expected: ResourceKind3D::Sampler,
                    actual: payload.kind(),
                });
            }
        };
        Ok(Self {
            handle,
            descriptor,
            _ownership: ownership,
        })
    }

    pub const fn handle(&self) -> &SamplerHandle {
        &self.handle
    }

    pub const fn descriptor(&self) -> Sampler3D {
        self.descriptor
    }
}

impl fmt::Debug for SamplerLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SamplerLease")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct AppearanceResource {
    descriptor: AppearanceSourceDescriptor,
    texture: Option<TextureLease>,
    sampler: Option<SamplerLease>,
}

#[derive(Clone)]
pub struct AppearanceLease {
    handle: AppearanceHandle,
    descriptor: AppearanceSourceDescriptor,
    texture: Option<TextureLease>,
    sampler: Option<SamplerLease>,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl AppearanceLease {
    fn try_new(
        handle: AppearanceHandle,
        ownership: Arc<ResourceLeaseOwnership>,
    ) -> Result<Self, Context3DError> {
        let resource = match &ownership.record().payload {
            ResourcePayload::Appearance(resource) => resource.clone(),
            payload => {
                return Err(Context3DError::WrongResourceKind {
                    expected: ResourceKind3D::Appearance,
                    actual: payload.kind(),
                });
            }
        };
        Ok(Self {
            handle,
            descriptor: resource.descriptor,
            texture: resource.texture,
            sampler: resource.sampler,
            _ownership: ownership,
        })
    }

    pub const fn handle(&self) -> &AppearanceHandle {
        &self.handle
    }

    pub fn descriptor(&self) -> &AppearanceSourceDescriptor {
        &self.descriptor
    }

    pub const fn texture(&self) -> Option<&TextureLease> {
        self.texture.as_ref()
    }

    pub const fn sampler(&self) -> Option<&SamplerLease> {
        self.sampler.as_ref()
    }
}

impl fmt::Debug for AppearanceLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppearanceLease")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResourceUsage3D {
    pub live_resource_slots: u32,
    pub retained_generations: u32,
    /// Actual vertex/index buffer allocation bytes currently owned by GPU cache entries.
    pub resident_geometry_bytes: u64,
    pub retained_appearance_source_bytes: u64,
    pub retained_texture_source_bytes: u64,
    pub retained_sampler_descriptor_bytes: u64,
    pub registered_appearances: u32,
    /// Canonical vertex plus admitted fragment modules retained by this context.
    pub admitted_shader_modules: u32,
    /// Context-owned fallback and appearance textures currently resident on the GPU.
    pub resident_texture_bytes: u64,
    /// Render pipelines currently retained by the finite context cache.
    pub resident_pipeline_variants: u32,
    pub pending_requests: u32,
    pub pending_publications: u32,
    /// Retained CPU geometry and appearance source bytes across all live generations.
    pub retained_cpu_descriptor_bytes: u64,
}

#[derive(Clone)]
enum ContextGpuBinding {
    Live {
        gpu: GpuResources,
        device_lifetime: DeviceLifetimeObservation,
    },
    #[cfg(test)]
    CpuOnly {
        format: wgpu::TextureFormat,
        _device_identity: Arc<ContextIdentity>,
        _queue_identity: Arc<ContextIdentity>,
    },
}

enum ContextGpuBindingRequest {
    Live(GpuResources),
    #[cfg(test)]
    CpuOnly {
        format: wgpu::TextureFormat,
        device_identity: Arc<ContextIdentity>,
        queue_identity: Arc<ContextIdentity>,
    },
}

impl ContextGpuBindingRequest {
    fn admit(self) -> ContextGpuBinding {
        match self {
            Self::Live(gpu) => ContextGpuBinding::Live {
                device_lifetime: gpu.observe_device_lifetime(),
                gpu,
            },
            #[cfg(test)]
            Self::CpuOnly {
                format,
                device_identity,
                queue_identity,
            } => ContextGpuBinding::CpuOnly {
                format,
                _device_identity: device_identity,
                _queue_identity: queue_identity,
            },
        }
    }
}

impl ContextGpuBinding {
    fn format(&self) -> wgpu::TextureFormat {
        match self {
            Self::Live { gpu, .. } => gpu.format,
            #[cfg(test)]
            Self::CpuOnly { format, .. } => *format,
        }
    }

    fn matches(&self, gpu: &GpuResources) -> bool {
        match self {
            Self::Live { gpu: captured, .. } => {
                Arc::ptr_eq(&captured.device, &gpu.device)
                    && Arc::ptr_eq(&captured.queue, &gpu.queue)
                    && captured.format == gpu.format
            }
            #[cfg(test)]
            Self::CpuOnly { .. } => false,
        }
    }

    fn gpu(&self) -> Result<&GpuResources, Context3DError> {
        match self {
            Self::Live { gpu, .. } => Ok(gpu),
            #[cfg(test)]
            Self::CpuOnly { .. } => Err(Context3DError::GpuMaterializationUnavailable),
        }
    }

    fn device_failure(&self) -> Option<GpuDeviceFailure> {
        match self {
            Self::Live {
                device_lifetime, ..
            } => device_lifetime.failure(),
            #[cfg(test)]
            Self::CpuOnly { .. } => None,
        }
    }
}

struct GeometryGpuAccounting {
    materialization_gate: Mutex<()>,
    resident_bytes: AtomicU64,
}

impl GeometryGpuAccounting {
    fn new() -> Self {
        Self {
            materialization_gate: Mutex::new(()),
            resident_bytes: AtomicU64::new(0),
        }
    }

    fn resident_bytes(&self) -> u64 {
        self.resident_bytes.load(Ordering::Acquire)
    }

    fn reserve_resident_bytes(
        self: &Arc<Self>,
        bytes: u64,
        maximum: u64,
    ) -> Result<GeometryGpuReservation<'_>, Context3DError> {
        let gate = self
            .materialization_gate
            .lock()
            .map_err(|_| Context3DError::GeometryCacheUnavailable)?;
        let requested_resident_bytes = self.resident_bytes().checked_add(bytes).ok_or(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ResidentGeometryBytes,
                requested: u64::MAX,
                maximum,
            },
        )?;
        validate_policy_budget(
            PolicyBudget3D::ResidentGeometryBytes,
            requested_resident_bytes,
            maximum,
        )?;
        Ok(GeometryGpuReservation {
            accounting: self,
            _gate: gate,
            resident_bytes: bytes,
            requested_resident_bytes,
        })
    }

    fn remove_resident_bytes(&self, bytes: u64) {
        let _gate = match self.materialization_gate.lock() {
            Ok(gate) => gate,
            Err(poisoned) => poisoned.into_inner(),
        };
        let resident_bytes = self.resident_bytes();
        debug_assert!(
            resident_bytes >= bytes,
            "GPU geometry residency release exceeds committed residency"
        );
        if let Some(remaining_bytes) = resident_bytes.checked_sub(bytes) {
            self.resident_bytes
                .store(remaining_bytes, Ordering::Release);
        }
    }
}

pub(super) struct GeometryGpuReservation<'a> {
    accounting: &'a Arc<GeometryGpuAccounting>,
    _gate: MutexGuard<'a, ()>,
    resident_bytes: u64,
    requested_resident_bytes: u64,
}

impl GeometryGpuReservation<'_> {
    pub(super) fn commit(self) -> GeometryGpuResidency {
        let residency = GeometryGpuResidency {
            accounting: self.accounting.clone(),
            resident_bytes: self.resident_bytes,
        };
        self.accounting
            .resident_bytes
            .store(self.requested_resident_bytes, Ordering::Release);
        residency
    }
}

pub(super) struct GeometryGpuResidency {
    accounting: Arc<GeometryGpuAccounting>,
    resident_bytes: u64,
}

impl Drop for GeometryGpuResidency {
    fn drop(&mut self) {
        self.accounting.remove_resident_bytes(self.resident_bytes);
    }
}

struct TextureGpuAccounting {
    materialization_gate: Mutex<()>,
    resident_bytes: AtomicU64,
}

impl TextureGpuAccounting {
    fn new() -> Self {
        Self {
            materialization_gate: Mutex::new(()),
            resident_bytes: AtomicU64::new(0),
        }
    }

    fn resident_bytes(&self) -> u64 {
        self.resident_bytes.load(Ordering::Acquire)
    }

    fn reserve_resident_bytes(
        self: &Arc<Self>,
        bytes: u64,
        fallback_bytes: u64,
        maximum: u64,
    ) -> Result<TextureGpuReservation<'_>, Context3DError> {
        let gate = self
            .materialization_gate
            .lock()
            .map_err(|_| Context3DError::TextureCacheUnavailable)?;
        let requested_resource_bytes = self.resident_bytes().checked_add(bytes).ok_or(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ResidentTextureBytes,
                requested: u64::MAX,
                maximum,
            },
        )?;
        let requested_total_bytes = fallback_bytes.checked_add(requested_resource_bytes).ok_or(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ResidentTextureBytes,
                requested: u64::MAX,
                maximum,
            },
        )?;
        validate_policy_budget(
            PolicyBudget3D::ResidentTextureBytes,
            requested_total_bytes,
            maximum,
        )?;
        Ok(TextureGpuReservation {
            accounting: self,
            _gate: gate,
            resident_bytes: bytes,
            requested_resource_bytes,
        })
    }

    fn remove_resident_bytes(&self, bytes: u64) {
        let _gate = match self.materialization_gate.lock() {
            Ok(gate) => gate,
            Err(poisoned) => poisoned.into_inner(),
        };
        let resident_bytes = self.resident_bytes();
        debug_assert!(
            resident_bytes >= bytes,
            "GPU texture residency release exceeds committed residency"
        );
        if let Some(remaining_bytes) = resident_bytes.checked_sub(bytes) {
            self.resident_bytes
                .store(remaining_bytes, Ordering::Release);
        }
    }
}

struct TextureGpuReservation<'a> {
    accounting: &'a Arc<TextureGpuAccounting>,
    _gate: MutexGuard<'a, ()>,
    resident_bytes: u64,
    requested_resource_bytes: u64,
}

impl TextureGpuReservation<'_> {
    fn commit(self) -> TextureGpuResidency {
        let residency = TextureGpuResidency {
            accounting: self.accounting.clone(),
            resident_bytes: self.resident_bytes,
        };
        self.accounting
            .resident_bytes
            .store(self.requested_resource_bytes, Ordering::Release);
        residency
    }
}

pub(super) struct TextureGpuResidency {
    accounting: Arc<TextureGpuAccounting>,
    resident_bytes: u64,
}

impl Drop for TextureGpuResidency {
    fn drop(&mut self) {
        self.accounting.remove_resident_bytes(self.resident_bytes);
    }
}

struct ShaderModuleAccounting {
    admission_gate: Mutex<()>,
    resident_modules: AtomicU32,
}

impl ShaderModuleAccounting {
    fn new() -> Self {
        Self {
            admission_gate: Mutex::new(()),
            resident_modules: AtomicU32::new(0),
        }
    }

    fn resident_modules(&self) -> u32 {
        self.resident_modules.load(Ordering::Acquire)
    }

    fn reserve_modules(
        self: &Arc<Self>,
        modules: u32,
        maximum: u32,
    ) -> Result<ShaderModuleReservation<'_>, Context3DError> {
        let gate = self
            .admission_gate
            .lock()
            .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
        let requested_modules = self.resident_modules().checked_add(modules).ok_or(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ShaderModules,
                requested: u64::MAX,
                maximum: u64::from(maximum),
            },
        )?;
        validate_policy_budget(
            PolicyBudget3D::ShaderModules,
            u64::from(requested_modules),
            u64::from(maximum),
        )?;
        Ok(ShaderModuleReservation {
            accounting: self,
            _gate: gate,
            modules,
            requested_modules,
        })
    }

    fn remove_modules(&self, modules: u32) {
        let _gate = match self.admission_gate.lock() {
            Ok(gate) => gate,
            Err(poisoned) => poisoned.into_inner(),
        };
        let resident_modules = self.resident_modules();
        debug_assert!(
            resident_modules >= modules,
            "appearance module release exceeds committed residency"
        );
        if let Some(remaining_modules) = resident_modules.checked_sub(modules) {
            self.resident_modules
                .store(remaining_modules, Ordering::Release);
        }
    }
}

pub(super) struct ShaderModuleReservation<'a> {
    accounting: &'a Arc<ShaderModuleAccounting>,
    _gate: MutexGuard<'a, ()>,
    modules: u32,
    requested_modules: u32,
}

impl ShaderModuleReservation<'_> {
    pub(super) fn commit_all(self) -> ShaderModuleResidency {
        let residency = ShaderModuleResidency {
            accounting: self.accounting.clone(),
            modules: self.modules,
        };
        self.accounting
            .resident_modules
            .store(self.requested_modules, Ordering::Release);
        residency
    }
    pub(super) fn commit_one(self) -> ShaderModuleResidency {
        debug_assert_eq!(self.modules, 1);
        let residency = ShaderModuleResidency {
            accounting: self.accounting.clone(),
            modules: 1,
        };
        self.accounting
            .resident_modules
            .store(self.requested_modules, Ordering::Release);
        residency
    }

    fn commit_pair(self) -> (ShaderModuleResidency, ShaderModuleResidency) {
        debug_assert_eq!(self.modules, 2);
        let first = ShaderModuleResidency {
            accounting: self.accounting.clone(),
            modules: 1,
        };
        let second = ShaderModuleResidency {
            accounting: self.accounting.clone(),
            modules: 1,
        };
        self.accounting
            .resident_modules
            .store(self.requested_modules, Ordering::Release);
        (first, second)
    }
}

pub(super) struct ShaderModuleResidency {
    accounting: Arc<ShaderModuleAccounting>,
    modules: u32,
}

impl Drop for ShaderModuleResidency {
    fn drop(&mut self) {
        self.accounting.remove_modules(self.modules);
    }
}

struct Context3DCore {
    id: ContextId,
    binding: ContextGpuBinding,
    limits: EffectiveLimits3D,
    domain_retention_limits: DomainRetentionLimits3D,
    abi_layout: AbiLayout3D,
    resources: Mutex<ResourceRegistry>,
    geometry_gpu_accounting: Arc<GeometryGpuAccounting>,
    texture_gpu_accounting: Arc<TextureGpuAccounting>,
    appearance_module_accounting: Arc<ShaderModuleAccounting>,
    canonical_vertex_module: Mutex<Option<Arc<CanonicalVertexModule>>>,
    raster_gpu: Mutex<Option<Arc<GpuState3D>>>,
    pbr_state: Mutex<Weak<super::pbr::PbrState3D>>,
    direct_program: Mutex<Weak<super::raster::PbrProgram3D>>,
    indirect_programs: Mutex<[Weak<super::raster::PbrProgram3D>; 8]>,
    mip_programs: Mutex<[Weak<super::raster::PbrProgram3D>; 8]>,
    reveal_programs: Mutex<[[Weak<super::raster::PbrProgram3D>; 8]; 2]>,
    viewport_accounting: Arc<ViewportAccounting>,
    dynamic_state_accounting: Arc<DynamicStateAccounting>,
    normal_cpu_retirements: Mutex<NormalCpuRetirements3D>,
    pending_work_accounting: Arc<PendingWorkAccounting>,
    domain_metadata_accounting: Arc<DomainMetadataAccounting>,
    #[cfg(test)]
    geometry_materializations: AtomicU64,
}

#[derive(Clone)]
pub struct Context3D {
    core: Arc<Context3DCore>,
    #[cfg(target_arch = "wasm32")]
    pub(super) gpu_preparation: Option<Arc<super::gpu_preparation::GpuPreparation3D>>,
}

impl fmt::Debug for Context3D {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Context3D")
            .field("format", &self.surface_format())
            .field("limits", &self.core.limits)
            .field(
                "domain_retention_limits",
                &self.core.domain_retention_limits,
            )
            .finish_non_exhaustive()
    }
}

impl Context3D {
    #[cfg(target_arch = "wasm32")]
    pub(super) fn same_preparation_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.core, &other.core)
    }

    pub fn new(gpu: GpuResources) -> Result<Self, Context3DError> {
        Self::new_with_domain_retention_limits(gpu, DomainRetentionLimits3D::conservative())
    }

    pub fn new_with_domain_retention_limits(
        gpu: GpuResources,
        domain_retention_limits: DomainRetentionLimits3D,
    ) -> Result<Self, Context3DError> {
        let hard = DeviceLimits3D::from_device(&gpu.device);
        validate_baseline_device(hard)?;
        let policy = PolicyLimits3D::conservative().clamped_to(hard);
        policy.validate()?;
        Self::from_parts(
            ContextGpuBindingRequest::Live(gpu),
            EffectiveLimits3D {
                profile: LimitProfile3D::Conservative,
                hard,
                policy,
            },
            domain_retention_limits,
            u64::MAX,
        )
    }

    pub fn with_policy(gpu: GpuResources, policy: PolicyLimits3D) -> Result<Self, Context3DError> {
        Self::with_policy_and_domain_retention_limits(
            gpu,
            policy,
            DomainRetentionLimits3D::conservative(),
        )
    }

    pub fn with_policy_and_domain_retention_limits(
        gpu: GpuResources,
        policy: PolicyLimits3D,
        domain_retention_limits: DomainRetentionLimits3D,
    ) -> Result<Self, Context3DError> {
        policy.validate()?;
        let hard = DeviceLimits3D::from_device(&gpu.device);
        validate_baseline_device(hard)?;
        validate_custom_policy_against_device(policy, hard)?;
        Self::from_parts(
            ContextGpuBindingRequest::Live(gpu),
            EffectiveLimits3D {
                profile: LimitProfile3D::Custom,
                hard,
                policy,
            },
            domain_retention_limits,
            u64::MAX,
        )
    }

    fn from_parts(
        binding: ContextGpuBindingRequest,
        limits: EffectiveLimits3D,
        domain_retention_limits: DomainRetentionLimits3D,
        generation_ceiling: u64,
    ) -> Result<Self, Context3DError> {
        if generation_ceiling == 0 {
            return Err(Context3DError::InvalidConfiguration {
                field: "generation_ceiling",
            });
        }
        let id = ContextId::new();
        let abi_layout = AbiLayout3D::try_new(limits.hard.min_uniform_buffer_offset_alignment)?;
        let binding = binding.admit();
        let resources = ResourceRegistry::new(id.clone(), limits, generation_ceiling);
        let context = Self {
            #[cfg(target_arch = "wasm32")]
            gpu_preparation: None,
            core: Arc::new(Context3DCore {
                id,
                binding,
                limits,
                domain_retention_limits,
                abi_layout,
                resources: Mutex::new(resources),
                geometry_gpu_accounting: Arc::new(GeometryGpuAccounting::new()),
                texture_gpu_accounting: Arc::new(TextureGpuAccounting::new()),
                appearance_module_accounting: Arc::new(ShaderModuleAccounting::new()),
                canonical_vertex_module: Mutex::new(None),
                raster_gpu: Mutex::new(None),
                pbr_state: Mutex::new(Weak::new()),
                direct_program: Mutex::new(Weak::new()),
                indirect_programs: Mutex::new(std::array::from_fn(|_| Weak::new())),
                mip_programs: Mutex::new(std::array::from_fn(|_| Weak::new())),
                reveal_programs: Mutex::new(std::array::from_fn(|_| {
                    std::array::from_fn(|_| Weak::new())
                })),
                viewport_accounting: Arc::new(ViewportAccounting::default()),
                dynamic_state_accounting: Arc::new(DynamicStateAccounting::default()),
                normal_cpu_retirements: Mutex::new(NormalCpuRetirements3D::default()),
                pending_work_accounting: Arc::new(PendingWorkAccounting::default()),
                domain_metadata_accounting: Arc::new(DomainMetadataAccounting::default()),
                #[cfg(test)]
                geometry_materializations: AtomicU64::new(0),
            }),
        };
        #[cfg(test)]
        super::audit::record_context();
        Ok(context)
    }

    pub fn limits(&self) -> &EffectiveLimits3D {
        &self.core.limits
    }

    pub fn domain_retention_limits(&self) -> DomainRetentionLimits3D {
        self.core.domain_retention_limits
    }

    pub fn domain_retention_usage(&self) -> Result<DomainRetentionUsage3D, DomainRetentionError3D> {
        let state = self
            .core
            .domain_metadata_accounting
            .state
            .lock()
            .map_err(|_| DomainRetentionError3D::AccountingUnavailable)?;
        Ok(DomainRetentionUsage3D {
            live_domain_compositions: state.live_compositions,
            live_domain_compositions_high_water: state.live_compositions_high_water,
            live_domain_states: state.live_states,
            live_domain_states_high_water: state.live_states_high_water,
            live_domain_frames: state.live_frames,
            live_domain_frames_high_water: state.live_frames_high_water,
            retained_domain_metadata_bytes: state.retained_bytes,
            retained_domain_metadata_bytes_high_water: state.retained_bytes_high_water,
        })
    }

    pub(crate) fn reserve_domain_metadata(
        &self,
        kind: DomainRetentionKind3,
        retained_bytes: u64,
    ) -> Result<DomainMetadataReservation3D<'_>, DomainRetentionError3D> {
        let accounting = &self.core.domain_metadata_accounting;
        let state = accounting
            .state
            .lock()
            .map_err(|_| DomainRetentionError3D::AccountingUnavailable)?;
        let (current_count, maximum_count, budget) = match kind {
            DomainRetentionKind3::Composition => (
                state.live_compositions,
                self.core
                    .domain_retention_limits
                    .max_live_domain_compositions(),
                DomainRetentionBudget3D::LiveDomainCompositions,
            ),
            DomainRetentionKind3::State => (
                state.live_states,
                self.core.domain_retention_limits.max_live_domain_states(),
                DomainRetentionBudget3D::LiveDomainStates,
            ),
            DomainRetentionKind3::Frame => (
                state.live_frames,
                self.core.domain_retention_limits.max_live_domain_frames(),
                DomainRetentionBudget3D::LiveDomainFrames,
            ),
        };
        let requested_count =
            current_count
                .checked_add(1)
                .ok_or(DomainRetentionError3D::ArithmeticOverflow {
                    operation: "counting retained immutable 3D domain values",
                })?;
        validate_domain_retention_budget(
            budget,
            u64::from(requested_count),
            u64::from(maximum_count),
        )?;
        let requested_bytes = state.retained_bytes.checked_add(retained_bytes).ok_or(
            DomainRetentionError3D::ArithmeticOverflow {
                operation: "summing retained 3D domain metadata bytes",
            },
        )?;
        validate_domain_retention_budget(
            DomainRetentionBudget3D::RetainedDomainMetadataBytes,
            requested_bytes,
            self.core
                .domain_retention_limits
                .max_retained_domain_metadata_bytes(),
        )?;
        Ok(DomainMetadataReservation3D {
            accounting,
            state,
            kind,
            requested_count,
            requested_bytes,
            retained_bytes,
        })
    }

    pub fn abi_layout(&self) -> AbiLayout3D {
        self.core.abi_layout
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.core.binding.format()
    }

    pub fn same_resource_family(&self, other: &Self) -> bool {
        self.core.id.same_as(&other.core.id)
    }

    pub(super) fn normal_cpu_issuer(&self) -> ContextId {
        self.core.id.clone()
    }

    pub(super) fn collect_normal_cpu_charges(&self) {
        let mut retired = match self.core.normal_cpu_retirements.lock() {
            Ok(held) => held,
            Err(poisoned) => poisoned.into_inner(),
        };
        retired.collect();
    }

    pub(super) fn reserve_normal_cpu_charge(
        &self,
        bytes: u64,
        ticket: Weak<()>,
    ) -> Result<Box<NormalCpuPendingCharge3D>, Context3DError> {
        let charge = self.reserve_dynamic_storage(0, 0, 1, bytes)?;
        Ok(Box::new(NormalCpuPendingCharge3D {
            ticket,
            _charge: charge,
            geometry: Vec::new(),
            next: None,
        }))
    }

    pub(super) fn escrow_reveal_geometry(
        &self,
        charge: &mut NormalCpuPendingCharge3D,
        geometry: Vec<ControlledResourceLease3D>,
    ) {
        self.escrow_cpu_geometry(charge, geometry);
    }

    pub(super) fn escrow_cpu_geometry(
        &self,
        charge: &mut NormalCpuPendingCharge3D,
        geometry: Vec<ControlledResourceLease3D>,
    ) {
        charge.geometry = geometry;
    }

    pub(super) fn retire_normal_cpu_charge(&self, mut node: Box<NormalCpuPendingCharge3D>) {
        let mut retired = match self.core.normal_cpu_retirements.lock() {
            Ok(held) => held,
            Err(poisoned) => poisoned.into_inner(),
        };
        node.next = retired.head.take();
        retired.head = Some(node);
        retired.collect();
    }

    pub fn matches_gpu_resources(&self, gpu: &GpuResources) -> bool {
        self.core.binding.matches(gpu)
    }

    pub fn device_failure(&self) -> Option<GpuDeviceFailure> {
        self.core.binding.device_failure()
    }

    pub(super) fn gpu_resources(&self) -> Result<GpuResources, Context3DError> {
        self.core.binding.gpu().cloned()
    }

    #[cfg(test)]
    pub(crate) fn gpu_resources_for_test(&self) -> GpuResources {
        self.gpu_resources().unwrap()
    }

    pub(super) fn reserve_controlled_shader_module(
        &self,
    ) -> Result<ShaderModuleReservation<'_>, Context3DError> {
        self.core
            .appearance_module_accounting
            .reserve_modules(1, self.core.limits.policy.max_shader_modules)
    }

    pub(super) fn reserve_controlled_shader_modules(
        &self,
        count: u32,
    ) -> Result<ShaderModuleReservation<'_>, Context3DError> {
        self.core
            .appearance_module_accounting
            .reserve_modules(count, self.core.limits.policy.max_shader_modules)
    }

    pub(super) fn direct_program(
        &self,
    ) -> super::gpu_preparation::PbrGpuResult3D<Arc<super::raster::PbrProgram3D>> {
        let mut cache = self
            .core
            .direct_program
            .lock()
            .map_err(|_| super::pbr::PbrError3D::Lock("context D program cache"))?;
        if let Some(program) = cache.upgrade() {
            return Ok(program);
        }
        let program = super::raster::PbrProgram3D::new_family(self, true)?;
        *cache = Arc::downgrade(&program);
        Ok(program)
    }

    pub(super) fn indirect_program(
        &self,
        profile: super::indirect::Presence3D,
    ) -> super::gpu_preparation::PbrGpuResult3D<Arc<super::raster::PbrProgram3D>> {
        if profile.0 >= 8 || !profile.indirect() {
            return Err((super::pbr::PbrError3D::Association(
                "invalid private E/G shader profile",
            ))
            .into());
        }
        let mut cache = self
            .core
            .indirect_programs
            .lock()
            .map_err(|_| super::pbr::PbrError3D::Lock("context E/G program cache"))?;
        if let Some(program) = cache[profile.0 as usize].upgrade() {
            return Ok(program);
        }
        let program = super::raster::PbrProgram3D::new_profile(self, profile)?;
        cache[profile.0 as usize] = Arc::downgrade(&program);
        Ok(program)
    }
    pub(super) fn mip_program(
        &self,
        profile: super::indirect::Presence3D,
    ) -> super::gpu_preparation::PbrGpuResult3D<Arc<super::raster::PbrProgram3D>> {
        if profile.0 >= 8 {
            return Err((super::pbr::PbrError3D::Association("invalid V2 D/E/G profile")).into());
        }
        let mut cache = self
            .core
            .mip_programs
            .lock()
            .map_err(|_| super::pbr::PbrError3D::Lock("V2 controlled program cache"))?;
        if let Some(p) = cache[profile.0 as usize].upgrade() {
            return Ok(p);
        }
        let p = super::raster::PbrProgram3D::new_sampling_profile(
            self,
            profile,
            super::mip_sampling::SamplingFamily3D::V2,
        )?;
        cache[profile.0 as usize] = Arc::downgrade(&p);
        Ok(p)
    }

    pub(super) fn reveal_program(
        &self,
        profile: super::indirect::Presence3D,
        sampling: super::mip_sampling::SamplingFamily3D,
    ) -> super::gpu_preparation::PbrGpuResult3D<Arc<super::raster::PbrProgram3D>> {
        if profile.0 >= 8 {
            return Err(
                (super::pbr::PbrError3D::Association("invalid reveal D/E/G profile")).into(),
            );
        }
        let family = usize::from(sampling == super::mip_sampling::SamplingFamily3D::V2);
        let mut cache = self
            .core
            .reveal_programs
            .lock()
            .map_err(|_| super::pbr::PbrError3D::Lock("context reveal program cache"))?;
        if let Some(program) = cache[family][profile.0 as usize].upgrade() {
            return Ok(program);
        }
        let program =
            super::raster::PbrProgram3D::new_reveal_sampling_profile(self, profile, sampling)?;
        cache[family][profile.0 as usize] = Arc::downgrade(&program);
        Ok(program)
    }

    #[cfg(test)]
    pub(crate) fn target_charges_for_test(&self) -> (u64, u64) {
        self.target_budget_usage().unwrap()
    }

    pub(super) fn target_budget_usage(&self) -> Result<(u64, u64), Context3DError> {
        let state = self
            .core
            .viewport_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::ViewportAccountingUnavailable)?;
        Ok((state.active_target_pixels, state.attachment_bytes))
    }

    #[cfg(test)]
    pub(crate) fn split_hdr_charges_for_test(
        &self,
        pixels: u64,
        scene: u64,
        finals: u64,
    ) -> Result<[Box<dyn Send + Sync>; 3], Context3DError> {
        let full = self.reserve_target_budget(
            pixels,
            scene
                .checked_add(finals)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "sizing HDR test reservation",
                })?,
        )?;
        let (active, scene, finals) = full.split_hdr(scene, finals);
        Ok([Box::new(active), Box::new(scene), Box::new(finals)])
    }

    pub(super) fn identity_token(&self) -> usize {
        self.core.id.token()
    }

    pub(super) fn context_identity(&self) -> ContextId {
        self.core.id.clone()
    }

    pub(super) fn reserve_view(&self) -> Result<ViewLease3D, Context3DError> {
        let mut state = self
            .core
            .viewport_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::ViewportAccountingUnavailable)?;
        let requested =
            state
                .live_views
                .checked_add(1)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "counting live 3D viewports",
                })?;
        let maximum = self.core.limits.policy.max_live_views;
        if requested > maximum {
            return Err(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::LiveViews,
                requested: u64::from(requested),
                maximum: u64::from(maximum),
            });
        }
        state.live_views = requested;
        drop(state);
        Ok(ViewLease3D {
            accounting: self.core.viewport_accounting.clone(),
        })
    }

    pub(super) fn reserve_target_budget(
        &self,
        active_target_pixels: u64,
        attachment_bytes: u64,
    ) -> Result<TargetBudgetLease3D, Context3DError> {
        let mut state = self
            .core
            .viewport_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::ViewportAccountingUnavailable)?;
        let requested_pixels = state
            .active_target_pixels
            .checked_add(active_target_pixels)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting active 3D target pixels",
            })?;
        let maximum_pixels = self.core.limits.policy.max_active_target_pixels;
        if requested_pixels > maximum_pixels {
            return Err(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ActiveTargetPixels,
                requested: requested_pixels,
                maximum: maximum_pixels,
            });
        }
        let requested_bytes = state.attachment_bytes.checked_add(attachment_bytes).ok_or(
            Context3DError::ArithmeticOverflow {
                operation: "counting resident 3D target attachment bytes",
            },
        )?;
        let maximum_bytes = self.core.limits.policy.max_total_attachment_bytes;
        if requested_bytes > maximum_bytes {
            return Err(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::TotalAttachmentBytes,
                requested: requested_bytes,
                maximum: maximum_bytes,
            });
        }
        state.active_target_pixels = requested_pixels;
        state.attachment_bytes = requested_bytes;
        drop(state);
        Ok(TargetBudgetLease3D {
            accounting: self.core.viewport_accounting.clone(),
            active_target_pixels,
            attachment_bytes,
        })
    }

    pub(super) fn reserve_dynamic_storage(
        &self,
        dynamic_storage_bytes: u64,
        dynamic_object_capacity: u64,
        dynamic_storage_slots: u32,
        retained_cpu_bytes: u64,
    ) -> Result<DynamicStorageLease3D, Context3DError> {
        self.collect_normal_cpu_charges();
        let mut state = self
            .core
            .dynamic_state_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::DynamicStateAccountingUnavailable)?;
        let requested_storage_bytes = state
            .dynamic_storage_bytes
            .checked_add(dynamic_storage_bytes)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting aggregate 3D dynamic-storage bytes",
            })?;
        let requested_object_capacity = state
            .dynamic_object_capacity
            .checked_add(dynamic_object_capacity)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting aggregate 3D dynamic object capacity",
            })?;
        let requested_slots = state
            .dynamic_storage_slots
            .checked_add(dynamic_storage_slots)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting live 3D dynamic-storage slots",
            })?;
        let requested_cpu_bytes = state
            .retained_cpu_bytes
            .checked_add(retained_cpu_bytes)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting retained 3D dynamic CPU staging bytes",
            })?;

        validate_policy_budget(
            PolicyBudget3D::TotalDynamicStorageBytes,
            requested_storage_bytes,
            self.core.limits.policy.max_total_dynamic_storage_bytes,
        )?;
        validate_policy_budget(
            PolicyBudget3D::TotalDynamicObjectCapacity,
            requested_object_capacity,
            self.core.limits.policy.max_total_dynamic_object_capacity,
        )?;
        validate_policy_budget(
            PolicyBudget3D::DynamicStorageSlots,
            u64::from(requested_slots),
            u64::from(self.core.limits.policy.max_dynamic_storage_slots),
        )?;
        validate_policy_budget(
            PolicyBudget3D::RetainedDynamicCpuBytes,
            requested_cpu_bytes,
            self.core.limits.policy.max_retained_dynamic_cpu_bytes,
        )?;

        state.dynamic_storage_bytes = requested_storage_bytes;
        state.dynamic_storage_bytes_high_water = state
            .dynamic_storage_bytes_high_water
            .max(requested_storage_bytes);
        state.dynamic_object_capacity = requested_object_capacity;
        state.dynamic_object_capacity_high_water = state
            .dynamic_object_capacity_high_water
            .max(requested_object_capacity);
        state.dynamic_storage_slots = requested_slots;
        state.dynamic_storage_slots_high_water =
            state.dynamic_storage_slots_high_water.max(requested_slots);
        state.retained_cpu_bytes = requested_cpu_bytes;
        state.retained_cpu_bytes_high_water =
            state.retained_cpu_bytes_high_water.max(requested_cpu_bytes);
        drop(state);

        Ok(DynamicStorageLease3D {
            accounting: self.core.dynamic_state_accounting.clone(),
            dynamic_storage_bytes,
            dynamic_object_capacity,
            dynamic_storage_slots,
            retained_cpu_bytes,
        })
    }

    pub(crate) fn reserve_model_state(&self) -> Result<ModelStateLease3D, Context3DError> {
        let accounting = &self.core.dynamic_state_accounting;
        let maximum = self.core.limits.policy.max_retained_model_states;
        let mut current = accounting.retained_model_states.load(Ordering::Acquire);
        let requested = loop {
            let requested = current
                .checked_add(1)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "counting retained immutable 3D model states",
                })?;
            validate_policy_budget(
                PolicyBudget3D::RetainedModelStates,
                u64::from(requested),
                u64::from(maximum),
            )?;
            match accounting.retained_model_states.compare_exchange_weak(
                current,
                requested,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break requested,
                Err(observed) => current = observed,
            }
        };
        accounting
            .retained_model_states_high_water
            .fetch_max(requested, Ordering::AcqRel);
        Ok(ModelStateLease3D {
            accounting: self.core.dynamic_state_accounting.clone(),
        })
    }

    pub(super) fn dynamic_storage_release_revision(&self) -> u64 {
        self.core
            .dynamic_state_accounting
            .storage_release_revision
            .load(Ordering::Acquire)
    }

    pub(crate) fn reserve_pending_request(&self) -> Result<PendingRequestLease3D, Context3DError> {
        let mut state = self
            .core
            .pending_work_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::PendingWorkAccountingUnavailable)?;
        let requested =
            state
                .requests
                .checked_add(1)
                .ok_or(Context3DError::PolicyBudgetExceeded {
                    budget: PolicyBudget3D::PendingRequests,
                    requested: u64::MAX,
                    maximum: u64::from(self.core.limits.policy.max_pending_requests),
                })?;
        validate_policy_budget(
            PolicyBudget3D::PendingRequests,
            u64::from(requested),
            u64::from(self.core.limits.policy.max_pending_requests),
        )?;
        state.requests = requested;
        Ok(PendingRequestLease3D {
            accounting: self.core.pending_work_accounting.clone(),
        })
    }

    pub(crate) fn reserve_pending_publication(
        &self,
    ) -> Result<PendingPublicationLease3D, Context3DError> {
        let mut state = self
            .core
            .pending_work_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::PendingWorkAccountingUnavailable)?;
        let requested =
            state
                .publications
                .checked_add(1)
                .ok_or(Context3DError::PolicyBudgetExceeded {
                    budget: PolicyBudget3D::PendingPublications,
                    requested: u64::MAX,
                    maximum: u64::from(self.core.limits.policy.max_pending_publications),
                })?;
        validate_policy_budget(
            PolicyBudget3D::PendingPublications,
            u64::from(requested),
            u64::from(self.core.limits.policy.max_pending_publications),
        )?;
        state.publications = requested;
        Ok(PendingPublicationLease3D {
            accounting: self.core.pending_work_accounting.clone(),
        })
    }

    pub fn register_geometry(
        &self,
        descriptor: GeometrySourceDescriptor,
    ) -> Result<GeometryLease, Context3DError> {
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) = registry.insert(ResourcePayload::Geometry(descriptor))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        GeometryLease::try_new(GeometryHandle { key }, ownership)
    }

    pub fn replace_geometry(
        &self,
        current: &GeometryHandle,
        descriptor: GeometrySourceDescriptor,
    ) -> Result<GeometryLease, Context3DError> {
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) =
                registry.replace(&current.key, ResourcePayload::Geometry(descriptor))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        GeometryLease::try_new(GeometryHandle { key }, ownership)
    }

    pub fn lease_geometry(&self, handle: &GeometryHandle) -> Result<GeometryLease, Context3DError> {
        let ownership = {
            let mut registry = self.registry()?;
            let record = registry.lease(&handle.key, ResourceKind3D::Geometry)?;
            ResourceLeaseOwnership::acquire(record)?
        };
        GeometryLease::try_new(handle.clone(), ownership)
    }

    pub fn retire_geometry(&self, handle: &GeometryHandle) -> Result<(), Context3DError> {
        self.registry()?
            .retire(&handle.key, ResourceKind3D::Geometry)
    }

    pub fn register_texture(&self, descriptor: Texture3D) -> Result<TextureLease, Context3DError> {
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) = registry.insert(ResourcePayload::Texture(descriptor))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        TextureLease::try_new(TextureHandle { key }, ownership)
    }

    pub fn replace_texture(
        &self,
        current: &TextureHandle,
        descriptor: Texture3D,
    ) -> Result<TextureLease, Context3DError> {
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) =
                registry.replace(&current.key, ResourcePayload::Texture(descriptor))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        TextureLease::try_new(TextureHandle { key }, ownership)
    }

    pub fn lease_texture(&self, handle: &TextureHandle) -> Result<TextureLease, Context3DError> {
        let ownership = {
            let mut registry = self.registry()?;
            let record = registry.lease(&handle.key, ResourceKind3D::Texture)?;
            ResourceLeaseOwnership::acquire(record)?
        };
        TextureLease::try_new(handle.clone(), ownership)
    }

    pub fn retire_texture(&self, handle: &TextureHandle) -> Result<(), Context3DError> {
        self.registry()?
            .retire(&handle.key, ResourceKind3D::Texture)
    }

    pub fn register_sampler(&self, descriptor: Sampler3D) -> Result<SamplerLease, Context3DError> {
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) = registry.insert(ResourcePayload::Sampler(descriptor))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        SamplerLease::try_new(SamplerHandle { key }, ownership)
    }

    pub fn replace_sampler(
        &self,
        current: &SamplerHandle,
        descriptor: Sampler3D,
    ) -> Result<SamplerLease, Context3DError> {
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) =
                registry.replace(&current.key, ResourcePayload::Sampler(descriptor))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        SamplerLease::try_new(SamplerHandle { key }, ownership)
    }

    pub fn lease_sampler(&self, handle: &SamplerHandle) -> Result<SamplerLease, Context3DError> {
        let ownership = {
            let mut registry = self.registry()?;
            let record = registry.lease(&handle.key, ResourceKind3D::Sampler)?;
            ResourceLeaseOwnership::acquire(record)?
        };
        SamplerLease::try_new(handle.clone(), ownership)
    }

    pub fn retire_sampler(&self, handle: &SamplerHandle) -> Result<(), Context3DError> {
        self.registry()?
            .retire(&handle.key, ResourceKind3D::Sampler)
    }

    fn resolve_appearance_resource(
        &self,
        descriptor: AppearanceSourceDescriptor,
    ) -> Result<AppearanceResource, Context3DError> {
        let (texture, sampler) = if let Some(binding) = descriptor.texture_binding() {
            (
                Some(self.lease_texture(binding.texture())?),
                Some(self.lease_sampler(binding.sampler())?),
            )
        } else {
            (None, None)
        };
        Ok(AppearanceResource {
            descriptor,
            texture,
            sampler,
        })
    }

    pub fn register_appearance(
        &self,
        descriptor: AppearanceSourceDescriptor,
    ) -> Result<AppearanceLease, Context3DError> {
        let resource = self.resolve_appearance_resource(descriptor)?;
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) = registry.insert(ResourcePayload::Appearance(resource))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        AppearanceLease::try_new(AppearanceHandle { key }, ownership)
    }

    pub fn replace_appearance(
        &self,
        current: &AppearanceHandle,
        descriptor: AppearanceSourceDescriptor,
    ) -> Result<AppearanceLease, Context3DError> {
        let resource = self.resolve_appearance_resource(descriptor)?;
        let (key, ownership) = {
            let mut registry = self.registry()?;
            let (key, record) =
                registry.replace(&current.key, ResourcePayload::Appearance(resource))?;
            let ownership = ResourceLeaseOwnership::acquire(record)?;
            (key, ownership)
        };
        AppearanceLease::try_new(AppearanceHandle { key }, ownership)
    }

    pub fn lease_appearance(
        &self,
        handle: &AppearanceHandle,
    ) -> Result<AppearanceLease, Context3DError> {
        let ownership = {
            let mut registry = self.registry()?;
            let record = registry.lease(&handle.key, ResourceKind3D::Appearance)?;
            ResourceLeaseOwnership::acquire(record)?
        };
        AppearanceLease::try_new(handle.clone(), ownership)
    }

    pub fn retire_appearance(&self, handle: &AppearanceHandle) -> Result<(), Context3DError> {
        self.registry()?
            .retire(&handle.key, ResourceKind3D::Appearance)
    }

    pub fn resource_usage(&self) -> Result<ResourceUsage3D, Context3DError> {
        self.collect_normal_cpu_charges();
        let mut usage = self.registry()?.usage()?;
        usage.resident_geometry_bytes = self.core.geometry_gpu_accounting.resident_bytes();
        usage.admitted_shader_modules = self.core.appearance_module_accounting.resident_modules();
        usage.resident_texture_bytes = self.core.texture_gpu_accounting.resident_bytes();
        if let Some(raster_gpu) = self
            .core
            .raster_gpu
            .lock()
            .map_err(|_| Context3DError::RasterCacheUnavailable)?
            .as_ref()
        {
            usage.resident_texture_bytes = usage
                .resident_texture_bytes
                .checked_add(raster_gpu.resident_texture_bytes())
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "summing resident 3D texture bytes",
                })?;
            usage.resident_pipeline_variants = raster_gpu.pipeline_count()?;
        }
        let pending = self
            .core
            .pending_work_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::PendingWorkAccountingUnavailable)?;
        usage.pending_requests = pending.requests;
        usage.pending_publications = pending.publications;
        Ok(usage)
    }

    pub(super) fn raster_gpu(&self) -> Result<Arc<GpuState3D>, Context3DError> {
        let mut cached = self
            .core
            .raster_gpu
            .lock()
            .map_err(|_| Context3DError::RasterCacheUnavailable)?;
        if let Some(raster_gpu) = cached.as_ref() {
            return Ok(raster_gpu.clone());
        }

        let gpu = self.core.binding.gpu()?;
        let raster_gpu = Arc::new(GpuState3D::new(
            gpu,
            self.core.abi_layout,
            self.core.limits.policy,
        )?);
        *cached = Some(raster_gpu.clone());
        #[cfg(test)]
        {
            super::audit::record_raster_state();
            super::audit::record_gpu_texture();
            super::audit::record_gpu_sampler();
        }
        Ok(raster_gpu)
    }

    #[allow(dead_code)]
    fn admit_appearance(
        &self,
        handle: &AppearanceHandle,
        available_semantics: GeometrySemantics,
    ) -> Result<AppearanceGpuLease, Context3DError> {
        let lease = self.lease_appearance(handle)?;
        self.admit_appearance_lease(&lease, available_semantics)
    }

    pub(crate) fn admit_appearance_lease(
        &self,
        lease: &AppearanceLease,
        available_semantics: GeometrySemantics,
    ) -> Result<AppearanceGpuLease, Context3DError> {
        if !self.core.id.same_as(&lease.handle.key.context) {
            return Err(Context3DError::ContextMismatch);
        }
        let handle = &lease.handle;
        let ownership = lease._ownership.clone();
        let record = ownership.record.clone();
        let descriptor = &lease.descriptor;
        let required_semantics = descriptor.required_semantics();
        if !available_semantics.contains(required_semantics) {
            return Err(Context3DError::MissingGeometrySemantics {
                required: required_semantics,
                available: available_semantics,
            });
        }

        let mut admitted_cache = record
            .admitted_appearance
            .lock()
            .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
        if let Some(appearance) = admitted_cache.as_ref() {
            let canonical_vertex = self
                .core
                .canonical_vertex_module
                .lock()
                .map_err(|_| Context3DError::AppearanceCacheUnavailable)?
                .as_ref()
                .cloned()
                .ok_or(Context3DError::AppearanceCacheUnavailable)?;
            return Ok(AppearanceGpuLease {
                handle: handle.clone(),
                appearance: appearance.clone(),
                canonical_vertex,
                texture: lease.texture.clone(),
                sampler: lease.sampler.clone(),
                _ownership: ownership,
            });
        }

        let compiler_limits = AppearanceCompilerLimits {
            max_functions: self.core.limits.policy.max_appearance_functions,
            max_nesting: self.core.limits.policy.max_appearance_nesting,
            max_ir_nodes: self.core.limits.policy.max_appearance_ir_nodes,
        };
        let compiled_fragment = compile_fragment_appearance(descriptor, compiler_limits)?;
        let mut canonical_cache = self
            .core
            .canonical_vertex_module
            .lock()
            .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;

        let (canonical_vertex, fragment_residency) =
            if let Some(canonical_vertex) = canonical_cache.as_ref() {
                let reservation = self
                    .core
                    .appearance_module_accounting
                    .reserve_modules(1, self.core.limits.policy.max_shader_modules)?;
                (canonical_vertex.clone(), reservation.commit_one())
            } else {
                let compiled_vertex = compile_canonical_vertex(compiler_limits)?;
                let reservation = self
                    .core
                    .appearance_module_accounting
                    .reserve_modules(2, self.core.limits.policy.max_shader_modules)?;
                let (vertex_residency, fragment_residency) = reservation.commit_pair();
                let canonical_vertex = Arc::new(CanonicalVertexModule {
                    module: compiled_vertex.module,
                    ir_nodes: compiled_vertex.ir_nodes,
                    synthetic_spans_are_undefined: compiled_vertex.synthetic_spans_are_undefined,
                    gpu_module: Mutex::new(None),
                    _residency: vertex_residency,
                });
                *canonical_cache = Some(canonical_vertex.clone());
                (canonical_vertex, fragment_residency)
            };

        let appearance = Arc::new(AdmittedAppearance {
            module: compiled_fragment.module,
            source_hash: compiled_fragment.source_hash,
            required_semantics: compiled_fragment.required_semantics,
            mode: compiled_fragment.mode,
            source_ir_nodes: compiled_fragment.source_ir_nodes,
            final_ir_nodes: compiled_fragment.final_ir_nodes,
            source_spans_preserved: compiled_fragment.source_spans_preserved,
            synthetic_spans_are_undefined: compiled_fragment.synthetic_spans_are_undefined,
            abi_layout: self.core.abi_layout,
            fallback: AppearanceFallbackBindingContract::OPAQUE_WHITE,
            gpu_module: Mutex::new(None),
            scene_gpu_module: Mutex::new(None),
            _residency: fragment_residency,
        });
        *admitted_cache = Some(appearance.clone());
        Ok(AppearanceGpuLease {
            handle: handle.clone(),
            appearance,
            canonical_vertex,
            texture: lease.texture.clone(),
            sampler: lease.sampler.clone(),
            _ownership: ownership,
        })
    }

    #[allow(dead_code)]
    fn materialize_geometry(
        &self,
        handle: &GeometryHandle,
        upload_budget: &mut PreparationUploadBudget3D,
    ) -> Result<GeometryGpuLease, Context3DError> {
        let lease = self.lease_geometry(handle)?;
        self.materialize_geometry_lease(&lease, upload_budget)
    }

    pub(crate) fn materialize_geometry_lease(
        &self,
        lease: &GeometryLease,
        upload_budget: &mut PreparationUploadBudget3D,
    ) -> Result<GeometryGpuLease, Context3DError> {
        if !self.core.id.same_as(&upload_budget.context) {
            return Err(Context3DError::ContextMismatch);
        }
        if !self.core.id.same_as(&lease.handle.key.context) {
            return Err(Context3DError::ContextMismatch);
        }
        let handle = &lease.handle;
        let ownership = lease._ownership.clone();
        let record = ownership.record.clone();
        let descriptor = &lease.descriptor;
        let mut cached = record
            .materialized_geometry
            .lock()
            .map_err(|_| Context3DError::GeometryCacheUnavailable)?;
        if let Some(geometry) = cached.as_ref() {
            return Ok(GeometryGpuLease {
                handle: handle.clone(),
                geometry: geometry.clone(),
                _ownership: ownership,
            });
        }

        let packed = CanonicalGeometryBytes::pack(descriptor)?;
        let upload_bytes = packed.upload_bytes()?;
        let resident_bytes = packed.gpu_resident_bytes()?;
        let remaining_after_upload = upload_budget.remaining_after(upload_bytes)?;
        validate_hard_limit(
            HardLimit3D::BufferSize,
            u64::try_from(packed.vertex_bytes().len()).map_err(|_| {
                Context3DError::ArithmeticOverflow {
                    operation: "representing a canonical vertex buffer size",
                }
            })?,
            self.core.limits.hard.max_buffer_size,
        )?;
        validate_hard_limit(
            HardLimit3D::BufferSize,
            packed.index_buffer_size(),
            self.core.limits.hard.max_buffer_size,
        )?;
        validate_policy_budget(
            PolicyBudget3D::MeshBytes,
            resident_bytes,
            self.core.limits.policy.max_mesh_bytes,
        )?;
        validate_policy_budget(
            PolicyBudget3D::UploadBytesPerFrame,
            upload_bytes,
            self.core.limits.policy.max_upload_bytes_per_frame,
        )?;

        let gpu = self.core.binding.gpu()?;
        let accounting = &self.core.geometry_gpu_accounting;
        let residency_reservation = accounting.reserve_resident_bytes(
            resident_bytes,
            self.core.limits.policy.max_resident_geometry_bytes,
        )?;

        // wgpu reports allocation and device failures through its device error boundary, not
        // this method's typed pre-wgpu validation result.
        let vertex_buffer = Arc::new(gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Halcyon 3D Geometry Vertex Buffer"),
                contents: packed.vertex_bytes(),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        let index_buffer = if packed.index_bytes().is_empty() {
            None
        } else {
            Some(Arc::new(gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Halcyon 3D Geometry Index Buffer"),
                    contents: packed.index_bytes(),
                    usage: wgpu::BufferUsages::INDEX,
                },
            )))
        };
        let residency = residency_reservation.commit();
        let geometry = Arc::new(MaterializedGeometry {
            device: Arc::downgrade(&gpu.device),
            vertex_buffer,
            index_buffer,
            vertex_count: packed.vertex_count(),
            index_count: packed.index_count(),
            index_format: packed.index_format(),
            upload_bytes,
            residency,
        });
        upload_budget.remaining_bytes = remaining_after_upload;
        *cached = Some(geometry.clone());
        #[cfg(test)]
        {
            super::audit::record_gpu_buffers(1 + u64::from(geometry.index_buffer.is_some()));
            let _ = self.core.geometry_materializations.fetch_update(
                Ordering::AcqRel,
                Ordering::Acquire,
                |count| count.checked_add(1),
            );
        }
        Ok(GeometryGpuLease {
            handle: handle.clone(),
            geometry,
            _ownership: ownership,
        })
    }

    pub(crate) fn materialize_texture_lease(
        &self,
        lease: &TextureLease,
        upload_budget: &mut PreparationUploadBudget3D,
    ) -> Result<TextureGpuLease, Context3DError> {
        if !self.core.id.same_as(&upload_budget.context)
            || !self.core.id.same_as(&lease.handle.key.context)
        {
            return Err(Context3DError::ContextMismatch);
        }
        let ownership = lease._ownership.clone();
        let record = ownership.record.clone();
        let descriptor = &lease.descriptor;
        let mut cached = record
            .materialized_texture
            .lock()
            .map_err(|_| Context3DError::TextureCacheUnavailable)?;
        if let Some(texture) = cached.as_ref() {
            return Ok(TextureGpuLease {
                handle: lease.handle.clone(),
                texture: texture.clone(),
                _ownership: ownership,
            });
        }

        let upload_bytes = descriptor.texel_byte_len();
        let remaining_after_upload = upload_budget.remaining_after(upload_bytes)?;
        validate_hard_limit(
            HardLimit3D::TextureDimension2D,
            u64::from(descriptor.width().max(descriptor.height())),
            u64::from(self.core.limits.hard.max_texture_dimension_2d),
        )?;
        validate_policy_budget(
            PolicyBudget3D::TextureBytes,
            upload_bytes,
            self.core.limits.policy.max_texture_bytes,
        )?;
        validate_policy_budget(
            PolicyBudget3D::UploadBytesPerFrame,
            upload_bytes,
            self.core.limits.policy.max_upload_bytes_per_frame,
        )?;
        let bytes_per_row =
            descriptor
                .width()
                .checked_mul(4)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "sizing a 3D texture upload row",
                })?;

        let gpu = self.core.binding.gpu()?;
        let raster_gpu = self.raster_gpu()?;
        let residency_reservation = self.core.texture_gpu_accounting.reserve_resident_bytes(
            upload_bytes,
            raster_gpu.resident_texture_bytes(),
            self.core.limits.policy.max_resident_texture_bytes,
        )?;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Halcyon 3D Appearance Texture"),
            size: wgpu::Extent3d {
                width: descriptor.width(),
                height: descriptor.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: descriptor.format().as_wgpu(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            descriptor.texels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(descriptor.height()),
            },
            wgpu::Extent3d {
                width: descriptor.width(),
                height: descriptor.height(),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let residency = residency_reservation.commit();
        let texture = Arc::new(MaterializedTexture {
            texture,
            view,
            upload_bytes,
            residency,
        });
        upload_budget.remaining_bytes = remaining_after_upload;
        *cached = Some(texture.clone());
        #[cfg(test)]
        super::audit::record_gpu_texture();
        Ok(TextureGpuLease {
            handle: lease.handle.clone(),
            texture,
            _ownership: ownership,
        })
    }

    pub(crate) fn materialize_sampler_lease(
        &self,
        lease: &SamplerLease,
    ) -> Result<SamplerGpuLease, Context3DError> {
        if !self.core.id.same_as(&lease.handle.key.context) {
            return Err(Context3DError::ContextMismatch);
        }
        let ownership = lease._ownership.clone();
        let record = ownership.record.clone();
        let mut cached = record
            .materialized_sampler
            .lock()
            .map_err(|_| Context3DError::SamplerCacheUnavailable)?;
        if let Some(sampler) = cached.as_ref() {
            return Ok(SamplerGpuLease {
                handle: lease.handle.clone(),
                sampler: sampler.clone(),
                _ownership: ownership,
            });
        }

        let gpu = self.core.binding.gpu()?;
        let descriptor = lease.descriptor;
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Halcyon 3D Appearance Sampler"),
            address_mode_u: descriptor.address_mode_u().as_wgpu(),
            address_mode_v: descriptor.address_mode_v().as_wgpu(),
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: descriptor.mag_filter().as_wgpu(),
            min_filter: descriptor.min_filter().as_wgpu(),
            mipmap_filter: match descriptor.min_filter().as_wgpu() {
                wgpu::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
                wgpu::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
            },
            ..Default::default()
        });
        let sampler = Arc::new(MaterializedSampler { sampler });
        *cached = Some(sampler.clone());
        #[cfg(test)]
        super::audit::record_gpu_sampler();
        Ok(SamplerGpuLease {
            handle: lease.handle.clone(),
            sampler,
            _ownership: ownership,
        })
    }

    pub(crate) fn materialize_appearance_binding(
        &self,
        appearance: &AppearanceGpuLease,
        upload_budget: &mut PreparationUploadBudget3D,
    ) -> Result<Option<AppearanceTextureGpuLease>, Context3DError> {
        match (appearance.texture.as_ref(), appearance.sampler.as_ref()) {
            (Some(texture), Some(sampler)) => Ok(Some(AppearanceTextureGpuLease {
                texture: self.materialize_texture_lease(texture, upload_budget)?,
                sampler: self.materialize_sampler_lease(sampler)?,
            })),
            (None, None) => Ok(None),
            _ => Err(Context3DError::AppearanceCacheUnavailable),
        }
    }

    #[allow(dead_code)]
    fn evict_unleased_geometry_cache(
        &self,
        handle: &GeometryHandle,
    ) -> Result<bool, Context3DError> {
        let registry = self.registry()?;
        let (_, record, _, _) = registry.validate_current(&handle.key, ResourceKind3D::Geometry)?;
        if record.has_lease_owners() {
            return Ok(false);
        }
        let removed = record
            .materialized_geometry
            .lock()
            .map_err(|_| Context3DError::GeometryCacheUnavailable)?
            .take()
            .is_some();
        Ok(removed)
    }

    fn registry(&self) -> Result<std::sync::MutexGuard<'_, ResourceRegistry>, Context3DError> {
        self.core
            .resources
            .lock()
            .map_err(|_| Context3DError::RegistryUnavailable)
    }

    #[allow(dead_code)]
    pub(crate) fn begin_preparation_upload_budget(&self) -> PreparationUploadBudget3D {
        PreparationUploadBudget3D {
            context: self.core.id.clone(),
            remaining_bytes: self.core.limits.policy.max_upload_bytes_per_frame,
        }
    }

    #[cfg(test)]
    pub(crate) fn dynamic_state_accounting_for_test(
        &self,
    ) -> Result<DynamicStateAccountingSnapshot3D, Context3DError> {
        let state = self
            .core
            .dynamic_state_accounting
            .state
            .lock()
            .map_err(|_| Context3DError::DynamicStateAccountingUnavailable)?;
        Ok(DynamicStateAccountingSnapshot3D {
            dynamic_storage_bytes: state.dynamic_storage_bytes,
            dynamic_storage_bytes_high_water: state.dynamic_storage_bytes_high_water,
            dynamic_object_capacity: state.dynamic_object_capacity,
            dynamic_object_capacity_high_water: state.dynamic_object_capacity_high_water,
            dynamic_storage_slots: state.dynamic_storage_slots,
            dynamic_storage_slots_high_water: state.dynamic_storage_slots_high_water,
            retained_cpu_bytes: state.retained_cpu_bytes,
            retained_cpu_bytes_high_water: state.retained_cpu_bytes_high_water,
            retained_model_states: self
                .core
                .dynamic_state_accounting
                .retained_model_states
                .load(Ordering::Acquire),
            retained_model_states_high_water: self
                .core
                .dynamic_state_accounting
                .retained_model_states_high_water
                .load(Ordering::Acquire),
        })
    }

    #[cfg(test)]
    pub(crate) fn geometry_cpu_escrow_witness_for_test(
        &self,
        previous: Option<&super::PbrGeometry3D>,
    ) -> Result<
        (
            Weak<impl Sized + use<>>,
            Option<Weak<impl Sized + use<>>>,
            impl Fn() -> Option<(u32, u64, u64)> + use<>,
        ),
        ControlledResourceError3D,
    > {
        let record = previous
            .map(|handle| {
                self.lease_controlled_source(&handle.0.0, ControlledResourceKind3D::PbrGeometry)
                    .map(|lease| Arc::downgrade(&lease.record))
            })
            .transpose()?;
        let accounting = Arc::downgrade(&self.core.dynamic_state_accounting);
        // Observe the counters without keeping the accounting alive or collecting retirements.
        let read = move || {
            let accounting = accounting.upgrade()?;
            let state = accounting.state.lock().unwrap();
            Some((
                state.dynamic_storage_slots,
                state.retained_cpu_bytes,
                accounting.storage_release_revision.load(Ordering::Acquire),
            ))
        };
        Ok((Arc::downgrade(&self.core), record, read))
    }

    #[cfg(test)]
    pub(crate) fn new_cpu_only_for_test(
        policy: PolicyLimits3D,
        generation_ceiling: u64,
    ) -> Result<Self, Context3DError> {
        Self::new_cpu_only_with_domain_retention_for_test(
            policy,
            DomainRetentionLimits3D::conservative(),
            generation_ceiling,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_cpu_only_with_domain_retention_for_test(
        policy: PolicyLimits3D,
        domain_retention_limits: DomainRetentionLimits3D,
        generation_ceiling: u64,
    ) -> Result<Self, Context3DError> {
        Self::new_cpu_only_with_hard_and_domain_retention_for_test(
            policy,
            domain_retention_limits,
            DeviceLimits3D::baseline_for_test(),
            generation_ceiling,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_cpu_only_with_hard_for_test(
        policy: PolicyLimits3D,
        hard: DeviceLimits3D,
        generation_ceiling: u64,
    ) -> Result<Self, Context3DError> {
        Self::new_cpu_only_with_hard_and_domain_retention_for_test(
            policy,
            DomainRetentionLimits3D::conservative(),
            hard,
            generation_ceiling,
        )
    }

    #[cfg(test)]
    fn new_cpu_only_with_hard_and_domain_retention_for_test(
        policy: PolicyLimits3D,
        domain_retention_limits: DomainRetentionLimits3D,
        hard: DeviceLimits3D,
        generation_ceiling: u64,
    ) -> Result<Self, Context3DError> {
        policy.validate()?;
        validate_baseline_device(hard)?;
        validate_custom_policy_against_device(policy, hard)?;
        Self::from_parts(
            ContextGpuBindingRequest::CpuOnly {
                format: wgpu::TextureFormat::Bgra8Unorm,
                device_identity: Arc::new(ContextIdentity),
                queue_identity: Arc::new(ContextIdentity),
            },
            EffectiveLimits3D {
                profile: LimitProfile3D::Custom,
                hard,
                policy,
            },
            domain_retention_limits,
            generation_ceiling,
        )
    }

    #[cfg(test)]
    pub(crate) fn wrong_kind_geometry_handle_for_test(
        appearance: &AppearanceHandle,
    ) -> GeometryHandle {
        GeometryHandle {
            key: appearance.key.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn geometry_handle_with_slot_for_test(
        handle: &GeometryHandle,
        slot: u32,
    ) -> GeometryHandle {
        let mut key = handle.key.clone();
        key.slot = slot;
        GeometryHandle { key }
    }

    #[cfg(test)]
    pub(crate) fn wrong_kind_appearance_handle_for_test(
        geometry: &GeometryHandle,
    ) -> AppearanceHandle {
        AppearanceHandle {
            key: geometry.key.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn appearance_handle_with_slot_for_test(
        handle: &AppearanceHandle,
        slot: u32,
    ) -> AppearanceHandle {
        let mut key = handle.key.clone();
        key.slot = slot;
        AppearanceHandle { key }
    }

    #[cfg(test)]
    pub(crate) fn wrong_kind_texture_handle_for_test(sampler: &SamplerHandle) -> TextureHandle {
        TextureHandle {
            key: sampler.key.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn wrong_kind_sampler_handle_for_test(texture: &TextureHandle) -> SamplerHandle {
        SamplerHandle {
            key: texture.key.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn admit_appearance_for_test(
        &self,
        handle: &AppearanceHandle,
        available_semantics: GeometrySemantics,
    ) -> Result<AppearanceGpuLease, Context3DError> {
        self.admit_appearance(handle, available_semantics)
    }

    #[cfg(test)]
    pub(crate) fn materialize_geometry_for_test(
        &self,
        handle: &GeometryHandle,
        remaining_upload_bytes: u64,
    ) -> Result<GeometryGpuLease, Context3DError> {
        let mut upload_budget = self.preparation_upload_budget_for_test(remaining_upload_bytes);
        self.materialize_geometry(handle, &mut upload_budget)
    }

    #[cfg(test)]
    pub(crate) fn materialize_geometry_with_budget_for_test(
        &self,
        handle: &GeometryHandle,
        upload_budget: &mut PreparationUploadBudget3D,
    ) -> Result<GeometryGpuLease, Context3DError> {
        self.materialize_geometry(handle, upload_budget)
    }

    #[cfg(test)]
    pub(crate) fn evict_unleased_geometry_cache_for_test(
        &self,
        handle: &GeometryHandle,
    ) -> Result<bool, Context3DError> {
        self.evict_unleased_geometry_cache(handle)
    }

    #[cfg(test)]
    pub(crate) fn preparation_upload_budget_for_test(
        &self,
        remaining_bytes: u64,
    ) -> PreparationUploadBudget3D {
        PreparationUploadBudget3D {
            context: self.core.id.clone(),
            remaining_bytes: remaining_bytes
                .min(self.core.limits.policy.max_upload_bytes_per_frame),
        }
    }

    #[cfg(test)]
    pub(crate) fn geometry_materialization_count_for_test(&self) -> u64 {
        self.core.geometry_materializations.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn gpu_resident_geometry_bytes_for_test(&self) -> u64 {
        self.core.geometry_gpu_accounting.resident_bytes()
    }

    #[cfg(test)]
    pub(crate) fn admitted_shader_module_count_for_test(&self) -> u32 {
        self.core.appearance_module_accounting.resident_modules()
    }
}

fn validate_baseline_device(hard: DeviceLimits3D) -> Result<(), Context3DError> {
    let minimums = [
        (
            HardLimit3D::TextureDimension2D,
            u64::from(hard.max_texture_dimension_2d),
            1,
        ),
        (HardLimit3D::BindGroups, u64::from(hard.max_bind_groups), 3),
        (
            HardLimit3D::SampledTexturesPerShaderStage,
            u64::from(hard.max_sampled_textures_per_shader_stage),
            1,
        ),
        (
            HardLimit3D::SamplersPerShaderStage,
            u64::from(hard.max_samplers_per_shader_stage),
            1,
        ),
        (
            HardLimit3D::UniformBufferBindingSize,
            hard.max_uniform_buffer_binding_size,
            112,
        ),
        (
            HardLimit3D::VertexBuffers,
            u64::from(hard.max_vertex_buffers),
            1,
        ),
        (HardLimit3D::BufferSize, hard.max_buffer_size, 36),
        (
            HardLimit3D::VertexAttributes,
            u64::from(hard.max_vertex_attributes),
            4,
        ),
        (
            HardLimit3D::VertexBufferArrayStride,
            u64::from(hard.max_vertex_buffer_array_stride),
            36,
        ),
        (
            HardLimit3D::UniformBufferOffsetAlignment,
            u64::from(hard.min_uniform_buffer_offset_alignment),
            1,
        ),
        (
            HardLimit3D::InterStageShaderVariables,
            u64::from(hard.max_inter_stage_shader_variables),
            5, // Four locations and the fragment front-facing builtin.
        ),
    ];
    if let Some((limit, maximum, requested)) = minimums
        .into_iter()
        .find(|(_, maximum, requested)| maximum < requested)
    {
        return Err(Context3DError::HardDeviceLimitExceeded {
            limit,
            requested,
            maximum,
        });
    }
    Ok(())
}

fn validate_custom_policy_against_device(
    policy: PolicyLimits3D,
    hard: DeviceLimits3D,
) -> Result<(), Context3DError> {
    validate_hard_limit(
        HardLimit3D::TextureDimension2D,
        u64::from(policy.max_target_dimension_2d),
        u64::from(hard.max_texture_dimension_2d),
    )?;
    validate_hard_limit(
        HardLimit3D::TextureDimension2D,
        u64::from(policy.max_texture_dimension_2d),
        u64::from(hard.max_texture_dimension_2d),
    )?;
    validate_hard_limit(
        HardLimit3D::BufferSize,
        policy.max_mesh_bytes,
        hard.max_buffer_size,
    )?;
    let vertex_bytes = u64::from(policy.max_vertices_per_mesh)
        .checked_mul(36)
        .ok_or(Context3DError::InvalidConfiguration {
            field: "max_vertices_per_mesh",
        })?;
    validate_hard_limit(HardLimit3D::BufferSize, vertex_bytes, hard.max_buffer_size)?;
    let index_bytes = u64::from(policy.max_indices_per_mesh)
        .checked_mul(4)
        .ok_or(Context3DError::InvalidConfiguration {
            field: "max_indices_per_mesh",
        })?;
    validate_hard_limit(HardLimit3D::BufferSize, index_bytes, hard.max_buffer_size)
}

fn validate_hard_limit(
    limit: HardLimit3D,
    requested: u64,
    maximum: u64,
) -> Result<(), Context3DError> {
    if requested > maximum {
        return Err(Context3DError::HardDeviceLimitExceeded {
            limit,
            requested,
            maximum,
        });
    }
    Ok(())
}

#[derive(Clone)]
enum ResourcePayload {
    Geometry(GeometrySourceDescriptor),
    Appearance(AppearanceResource),
    Texture(Texture3D),
    Sampler(Sampler3D),
}

impl ResourcePayload {
    fn kind(&self) -> ResourceKind3D {
        match self {
            Self::Geometry(_) => ResourceKind3D::Geometry,
            Self::Appearance(_) => ResourceKind3D::Appearance,
            Self::Texture(_) => ResourceKind3D::Texture,
            Self::Sampler(_) => ResourceKind3D::Sampler,
        }
    }

    fn resident_bytes(&self) -> u64 {
        match self {
            Self::Geometry(descriptor) => descriptor.canonical_total_bytes(),
            Self::Appearance(resource) => resource.descriptor.source_byte_len(),
            Self::Texture(descriptor) => descriptor.texel_byte_len(),
            Self::Sampler(_) => std::mem::size_of::<Sampler3D>() as u64,
        }
    }
}

struct ResourceGeneration {
    payload: ResourcePayload,
    lease_owners: AtomicU32,
    materialized_geometry: Mutex<Option<Arc<MaterializedGeometry>>>,
    materialized_texture: Mutex<Option<Arc<MaterializedTexture>>>,
    materialized_sampler: Mutex<Option<Arc<MaterializedSampler>>>,
    admitted_appearance: Mutex<Option<Arc<AdmittedAppearance>>>,
}

impl ResourceGeneration {
    fn new(payload: ResourcePayload) -> Self {
        Self {
            payload,
            lease_owners: AtomicU32::new(0),
            materialized_geometry: Mutex::new(None),
            materialized_texture: Mutex::new(None),
            materialized_sampler: Mutex::new(None),
            admitted_appearance: Mutex::new(None),
        }
    }

    fn has_lease_owners(&self) -> bool {
        self.lease_owners.load(Ordering::Acquire) != 0
    }
}

struct ResourceLeaseOwnership {
    record: Arc<ResourceGeneration>,
}

impl ResourceLeaseOwnership {
    fn acquire(record: Arc<ResourceGeneration>) -> Result<Arc<Self>, Context3DError> {
        record
            .lease_owners
            .try_update(Ordering::AcqRel, Ordering::Acquire, |owners| {
                owners.checked_add(1)
            })
            .map_err(|_| Context3DError::ArithmeticOverflow {
                operation: "counting resource lease owners",
            })?;
        Ok(Arc::new(Self { record }))
    }

    fn record(&self) -> &ResourceGeneration {
        &self.record
    }
}

impl Drop for ResourceLeaseOwnership {
    fn drop(&mut self) {
        let _ =
            self.record
                .lease_owners
                .try_update(Ordering::AcqRel, Ordering::Acquire, |owners| {
                    owners.checked_sub(1)
                });
    }
}

#[allow(dead_code)]
struct MaterializedGeometry {
    // Identity only. Context3D owns the captured Device; cached resources can
    // enter Queue completion custody and must not keep its callback owner alive.
    device: Weak<wgpu::Device>,
    vertex_buffer: Arc<wgpu::Buffer>,
    index_buffer: Option<Arc<wgpu::Buffer>>,
    vertex_count: u32,
    index_count: u32,
    index_format: CanonicalIndexFormat,
    upload_bytes: u64,
    residency: GeometryGpuResidency,
}

#[allow(dead_code)]
struct MaterializedTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    upload_bytes: u64,
    residency: TextureGpuResidency,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct TextureGpuLease {
    handle: TextureHandle,
    texture: Arc<MaterializedTexture>,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl TextureGpuLease {
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.texture.view
    }
}

#[allow(dead_code)]
struct MaterializedSampler {
    sampler: wgpu::Sampler,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct SamplerGpuLease {
    handle: SamplerHandle,
    sampler: Arc<MaterializedSampler>,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl SamplerGpuLease {
    pub(crate) fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler.sampler
    }
}

#[derive(Clone)]
pub(crate) struct AppearanceTextureGpuLease {
    texture: TextureGpuLease,
    sampler: SamplerGpuLease,
}

impl AppearanceTextureGpuLease {
    pub(crate) fn texture_view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }

    pub(crate) fn sampler(&self) -> &wgpu::Sampler {
        self.sampler.sampler()
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct GeometryGpuLease {
    handle: GeometryHandle,
    geometry: Arc<MaterializedGeometry>,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl fmt::Debug for GeometryGpuLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeometryGpuLease")
            .field("handle", &self.handle)
            .field("vertex_count", &self.geometry.vertex_count)
            .field("index_count", &self.geometry.index_count)
            .field("index_format", &self.geometry.index_format)
            .finish_non_exhaustive()
    }
}

impl GeometryGpuLease {
    pub(crate) fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.geometry.vertex_buffer
    }

    pub(crate) fn index_buffer(&self) -> Option<&wgpu::Buffer> {
        self.geometry.index_buffer.as_deref()
    }

    pub(crate) fn vertex_count(&self) -> u32 {
        self.geometry.vertex_count
    }

    pub(crate) fn index_count(&self) -> u32 {
        self.geometry.index_count
    }

    pub(crate) fn index_format(&self) -> CanonicalIndexFormat {
        self.geometry.index_format
    }
}

#[cfg(test)]
impl GeometryGpuLease {
    pub(crate) const fn handle(&self) -> &GeometryHandle {
        &self.handle
    }

    pub(crate) fn vertex_buffer_size(&self) -> u64 {
        self.geometry.vertex_buffer.size()
    }

    pub(crate) fn index_buffer_size(&self) -> Option<u64> {
        self.geometry
            .index_buffer
            .as_ref()
            .map(|buffer| buffer.size())
    }

    pub(crate) fn upload_bytes(&self) -> u64 {
        self.geometry.upload_bytes
    }

    pub(crate) fn resident_bytes(&self) -> u64 {
        self.geometry.residency.resident_bytes
    }

    pub(crate) fn same_buffers(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.geometry.vertex_buffer, &other.geometry.vertex_buffer)
            && match (&self.geometry.index_buffer, &other.geometry.index_buffer) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
    }

    pub(crate) fn matches_device(&self, gpu: &GpuResources) -> bool {
        Weak::ptr_eq(&self.geometry.device, &Arc::downgrade(&gpu.device))
    }
}

#[allow(dead_code)]
struct CanonicalVertexModule {
    module: naga::Module,
    ir_nodes: u64,
    synthetic_spans_are_undefined: bool,
    gpu_module: Mutex<Option<Arc<wgpu::ShaderModule>>>,
    _residency: ShaderModuleResidency,
}

#[allow(dead_code)]
struct AdmittedAppearance {
    module: naga::Module,
    source_hash: [u8; 32],
    required_semantics: GeometrySemantics,
    mode: AppearanceMode,
    source_ir_nodes: u64,
    final_ir_nodes: u64,
    source_spans_preserved: bool,
    synthetic_spans_are_undefined: bool,
    abi_layout: AbiLayout3D,
    fallback: AppearanceFallbackBindingContract,
    gpu_module: Mutex<Option<Arc<wgpu::ShaderModule>>>,
    scene_gpu_module: Mutex<Option<SceneAppearanceModule3D>>,
    _residency: ShaderModuleResidency,
}

struct SceneAppearanceModule3D {
    module: Arc<wgpu::ShaderModule>,
    _residency: ShaderModuleResidency,
    _metadata: DynamicStorageLease3D,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct AppearanceGpuLease {
    handle: AppearanceHandle,
    appearance: Arc<AdmittedAppearance>,
    canonical_vertex: Arc<CanonicalVertexModule>,
    texture: Option<TextureLease>,
    sampler: Option<SamplerLease>,
    _ownership: Arc<ResourceLeaseOwnership>,
}

impl fmt::Debug for AppearanceGpuLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppearanceGpuLease")
            .field("handle", &self.handle)
            .field("source_hash", &self.appearance.source_hash)
            .field("mode", &self.appearance.mode)
            .finish_non_exhaustive()
    }
}

impl AppearanceGpuLease {
    pub(crate) fn source_hash(&self) -> [u8; 32] {
        self.appearance.source_hash
    }

    pub(crate) fn mode(&self) -> AppearanceMode {
        self.appearance.mode
    }

    pub(crate) fn materialize_shader_modules(
        &self,
        device: &wgpu::Device,
    ) -> Result<(Arc<wgpu::ShaderModule>, Arc<wgpu::ShaderModule>), Context3DError> {
        let vertex = self.materialize_vertex_module(device)?;
        let fragment = {
            let mut cached = self
                .appearance
                .gpu_module
                .lock()
                .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
            if let Some(module) = cached.as_ref() {
                module.clone()
            } else {
                let module = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Halcyon 3D Controlled Fragment Module"),
                    source: super::appearance::materialize_shader_source(
                        self.appearance.module.clone(),
                    )?,
                }));
                *cached = Some(module.clone());
                #[cfg(test)]
                super::audit::record_shader_module();
                module
            }
        };
        Ok((vertex, fragment))
    }

    pub(crate) fn materialize_scene_shader_modules(
        &self,
        context: &Context3D,
        device: &wgpu::Device,
    ) -> Result<(Arc<wgpu::ShaderModule>, Arc<wgpu::ShaderModule>), Context3DError> {
        let mut cached = self
            .appearance
            .scene_gpu_module
            .lock()
            .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
        let vertex = self.materialize_vertex_module(device)?;
        if let Some(module) = cached.as_ref() {
            return Ok((vertex, module.module.clone()));
        }
        let reservation = context.reserve_controlled_shader_module()?;
        let metadata = context.reserve_dynamic_storage(
            0,
            0,
            1,
            std::mem::size_of::<SceneAppearanceModule3D>() as u64
                + std::mem::size_of::<Mutex<Option<SceneAppearanceModule3D>>>() as u64,
        )?;
        let mut ir = self.appearance.module.clone();
        let error = |detail: String| Appearance3DAdmissionError::InternalCompilerAdapter {
            stage: "R027 MRT4 fragment output packing",
            detail: detail.into(),
        };
        super::hdr::scene_fragment_outputs(
            &mut ir,
            &[super::appearance::FRAGMENT_ENTRY_POINT_NAME],
        )
        .map_err(error)?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&ir)
        .map_err(|e| error(e.to_string()))?;
        let source = super::appearance::materialize_shader_source(ir)?;
        let allocation_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Halcyon controlled appearance R027 MRT4 output"),
            source,
        }));
        let validation = pollster::block_on(validation_scope.pop());
        let allocation = pollster::block_on(allocation_scope.pop());
        if let Some(e) = validation.or(allocation) {
            return Err(error(e.to_string()).into());
        }
        *cached = Some(SceneAppearanceModule3D {
            module: module.clone(),
            _residency: reservation.commit_one(),
            _metadata: metadata,
        });
        #[cfg(test)]
        super::audit::record_shader_module();
        Ok((vertex, module))
    }

    fn materialize_vertex_module(
        &self,
        device: &wgpu::Device,
    ) -> Result<Arc<wgpu::ShaderModule>, Context3DError> {
        let vertex = {
            let mut cached = self
                .canonical_vertex
                .gpu_module
                .lock()
                .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
            if let Some(module) = cached.as_ref() {
                module.clone()
            } else {
                let module = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Halcyon 3D Canonical Vertex Module"),
                    source: super::appearance::materialize_shader_source(
                        self.canonical_vertex.module.clone(),
                    )?,
                }));
                *cached = Some(module.clone());
                #[cfg(test)]
                super::audit::record_shader_module();
                module
            }
        };
        Ok(vertex)
    }
}

#[cfg(test)]
impl AppearanceGpuLease {
    pub(crate) const fn handle(&self) -> &AppearanceHandle {
        &self.handle
    }

    pub(crate) fn same_admitted_module(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.appearance, &other.appearance)
    }

    pub(crate) fn same_canonical_vertex_module(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.canonical_vertex, &other.canonical_vertex)
    }

    pub(crate) fn required_semantics(&self) -> GeometrySemantics {
        self.appearance.required_semantics
    }

    pub(crate) fn source_ir_nodes(&self) -> u64 {
        self.appearance.source_ir_nodes
    }

    pub(crate) fn final_ir_nodes(&self) -> u64 {
        self.appearance.final_ir_nodes
    }

    pub(crate) fn canonical_vertex_ir_nodes(&self) -> u64 {
        self.canonical_vertex.ir_nodes
    }

    pub(crate) fn source_spans_preserved(&self) -> bool {
        self.appearance.source_spans_preserved
    }

    pub(crate) fn synthetic_spans_are_undefined(&self) -> bool {
        self.appearance.synthetic_spans_are_undefined
            && self.canonical_vertex.synthetic_spans_are_undefined
    }

    pub(crate) fn abi_layout(&self) -> AbiLayout3D {
        self.appearance.abi_layout
    }

    pub(crate) fn fallback(&self) -> AppearanceFallbackBindingContract {
        self.appearance.fallback
    }

    pub(crate) fn validate_roundtrip(&self) -> Result<(), Appearance3DAdmissionError> {
        super::appearance::validate_wgsl_roundtrip(&self.appearance.module)?;
        super::appearance::validate_wgsl_roundtrip(&self.canonical_vertex.module)
    }

    pub(crate) fn emitted_shader_sources_for_test(
        &self,
    ) -> Result<(String, String), Appearance3DAdmissionError> {
        Ok((
            super::appearance::emitted_wgsl(&self.canonical_vertex.module)?,
            super::appearance::emitted_wgsl(&self.appearance.module)?,
        ))
    }

    pub(crate) fn fragment_wrapper_proof(
        &self,
    ) -> Result<(String, bool), Appearance3DAdmissionError> {
        super::appearance::fragment_wrapper_proof(&self.appearance.module)
    }

    pub(crate) fn constructs_direct_naga_sources(&self) -> bool {
        let fragment =
            wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(self.appearance.module.clone()));
        let vertex = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(
            self.canonical_vertex.module.clone(),
        ));
        matches!(fragment, wgpu::ShaderSource::Naga(_))
            && matches!(vertex, wgpu::ShaderSource::Naga(_))
    }
}

enum SlotState {
    Vacant {
        next_generation: u64,
    },
    Occupied {
        kind: ResourceKind3D,
        generation: u64,
        no_further_generations: bool,
        record: Arc<ResourceGeneration>,
    },
    Retiring {
        kind: ResourceKind3D,
        generation: u64,
        no_further_generations: bool,
        record: Weak<ResourceGeneration>,
    },
    Exhausted {
        last_generation: u64,
    },
    Controlled {
        kind: ControlledResourceKind3D,
        generation: u64,
        record: Arc<ControlledResourceRecord3D>,
    },
    ControlledRetiring {
        kind: ControlledResourceKind3D,
        generation: u64,
        record: Weak<ControlledResourceRecord3D>,
    },
}

struct ResourceSlot {
    state: SlotState,
    retired: Vec<Weak<ResourceGeneration>>,
    controlled_retired: Vec<Weak<ControlledResourceRecord3D>>,
}

struct ResourceRegistry {
    context: ContextId,
    limits: EffectiveLimits3D,
    generation_ceiling: u64,
    slots: Vec<ResourceSlot>,
    controlled_defaults: Option<(Weak<super::pbr::PbrState3D>, Vec<ResourceKey>)>,
}

impl ResourceRegistry {
    fn new(context: ContextId, limits: EffectiveLimits3D, generation_ceiling: u64) -> Self {
        Self {
            context,
            limits,
            generation_ceiling,
            slots: Vec::new(),
            controlled_defaults: None,
        }
    }

    fn insert(
        &mut self,
        payload: ResourcePayload,
    ) -> Result<(ResourceKey, Arc<ResourceGeneration>), Context3DError> {
        self.collect()?;
        self.admit_payload(&payload, None)?;

        let vacant = self
            .slots
            .iter()
            .position(|slot| matches!(&slot.state, SlotState::Vacant { .. }));
        let slot_index = if let Some(slot) = vacant {
            slot
        } else {
            let requested = u64::try_from(self.slots.len())
                .ok()
                .and_then(|count| count.checked_add(1))
                .ok_or(Context3DError::PolicyBudgetExceeded {
                    budget: PolicyBudget3D::ResourceSlots,
                    requested: u64::MAX,
                    maximum: u64::from(self.limits.policy.max_resource_slots),
                })?;
            if requested > u64::from(self.limits.policy.max_resource_slots) {
                if let Some((slot, _)) = self
                    .slots
                    .iter()
                    .enumerate()
                    .find(|(_, slot)| matches!(&slot.state, SlotState::Exhausted { .. }))
                {
                    let slot =
                        u32::try_from(slot).map_err(|_| Context3DError::ArithmeticOverflow {
                            operation: "representing an exhausted resource slot",
                        })?;
                    return Err(Context3DError::GenerationExhausted { slot });
                }
                return Err(Context3DError::PolicyBudgetExceeded {
                    budget: PolicyBudget3D::ResourceSlots,
                    requested,
                    maximum: u64::from(self.limits.policy.max_resource_slots),
                });
            }
            self.slots.push(ResourceSlot {
                state: SlotState::Vacant { next_generation: 1 },
                retired: Vec::new(),
                controlled_retired: Vec::new(),
            });
            self.slots.len() - 1
        };

        let generation = match &self.slots[slot_index].state {
            SlotState::Vacant { next_generation } => *next_generation,
            _ => {
                return Err(Context3DError::ArithmeticOverflow {
                    operation: "selecting a vacant resource slot",
                });
            }
        };
        let kind = payload.kind();
        let record = Arc::new(ResourceGeneration::new(payload));
        self.slots[slot_index].state = SlotState::Occupied {
            kind,
            generation,
            no_further_generations: generation >= self.generation_ceiling,
            record: record.clone(),
        };
        let requested_slot =
            u64::try_from(slot_index).map_err(|_| Context3DError::ArithmeticOverflow {
                operation: "representing a resource slot budget request",
            })?;
        let slot = u32::try_from(slot_index).map_err(|_| Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::ResourceSlots,
            requested: requested_slot,
            maximum: u64::from(self.limits.policy.max_resource_slots),
        })?;
        #[cfg(test)]
        super::audit::record_resource_generation();
        Ok((
            ResourceKey {
                context: self.context.clone(),
                kind,
                slot,
                generation,
            },
            record,
        ))
    }

    fn replace(
        &mut self,
        current: &ResourceKey,
        payload: ResourcePayload,
    ) -> Result<(ResourceKey, Arc<ResourceGeneration>), Context3DError> {
        self.collect()?;
        let expected = payload.kind();
        let (slot_index, old_record, generation, no_further_generations) =
            self.validate_current(current, expected)?;
        if no_further_generations {
            return Err(Context3DError::GenerationExhausted { slot: current.slot });
        }
        self.admit_payload(&payload, Some((&old_record, expected)))?;
        let next_generation = generation
            .checked_add(1)
            .filter(|generation| *generation <= self.generation_ceiling)
            .ok_or(Context3DError::GenerationExhausted { slot: current.slot })?;
        let record = Arc::new(ResourceGeneration::new(payload));
        let slot = &mut self.slots[slot_index];
        slot.retired.push(Arc::downgrade(&old_record));
        slot.state = SlotState::Occupied {
            kind: expected,
            generation: next_generation,
            no_further_generations: next_generation >= self.generation_ceiling,
            record: record.clone(),
        };
        #[cfg(test)]
        super::audit::record_resource_generation();
        Ok((
            ResourceKey {
                context: self.context.clone(),
                kind: expected,
                slot: current.slot,
                generation: next_generation,
            },
            record,
        ))
    }

    fn lease(
        &mut self,
        key: &ResourceKey,
        expected: ResourceKind3D,
    ) -> Result<Arc<ResourceGeneration>, Context3DError> {
        self.collect()?;
        let (_, record, _, _) = self.validate_current(key, expected)?;
        Ok(record)
    }

    fn retire(
        &mut self,
        key: &ResourceKey,
        expected: ResourceKind3D,
    ) -> Result<(), Context3DError> {
        self.collect()?;
        let (slot_index, record, generation, no_further_generations) =
            self.validate_current(key, expected)?;
        self.slots[slot_index].state = SlotState::Retiring {
            kind: expected,
            generation,
            no_further_generations,
            record: Arc::downgrade(&record),
        };
        Ok(())
    }

    fn validate_current(
        &self,
        key: &ResourceKey,
        expected: ResourceKind3D,
    ) -> Result<(usize, Arc<ResourceGeneration>, u64, bool), Context3DError> {
        if !self.context.same_as(&key.context) {
            return Err(Context3DError::ContextMismatch);
        }
        if key.kind != expected {
            return Err(Context3DError::WrongResourceKind {
                expected,
                actual: key.kind,
            });
        }
        let slot_index = usize::try_from(key.slot)
            .map_err(|_| Context3DError::SlotOutOfRange { slot: key.slot })?;
        let slot = self
            .slots
            .get(slot_index)
            .ok_or(Context3DError::SlotOutOfRange { slot: key.slot })?;
        match &slot.state {
            SlotState::Occupied {
                kind,
                generation,
                no_further_generations,
                record,
            } => {
                if *kind != expected {
                    return Err(Context3DError::WrongResourceKind {
                        expected,
                        actual: *kind,
                    });
                }
                if *generation != key.generation {
                    return Err(Context3DError::StaleGeneration {
                        slot: key.slot,
                        requested: key.generation,
                        live: Some(*generation),
                    });
                }
                Ok((
                    slot_index,
                    record.clone(),
                    *generation,
                    *no_further_generations,
                ))
            }
            SlotState::Retiring {
                generation, kind, ..
            } => {
                if *kind != expected {
                    return Err(Context3DError::WrongResourceKind {
                        expected,
                        actual: *kind,
                    });
                }
                self.freed_or_stale(key, *generation)
            }
            SlotState::Vacant { next_generation } => self.freed_or_stale(
                key,
                next_generation
                    .checked_sub(1)
                    .ok_or(Context3DError::ArithmeticOverflow {
                        operation: "recovering the last vacant-slot generation",
                    })?,
            ),
            SlotState::Exhausted { last_generation } => self.freed_or_stale(key, *last_generation),
            SlotState::Controlled { generation, .. }
            | SlotState::ControlledRetiring { generation, .. } => {
                self.freed_or_stale(key, *generation)
            }
        }
    }

    fn freed_or_stale<T>(
        &self,
        key: &ResourceKey,
        last_generation: u64,
    ) -> Result<T, Context3DError> {
        if key.generation == last_generation {
            Err(Context3DError::FreedSlot {
                slot: key.slot,
                generation: key.generation,
            })
        } else {
            Err(Context3DError::StaleGeneration {
                slot: key.slot,
                requested: key.generation,
                live: None,
            })
        }
    }

    fn admit_payload(
        &self,
        payload: &ResourcePayload,
        replacing: Option<(&Arc<ResourceGeneration>, ResourceKind3D)>,
    ) -> Result<(), Context3DError> {
        match payload {
            ResourcePayload::Geometry(descriptor) => {
                validate_hard_limit(
                    HardLimit3D::BufferSize,
                    descriptor.canonical_vertex_bytes(),
                    self.limits.hard.max_buffer_size,
                )?;
                validate_hard_limit(
                    HardLimit3D::BufferSize,
                    descriptor.canonical_index_bytes(),
                    self.limits.hard.max_buffer_size,
                )?;
                validate_policy_budget(
                    PolicyBudget3D::MeshBytes,
                    descriptor.canonical_total_bytes(),
                    self.limits.policy.max_mesh_bytes,
                )?;
                validate_policy_budget(
                    PolicyBudget3D::MeshVertices,
                    descriptor.vertex_count(),
                    u64::from(self.limits.policy.max_vertices_per_mesh),
                )?;
                validate_policy_budget(
                    PolicyBudget3D::MeshIndices,
                    descriptor.index_count(),
                    u64::from(self.limits.policy.max_indices_per_mesh),
                )?;
            }
            ResourcePayload::Appearance(resource) => validate_policy_budget(
                PolicyBudget3D::AppearanceSourceBytes,
                resource.descriptor.source_byte_len(),
                self.limits.policy.max_appearance_source_bytes,
            )?,
            ResourcePayload::Texture(descriptor) => {
                let maximum_dimension = u64::from(descriptor.width().max(descriptor.height()));
                validate_hard_limit(
                    HardLimit3D::TextureDimension2D,
                    maximum_dimension,
                    u64::from(self.limits.hard.max_texture_dimension_2d),
                )?;
                validate_policy_budget(
                    PolicyBudget3D::TextureDimension2D,
                    maximum_dimension,
                    u64::from(self.limits.policy.max_texture_dimension_2d),
                )?;
                validate_policy_budget(
                    PolicyBudget3D::TextureBytes,
                    descriptor.texel_byte_len(),
                    self.limits.policy.max_texture_bytes,
                )?;
            }
            ResourcePayload::Sampler(_) => {}
        }

        let usage = self.usage_without_collect()?;
        let old_is_retained = replacing
            .map(|(record, _)| record.has_lease_owners())
            .unwrap_or(false);
        let retained_after_replacement = if replacing.is_some() && !old_is_retained {
            usage
                .retained_generations
                .checked_sub(1)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "subtracting a replaced retained generation",
                })?
        } else {
            usage.retained_generations
        };
        let requested_generations = u64::from(retained_after_replacement).checked_add(1).ok_or(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::RetainedGenerations,
                requested: u64::MAX,
                maximum: u64::from(self.limits.policy.max_retained_generations),
            },
        )?;
        validate_policy_budget(
            PolicyBudget3D::RetainedGenerations,
            requested_generations,
            u64::from(self.limits.policy.max_retained_generations),
        )?;

        let replaced_bytes = replacing
            .filter(|_| !old_is_retained)
            .map(|(record, _)| record.payload.resident_bytes())
            .unwrap_or(0);
        let requested_cache = usage
            .retained_cpu_descriptor_bytes
            .checked_sub(replaced_bytes)
            .and_then(|bytes| bytes.checked_add(payload.resident_bytes()))
            .ok_or(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::CacheResidencyBytes,
                requested: u64::MAX,
                maximum: self.limits.policy.max_cache_residency_bytes,
            })?;
        validate_policy_budget(
            PolicyBudget3D::CacheResidencyBytes,
            requested_cache,
            self.limits.policy.max_cache_residency_bytes,
        )?;

        if matches!(payload, ResourcePayload::Appearance(_)) {
            let registration_delta = u64::from(replacing.is_none());
            let requested = u64::from(usage.registered_appearances)
                .checked_add(registration_delta)
                .ok_or(Context3DError::PolicyBudgetExceeded {
                    budget: PolicyBudget3D::RegisteredAppearances,
                    requested: u64::MAX,
                    maximum: u64::from(self.limits.policy.max_registered_appearances),
                })?;
            validate_policy_budget(
                PolicyBudget3D::RegisteredAppearances,
                requested,
                u64::from(self.limits.policy.max_registered_appearances),
            )?;
        }
        Ok(())
    }

    fn collect(&mut self) -> Result<(), Context3DError> {
        if self
            .controlled_defaults
            .as_ref()
            .is_some_and(|(owner, _)| owner.strong_count() == 0)
        {
            let (_, keys) = self
                .controlled_defaults
                .take()
                .expect("matched default family");
            for key in keys {
                if let Ok((index, record, generation, no_further_generations)) =
                    self.validate_current(&key, key.kind)
                {
                    self.slots[index].state = SlotState::Retiring {
                        kind: key.kind,
                        generation,
                        no_further_generations,
                        record: Arc::downgrade(&record),
                    };
                }
            }
        }
        for slot in &mut self.slots {
            slot.retired.retain(|record| record.strong_count() > 0);
            slot.controlled_retired
                .retain(|record| record.strong_count() > 0);
            let transition = match &slot.state {
                SlotState::Retiring {
                    generation,
                    no_further_generations,
                    record,
                    ..
                } if record.strong_count() == 0 => Some(if *no_further_generations {
                    SlotState::Exhausted {
                        last_generation: *generation,
                    }
                } else {
                    SlotState::Vacant {
                        next_generation: generation.checked_add(1).ok_or(
                            Context3DError::ArithmeticOverflow {
                                operation: "advancing a retired slot generation",
                            },
                        )?,
                    }
                }),
                SlotState::ControlledRetiring {
                    generation, record, ..
                } if record.strong_count() == 0 => {
                    Some(if *generation >= self.generation_ceiling {
                        SlotState::Exhausted {
                            last_generation: *generation,
                        }
                    } else {
                        SlotState::Vacant {
                            next_generation: generation.checked_add(1).ok_or(
                                Context3DError::ArithmeticOverflow {
                                    operation: "advancing a controlled slot generation",
                                },
                            )?,
                        }
                    })
                }
                _ => None,
            };
            if let Some(transition) = transition {
                slot.state = transition;
            }
        }
        Ok(())
    }

    fn usage(&mut self) -> Result<ResourceUsage3D, Context3DError> {
        self.collect()?;
        self.usage_without_collect()
    }

    fn usage_without_collect(&self) -> Result<ResourceUsage3D, Context3DError> {
        let mut usage = ResourceUsage3D::default();
        self.add_controlled_usage(&mut usage)?;
        for slot in &self.slots {
            match &slot.state {
                SlotState::Occupied { record, .. } => {
                    usage.live_resource_slots = usage.live_resource_slots.checked_add(1).ok_or(
                        Context3DError::ArithmeticOverflow {
                            operation: "counting live resource slots",
                        },
                    )?;
                    if matches!(&record.payload, ResourcePayload::Appearance(_)) {
                        usage.registered_appearances = usage
                            .registered_appearances
                            .checked_add(1)
                            .ok_or(Context3DError::ArithmeticOverflow {
                                operation: "counting registered appearances",
                            })?;
                    }
                    add_record_usage(&mut usage, record)?;
                }
                SlotState::Retiring { record, .. } => {
                    if let Some(record) = record.upgrade() {
                        usage.live_resource_slots = usage
                            .live_resource_slots
                            .checked_add(1)
                            .ok_or(Context3DError::ArithmeticOverflow {
                                operation: "counting retiring resource slots",
                            })?;
                        add_record_usage(&mut usage, &record)?;
                    }
                }
                SlotState::Vacant { .. }
                | SlotState::Exhausted { .. }
                | SlotState::Controlled { .. }
                | SlotState::ControlledRetiring { .. } => {}
            }
            for record in &slot.retired {
                if let Some(record) = record.upgrade() {
                    add_record_usage(&mut usage, &record)?;
                }
            }
        }
        Ok(usage)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlledResourceKind3D {
    PbrGeometry,
    PbrMaterial,
    EnvironmentDiffuse,
    EnvironmentSpecular,
    EnvironmentLut,
    StaticIrradiance,
    MaterialMipChain,
    MaterialMipSampler,
    LoadingPresenceProxy,
}

#[derive(Clone)]
pub(super) struct ControlledResourceKey3D {
    context: ContextId,
    kind: ControlledResourceKind3D,
    slot: u32,
    generation: u64,
}

impl PartialEq for ControlledResourceKey3D {
    fn eq(&self, other: &Self) -> bool {
        self.context.same_as(&other.context)
            && self.kind == other.kind
            && self.slot == other.slot
            && self.generation == other.generation
    }
}
impl Eq for ControlledResourceKey3D {}
impl ControlledResourceKey3D {
    pub(super) fn slot_generation(&self) -> (u32, u64) {
        (self.slot, self.generation)
    }
}
impl fmt::Debug for ControlledResourceKey3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControlledResourceKey3D")
            .field("kind", &self.kind)
            .field("slot", &self.slot)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum ControlledResourceError3D {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error("controlled resource kind mismatch: expected {expected:?}, got {actual:?}")]
    Kind {
        expected: ControlledResourceKind3D,
        actual: ControlledResourceKind3D,
    },
    #[error("controlled resource slot {slot} generation {generation} is no longer current")]
    Stale { slot: u32, generation: u64 },
    #[error("controlled resource payload does not match its admitted kind")]
    Payload,
}

pub(super) struct DeferredTexture3D {
    pub(super) lease: TextureGpuLease,
    pending: Option<(Arc<ResourceGeneration>, Texture3D)>,
    bytes_per_row: u32,
}

pub(super) struct DeferredGeometry3D {
    pub(super) lease: GeometryGpuLease,
    pending: Option<Arc<ResourceGeneration>>,
    vertices: Vec<u8>,
    indices: Vec<u8>,
    _staging: Option<DynamicStorageLease3D>,
}
impl DeferredGeometry3D {
    pub(super) fn commit(self, queue: &wgpu::Queue) {
        if let Some(record) = self.pending {
            queue.write_buffer(&self.lease.geometry.vertex_buffer, 0, &self.vertices);
            if let Some(index) = &self.lease.geometry.index_buffer {
                queue.write_buffer(index, 0, &self.indices);
            }
            if let Ok(mut cache) = record.materialized_geometry.lock() {
                if cache.is_none() {
                    *cache = Some(self.lease.geometry.clone());
                }
            }
        }
    }
}

impl GeometryGpuLease {
    pub(super) fn retained_clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            geometry: self.geometry.clone(),
            _ownership: self._ownership.clone(),
        }
    }
}
impl TextureGpuLease {
    pub(super) fn retained_clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            texture: self.texture.clone(),
            _ownership: self._ownership.clone(),
        }
    }
}
impl SamplerGpuLease {
    pub(super) fn retained_clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            sampler: self.sampler.clone(),
            _ownership: self._ownership.clone(),
        }
    }
}

impl DeferredTexture3D {
    pub(super) fn commit(self, queue: &wgpu::Queue) -> TextureGpuLease {
        if let Some((record, source)) = self.pending {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.lease.texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                source.texels(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_row),
                    rows_per_image: Some(source.height()),
                },
                wgpu::Extent3d {
                    width: source.width(),
                    height: source.height(),
                    depth_or_array_layers: 1,
                },
            );
            // A concurrent materialization may already own the canonical cache;
            // this independently charged lease stays valid until its own use ends.
            if let Ok(mut cache) = record.materialized_texture.lock() {
                if cache.is_none() {
                    *cache = Some(self.lease.texture.clone());
                }
            }
            #[cfg(test)]
            super::audit::record_gpu_texture();
        }
        self.lease
    }
}

pub(super) struct ControlledResourceRecord3D {
    source: Arc<dyn Any + Send + Sync>,
    bytes: u64,
}

#[derive(Clone)]
pub(super) struct ControlledResourceLease3D {
    pub(super) key: ControlledResourceKey3D,
    record: Arc<ControlledResourceRecord3D>,
}

impl ControlledResourceLease3D {
    pub(super) fn source<T: Any + Send + Sync>(&self) -> Result<Arc<T>, ControlledResourceError3D> {
        self.record
            .source
            .clone()
            .downcast()
            .map_err(|_| ControlledResourceError3D::Payload)
    }

    pub(super) fn belongs_to(&self, context: &Context3D) -> bool {
        self.key.context.same_as(&context.core.id)
    }
}

impl ResourceRegistry {
    fn add_controlled_usage(&self, usage: &mut ResourceUsage3D) -> Result<(), Context3DError> {
        let mut add = |record: &ControlledResourceRecord3D| -> Result<(), Context3DError> {
            usage.retained_generations = usage.retained_generations.checked_add(1).ok_or(
                Context3DError::ArithmeticOverflow {
                    operation: "counting private controlled generations",
                },
            )?;
            usage.retained_cpu_descriptor_bytes = usage
                .retained_cpu_descriptor_bytes
                .checked_add(record.bytes)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "charging private controlled source bytes",
                })?;
            Ok(())
        };
        for slot in &self.slots {
            match &slot.state {
                SlotState::Controlled { record, .. } => {
                    usage.live_resource_slots = usage.live_resource_slots.checked_add(1).ok_or(
                        Context3DError::ArithmeticOverflow {
                            operation: "counting controlled slots",
                        },
                    )?;
                    add(record)?;
                }
                SlotState::ControlledRetiring { record, .. } => {
                    if let Some(record) = record.upgrade() {
                        usage.live_resource_slots = usage
                            .live_resource_slots
                            .checked_add(1)
                            .ok_or(Context3DError::ArithmeticOverflow {
                                operation: "counting retiring controlled slots",
                            })?;
                        add(&record)?;
                    }
                }
                _ => {}
            }
            for record in slot.controlled_retired.iter().filter_map(Weak::upgrade) {
                add(&record)?;
            }
        }
        Ok(())
    }

    fn controlled_current(
        &self,
        key: &ControlledResourceKey3D,
        kind: ControlledResourceKind3D,
    ) -> Result<Arc<ControlledResourceRecord3D>, ControlledResourceError3D> {
        if !key.context.same_as(&self.context) {
            return Err(Context3DError::ContextMismatch.into());
        }
        if key.kind != kind {
            return Err(ControlledResourceError3D::Kind {
                expected: kind,
                actual: key.kind,
            });
        }
        let slot = self
            .slots
            .get(key.slot as usize)
            .ok_or(ControlledResourceError3D::Stale {
                slot: key.slot,
                generation: key.generation,
            })?;
        match &slot.state {
            SlotState::Controlled {
                kind: actual,
                generation,
                record,
            } => {
                if *actual != kind {
                    return Err(ControlledResourceError3D::Kind {
                        expected: kind,
                        actual: *actual,
                    });
                }
                if *generation == key.generation {
                    return Ok(record.clone());
                }
            }
            SlotState::ControlledRetiring { kind: actual, .. } if *actual != kind => {
                return Err(ControlledResourceError3D::Kind {
                    expected: kind,
                    actual: *actual,
                });
            }
            _ => {}
        }
        Err(ControlledResourceError3D::Stale {
            slot: key.slot,
            generation: key.generation,
        })
    }

    fn insert_controlled(
        &mut self,
        kind: ControlledResourceKind3D,
        source: Arc<dyn Any + Send + Sync>,
        bytes: u64,
        previous: Option<&ControlledResourceKey3D>,
    ) -> Result<ControlledResourceKey3D, ControlledResourceError3D> {
        self.insert_controlled_with(kind, || source, bytes, previous)
    }

    fn insert_controlled_with(
        &mut self,
        kind: ControlledResourceKind3D,
        source: impl FnOnce() -> Arc<dyn Any + Send + Sync>,
        bytes: u64,
        previous: Option<&ControlledResourceKey3D>,
    ) -> Result<ControlledResourceKey3D, ControlledResourceError3D> {
        self.collect()?;
        let replaced = previous
            .map(|key| self.controlled_current(key, kind))
            .transpose()?;
        let reusable = self
            .slots
            .iter()
            .position(|s| matches!(s.state, SlotState::Vacant { .. }));
        let slot_index = previous
            .map(|k| k.slot as usize)
            .or(reusable)
            .unwrap_or(self.slots.len());
        let new_slot = slot_index == self.slots.len();
        let generation = if let Some(key) = previous {
            key.generation.checked_add(1)
        } else if new_slot {
            Some(1)
        } else {
            match self.slots[slot_index].state {
                SlotState::Vacant { next_generation } => Some(next_generation),
                _ => None,
            }
        }
        .filter(|g| *g <= self.generation_ceiling)
        .ok_or(Context3DError::GenerationExhausted {
            slot: slot_index as u32,
        })?;
        let retained_old = replaced.as_ref().is_some_and(|r| Arc::strong_count(r) > 2);
        let usage = self.usage_without_collect()?;
        let old_bytes = replaced
            .as_ref()
            .filter(|_| !retained_old)
            .map_or(0, |r| r.bytes);
        let removed = u32::from(replaced.is_some() && !retained_old);
        validate_policy_budget(
            PolicyBudget3D::ResourceSlots,
            (self.slots.len() as u64)
                .checked_add(u64::from(new_slot))
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "counting shared resource slots",
                })?,
            u64::from(self.limits.policy.max_resource_slots),
        )?;
        validate_policy_budget(
            PolicyBudget3D::RetainedGenerations,
            u64::from(
                usage
                    .retained_generations
                    .checked_sub(removed)
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Context3DError::ArithmeticOverflow {
                        operation: "charging private controlled generations",
                    })?,
            ),
            u64::from(self.limits.policy.max_retained_generations),
        )?;
        validate_policy_budget(
            PolicyBudget3D::CacheResidencyBytes,
            usage
                .retained_cpu_descriptor_bytes
                .checked_sub(old_bytes)
                .and_then(|n| n.checked_add(bytes))
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "charging private controlled metadata",
                })?,
            self.limits.policy.max_cache_residency_bytes,
        )?;
        // HAL381's borrowed sealed input must not acquire strong Context custody
        // until the same existing slot/generation/byte checks have succeeded.
        let record = Arc::new(ControlledResourceRecord3D {
            source: source(),
            bytes,
        });
        if new_slot {
            self.slots.push(ResourceSlot {
                state: SlotState::Vacant {
                    next_generation: generation,
                },
                retired: Vec::new(),
                controlled_retired: Vec::new(),
            });
        }
        let slot = &mut self.slots[slot_index];
        if let Some(old) = replaced {
            slot.controlled_retired.push(Arc::downgrade(&old));
        }
        slot.state = SlotState::Controlled {
            kind,
            generation,
            record,
        };
        #[cfg(test)]
        super::audit::record_resource_generation();
        Ok(ControlledResourceKey3D {
            context: self.context.clone(),
            kind,
            slot: slot_index as u32,
            generation,
        })
    }
}

impl Context3D {
    pub(super) fn register_controlled_source_with<T: Any + Send + Sync>(
        &self,
        kind: ControlledResourceKind3D,
        bytes: u64,
        source: impl FnOnce() -> Arc<T>,
    ) -> Result<(ControlledResourceKey3D, ControlledResourceLease3D), ControlledResourceError3D>
    {
        let mut registry = self.registry()?;
        let key = registry.insert_controlled_with(kind, || source(), bytes, None)?;
        let record = registry.controlled_current(&key, kind)?;
        let lease = ControlledResourceLease3D {
            key: key.clone(),
            record,
        };
        Ok((key, lease))
    }

    pub(super) fn register_controlled_source<T: Any + Send + Sync>(
        &self,
        kind: ControlledResourceKind3D,
        source: Arc<T>,
        bytes: u64,
        previous: Option<&ControlledResourceKey3D>,
    ) -> Result<ControlledResourceKey3D, ControlledResourceError3D> {
        self.registry()?
            .insert_controlled(kind, source, bytes, previous)
    }

    pub(super) fn pbr_state(&self) -> Result<Arc<super::pbr::PbrState3D>, super::pbr::PbrError3D> {
        let mut held = self
            .core
            .pbr_state
            .lock()
            .map_err(|_| super::pbr::PbrError3D::Lock("PBR state"))?;
        if let Some(state) = held.upgrade() {
            return Ok(state);
        }
        let state = Arc::new(super::pbr::PbrState3D::new(self)?);
        let keys = state
            .default_handles()
            .into_iter()
            .flat_map(|(t, s)| [t.key, s.key])
            .collect();
        self.registry()?.controlled_defaults = Some((Arc::downgrade(&state), keys));
        *held = Arc::downgrade(&state);
        Ok(state)
    }

    pub(super) fn reserve_controlled_geometry_bytes(
        &self,
        bytes: u64,
    ) -> Result<GeometryGpuReservation<'_>, Context3DError> {
        self.core
            .geometry_gpu_accounting
            .reserve_resident_bytes(bytes, self.core.limits.policy.max_resident_geometry_bytes)
    }

    pub(super) fn reserve_shadow_texture_bytes(
        &self,
        bytes: u64,
    ) -> Result<TextureGpuResidency, Context3DError> {
        validate_policy_budget(
            PolicyBudget3D::TextureBytes,
            bytes,
            self.core.limits.policy.max_texture_bytes,
        )?;
        let fallback = self.raster_gpu()?.resident_texture_bytes();
        Ok(self
            .core
            .texture_gpu_accounting
            .reserve_resident_bytes(
                bytes,
                fallback,
                self.core.limits.policy.max_resident_texture_bytes,
            )?
            .commit())
    }

    /// Prepare an immutable source texture without enqueueing any upload. The
    /// whole target candidate reserves/validates first, then commits its writes.
    pub(super) fn prepare_controlled_texture(
        &self,
        lease: &TextureLease,
        upload: &mut PreparationUploadBudget3D,
    ) -> Result<DeferredTexture3D, Context3DError> {
        if !self.core.id.same_as(&lease.handle.key.context)
            || !self.core.id.same_as(&upload.context)
        {
            return Err(Context3DError::ContextMismatch);
        }
        let ownership = lease._ownership.clone();
        let record = ownership.record.clone();
        let cached = record
            .materialized_texture
            .lock()
            .map_err(|_| Context3DError::TextureCacheUnavailable)?;
        if let Some(texture) = cached.as_ref() {
            return Ok(DeferredTexture3D {
                lease: TextureGpuLease {
                    handle: lease.handle.clone(),
                    texture: texture.clone(),
                    _ownership: ownership,
                },
                pending: None,
                bytes_per_row: 0,
            });
        }
        drop(cached);
        let descriptor = lease.descriptor.clone();
        let bytes_per_row =
            descriptor
                .width()
                .checked_mul(4)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "sizing controlled texture upload row",
                })?;
        let bytes = descriptor.texel_byte_len();
        let remaining = upload.remaining_after(bytes)?;
        let gpu = self.gpu_resources()?;
        let raster = self.raster_gpu()?;
        let reserved = self.core.texture_gpu_accounting.reserve_resident_bytes(
            bytes,
            raster.resident_texture_bytes(),
            self.limits().policy.max_resident_texture_bytes,
        )?;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("R024 qualified immutable material source"),
            size: wgpu::Extent3d {
                width: descriptor.width(),
                height: descriptor.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: descriptor.format().as_wgpu(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let texture = Arc::new(MaterializedTexture {
            texture,
            view,
            upload_bytes: bytes,
            residency: reserved.commit(),
        });
        upload.remaining_bytes = remaining;
        Ok(DeferredTexture3D {
            lease: TextureGpuLease {
                handle: lease.handle.clone(),
                texture,
                _ownership: ownership,
            },
            pending: Some((record, descriptor)),
            bytes_per_row,
        })
    }

    pub(super) fn prepare_controlled_legacy_geometry(
        &self,
        lease: &GeometryLease,
        upload: &mut PreparationUploadBudget3D,
    ) -> Result<DeferredGeometry3D, Context3DError> {
        if !self.core.id.same_as(&lease.handle.key.context)
            || !self.core.id.same_as(&upload.context)
        {
            return Err(Context3DError::ContextMismatch);
        }
        let ownership = lease._ownership.clone();
        let record = ownership.record.clone();
        let cached = record
            .materialized_geometry
            .lock()
            .map_err(|_| Context3DError::GeometryCacheUnavailable)?;
        if let Some(geometry) = cached.as_ref() {
            return Ok(DeferredGeometry3D {
                lease: GeometryGpuLease {
                    handle: lease.handle.clone(),
                    geometry: geometry.clone(),
                    _ownership: ownership,
                },
                pending: None,
                vertices: Vec::new(),
                indices: Vec::new(),
                _staging: None,
            });
        }
        drop(cached);
        let packed = CanonicalGeometryBytes::pack(&lease.descriptor)?;
        let mut indices = packed.index_bytes().to_vec();
        let index_bytes = usize::try_from(packed.index_buffer_size()).map_err(|_| {
            Context3DError::ArithmeticOverflow {
                operation: "addressing controlled index upload",
            }
        })?;
        indices
            .try_reserve_exact(index_bytes - indices.len())
            .map_err(|_| Context3DError::ArithmeticOverflow {
                operation: "reserving controlled index upload",
            })?;
        indices.resize(index_bytes, 0);
        let vertices = packed.vertex_bytes().to_vec();
        let bytes = (vertices.len() as u64)
            .checked_add(indices.len() as u64)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "sizing deferred legacy geometry",
            })?;
        validate_hard_limit(
            HardLimit3D::BufferSize,
            vertices.len() as u64,
            self.limits().hard.max_buffer_size,
        )?;
        validate_hard_limit(
            HardLimit3D::BufferSize,
            indices.len() as u64,
            self.limits().hard.max_buffer_size,
        )?;
        validate_policy_budget(
            PolicyBudget3D::MeshBytes,
            bytes,
            self.limits().policy.max_mesh_bytes,
        )?;
        let remaining = upload.remaining_after(bytes)?;
        let staging = self.reserve_dynamic_storage(0, 0, 0, bytes)?;
        let reservation = self.reserve_controlled_geometry_bytes(bytes)?;
        let gpu = self.gpu_resources()?;
        let vertex_buffer = Arc::new(gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("private scene canonical legacy vertices"),
            size: vertices.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let index_buffer = if indices.is_empty() {
            None
        } else {
            Some(Arc::new(gpu.device.create_buffer(
                &wgpu::BufferDescriptor {
                    label: Some("private scene canonical legacy indices"),
                    size: indices.len() as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            )))
        };
        let geometry = Arc::new(MaterializedGeometry {
            device: Arc::downgrade(&gpu.device),
            vertex_buffer,
            index_buffer,
            vertex_count: packed.vertex_count(),
            index_count: packed.index_count(),
            index_format: packed.index_format(),
            upload_bytes: bytes,
            residency: reservation.commit(),
        });
        upload.remaining_bytes = remaining;
        Ok(DeferredGeometry3D {
            lease: GeometryGpuLease {
                handle: lease.handle.clone(),
                geometry,
                _ownership: ownership,
            },
            pending: Some(record),
            vertices,
            indices,
            _staging: Some(staging),
        })
    }

    pub(super) fn lease_controlled_source(
        &self,
        key: &ControlledResourceKey3D,
        kind: ControlledResourceKind3D,
    ) -> Result<ControlledResourceLease3D, ControlledResourceError3D> {
        let record = self.registry()?.controlled_current(key, kind)?;
        Ok(ControlledResourceLease3D {
            key: key.clone(),
            record,
        })
    }

    pub(super) fn retire_controlled_source(
        &self,
        key: &ControlledResourceKey3D,
        kind: ControlledResourceKind3D,
    ) -> Result<(), ControlledResourceError3D> {
        let mut registry = self.registry()?;
        let record = registry.controlled_current(key, kind)?;
        let slot = &mut registry.slots[key.slot as usize];
        slot.state = SlotState::ControlledRetiring {
            kind,
            generation: key.generation,
            record: Arc::downgrade(&record),
        };
        Ok(())
    }
}

fn add_record_usage(
    usage: &mut ResourceUsage3D,
    record: &ResourceGeneration,
) -> Result<(), Context3DError> {
    usage.retained_generations =
        usage
            .retained_generations
            .checked_add(1)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting retained generations",
            })?;
    let bytes = record.payload.resident_bytes();
    usage.retained_cpu_descriptor_bytes = usage
        .retained_cpu_descriptor_bytes
        .checked_add(bytes)
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "summing retained CPU descriptor bytes",
        })?;
    match &record.payload {
        ResourcePayload::Appearance(_) => {
            usage.retained_appearance_source_bytes = usage
                .retained_appearance_source_bytes
                .checked_add(bytes)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "summing retained appearance source bytes",
                })?;
        }
        ResourcePayload::Texture(_) => {
            usage.retained_texture_source_bytes = usage
                .retained_texture_source_bytes
                .checked_add(bytes)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "summing retained texture source bytes",
                })?;
        }
        ResourcePayload::Sampler(_) => {
            usage.retained_sampler_descriptor_bytes = usage
                .retained_sampler_descriptor_bytes
                .checked_add(bytes)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "summing retained sampler descriptor bytes",
                })?;
        }
        ResourcePayload::Geometry(_) => {}
    }
    Ok(())
}

pub(super) fn validate_policy_budget(
    budget: PolicyBudget3D,
    requested: u64,
    maximum: u64,
) -> Result<(), Context3DError> {
    if requested > maximum {
        return Err(Context3DError::PolicyBudgetExceeded {
            budget,
            requested,
            maximum,
        });
    }
    Ok(())
}

fn validate_domain_retention_budget(
    budget: DomainRetentionBudget3D,
    requested: u64,
    maximum: u64,
) -> Result<(), DomainRetentionError3D> {
    if requested > maximum {
        return Err(DomainRetentionError3D::BudgetExceeded {
            budget,
            requested,
            maximum,
        });
    }
    Ok(())
}
