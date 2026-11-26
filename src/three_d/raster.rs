use super::mip_sampling::{
    MaterialSamplerGpu3D, MaterialSamplerLease3D, MaterialTextureGpu3D, MaterialTextureLease3D,
    SamplingFamily3D,
};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Condvar, Mutex};

use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::renderer::wgpu_renderer::GpuResources;

use super::appearance::{
    APPEARANCE_BLOCK_SIZE, AppearanceBlockBytes, OBJECT_BLOCK_SIZE, ObjectBlockBytes,
    VIEW_BLOCK_SIZE, ViewBlockBytes,
};
use super::context::{
    AppearanceGpuLease, AppearanceTextureGpuLease, DynamicStorageLease3D, GeometryGpuLease,
};
use super::{
    AbiLayout3D, AppearanceMode, CANONICAL_COLOR_OFFSET, CANONICAL_NORMAL_OFFSET,
    CANONICAL_POSITION_OFFSET, CANONICAL_UV_OFFSET, CANONICAL_VERTEX_STRIDE, CanonicalIndexFormat,
    Context3D, Context3DError, Math3DError, ModelState, ModelStateError, PolicyBudget3D,
    PolicyLimits3D, RasterMode, RenderSnapshot, ViewDescriptor3, WorldPoint3,
};

const ABI_VERSION: u32 = 1;
pub(super) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub(super) const FALLBACK_TEXTURE_BYTES: u64 = 4;
const DEFAULT_COLOR_FACTOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: CANONICAL_POSITION_OFFSET,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: CANONICAL_NORMAL_OFFSET,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: CANONICAL_UV_OFFSET,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Unorm8x4,
        offset: CANONICAL_COLOR_OFFSET,
        shader_location: 3,
    },
];

#[derive(Debug, Error)]
pub(crate) enum HeadlessRaster3DError {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error(transparent)]
    Math(#[from] Math3DError),
    #[error(transparent)]
    Model(#[from] ModelStateError),
    #[error("headless 3D target extent must be nonzero, got {width}x{height}")]
    InactiveExtent { width: u32, height: u32 },
    #[error("unsupported baseline 3D sample count {requested}; expected 1 or 4")]
    UnsupportedSampleCount { requested: u32 },
    #[error("target format {format:?} does not support the required sample count/usages")]
    UnsupportedTargetFormat {
        format: wgpu::TextureFormat,
        sample_count: u32,
    },
    #[error("Depth32Float does not support the required sample count/usages")]
    UnsupportedDepthFormat { sample_count: u32 },
    #[error("unable to reserve bounded {block} uniform storage for {bytes} bytes")]
    UniformStorageUnavailable { block: &'static str, bytes: u64 },
    #[error("dynamic uniform offset {offset} is not representable as u32")]
    DynamicOffsetOutOfRange { offset: u64 },
    #[error("render snapshot packet and committed-lease arrays are inconsistent")]
    SnapshotInvariant,
    #[cfg(test)]
    #[error("headless 3D readback supports only four-byte RGBA/BGRA formats, got {format:?}")]
    UnsupportedReadbackFormat { format: wgpu::TextureFormat },
    #[cfg(test)]
    #[error("headless 3D readback mapping failed")]
    ReadbackFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GpuLayoutContract3D {
    view_min_size: u64,
    object_min_size: u64,
    appearance_min_size: u64,
    view_dynamic_stride: u64,
    object_dynamic_stride: u64,
    appearance_dynamic_stride: u64,
    view_has_dynamic_offset: bool,
    object_has_dynamic_offset: bool,
    appearance_has_dynamic_offset: bool,
    exact_bind_group_count: u32,
}

impl GpuLayoutContract3D {
    #[cfg(test)]
    pub(crate) const fn view_min_size(self) -> u64 {
        self.view_min_size
    }

    #[cfg(test)]
    pub(crate) const fn object_min_size(self) -> u64 {
        self.object_min_size
    }

    #[cfg(test)]
    pub(crate) const fn appearance_min_size(self) -> u64 {
        self.appearance_min_size
    }

    #[cfg(test)]
    pub(crate) const fn dynamic_strides(self) -> [u64; 3] {
        [
            self.view_dynamic_stride,
            self.object_dynamic_stride,
            self.appearance_dynamic_stride,
        ]
    }

    #[cfg(test)]
    pub(crate) const fn all_uniforms_use_dynamic_offsets(self) -> bool {
        self.view_has_dynamic_offset
            && self.object_has_dynamic_offset
            && self.appearance_has_dynamic_offset
    }

    #[cfg(test)]
    pub(crate) const fn exact_bind_group_count(self) -> u32 {
        self.exact_bind_group_count
    }
}

struct BindGroupLayouts3D {
    view: wgpu::BindGroupLayout,
    object: wgpu::BindGroupLayout,
    appearance: wgpu::BindGroupLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DepthStateKey {
    format: wgpu::TextureFormat,
    write_enabled: bool,
    compare: wgpu::CompareFunction,
    bias_constant: i32,
    bias_slope_scale_bits: u32,
    bias_clamp_bits: u32,
}

impl DepthStateKey {
    const REVERSE_Z: Self = Self {
        format: DEPTH_FORMAT,
        write_enabled: true,
        compare: wgpu::CompareFunction::Greater,
        bias_constant: 0,
        bias_slope_scale_bits: 0.0_f32.to_bits(),
        bias_clamp_bits: 0.0_f32.to_bits(),
    };

    const fn as_wgpu(self) -> wgpu::DepthStencilState {
        wgpu::DepthStencilState {
            format: self.format,
            depth_write_enabled: Some(self.write_enabled),
            depth_compare: Some(self.compare),
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState::IGNORE,
                back: wgpu::StencilFaceState::IGNORE,
                read_mask: 0,
                write_mask: 0,
            },
            bias: wgpu::DepthBiasState {
                constant: self.bias_constant,
                slope_scale: f32::from_bits(self.bias_slope_scale_bits),
                clamp: f32::from_bits(self.bias_clamp_bits),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ColorTargetLayout3D {
    Single(wgpu::TextureFormat),
    HdrMrt4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    abi_version: u32,
    appearance_source_hash: [u8; 32],
    appearance_mode: AppearanceMode,
    raster_mode: RasterMode,
    color_target: ColorTargetLayout3D,
    depth: DepthStateKey,
    sample_count: u32,
}

impl PipelineKey {
    fn new(
        appearance: &AppearanceGpuLease,
        raster_mode: RasterMode,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        Self {
            abi_version: ABI_VERSION,
            appearance_source_hash: appearance.source_hash(),
            appearance_mode: appearance.mode(),
            raster_mode,
            color_target: ColorTargetLayout3D::Single(color_format),
            depth: DepthStateKey::REVERSE_Z,
            sample_count,
        }
    }
}

struct PipelineRequest<'a> {
    key: PipelineKey,
    appearance: &'a AppearanceGpuLease,
}

struct PipelineReservation {
    completed: Mutex<bool>,
    completion: Condvar,
}

impl PipelineReservation {
    fn new() -> Self {
        Self {
            completed: Mutex::new(false),
            completion: Condvar::new(),
        }
    }

    fn wait(&self) -> Result<(), Context3DError> {
        let mut completed = self
            .completed
            .lock()
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
        while !*completed {
            completed = self
                .completion
                .wait(completed)
                .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
        }
        Ok(())
    }

    fn complete(&self) {
        let mut completed = match self.completed.lock() {
            Ok(completed) => completed,
            Err(poisoned) => poisoned.into_inner(),
        };
        *completed = true;
        self.completion.notify_all();
    }
}

enum PipelineCacheEntry {
    Reserved(Arc<PipelineReservation>),
    Ready(Arc<wgpu::RenderPipeline>),
}

#[derive(Default)]
struct PipelineCache {
    entries: HashMap<PipelineKey, PipelineCacheEntry>,
    controlled_reserved: u32,
    controlled_ready: u32,
}

impl PipelineCache {
    fn ready_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| matches!(entry, PipelineCacheEntry::Ready(_)))
            .count()
            + self.controlled_ready as usize
    }
}

struct OwnedPipelineReservation<'appearance> {
    key: PipelineKey,
    appearance: &'appearance AppearanceGpuLease,
    reservation: Arc<PipelineReservation>,
}

struct PipelineReservationBatch<'cache, 'appearance> {
    cache: &'cache Mutex<PipelineCache>,
    owned: Vec<OwnedPipelineReservation<'appearance>>,
    active: bool,
}

impl<'appearance> PipelineReservationBatch<'_, 'appearance> {
    fn is_empty(&self) -> bool {
        self.owned.is_empty()
    }

    fn owned(&self) -> &[OwnedPipelineReservation<'appearance>] {
        &self.owned
    }

    fn publish(mut self, pipelines: Vec<Arc<wgpu::RenderPipeline>>) -> Result<(), Context3DError> {
        if pipelines.len() != self.owned.len() {
            return Err(Context3DError::PipelineCacheUnavailable);
        }

        let mut cache = self
            .cache
            .lock()
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
        for owned in &self.owned {
            match cache.entries.get(&owned.key) {
                Some(PipelineCacheEntry::Reserved(reservation))
                    if Arc::ptr_eq(reservation, &owned.reservation) => {}
                _ => return Err(Context3DError::PipelineCacheUnavailable),
            }
        }
        for (owned, pipeline) in self.owned.iter().zip(pipelines) {
            cache
                .entries
                .insert(owned.key, PipelineCacheEntry::Ready(pipeline));
        }
        self.active = false;
        drop(cache);
        for owned in &self.owned {
            owned.reservation.complete();
        }
        Ok(())
    }
}

impl Drop for PipelineReservationBatch<'_, '_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let mut cache = match self.cache.lock() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        for owned in &self.owned {
            let owns_entry = matches!(
                cache.entries.get(&owned.key),
                Some(PipelineCacheEntry::Reserved(reservation))
                    if Arc::ptr_eq(reservation, &owned.reservation)
            );
            if owns_entry {
                cache.entries.remove(&owned.key);
            }
        }
        drop(cache);
        for owned in &self.owned {
            owned.reservation.complete();
        }
    }
}

enum ValidatedDraw {
    Unindexed,
    Indexed16(wgpu::Buffer),
    Indexed32(wgpu::Buffer),
}

pub(super) struct GpuState3D {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    layouts: BindGroupLayouts3D,
    pipeline_layout: wgpu::PipelineLayout,
    fallback_texture: wgpu::Texture,
    fallback_view: wgpu::TextureView,
    fallback_sampler: wgpu::Sampler,
    layout_contract: GpuLayoutContract3D,
    pipelines: Arc<Mutex<PipelineCache>>,
}

// Retain the shared accounting/cache, never the Device/Queue owner in a Queue
// completion callback. A strong GpuState3D here would form a retirement cycle.
pub(super) struct ControlledPipelineResidency {
    cache: Arc<Mutex<PipelineCache>>,
    ready: bool,
}

impl ControlledPipelineResidency {
    pub(super) fn commit(&mut self) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        assert!(!self.ready);
        cache.controlled_reserved = cache
            .controlled_reserved
            .checked_sub(1)
            .expect("reserved controlled pipeline");
        cache.controlled_ready = cache
            .controlled_ready
            .checked_add(1)
            .expect("bounded controlled pipeline count");
        self.ready = true;
    }
}

impl Drop for ControlledPipelineResidency {
    fn drop(&mut self) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let count = if self.ready {
            &mut cache.controlled_ready
        } else {
            &mut cache.controlled_reserved
        };
        *count = count
            .checked_sub(1)
            .expect("controlled pipeline residency released once");
    }
}

impl GpuState3D {
    pub(super) fn reserve_controlled_pipeline(
        self: &Arc<Self>,
        maximum: u32,
    ) -> Result<ControlledPipelineResidency, Context3DError> {
        let mut cache = self
            .pipelines
            .lock()
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
        let requested = cache.entries.len() as u64
            + u64::from(cache.controlled_reserved)
            + u64::from(cache.controlled_ready)
            + 1;
        validate_budget(
            PolicyBudget3D::PipelineVariants,
            requested,
            u64::from(maximum),
        )?;
        cache.controlled_reserved =
            cache
                .controlled_reserved
                .checked_add(1)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "reserving controlled pipeline",
                })?;
        Ok(ControlledPipelineResidency {
            cache: self.pipelines.clone(),
            ready: false,
        })
    }

    pub(super) fn new(
        gpu: &GpuResources,
        abi: AbiLayout3D,
        policy: PolicyLimits3D,
    ) -> Result<Self, Context3DError> {
        validate_budget(
            PolicyBudget3D::TextureBytes,
            FALLBACK_TEXTURE_BYTES,
            policy.max_texture_bytes,
        )?;
        validate_budget(
            PolicyBudget3D::ResidentTextureBytes,
            FALLBACK_TEXTURE_BYTES,
            policy.max_resident_texture_bytes,
        )?;

        let view = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Halcyon 3D View Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(VIEW_BLOCK_SIZE),
                    },
                    count: None,
                }],
            });
        let object = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Halcyon 3D Object Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(OBJECT_BLOCK_SIZE),
                    },
                    count: None,
                }],
            });
        let appearance = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Halcyon 3D Appearance Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: NonZeroU64::new(APPEARANCE_BLOCK_SIZE),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Halcyon 3D Canonical Pipeline Layout"),
                bind_group_layouts: &[Some(&view), Some(&object), Some(&appearance)],
                immediate_size: 0,
            });

        let fallback_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Halcyon 3D Opaque White Fallback Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &fallback_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let fallback_view = fallback_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let fallback_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Halcyon 3D Fixed Filtering Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            layouts: BindGroupLayouts3D {
                view,
                object,
                appearance,
            },
            pipeline_layout,
            fallback_texture,
            fallback_view,
            fallback_sampler,
            layout_contract: GpuLayoutContract3D {
                view_min_size: VIEW_BLOCK_SIZE,
                object_min_size: OBJECT_BLOCK_SIZE,
                appearance_min_size: APPEARANCE_BLOCK_SIZE,
                view_dynamic_stride: abi.view_dynamic_stride(),
                object_dynamic_stride: abi.object_dynamic_stride(),
                appearance_dynamic_stride: abi.appearance_dynamic_stride(),
                view_has_dynamic_offset: true,
                object_has_dynamic_offset: true,
                appearance_has_dynamic_offset: true,
                exact_bind_group_count: 3,
            },
            pipelines: Arc::new(Mutex::new(PipelineCache::default())),
        })
    }

    pub(super) fn resident_texture_bytes(&self) -> u64 {
        let _ = &self.fallback_texture;
        FALLBACK_TEXTURE_BYTES
    }

    pub(super) fn pipeline_count(&self) -> Result<u32, Context3DError> {
        let count = self
            .pipelines
            .lock()
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?
            .ready_count();
        u32::try_from(count).map_err(|_| Context3DError::ArithmeticOverflow {
            operation: "representing resident 3D pipeline variants",
        })
    }

    fn prepare_pipelines(
        &self,
        context: &Context3D,
        requests: &[PipelineRequest<'_>],
        maximum: u32,
    ) -> Result<Vec<Arc<wgpu::RenderPipeline>>, Context3DError> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(requests.len())
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;

        loop {
            let reservations = self.reserve_pipeline_misses(requests, maximum)?;
            if !reservations.is_empty() {
                let mut pipelines = Vec::new();
                pipelines
                    .try_reserve_exact(reservations.owned().len())
                    .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
                for owned in reservations.owned() {
                    pipelines.push(self.create_pipeline(context, owned.key, owned.appearance)?);
                }
                reservations.publish(pipelines)?;
            }

            let waiting = {
                let cache = self
                    .pipelines
                    .lock()
                    .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
                result.clear();
                let mut waiting = None;
                for request in requests {
                    match cache.entries.get(&request.key) {
                        Some(PipelineCacheEntry::Ready(pipeline)) => {
                            result.push(pipeline.clone());
                        }
                        Some(PipelineCacheEntry::Reserved(reservation)) => {
                            waiting = Some(reservation.clone());
                            break;
                        }
                        None => break,
                    }
                }
                if result.len() == requests.len() {
                    return Ok(result);
                }
                waiting
            };

            if let Some(reservation) = waiting {
                reservation.wait()?;
            }
        }
    }

    fn reserve_pipeline_misses<'appearance>(
        &self,
        requests: &[PipelineRequest<'appearance>],
        maximum: u32,
    ) -> Result<PipelineReservationBatch<'_, 'appearance>, Context3DError> {
        let mut missing = Vec::new();
        missing
            .try_reserve_exact(requests.len())
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;

        let mut cache = self
            .pipelines
            .lock()
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
        for request in requests {
            if !cache.entries.contains_key(&request.key)
                && !missing
                    .iter()
                    .any(|(key, _): &(PipelineKey, &AppearanceGpuLease)| *key == request.key)
            {
                missing.push((request.key, request.appearance));
            }
        }

        let requested = cache
            .entries
            .len()
            .checked_add(cache.controlled_reserved as usize)
            .and_then(|count| count.checked_add(cache.controlled_ready as usize))
            .and_then(|count| count.checked_add(missing.len()))
            .ok_or(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::PipelineVariants,
                requested: u64::MAX,
                maximum: u64::from(maximum),
            })?;
        validate_budget(
            PolicyBudget3D::PipelineVariants,
            u64::try_from(requested).map_err(|_| Context3DError::ArithmeticOverflow {
                operation: "representing requested 3D pipeline variants",
            })?,
            u64::from(maximum),
        )?;
        cache
            .entries
            .try_reserve(missing.len())
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(missing.len())
            .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
        for (key, appearance) in missing {
            let reservation = Arc::new(PipelineReservation::new());
            cache
                .entries
                .insert(key, PipelineCacheEntry::Reserved(reservation.clone()));
            owned.push(OwnedPipelineReservation {
                key,
                appearance,
                reservation,
            });
        }
        drop(cache);

        Ok(PipelineReservationBatch {
            cache: &self.pipelines,
            owned,
            active: true,
        })
    }

    fn create_pipeline(
        &self,
        context: &Context3D,
        key: PipelineKey,
        appearance: &AppearanceGpuLease,
    ) -> Result<Arc<wgpu::RenderPipeline>, Context3DError> {
        let (vertex_module, fragment_module) = match key.color_target {
            ColorTargetLayout3D::Single(_) => {
                appearance.materialize_shader_modules(&self.device)?
            }
            ColorTargetLayout3D::HdrMrt4 => {
                appearance.materialize_scene_shader_modules(context, &self.device)?
            }
        };
        let targets = match key.color_target {
            ColorTargetLayout3D::Single(format) => vec![Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            ColorTargetLayout3D::HdrMrt4 => super::hdr::scene_color_targets().to_vec(),
        };
        let pipeline = Arc::new(
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Halcyon 3D Canonical Render Pipeline"),
                    layout: Some(&self.pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vertex_module,
                        // Admission proves a sole stage entry; WGSL emission may rename it.
                        entry_point: None,
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: CANONICAL_VERTEX_STRIDE,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &VERTEX_ATTRIBUTES,
                        })],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fragment_module,
                        entry_point: None,
                        targets: &targets,
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: key
                            .raster_mode
                            .culls_back_faces()
                            .then_some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(key.depth.as_wgpu()),
                    multisample: wgpu::MultisampleState {
                        count: key.sample_count,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview_mask: None,
                    cache: None,
                }),
        );
        #[cfg(test)]
        super::audit::record_pipeline();
        Ok(pipeline)
    }
}

fn validate_budget(
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

/// One fixed R024 program; its module and every retained pipeline use the same
/// Context3D ledgers as legacy appearance and the private R027 tone owner.
pub(super) struct PbrProgram3D {
    pub(super) module: wgpu::ShaderModule,
    pub(super) layouts: [wgpu::BindGroupLayout; 3],
    layout: wgpu::PipelineLayout,
    pipelines: Mutex<std::collections::HashMap<(u32, bool, bool), Arc<PbrPipeline3D>>>,
    direct_layout: Option<wgpu::BindGroupLayout>,
    color_layout: Option<wgpu::BindGroupLayout>,
    profile: super::indirect::Presence3D,
    sampling: SamplingFamily3D,
    #[cfg(test)]
    pub(super) object_binding_bytes: u64,
    shadow_layout: Option<wgpu::PipelineLayout>,
    shadow_pipelines: Mutex<HashMap<(bool, bool), Arc<PbrPipeline3D>>>,
    _module_charge: super::context::ShaderModuleResidency,
    #[cfg(test)]
    pub(crate) observation_layout: wgpu::BindGroupLayout,
    #[cfg(test)]
    observation_pipeline_layout: wgpu::PipelineLayout,
}

pub(super) struct PbrPipeline3D {
    pub(super) pipeline: wgpu::RenderPipeline,
    _charge: ControlledPipelineResidency,
}

pub(super) struct PbrGeometryGpu3D {
    pub(super) vertex: wgpu::Buffer,
    pub(super) face: Option<wgpu::Buffer>,
    pub(super) index: Option<wgpu::Buffer>,
    pub(super) vertex_count: u32,
    pub(super) index_count: u32,
    pub(super) index_format: Option<wgpu::IndexFormat>,
    pub(super) _charge: super::context::GeometryGpuResidency,
}

struct RetainedQualifiedDirectSource3D {
    source: super::direct_shadow::QualifiedDirectSource3D,
    // One explicit charge follows the last immutable CPU user, including old
    // in-flight target generations. No GPU handle or writable storage is shared.
    _cpu: DynamicStorageLease3D,
}

impl RetainedQualifiedDirectSource3D {
    fn holder_bytes() -> u64 {
        (std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()) as u64
    }
}

struct DirectGpuRetention3D {
    _depth_2d: wgpu::Texture,
    _depth_cube: wgpu::Texture,
    faces: Vec<wgpu::TextureView>,
    light: wgpu::Buffer,
    transforms: wgpu::Buffer,
    views: wgpu::Buffer,
    view_binding: wgpu::BindGroup,
    binding: wgpu::BindGroup,
    sampled_2d: wgpu::TextureView,
    sampled_cube: wgpu::TextureView,
    sampler: wgpu::Sampler,
    _programs: Vec<Arc<PbrProgram3D>>,
    pipelines: HashMap<(SamplingFamily3D, bool, bool), Arc<PbrPipeline3D>>,
    _attachments: super::context::TargetBudgetLease3D,
    _textures: super::context::TextureGpuResidency,
    _storage: DynamicStorageLease3D,
}

struct DirectTarget3D {
    resources: Arc<DirectGpuRetention3D>,
    #[cfg(test)]
    passes_encoded: std::sync::atomic::AtomicU64,
    frame: super::direct_shadow::DirectFrame3D,
    qualified_source: Arc<RetainedQualifiedDirectSource3D>,
    view_bytes: Vec<u8>,
    view_stride: u32,
}

impl std::ops::Deref for DirectTarget3D {
    type Target = DirectGpuRetention3D;
    fn deref(&self) -> &Self::Target {
        &self.resources
    }
}

fn direct_bind_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    direct_bind_layout_stage(device, wgpu::ShaderStages::FRAGMENT)
}
fn direct_bind_layout_stage(
    device: &wgpu::Device,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("R025 exact group3 b0..b5"),
        entries: &direct_bind_entries(visibility),
    })
}
fn direct_bind_entries(visibility: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry> {
    let mut entries = Vec::new();
    for (binding, size) in [(0, 608), (1, 1984)] {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(size),
            },
            count: None,
        });
    }
    for (binding, dimension) in [
        (2, wgpu::TextureViewDimension::D2Array),
        (4, wgpu::TextureViewDimension::CubeArray),
    ] {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: dimension,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: binding + 1,
            visibility,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            count: None,
        });
    }
    entries
}
fn indirect_bind_layout(
    device: &wgpu::Device,
    profile: super::indirect::Presence3D,
) -> wgpu::BindGroupLayout {
    indirect_bind_layout_stage(device, profile, wgpu::ShaderStages::FRAGMENT)
}
fn indirect_bind_layout_stage(
    device: &wgpu::Device,
    profile: super::indirect::Presence3D,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    let mut entries = if profile.direct() {
        direct_bind_entries(visibility)
    } else {
        Vec::new()
    };
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 6,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: NonZeroU64::new(112),
        },
        count: None,
    });
    if profile.environment() {
        for (binding, view_dimension) in [
            (7, wgpu::TextureViewDimension::Cube),
            (8, wgpu::TextureViewDimension::CubeArray),
            (9, wgpu::TextureViewDimension::D2),
        ] {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension,
                    multisampled: false,
                },
                count: None,
            });
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 10,
            visibility,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("R026 exact selected D/E/G group3"),
        entries: &entries,
    })
}

impl DirectTarget3D {
    fn new(
        context: &Context3D,
        scene: &super::pbr::MaterialSceneSource3D,
        source: &super::direct_shadow::DirectSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        extent: (u32, u32),
        programs: &[Arc<PbrProgram3D>],
        upload: &mut super::context::PreparationUploadBudget3D,
        material_bytes: u64,
        previous: Option<&Self>,
        predecessor_scene: Option<&super::pbr::MaterialSceneSource3D>,
        captured: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
        #[cfg(any(test, feature = "test-harness"))] mut timing: Option<&mut PbrPreparationTiming3D>,
    ) -> Result<Self, super::pbr::PbrError3D> {
        use super::direct_shadow::POOL_BYTES;
        use super::pbr::PbrError3D;
        let program = &programs[0];
        let gpu = context.gpu_resources()?;
        let p = context.limits().policy;
        let hdr = u64::from(extent.0)
            .checked_mul(u64::from(extent.1))
            .and_then(|n| {
                n.checked_mul(
                    super::hdr::SCENE_DEPTH_BYTES_PER_SAMPLE * u64::from(scene.samples)
                        + super::hdr::FINAL_PAIR_BYTES_PER_PIXEL,
                )
            })
            .ok_or(PbrError3D::Association(
                "complete direct/HDR target byte overflow",
            ))?;
        let complete = hdr.checked_add(POOL_BYTES).ok_or(PbrError3D::Association(
            "complete direct target byte overflow",
        ))?;
        validate_budget(
            PolicyBudget3D::SingleTargetAttachmentBytes,
            complete,
            p.max_single_target_attachment_bytes,
        )?;
        validate_budget(
            PolicyBudget3D::TextureDimension2D,
            1024,
            u64::from(p.max_texture_dimension_2d),
        )?;
        let feature = DEPTH_FORMAT.guaranteed_format_features(gpu.device.features());
        if !feature
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
            || !feature.flags.sample_count_supported(1)
        {
            return Err(PbrError3D::Direct {
                field: "depth capability",
                cause: "Depth32Float attachment/comparison sampling unavailable",
            });
        }
        let align = u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment).max(16);
        let round = |n: u64| (n + align - 1) / align * align;
        let stride = round(80);
        let view_size = stride * 31;
        let light_size = round(608);
        let transforms_size = round(1984);
        let storage = view_size + light_size + transforms_size;
        validate_budget(
            PolicyBudget3D::DynamicStorageBytesPerTarget,
            material_bytes
                .checked_add(storage)
                .ok_or(PbrError3D::Association("direct uniform size overflow"))?,
            p.max_dynamic_storage_bytes_per_target,
        )?;
        validate_budget(
            PolicyBudget3D::DynamicUploadBytesPerFrame,
            material_bytes + view_size + 2592 + 16,
            p.max_dynamic_upload_bytes_per_frame,
        )?;
        if [view_size, light_size, transforms_size]
            .iter()
            .any(|n| *n > gpu.device.limits().max_buffer_size)
        {
            return Err(PbrError3D::Association(
                "shadow uniform buffer exceeds Device limit",
            ));
        }
        upload.consume(view_size + 2592)?;
        let corners = scene.snapshot.packets.iter().try_fold(0u64, |n, p| {
            let super::snapshot::CommittedMaterialPacket3D::PbrMaterial { geometry, .. } = p else {
                return Err(PbrError3D::Association("D mixed packet"));
            };
            n.checked_add(geometry.data.source.element_count() as u64)
                .ok_or(PbrError3D::Association(
                    "shadow source corner count overflow",
                ))
        })?;
        // Preserve the existing complete source/scratch bound. Move only its
        // source-array payload into one independently charged immutable lease;
        // each target still reserves the remaining projection/fit scratch and
        // metadata. The holder size includes its inline HDR validation witness;
        // source-holder/Arc overhead is charged explicitly too.
        let host_bytes = corners
            .checked_mul(24 * 6)
            .and_then(|n| {
                n.checked_add(
                    view_size * 2
                        + (std::mem::size_of::<Self>()
                            + std::mem::size_of::<DirectGpuRetention3D>()
                            + 2 * std::mem::size_of::<usize>()) as u64
                        + 31 * (std::mem::size_of::<wgpu::TextureView>() as u64 + 80),
                )
            })
            .ok_or(PbrError3D::Association(
                "shadow source/metadata byte overflow",
            ))?;
        let qualified_payload = super::direct_shadow::QualifiedDirectSource3D::payload_bytes(
            corners,
            source.directional.is_some(),
        )?;
        let host_bytes =
            host_bytes
                .checked_sub(qualified_payload)
                .ok_or(PbrError3D::Association(
                    "shadow source/scratch charge partition",
                ))?;
        let qualified_bytes = qualified_payload
            .checked_add(RetainedQualifiedDirectSource3D::holder_bytes())
            .ok_or(PbrError3D::Association(
                "qualified shadow source charge overflow",
            ))?;
        let qualified = previous
            .map(|old| &old.qualified_source)
            .filter(|q| q.source.matches(context, scene, origin));
        let storage_charge = context.reserve_dynamic_storage(
            storage,
            scene.snapshot.packets.len() as u64,
            1,
            host_bytes,
        )?;
        #[cfg(any(test, feature = "test-harness"))]
        let slot_compare_start = timing.as_ref().map(|_| std::time::Instant::now());
        let mut slots = predecessor_scene
            .zip(previous)
            .filter(|_| qualified.is_none())
            .and_then(|(predecessor, old)| {
                old.qualified_source.source.reserve_slot_successor(
                    context,
                    predecessor,
                    scene,
                    origin,
                    RetainedQualifiedDirectSource3D::holder_bytes(),
                    qualified_bytes,
                )
            });
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (timing.as_deref_mut(), slot_compare_start)
            && predecessor_scene.is_some()
        {
            t.direct_slot_compare += start.elapsed();
        }
        let qualified_charge = if qualified.is_none() && slots.is_none() {
            Some(context.reserve_dynamic_storage(0, 0, 0, qualified_bytes)?)
        } else {
            None
        };
        #[cfg(any(test, feature = "test-harness"))]
        let projection_body_before = timing
            .as_ref()
            .map_or(std::time::Duration::ZERO, |t| t.direct_projection_body);
        #[cfg(any(test, feature = "test-harness"))]
        let projection_start = timing.as_ref().map(|_| std::time::Instant::now());
        let (frame, new_qualified) = source.prepare_frame_with_slots(
            scene,
            view,
            origin,
            extent.0,
            extent.1,
            qualified.map(|q| &q.source),
            captured,
            slots.as_mut(),
            #[cfg(any(test, feature = "test-harness"))]
            timing.as_deref_mut(),
        )?;
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (timing.as_deref_mut(), projection_start) {
            let elapsed = start.elapsed();
            let body = t.direct_projection_body - projection_body_before;
            t.direct_projection += elapsed;
            t.direct_projection_wrapper += elapsed
                .checked_sub(body)
                .expect("direct CPU body fits its enclosing clock");
            t.direct_projection_partition_complete = true;
            t.shadow_faces += frame.active_faces.len();
            t.direct_source_qualification_reused = qualified.is_some();
            let fit = t.direct_cascade_fit;
            let depth = t.direct_depth_certification;
            let xy = t.direct_xy_pcf;
            let partition = t.direct_source_validation
                + t.direct_source_basis
                + t.direct_receiver_clipping
                + fit.into_iter().sum::<std::time::Duration>()
                + depth.into_iter().sum::<std::time::Duration>()
                + xy.into_iter().sum::<std::time::Duration>()
                + t.direct_light_other
                + t.direct_projection_wrapper;
            let fov_bits = match view.projection() {
                super::Projection3::Perspective(p) => p.vertical_fov_radians().to_bits(),
                super::Projection3::Orthographic(_) => 0,
            };
            // Emit after stopping the enclosing timer. Internal clock/record
            // overhead remains in its observed segments and wrapper remainder;
            // this diagnostic output is outside direct_projection.
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!(
                    "R025_DIRECT_CPU source={} revision={} hdr_identity={} hdr_revision={} samples={} width={} height={} fov_bits={} source_basis_reused={} source_validation_reused={} outer_ns={} validation_ns={} basis_ns={} clip_ns={} fit0_ns={} fit1_ns={} fit2_ns={} depth0_ns={} depth1_ns={} depth2_ns={} xy0_ns={} xy1_ns={} xy2_ns={} light_other_ns={} wrapper_ns={} body_ns={} partition_ns={} source_triangles={} receiver_triangles={} cascades={} candidate_attempts={} fast_triangle_visits={} fallback_triangle_visits={} frustum_corner_tests={} projection_complete={} view={view:?}\n",
                    source.identity,
                    source.revision,
                    scene.hdr.identity,
                    scene.hdr.revision,
                    scene.samples,
                    extent.0,
                    extent.1,
                    fov_bits,
                    qualified.is_some(),
                    t.direct_source_validation_reused,
                    t.direct_projection.as_nanos(),
                    t.direct_source_validation.as_nanos(),
                    t.direct_source_basis.as_nanos(),
                    t.direct_receiver_clipping.as_nanos(),
                    fit[0].as_nanos(),
                    fit[1].as_nanos(),
                    fit[2].as_nanos(),
                    depth[0].as_nanos(),
                    depth[1].as_nanos(),
                    depth[2].as_nanos(),
                    xy[0].as_nanos(),
                    xy[1].as_nanos(),
                    xy[2].as_nanos(),
                    t.direct_light_other.as_nanos(),
                    t.direct_projection_wrapper.as_nanos(),
                    t.direct_projection_body.as_nanos(),
                    partition.as_nanos(),
                    t.direct_source_triangles,
                    t.direct_receiver_triangles,
                    t.direct_cascades,
                    t.direct_candidate_attempts,
                    t.direct_fast_triangle_visits,
                    t.direct_fallback_triangle_visits,
                    t.direct_frustum_corner_tests,
                    t.direct_projection_partition_complete,
                ),
            );
        }
        let qualified_source = match new_qualified {
            Some(source) => Arc::new(RetainedQualifiedDirectSource3D {
                source,
                _cpu: slots
                    .map(|slots| slots.into_table_charge())
                    .or(qualified_charge)
                    .expect("new source reserved before qualification"),
            }),
            None => qualified.expect("exact qualified source retained").clone(),
        };
        let draws = (frame.active_faces.len() as u64)
            .checked_mul(scene.snapshot.packets.len() as u64)
            .ok_or(PbrError3D::Association("shadow face/caster work overflow"))?;
        validate_budget(
            PolicyBudget3D::DrawPacketsPerSnapshot,
            draws,
            u64::from(p.max_draw_packets_per_snapshot),
        )?;
        let attachment_charge = context.reserve_target_budget(0, POOL_BYTES)?;
        let texture_charge = context.reserve_shadow_texture_bytes(POOL_BYTES)?;
        let mut pipelines = HashMap::new();
        for packet in scene.snapshot.packets.iter() {
            let super::snapshot::CommittedMaterialPacket3D::PbrMaterial {
                raster, material, ..
            } = packet
            else {
                unreachable!()
            };
            let family = material.data.source.sampling;
            let program = programs
                .iter()
                .find(|p| p.sampling == family)
                .expect("captured sampling program");
            for cube in [false, true] {
                let cull = raster.culls_back_faces();
                if !pipelines.contains_key(&(family, cull, cube)) {
                    pipelines.insert(
                        (family, cull, cube),
                        program.shadow_pipeline(context, cull, cube)?,
                    );
                }
            }
        }
        #[cfg(any(test, feature = "test-harness"))]
        let creations = GpuCreations3D::default();
        let make_depth = |size, layers, label| {
            #[cfg(any(test, feature = "test-harness"))]
            creations.bump(2);
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: layers,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let depth_2d = make_depth(1024, 7, "R025 fixed seven1024 depth layers");
        let depth_cube = make_depth(512, 24, "R025 fixed four512 depth cubes");
        #[cfg(any(test, feature = "test-harness"))]
        creations.bump(3);
        let sampled_2d = depth_2d.create_view(&wgpu::TextureViewDescriptor {
            label: Some("R025 2D array seven"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        #[cfg(any(test, feature = "test-harness"))]
        creations.bump(3);
        let sampled_cube = depth_cube.create_view(&wgpu::TextureViewDescriptor {
            label: Some("R025 cube array four"),
            dimension: Some(wgpu::TextureViewDimension::CubeArray),
            ..Default::default()
        });
        let mut faces = Vec::with_capacity(31);
        for i in 0..31 {
            let (texture, layer) = if i < 7 {
                (&depth_2d, i)
            } else {
                (&depth_cube, i - 7)
            };
            #[cfg(any(test, feature = "test-harness"))]
            creations.bump(3);
            faces.push(texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("R025 exact writable single face; never sampled in depth pass"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            }));
        }
        let make_uniform = |label, size| {
            #[cfg(any(test, feature = "test-harness"))]
            creations.bump(0);
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let light = make_uniform("R025 Direct608", light_size);
        let transforms = make_uniform("R025 Shadow1984", transforms_size);
        let views = make_uniform("R025 per-active-face exact View80 arena", view_size);
        #[cfg(any(test, feature = "test-harness"))]
        creations.bump(4);
        let sampler = direct_comparison_sampler(&gpu.device);
        #[cfg(any(test, feature = "test-harness"))]
        creations.bump(1);
        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("R025 source-qualified current pool and uniforms"),
            layout: program.direct_layout.as_ref().expect("D layout"),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &light,
                        offset: 0,
                        size: NonZeroU64::new(608),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &transforms,
                        offset: 0,
                        size: NonZeroU64::new(1984),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&sampled_2d),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&sampled_cube),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        #[cfg(any(test, feature = "test-harness"))]
        creations.bump(1);
        let view_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("R025 exact shadow View80"),
            layout: &program.layouts[0],
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &views,
                    offset: 0,
                    size: NonZeroU64::new(80),
                }),
            }],
        });
        let mut view_bytes = vec![0u8; view_size as usize];
        for &i in &frame.active_faces {
            let view = ViewBlockBytes::new(frame.transforms.matrices[i], [0.0; 3]);
            let at = i * stride as usize;
            view_bytes[at..at + 80].copy_from_slice(bytemuck::bytes_of(&view));
        }
        #[cfg(any(test, feature = "test-harness"))]
        creations.add_to(timing.as_deref_mut());
        Ok(Self {
            resources: Arc::new(DirectGpuRetention3D {
                _depth_2d: depth_2d,
                _depth_cube: depth_cube,
                faces,
                light,
                transforms,
                views,
                view_binding,
                binding,
                sampled_2d,
                sampled_cube,
                sampler,
                _programs: programs.to_vec(),
                pipelines,
                _attachments: attachment_charge,
                _textures: texture_charge,
                _storage: storage_charge,
            }),
            #[cfg(test)]
            passes_encoded: std::sync::atomic::AtomicU64::new(0),
            frame,
            qualified_source,
            view_bytes,
            view_stride: stride as u32,
        })
    }
    fn upload(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.light, 0, bytemuck::bytes_of(&self.frame.light));
        queue.write_buffer(
            &self.transforms,
            0,
            bytemuck::bytes_of(&self.frame.transforms),
        );
        queue.write_buffer(&self.views, 0, &self.view_bytes);
    }
}

fn encode_direct_shadows(
    state: Option<&MaterialGpuRetention3D>,
    packed: &MaterialFrameBytes3D,
    encoder: &mut wgpu::CommandEncoder,
) {
    let Some(state) = state else {
        return;
    };
    let Some(direct) = &state.direct else {
        return;
    };
    for &face in &direct.frame.active_faces {
        #[cfg(test)]
        direct
            .passes_encoded
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("R025 complete geometric caster depth; no sampled-pool feedback"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &direct.faces[face],
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        });
        for (i, draw) in state.draws.iter().enumerate() {
            let MaterialGeometry3D::Pbr { gpu, .. } = &draw.geometry else {
                unreachable!("D source family qualified")
            };
            let cull = !draw
                ._material
                .as_ref()
                .expect("D material")
                .data
                .source
                .double_sided;
            let sampling = draw._material.as_ref().unwrap().data.source.sampling;
            pass.set_pipeline(&direct.pipelines[&(sampling, cull, face >= 7)].pipeline);
            pass.set_bind_group(0, &direct.view_binding, &[face as u32 * direct.view_stride]);
            pass.set_bind_group(
                1,
                state.uniforms.pbr_objects.as_ref().expect("D Object112"),
                &[packed.object_offsets[i]],
            );
            pass.set_bind_group(2, &draw.binding, &[packed.material_offsets[i]]);
            pass.set_vertex_buffer(0, gpu.vertex.slice(..));
            pass.set_vertex_buffer(
                1,
                gpu.face.as_ref().expect("D source face stream").slice(..),
            );
            pass.draw(0..gpu.vertex_count, 0..1);
        }
    }
}

pub(crate) struct MaterialFrameBytes3D {
    object_binding_bytes: u64,
    view: Vec<u8>,
    objects: Vec<u8>,
    materials: Vec<u8>,
    object_offsets: Vec<u32>,
    material_offsets: Vec<u32>,
}

impl MaterialFrameBytes3D {
    fn empty() -> Self {
        Self {
            object_binding_bytes: 112,
            view: Vec::new(),
            objects: Vec::new(),
            materials: Vec::new(),
            object_offsets: Vec::new(),
            material_offsets: Vec::new(),
        }
    }
}

#[cfg(any(test, feature = "test-harness"))]
#[derive(Default)]
struct GpuCreations3D(std::cell::Cell<[u64; 5]>);

#[cfg(any(test, feature = "test-harness"))]
impl GpuCreations3D {
    fn bump(&self, kind: usize) {
        let mut counts = self.0.get();
        counts[kind] += 1;
        self.0.set(counts);
    }

    fn add_to(&self, timing: Option<&mut PbrPreparationTiming3D>) {
        if let Some(timing) = timing {
            for (total, count) in timing.gpu_creations.iter_mut().zip(self.0.get()) {
                *total += count;
            }
        }
    }
}

/// CPU preparation stages for one opt-in diagnostic request.
///
/// `total` encloses `frame_total`, `uv_main`, and `direct_total`.
/// `frame_total` encloses `frame_validation` and `vertex_projection`.
/// `direct_total` encloses `direct_projection` and `uv_shadow`.
/// Removed GPU-model vertex/UV stages, counts, replay and sidecar bytes stay zero.
/// Subtract children from their parent before combining stages. Each remainder
/// is unisolated CPU work: in particular, allocation, resource preparation and
/// synchronous error-scope waits have no separate timers here. These durations
/// do not isolate GPU execution. Counters describe visited/planned work and do
/// not establish an upload count or GPU completion.
/// No clocks, records or extra fields exist without tests or `test-harness`.
#[cfg(any(test, feature = "test-harness"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PbrPreparationTiming3D {
    pub total: std::time::Duration,
    pub frame_total: std::time::Duration,
    pub frame_validation: std::time::Duration,
    pub vertex_projection: std::time::Duration,
    pub uv_main: std::time::Duration,
    pub uv_main_domain: std::time::Duration,
    pub uv_shadow: std::time::Duration,
    pub uv_shadow_domain: std::time::Duration,
    pub direct_total: std::time::Duration,
    pub direct_projection: std::time::Duration,
    /// Exclusive internal source phases. Cascade arrays are indexed0..2.
    pub direct_source_validation: std::time::Duration,
    pub direct_source_basis: std::time::Duration,
    pub direct_slots_reused: usize,
    pub direct_slots_rebuilt: usize,
    pub direct_slot_compare: std::time::Duration,
    pub direct_slot_rebuild: std::time::Duration,
    pub direct_slot_recompose: std::time::Duration,
    pub direct_slot_reserved_cpu_bytes: u64,
    pub direct_slot_seed: std::time::Duration,
    pub direct_slot_seed_cpu_bytes: u64,
    pub direct_receiver_clipping: std::time::Duration,
    pub direct_cascade_fit: [std::time::Duration; 3],
    pub direct_depth_certification: [std::time::Duration; 3],
    pub direct_xy_pcf: [std::time::Duration; 3],
    /// Other light work and boundary bookkeeping inside the measured function.
    pub direct_light_other: std::time::Duration,
    /// Inclusive checksum of the internal phases; do not add it again.
    pub direct_projection_body: std::time::Duration,
    /// Enclosing call minus internal body, including clock/receipt setup/return.
    /// Internal phases plus this remainder equal direct_projection exactly.
    pub direct_projection_wrapper: std::time::Duration,
    pub direct_projection_partition_complete: bool,
    pub direct_source_triangles: usize,
    pub direct_receiver_triangles: usize,
    pub direct_cascades: usize,
    pub direct_candidate_attempts: u64,
    pub direct_fast_triangle_visits: u64,
    pub direct_fallback_triangle_visits: u64,
    pub direct_frustum_corner_tests: u64,
    pub branch: &'static str,
    pub attempts: usize,
    pub direct_present: bool,
    pub same_snapshot: bool,
    pub view_changed: bool,
    pub direct_reused: bool,
    /// Immutable CPU source basis only; does not mean shadow-map/frame reuse.
    pub direct_source_qualification_reused: bool,
    /// Exact retained radiance or authenticated current whole-scene source proof.
    pub direct_source_validation_reused: bool,
    pub vertices: usize,
    pub main_triangles: usize,
    /// Removed proof-only GPU model counters; zero in runtime preparation.
    pub main_uv_packets: usize,
    pub main_uv_qualified: usize,
    pub main_uv_excluded: usize,
    pub main_uv_conditioned: usize,
    pub main_uv_timed_packets: usize,
    pub main_uv_substages: [std::time::Duration; 9],
    pub main_uv_substage_wrapper: std::time::Duration,
    pub main_uv_substage_counts: [usize; 14],
    pub main_object_dot_calls: usize,
    pub main_projection_dot_calls: usize,
    pub main_object_corner_reuses: usize,
    /// Removed runtime model replay; always zero.
    pub main_object_control: std::time::Duration,
    /// Removed runtime sidecar population; always zero.
    pub object_population: std::time::Duration,
    pub object_interval_bytes: u64,
    pub shadow_triangles: usize,
    pub shadow_faces: usize,
    pub uniform_buffers: usize,
    pub geometry_plans: usize,
    pub texture_plans: usize,
    pub mip_plans: usize,
    /// Actual buffer/bind-group/texture/view/sampler creation calls, in that order.
    pub gpu_creations: [u64; 5],
    pub material_group2_creations: usize,
    pub material_group2_hits: usize,
    pub material_group2_scratch_bytes: u64,
    pub material_group2_create_cpu: std::time::Duration,
    pub material_group2_lookup_cpu: std::time::Duration,
    pub capacity_reused: bool,
    pub capacity_slots: usize,
    pub capacity_ready_revision: u64,
}

fn material_frame_bytes(
    context: &Context3D,
    source: &super::pbr::MaterialSceneSource3D,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    width: u32,
    height: u32,
    reveal: Option<&super::object_reveal::ObjectRevealFrame3D>,
    captured: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
    #[cfg(any(test, feature = "test-harness"))] mut timing: Option<&mut PbrPreparationTiming3D>,
) -> Result<MaterialFrameBytes3D, super::pbr::PbrError3D> {
    use super::pbr::PbrError3D;
    use super::snapshot::CommittedMaterialPacket3D;
    #[cfg(any(test, feature = "test-harness"))]
    let validation_start = timing.as_ref().map(|_| std::time::Instant::now());
    let validation = super::pbr::validate_material_scene_source(context, source, captured);
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(t), Some(start)) = (timing.as_deref_mut(), validation_start) {
        t.frame_validation += start.elapsed();
    }
    validation?;
    if reveal.is_some_and(|r| !r.extent.matches_source(source)) {
        return Err(PbrError3D::Association(
            "reveal frame belongs to another immutable source",
        ));
    }
    if width == 0 || height == 0 {
        return Err(PbrError3D::Association("inactive material target"));
    }
    if source.snapshot.packets.is_empty() && source.direct.is_none() && source.indirect.is_none() {
        return Ok(MaterialFrameBytes3D {
            object_binding_bytes: 112,
            view: Vec::new(),
            objects: Vec::new(),
            materials: Vec::new(),
            object_offsets: Vec::new(),
            material_offsets: Vec::new(),
        });
    }
    let align = u64::from(context.limits().hard.min_uniform_buffer_offset_alignment).max(16);
    let round = |size: u64| {
        size.checked_add(align - 1)
            .map(|n| n / align * align)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "aligning PBR uniform storage",
            })
    };
    let object_binding_bytes = if reveal.is_some() { 128 } else { 112 };
    let reveal_tail = reveal
        .map(|r| r.extent.parameters(r.progress, origin))
        .transpose()?;
    let object_stride = round(object_binding_bytes)?;
    let material_stride = round(64)?;
    let count = source.snapshot.packets.len() as u64;
    let [view_size, object_size, material_size] = [
        Some(round(80)?),
        object_stride.checked_mul(count.max(1)),
        material_stride.checked_mul(count.max(1)),
    ]
    .map(|n| {
        n.ok_or(Context3DError::ArithmeticOverflow {
            operation: "sizing material uniform arenas",
        })
    });
    let sizes = [view_size?, object_size?, material_size?];
    let mut arenas: Vec<Vec<u8>> = Vec::new();
    for size in sizes {
        if size > context.limits().hard.max_buffer_size {
            return Err(Context3DError::HardDeviceLimitExceeded {
                limit: super::HardLimit3D::BufferSize,
                requested: size,
                maximum: context.limits().hard.max_buffer_size,
            }
            .into());
        }
        let len = usize::try_from(size)
            .map_err(|_| PbrError3D::Association("uniform arena not addressable"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| PbrError3D::Association("uniform CPU storage unavailable"))?;
        bytes.resize(len, 0);
        arenas.push(bytes);
    }
    let vb = pack_material_view(source, view, origin, width, height)?;
    arenas[0][..80].copy_from_slice(bytemuck::bytes_of(&vb));
    let mut object_offsets = Vec::new();
    let mut material_offsets = Vec::new();
    for (i, (packet, placement)) in source
        .snapshot
        .packets
        .iter()
        .zip(source.model.placements.iter())
        .enumerate()
    {
        let transform = placement.checked_transform(origin)?;
        let normal = transform
            .normal_to_world()
            .columns()
            .map(|v| [v[0], v[1], v[2], 0.0]);
        let ob = if let Some(direct) = &source.direct {
            super::direct_shadow::metric_object(*placement, origin, direct.meters_per_scene_unit)?
        } else {
            super::appearance::ObjectBlockBytes::new(transform.model_to_render().columns(), normal)
        };
        let oo = (i as u64)
            .checked_mul(object_stride)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(PbrError3D::Association(
                "object dynamic offset exceeds actual u32 domain",
            ))?;
        let mo = (i as u64)
            .checked_mul(material_stride)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(PbrError3D::Association(
                "material dynamic offset exceeds actual u32 domain",
            ))?;
        arenas[1][oo as usize..oo as usize + 112].copy_from_slice(bytemuck::bytes_of(&ob));
        if let Some(tail) = &reveal_tail {
            arenas[1][oo as usize + 112..oo as usize + 128]
                .copy_from_slice(bytemuck::bytes_of(tail));
        }
        match packet {
            CommittedMaterialPacket3D::PbrMaterial { material, .. } => {
                let block = material.data.source.block();
                arenas[2][mo as usize..mo as usize + 64]
                    .copy_from_slice(bytemuck::bytes_of(&block));
            }
            CommittedMaterialPacket3D::LegacyAppearance { .. } => {
                let block = super::appearance::AppearanceBlockBytes::new(DEFAULT_COLOR_FACTOR);
                arenas[2][mo as usize..mo as usize + 16]
                    .copy_from_slice(bytemuck::bytes_of(&block));
            }
        }
        object_offsets.push(oo);
        material_offsets.push(mo);
    }
    let mut it = arenas.into_iter();
    Ok(MaterialFrameBytes3D {
        object_binding_bytes,
        view: it.next().expect("view arena"),
        objects: it.next().expect("object arena"),
        materials: it.next().expect("material arena"),
        object_offsets,
        material_offsets,
    })
}

struct MaterialUniforms3D {
    view: wgpu::Buffer,
    objects: wgpu::Buffer,
    materials: wgpu::Buffer,
    legacy_view: wgpu::BindGroup,
    legacy_objects: wgpu::BindGroup,
    pbr_view: Option<wgpu::BindGroup>,
    pbr_objects: Option<wgpu::BindGroup>,
    _charge: DynamicStorageLease3D,
    #[cfg(test)]
    observation: Option<MaterialObservation3D>,
}

#[cfg(test)]
pub(crate) struct MaterialObservation3D {
    pub(crate) values: wgpu::Buffer,
    _header: wgpu::Buffer,
    binding: wgpu::BindGroup,
    pub(crate) bytes: u64,
}

enum MaterialGeometry3D {
    Legacy(GeometryGpuLease),
    Pbr {
        gpu: Arc<PbrGeometryGpu3D>,
        _source: super::pbr::PbrGeometryLease3D,
    },
}
enum MaterialPipeline3D {
    Legacy {
        pipeline: Arc<wgpu::RenderPipeline>,
        _module: AppearanceGpuLease,
    },
    Pbr(Arc<PbrPipeline3D>),
}
struct MaterialDraw3D {
    geometry: MaterialGeometry3D,
    pipeline: MaterialPipeline3D,
    binding: wgpu::BindGroup,
    _material: Option<super::pbr::PbrMaterialLease3D>,
    textures: Vec<MaterialTextureGpu3D>,
    samplers: Vec<MaterialSamplerGpu3D>,
}

struct MaterialGroup2Entry3D<'a> {
    generation: (u32, u64),
    first_draw: Option<(usize, &'a wgpu::BindGroupLayout)>,
}

fn material_group2_table<'a>(
    packets: &[super::snapshot::CommittedMaterialPacket3D],
    packed: &MaterialFrameBytes3D,
    packing_reserve: u64,
) -> (Vec<MaterialGroup2Entry3D<'a>>, u64) {
    use super::snapshot::CommittedMaterialPacket3D::PbrMaterial;
    let generations = packets.iter().filter_map(|packet| match packet {
        PbrMaterial { material, .. } => Some(material.handle.0.slot_generation()),
        _ => None,
    });
    let count = generations.clone().count();
    if count < 2 {
        return (Vec::new(), 0);
    }
    let Ok(layout) = std::alloc::Layout::array::<MaterialGroup2Entry3D<'_>>(count) else {
        return (Vec::new(), 0);
    };
    let bytes = layout.size() as u64;
    // The table borrows the reserved packing space while only the original
    // packed arenas and offsets are live. It is gone before any upload or
    // capacity-ledger copy. Draw metadata and staging have separate charges.
    let live = [
        packed.view.capacity(),
        packed.objects.capacity(),
        packed.materials.capacity(),
        std::mem::size_of::<u32>() * packed.object_offsets.capacity(),
        std::mem::size_of::<u32>() * packed.material_offsets.capacity(),
    ];
    if !live
        .into_iter()
        .try_fold(bytes, |total, n| total.checked_add(n as u64))
        .is_some_and(|total| total <= packing_reserve)
    {
        return (Vec::new(), 0);
    }
    let mut groups = Vec::new();
    if groups.try_reserve_exact(count).is_err() {
        return (Vec::new(), 0);
    }
    groups.extend(generations.map(|generation| MaterialGroup2Entry3D {
        generation,
        first_draw: None,
    }));
    groups.sort_unstable_by_key(|entry| entry.generation);
    groups.dedup_by_key(|entry| entry.generation);
    if groups.len() == count {
        return (Vec::new(), bytes);
    }
    (groups, bytes)
}

struct MaterialGpuRetention3D {
    uniforms: MaterialUniforms3D,
    draws: Vec<MaterialDraw3D>,
    _programs: Vec<Arc<PbrProgram3D>>,
    direct: Option<Arc<DirectTarget3D>>,
    indirect: Option<IndirectTarget3D>,
}

struct IndirectTarget3D {
    binding: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    bytes: Vec<u8>,
    stride: u32,
    _assets: Vec<Arc<super::indirect::EnvironmentGpu3D>>,
    _sampler: Option<super::context::SamplerGpuLease>,
    _source: Arc<super::indirect::IndirectSource3D>,
    _storage: DynamicStorageLease3D,
}
impl IndirectTarget3D {
    fn new(
        context: &Context3D,
        source: &Arc<super::indirect::IndirectSource3D>,
        scene: &super::pbr::MaterialSceneSource3D,
        program: &PbrProgram3D,
        direct: Option<&DirectTarget3D>,
        upload: &mut super::context::PreparationUploadBudget3D,
        material_bytes: u64,
    ) -> Result<(Self, Vec<super::indirect::DeferredEnvironmentUpload3D>), super::pbr::PbrError3D>
    {
        use super::pbr::PbrError3D;
        let gpu = context.gpu_resources()?;
        let align = u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment).max(16);
        let stride = (112 + align - 1) / align * align;
        let size = stride
            .checked_mul((scene.snapshot.packets.len() as u64).max(1))
            .ok_or(PbrError3D::Association("indirect arena overflow"))?;
        let combined = material_bytes
            .checked_add(size)
            .ok_or(PbrError3D::Association(
                "combined material/indirect storage overflow",
            ))?;
        validate_budget(
            PolicyBudget3D::DynamicStorageBytesPerTarget,
            combined,
            context.limits().policy.max_dynamic_storage_bytes_per_target,
        )?;
        if size > gpu.device.limits().max_buffer_size || size > u64::from(u32::MAX) {
            return Err(PbrError3D::Association(
                "indirect uniform Device/offset limit",
            ));
        }
        upload.consume(size)?;
        let metadata = size
            .checked_add(std::mem::size_of::<Self>() as u64)
            .ok_or(PbrError3D::Association("indirect metadata overflow"))?;
        let storage = context.reserve_dynamic_storage(
            size,
            scene.snapshot.packets.len() as u64,
            1,
            metadata,
        )?;
        let mut bytes = vec![0u8; size as usize];
        for i in 0..scene.snapshot.packets.len().max(1) {
            let at = i * stride as usize;
            bytes[at..at + 112].copy_from_slice(bytemuck::bytes_of(&source.block(i)));
        }
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("R026 exact per-draw Indirect112 slots"),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut assets = Vec::with_capacity(3);
        let mut pending = Vec::with_capacity(3);
        let mut sampler = None;
        if let Some(e) = &source.environment {
            for asset in &e.assets {
                let (resident, write) = context.prepare_environment_asset(asset, upload)?;
                assets.push(resident);
                if let Some(write) = write {
                    pending.push(write);
                }
            }
            sampler = Some(context.materialize_sampler_lease(&e.sampler)?);
        }
        let mut entries = Vec::with_capacity(11);
        if let Some(d) = direct {
            entries.extend([
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: d.light.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: d.transforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&d.sampled_2d),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&d.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&d.sampled_cube),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&d.sampler),
                },
            ]);
        }
        entries.push(wgpu::BindGroupEntry {
            binding: 6,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new(112),
            }),
        });
        if let Some(s) = &sampler {
            for (binding, asset) in (7..=9).zip(&assets) {
                entries.push(wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(&asset.view),
                });
            }
            entries.push(wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::Sampler(s.sampler()),
            });
        }
        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("R026 complete captured D/E/G color association"),
            layout: program.color_layout.as_ref().expect("E/G color layout"),
            entries: &entries,
        });
        Ok((
            Self {
                binding,
                buffer,
                bytes,
                stride: stride as u32,
                _assets: assets,
                _sampler: sampler,
                _source: source.clone(),
                _storage: storage,
            },
            pending,
        ))
    }
}

struct PbrGeometryUpload3D {
    source: Arc<super::pbr::PbrGeometryData3D>,
    gpu: Arc<PbrGeometryGpu3D>,
    vertices: Vec<u8>,
    indices: Vec<u8>,
    faces: Vec<u8>,
    _staging: DynamicStorageLease3D,
}
struct MaterialSceneUploads3D {
    geometry: Vec<PbrGeometryUpload3D>,
    legacy_geometry: Vec<super::context::DeferredGeometry3D>,
    textures: Vec<super::context::DeferredTexture3D>,
    environment: Vec<super::indirect::DeferredEnvironmentUpload3D>,
    mips: Vec<super::mip_sampling::MipUpload3D>,
    #[cfg(test)]
    header: Option<(wgpu::Buffer, [u32; 4])>,
}
impl MaterialSceneUploads3D {
    fn has_no_new_uploads(&self, committed: &PreparedMaterialScene3D) -> bool {
        self.geometry.is_empty()
            && self.legacy_geometry.is_empty()
            // A cached V1 plan contains no transfer. A pending plan owns a new
            // texture/view, so only exact already-submitted views qualify here.
            && self.textures.iter().all(|plan| committed.retained().is_some_and(|state|
                state.draws.iter().flat_map(|draw| &draw.textures)
                    .any(|texture| texture.view() == plan.lease.view())))
            && self.environment.is_empty()
            && self.mips.is_empty()
            && {
                #[cfg(test)]
                {
                    self.header.is_none()
                }
                #[cfg(not(test))]
                {
                    true
                }
            }
    }

    fn commit(self, queue: &wgpu::Queue) {
        for write in self.mips {
            write.commit(queue);
        }
        for write in self.environment {
            write.commit(queue);
        }
        for upload in self.geometry {
            queue.write_buffer(&upload.gpu.vertex, 0, &upload.vertices);
            if let Some(index) = &upload.gpu.index {
                queue.write_buffer(index, 0, &upload.indices);
            }
            if let Some(face) = &upload.gpu.face {
                queue.write_buffer(face, 0, &upload.faces);
            }
            if upload.gpu.face.is_some() {
                if let Ok(mut cache) = upload.source.direct_gpu.lock() {
                    if cache.as_ref().and_then(std::sync::Weak::upgrade).is_none() {
                        *cache = Some(Arc::downgrade(&upload.gpu));
                    }
                }
            } else if let Ok(mut cache) = upload.source.gpu.lock() {
                if cache.is_none() {
                    *cache = Some(upload.gpu);
                }
            }
        }
        for upload in self.legacy_geometry {
            upload.commit(queue);
        }
        for upload in self.textures {
            let _ = upload.commit(queue);
        }
        #[cfg(test)]
        if let Some((buffer, header)) = self.header {
            queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&header));
        }
    }
}

#[derive(Default)]
struct CapacityMap3D {
    done: std::sync::atomic::AtomicBool,
    succeeded: std::sync::atomic::AtomicBool,
    retiring: Mutex<Option<MaterialCapacityRetirement3D>>,
}

struct MaterialCapacityInner3D {
    retained: MaterialGpuRetention3D,
    packed: MaterialFrameBytes3D,
    staging: wgpu::Buffer,
    ledger: Vec<u8>,
    initialized: bool,
    _charge: DynamicStorageLease3D,
    _target_charge: MaterialCapacityTargetCharge3D,
}

// Only GPU handles, immutable leaf leases and accounting enter callbacks. The
// qualified source/snapshot/Context graph is released on the target's stack.
struct MaterialCapacityRetirement3D {
    _retained: MaterialGpuRetention3D,
    _direct: Option<Arc<DirectGpuRetention3D>>,
    _staging: wgpu::Buffer,
    _charge: DynamicStorageLease3D,
    _target_charge: MaterialCapacityTargetCharge3D,
}

impl MaterialCapacityInner3D {
    fn into_retirement(mut self) -> MaterialCapacityRetirement3D {
        assert!(self.retained.indirect.is_none());
        let direct = self
            .retained
            .direct
            .take()
            .map(|target| target.resources.clone());
        MaterialCapacityRetirement3D {
            _retained: self.retained,
            _direct: direct,
            _staging: self.staging,
            _charge: self._charge,
            _target_charge: self._target_charge,
        }
    }
}

struct MaterialCapacitySlot3D {
    // This Queue never enters either completion closure or the deferred payload.
    queue: Arc<wgpu::Queue>,
    completion: Arc<super::hdr::HdrWriterCompletion3D>,
    mapping: Arc<CapacityMap3D>,
    mapped: std::sync::atomic::AtomicBool,
    remapping: bool,
    available: bool,
    inner: Option<MaterialCapacityInner3D>,
}

impl Drop for MaterialCapacitySlot3D {
    fn drop(&mut self) {
        let mut held = self
            .inner
            .take()
            .map(MaterialCapacityInner3D::into_retirement);
        if self.remapping {
            // A pending map also retains native buffer custody. Keep its exact
            // accounting until the callback, even if the target closes first.
            let mut retiring = self.mapping.retiring.lock().expect("capacity map custody");
            if !self.mapping.done.load(std::sync::atomic::Ordering::Acquire) {
                *retiring = held.take();
            }
        }
        if self.completion.submitted() && !self.completion.complete() {
            // This is after a known covering submit. The payload owns no Queue,
            // Context3D or Device, and cannot keep its callback's Queue alive.
            crate::renderer::on_submitted_work_done(&self.queue, move || drop(held));
        }
    }
}

impl MaterialCapacitySlot3D {
    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &super::hdr::HdrScene3D,
        depth: &wgpu::TextureView,
        tone: &wgpu::Buffer,
        tone_bytes: &[u8; 16],
    ) {
        use std::sync::atomic::Ordering;
        let inner = self.inner.as_ref().expect("live target capacity");
        assert!(self.mapped.swap(false, Ordering::AcqRel));
        {
            let mut mapped = inner.staging.slice(..).get_mapped_range_mut()
                .expect("rearmed capacity has a writable staging range");
            mapped.slice(..16).copy_from_slice(tone_bytes);
            let view_end = 16 + inner.packed.view.len();
            mapped.slice(16..view_end).copy_from_slice(&inner.ledger[16..view_end]);
            let constants_end =
                view_end + inner.packed.objects.len() + inner.packed.materials.len();
            if !inner.initialized {
                mapped.slice(view_end..constants_end)
                    .copy_from_slice(&inner.ledger[view_end..constants_end]);
            }
            mapped.slice(constants_end..).copy_from_slice(&inner.ledger[constants_end..]);
        }
        inner.staging.unmap();
        let mut offset = 0;
        let mut copy = |destination: &wgpu::Buffer, size: usize, changed: bool| {
            if changed && size != 0 {
                encoder.copy_buffer_to_buffer(&inner.staging, offset, destination, 0, size as u64);
            }
            offset += size as u64;
        };
        let u = &inner.retained.uniforms;
        let d = inner.retained.direct.as_ref().expect("D capacity");
        copy(tone, 16, true);
        copy(&u.view, inner.packed.view.len(), true);
        copy(&u.objects, inner.packed.objects.len(), !inner.initialized);
        copy(
            &u.materials,
            inner.packed.materials.len(),
            !inner.initialized,
        );
        copy(&d.light, 608, true);
        copy(&d.transforms, 1984, true);
        copy(&d.views, d.view_bytes.len(), true);
        encode_direct_shadows(Some(&inner.retained), &inner.packed, encoder);
        encode_material_scene(Some(&inner.retained), &inner.packed, encoder, scene, depth);
    }
}

/// One SurfaceId/target lifetime owns this reservoir. Prepared generations own
/// separate Arc references, so a current, pending or last-good slot is not free.
pub(crate) struct MaterialCapacity3D {
    source: Arc<super::pbr::MaterialSceneSource3D>,
    origin: WorldPoint3,
    extent: (u32, u32),
    slots: Box<[Arc<MaterialCapacitySlot3D>]>,
    ready_revision: u64,
    #[cfg(any(test, feature = "test-harness"))]
    checkouts: u64,
    #[cfg(any(test, feature = "test-harness"))]
    map_completions: u64,
    _metadata: DynamicStorageLease3D,
    budget: Arc<MaterialCapacityBudget3D>,
}

pub(crate) struct MaterialCapacityBudget3D {
    bytes: std::sync::atomic::AtomicU64,
    _metadata: DynamicStorageLease3D,
}

struct MaterialCapacityTargetCharge3D {
    owner: Arc<MaterialCapacityBudget3D>,
    bytes: u64,
}

impl Drop for MaterialCapacityTargetCharge3D {
    fn drop(&mut self) {
        let old = self
            .owner
            .bytes
            .fetch_sub(self.bytes, std::sync::atomic::Ordering::AcqRel);
        debug_assert!(old >= self.bytes);
    }
}

impl MaterialCapacityBudget3D {
    fn new(context: &Context3D) -> Result<Arc<Self>, super::pbr::PbrError3D> {
        Ok(Arc::new(Self {
            bytes: std::sync::atomic::AtomicU64::new(0),
            _metadata: context.reserve_dynamic_storage(
                0,
                0,
                0,
                (std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()) as u64,
            )?,
        }))
    }

    fn reserve(
        self: &Arc<Self>,
        context: &Context3D,
        bytes: u64,
        tone: u64,
    ) -> Result<MaterialCapacityTargetCharge3D, super::pbr::PbrError3D> {
        use std::sync::atomic::Ordering;
        let maximum = context.limits().policy.max_dynamic_storage_bytes_per_target;
        self.bytes
            .try_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                old.checked_add(bytes)
                    .filter(|total| total.checked_add(tone).is_some_and(|n| n <= maximum))
            })
            .map_err(|old| Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::DynamicStorageBytesPerTarget,
                requested: old
                    .checked_add(bytes)
                    .and_then(|n| n.checked_add(tone))
                    .unwrap_or(u64::MAX),
                maximum,
            })?;
        Ok(MaterialCapacityTargetCharge3D {
            owner: self.clone(),
            bytes,
        })
    }

    pub(crate) fn progress_retired(
        &self,
        context: &Context3D,
    ) -> Result<(), super::pbr::PbrError3D> {
        if self.bytes.load(std::sync::atomic::Ordering::Acquire) != 0 {
            context
                .gpu_resources()?
                .device
                .poll(wgpu::PollType::Poll)
                .map_err(|_| {
                    super::pbr::PbrError3D::Association("retired capacity completion poll failed")
                })?;
        }
        Ok(())
    }
}

impl MaterialCapacity3D {
    fn new(
        context: &Context3D,
        source: Arc<super::pbr::MaterialSceneSource3D>,
        origin: WorldPoint3,
        extent: (u32, u32),
        budget: Arc<MaterialCapacityBudget3D>,
    ) -> Result<Self, super::pbr::PbrError3D> {
        Ok(Self {
            source,
            origin,
            extent,
            slots: Vec::new().into_boxed_slice(),
            ready_revision: 0,
            #[cfg(any(test, feature = "test-harness"))]
            checkouts: 0,
            #[cfg(any(test, feature = "test-harness"))]
            map_completions: 0,
            budget,
            _metadata: context.reserve_dynamic_storage(
                0,
                0,
                0,
                std::mem::size_of::<Self>() as u64,
            )?,
        })
    }

    fn retain(
        &mut self,
        context: &Context3D,
        prepared: &mut PreparedMaterialScene3D,
        was_committed: bool,
        #[cfg(any(test, feature = "test-harness"))] mut timing: Option<&mut PbrPreparationTiming3D>,
    ) -> Result<(), super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        if prepared.capacity_slot.is_some() {
            return Ok(());
        }
        let resources = prepared
            .retained
            .as_ref()
            .ok_or(PbrError3D::Association("D capacity resources absent"))?;
        let direct = resources.direct.as_ref().ok_or(PbrError3D::Association(
            "D capacity shadow resources absent",
        ))?;
        let size = [
            16u64,
            prepared.packed.view.len() as u64,
            prepared.packed.objects.len() as u64,
            prepared.packed.materials.len() as u64,
            608,
            1984,
            direct.view_bytes.len() as u64,
        ]
        .into_iter()
        .try_fold(0u64, |n, x| n.checked_add(x))
        .ok_or(PbrError3D::Association("capacity upload ledger overflow"))?;
        let gpu = context.gpu_resources()?;
        if size % 4 != 0 || size > gpu.device.limits().max_buffer_size {
            return Err(PbrError3D::Association(
                "capacity upload ledger Device/alignment limit",
            ));
        }
        let storage = resources.uniforms.view.size()
            + resources.uniforms.objects.size()
            + resources.uniforms.materials.size()
            + direct.light.size()
            + direct.transforms.size()
            + direct.views.size();
        let target_charge = self.budget.reserve(
            context,
            storage
                .checked_add(size)
                .ok_or(PbrError3D::Association("capacity target storage overflow"))?,
            u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment).max(16),
        )?;
        // Each slot charges its final slice entry. During growth the old slice
        // additionally remains alive until the replacement slice is installed.
        let _growth = context.reserve_dynamic_storage(
            0,
            0,
            0,
            (self.slots.len() * std::mem::size_of::<Arc<MaterialCapacitySlot3D>>()) as u64,
        )?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(self.slots.len() + 1)
            .map_err(|_| PbrError3D::Association("capacity owner storage unavailable"))?;
        slots.extend(self.slots.iter().cloned());
        let metadata = size
            .checked_add(
                (std::mem::size_of::<MaterialCapacitySlot3D>()
                    + std::mem::size_of::<Arc<MaterialCapacitySlot3D>>()
                    + std::mem::size_of::<super::hdr::HdrWriterCompletion3D>()
                    + std::mem::size_of::<CapacityMap3D>()
                    + 6 * std::mem::size_of::<usize>()) as u64,
            )
            .ok_or(PbrError3D::Association("capacity metadata overflow"))?;
        let charge = context.reserve_dynamic_storage(size, 0, 1, metadata)?;
        let mut ledger = Vec::new();
        ledger
            .try_reserve_exact(
                usize::try_from(size)
                    .map_err(|_| PbrError3D::Association("capacity ledger not addressable"))?,
            )
            .map_err(|_| PbrError3D::Association("capacity ledger storage unavailable"))?;
        ledger.resize(size as usize, 0);
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("D target retained encoder-bound staging"),
            size,
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(t) = timing.as_deref_mut() {
            t.gpu_creations[0] += 1;
        }
        let validation = pollster::block_on(validation_scope.pop());
        let allocation = pollster::block_on(allocation_scope.pop());
        if let Some(error) = validation.or(allocation) {
            return Err(PbrError3D::Shader(error.to_string()));
        }
        let completion = Arc::new(super::hdr::HdrWriterCompletion3D::default());
        // A material installed in the target already has an actual successful
        // writer submit. This new observation covers that earlier submitted use.
        if was_committed {
            completion.record(&gpu.queue);
        }
        let mut inner = MaterialCapacityInner3D {
            retained: prepared
                .retained
                .take()
                .expect("qualified capacity resources"),
            packed: std::mem::replace(&mut prepared.packed, MaterialFrameBytes3D::empty()),
            staging,
            ledger,
            initialized: was_committed,
            _charge: charge,
            _target_charge: target_charge,
        };
        inner.update_ledger(true);
        let slot = Arc::new(MaterialCapacitySlot3D {
            queue: gpu.queue.clone(),
            completion,
            mapping: Arc::new(CapacityMap3D::default()),
            mapped: std::sync::atomic::AtomicBool::new(true),
            remapping: false,
            available: false,
            inner: Some(inner),
        });
        prepared.capacity_slot = Some(slot.clone());
        #[cfg(any(test, feature = "test-harness"))]
        if !was_committed {
            self.checkouts += 1;
        }
        slots.push(slot);
        self.slots = slots.into_boxed_slice();
        Ok(())
    }

    pub(crate) fn matches(
        &self,
        source: &super::pbr::MaterialSceneSource3D,
        origin: WorldPoint3,
        extent: (u32, u32),
    ) -> bool {
        let a = &self.source;
        Arc::ptr_eq(&a.snapshot, &source.snapshot)
            && Arc::ptr_eq(&a.model, &source.model)
            && matches!((&a.direct, &source.direct), (Some(a), Some(b)) if Arc::ptr_eq(a, b))
            && a.indirect.is_none()
            && source.indirect.is_none()
            && a.samples == source.samples
            && self.origin == origin
            && self.extent == extent
            && a.hdr.identity == source.hdr.identity
            && a.hdr.revision == source.hdr.revision
            && a.hdr.exposure_ev.to_bits() == source.hdr.exposure_ev.to_bits()
            && a.hdr.maximum_rgb.map(f32::to_bits) == source.hdr.maximum_rgb.map(f32::to_bits)
            && {
                #[cfg(test)]
                {
                    a.observe == source.observe
                }
                #[cfg(not(test))]
                {
                    true
                }
            }
    }

    pub(crate) fn ready_revision(&self) -> u64 {
        self.ready_revision
    }

    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn diagnostic(&self) -> [u64; 11] {
        let mut out = [0; 11];
        out[0] = self.slots.len() as u64;
        for slot in &self.slots {
            let kind = if Arc::strong_count(slot) > 1 {
                1
            } else if slot.available {
                2
            } else if slot.completion.submitted() && !slot.completion.complete() {
                3
            } else {
                4
            };
            out[kind] += 1;
            let inner = slot.inner.as_ref().unwrap();
            let u = &inner.retained.uniforms;
            let d = inner.retained.direct.as_ref().unwrap();
            out[5] += u.view.size()
                + u.objects.size()
                + u.materials.size()
                + d.light.size()
                + d.transforms.size()
                + d.views.size()
                + inner.staging.size();
            out[6] += super::direct_shadow::POOL_BYTES;
        }
        out[7] = self.checkouts;
        out[8] = self.map_completions;
        out[9] = self.ready_revision;
        out[10] = self.budget.bytes.load(std::sync::atomic::Ordering::Acquire);
        out
    }

    #[cfg(test)]
    pub(crate) fn counts_for_test(&self) -> (usize, usize, u64) {
        (
            self.slots.len(),
            self.slots.iter().filter(|s| s.available).count(),
            self.ready_revision,
        )
    }

    pub(crate) fn progress(&mut self, context: &Context3D) -> Result<(), super::pbr::PbrError3D> {
        use std::sync::atomic::Ordering;
        let gpu = context.gpu_resources()?;
        let pending = self.budget.bytes.load(Ordering::Acquire) != 0;
        if !pending {
            return Ok(());
        }
        gpu.device
            .poll(wgpu::PollType::Poll)
            .map_err(|_| super::pbr::PbrError3D::Association("capacity completion poll failed"))?;
        for slot in &mut self.slots {
            let Some(slot) = Arc::get_mut(slot) else {
                continue;
            };
            if slot.available || (slot.completion.submitted() && !slot.completion.complete()) {
                continue;
            }
            let inner = slot.inner.as_mut().expect("capacity payload");
            if inner
                .retained
                .direct
                .as_ref()
                .is_some_and(|d| Arc::strong_count(d) != 1)
            {
                continue;
            }
            if slot.completion.submitted() {
                inner.initialized = true;
            }
            if !slot.mapped.load(Ordering::Acquire) && !slot.remapping {
                slot.mapping.done.store(false, Ordering::Release);
                slot.mapping.succeeded.store(false, Ordering::Release);
                let mapping = slot.mapping.clone();
                inner
                    .staging
                    .slice(..)
                    .map_async(wgpu::MapMode::Write, move |result| {
                        mapping.succeeded.store(result.is_ok(), Ordering::Release);
                        let retiring = {
                            let mut retiring =
                                mapping.retiring.lock().expect("capacity map custody");
                            mapping.done.store(true, Ordering::Release);
                            retiring.take()
                        };
                        drop(retiring);
                    });
                slot.remapping = true;
            }
        }
        gpu.device
            .poll(wgpu::PollType::Poll)
            .map_err(|_| super::pbr::PbrError3D::Association("capacity remap poll failed"))?;
        for slot in &mut self.slots {
            let Some(slot) = Arc::get_mut(slot) else {
                continue;
            };
            if slot.available || (slot.completion.submitted() && !slot.completion.complete()) {
                continue;
            }
            if slot.remapping && slot.mapping.done.load(Ordering::Acquire) {
                if !slot.mapping.succeeded.load(Ordering::Acquire) {
                    return Err(super::pbr::PbrError3D::Association(
                        "capacity staging remap failed",
                    ));
                }
                slot.remapping = false;
                slot.mapped.store(true, Ordering::Release);
                #[cfg(any(test, feature = "test-harness"))]
                {
                    self.map_completions += 1;
                }
            }
            if slot.mapped.load(Ordering::Acquire)
                && slot
                    .inner
                    .as_ref()
                    .unwrap()
                    .retained
                    .direct
                    .as_ref()
                    .is_some_and(|d| Arc::strong_count(d) == 1)
            {
                slot.available = true;
                self.ready_revision = self.ready_revision.checked_add(1).ok_or(
                    super::pbr::PbrError3D::Association("capacity readiness revision exhausted"),
                )?;
            }
        }
        Ok(())
    }
}

impl MaterialCapacityInner3D {
    fn update_ledger(&mut self, constants: bool) {
        let direct = self.retained.direct.as_ref().expect("D capacity");
        let mut offset = 16;
        for (bytes, changed) in [
            (self.packed.view.as_slice(), true),
            (self.packed.objects.as_slice(), constants),
            (self.packed.materials.as_slice(), constants),
            (bytemuck::bytes_of(&direct.frame.light), true),
            (bytemuck::bytes_of(&direct.frame.transforms), true),
            (direct.view_bytes.as_slice(), true),
        ] {
            if changed {
                self.ledger[offset..offset + bytes.len()].copy_from_slice(bytes);
            }
            offset += bytes.len();
        }
        assert_eq!(offset, self.ledger.len());
    }
}

pub(crate) struct PreparedMaterialScene3D {
    source: Arc<super::pbr::MaterialSceneSource3D>,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    extent: (u32, u32),
    retained: Option<MaterialGpuRetention3D>,
    packed: MaterialFrameBytes3D,
    queue: Arc<wgpu::Queue>,
    write_scene: bool,
    write_shadows: bool,
    uploads: Option<MaterialSceneUploads3D>,
    capacity_slot: Option<Arc<MaterialCapacitySlot3D>>,
    upload_bytes: u64,
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct DirectProof3D {
    source_basis: std::sync::Weak<RetainedQualifiedDirectSource3D>,
    pub(crate) pool_identity: usize,
    pub(crate) passes_encoded: u64,
    pub(crate) counts: [u32; 4],
    pub(crate) faces: Vec<usize>,
    pub(crate) depth_2d: wgpu::Extent3d,
    pub(crate) depth_cube: wgpu::Extent3d,
    pub(crate) depth_format: wgpu::TextureFormat,
    pub(crate) depth_samples: u32,
    pub(crate) light_bytes: u64,
    pub(crate) transform_bytes: u64,
    pub(crate) primary_and_face_bytes: Vec<(u64, u64)>,
    pub(crate) packed: Vec<u8>,
    pub(crate) witnesses: Vec<u64>,
}

#[cfg(test)]
impl DirectProof3D {
    pub(crate) fn same_source_basis(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.source_basis, &other.source_basis)
    }

    pub(crate) fn slot_preflight_for_test(
        &self,
        context: &Context3D,
        previous: &super::pbr::MaterialSceneSource3D,
        scene: &super::pbr::MaterialSceneSource3D,
        origin: WorldPoint3,
    ) -> bool {
        self.source_basis
            .upgrade()
            .unwrap()
            .source
            .slot_preflight_for_test(
                context,
                previous,
                scene,
                origin,
                RetainedQualifiedDirectSource3D::holder_bytes(),
            )
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MaterialCapacityObjects3D {
    pub(crate) buffers: Vec<wgpu::Buffer>,
    pub(crate) bindings: Vec<wgpu::BindGroup>,
    pub(crate) material_offsets: Vec<u32>,
    pub(crate) object_offsets: Vec<u32>,
    pub(crate) textures: [wgpu::Texture; 2],
    pub(crate) views: Vec<wgpu::TextureView>,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) staging: Option<wgpu::Buffer>,
    pub(crate) cpu: [usize; 5],
}

impl Drop for PreparedMaterialScene3D {
    fn drop(&mut self) {
        if self.capacity_slot.take().is_some() {
            return;
        }
        if self.uploads.is_some() && self.source.direct.is_some() && self.source.indirect.is_none()
        {
            // The writer has never run. These D-only candidate destinations
            // have neither pending transfers nor submitted GPU use to retain.
            drop(self.retained.take());
            return;
        }
        if let Some(mut retained) = self.retained.take() {
            // Only actual resource/charge custody, not Context/Device/Queue owner,
            // enters this existing same-Queue completion observation.
            // Match MaterialCapacityInner3D::into_retirement: DirectTarget's
            // qualified CPU source owns a snapshot/Context and belongs on this
            // stack. Its exact shadow resources/charges still cover GPU use.
            let direct = retained
                .direct
                .take()
                .map(|target| target.resources.clone());
            crate::renderer::on_submitted_work_done(&self.queue, move || drop((retained, direct)));
        }
    }
}

impl PreparedMaterialScene3D {
    #[cfg(test)]
    pub(super) fn assert_direct_full_parity_for_test(&self) -> (usize, usize) {
        let direct = self.retained().unwrap().direct.as_ref().unwrap();
        direct.qualified_source.source.assert_full_parity_for_test(
            &self.source,
            self.view,
            self.origin,
            self.extent,
            &direct.frame,
        )
    }

    pub(super) fn hud_budget_base(
        &self,
        capacity: Option<&MaterialCapacityBudget3D>,
    ) -> (u64, u64) {
        let storage = if let Some(capacity) = capacity {
            capacity.bytes.load(std::sync::atomic::Ordering::Acquire)
        } else {
            self.retained().map_or(0, |r| {
                let uniforms = &r.uniforms;
                let direct = r
                    .direct
                    .as_ref()
                    .map_or(0, |d| d.light.size() + d.transforms.size() + d.views.size());
                let indirect = r.indirect.as_ref().map_or(0, |d| d.buffer.size());
                uniforms.view.size()
                    + uniforms.objects.size()
                    + uniforms.materials.size()
                    + direct
                    + indirect
            })
        };
        (storage, self.upload_bytes)
    }

    fn retained(&self) -> Option<&MaterialGpuRetention3D> {
        self.capacity_slot
            .as_ref()
            .and_then(|slot| slot.inner.as_ref())
            .map(|inner| &inner.retained)
            .or(self.retained.as_ref())
    }

    fn packed(&self) -> &MaterialFrameBytes3D {
        self.capacity_slot
            .as_ref()
            .and_then(|slot| slot.inner.as_ref())
            .map_or(&self.packed, |inner| &inner.packed)
    }

    pub(crate) fn encoder_completion(&self) -> Option<Arc<super::hdr::HdrWriterCompletion3D>> {
        self.capacity_slot
            .as_ref()
            .map(|slot| slot.completion.clone())
    }

    #[cfg(test)]
    pub(crate) fn direct_proof_for_test(&self) -> Option<DirectProof3D> {
        let state = self.retained()?;
        let direct = state.direct.as_ref()?;
        Some(DirectProof3D {
            source_basis: Arc::downgrade(&direct.qualified_source),
            counts: direct.frame.light.counts,
            pool_identity: Arc::as_ptr(direct) as usize,
            passes_encoded: direct
                .passes_encoded
                .load(std::sync::atomic::Ordering::Relaxed),
            faces: direct.frame.active_faces.clone(),
            depth_2d: direct._depth_2d.size(),
            depth_cube: direct._depth_cube.size(),
            depth_format: direct._depth_2d.format(),
            depth_samples: direct._depth_2d.sample_count(),
            light_bytes: direct.light.size(),
            transform_bytes: direct.transforms.size(),
            primary_and_face_bytes: state
                .draws
                .iter()
                .map(|d| match &d.geometry {
                    MaterialGeometry3D::Pbr { gpu, .. } => (
                        gpu.vertex.size(),
                        gpu.face.as_ref().map_or(0, wgpu::Buffer::size),
                    ),
                    _ => unreachable!(),
                })
                .collect(),
            packed: [
                bytemuck::bytes_of(&direct.frame.light),
                bytemuck::bytes_of(&direct.frame.transforms),
            ]
            .concat(),
            witnesses: direct
                .frame
                .depth_proofs
                .iter()
                .flat_map(|p| {
                    std::iter::once(p.t.to_bits())
                        .chain(p.row.map(|x| u64::from(x.to_bits())))
                        .chain(p.range.map(f64::to_bits))
                        .chain(p.corners.into_iter().flatten().map(f64::to_bits))
                })
                .collect(),
        })
    }

    #[cfg(test)]
    pub(crate) fn capacity_objects_for_test(&self) -> Option<MaterialCapacityObjects3D> {
        let state = self.retained()?;
        let u = &state.uniforms;
        let d = state.direct.as_ref()?;
        let packed = self.packed();
        Some(MaterialCapacityObjects3D {
            buffers: vec![
                u.view.clone(),
                u.objects.clone(),
                u.materials.clone(),
                d.light.clone(),
                d.transforms.clone(),
                d.views.clone(),
            ],
            bindings: [&u.legacy_view, &u.legacy_objects]
                .into_iter()
                .chain(u.pbr_view.iter())
                .chain(u.pbr_objects.iter())
                .chain([&d.binding, &d.view_binding])
                .chain(state.draws.iter().map(|p| &p.binding))
                .cloned()
                .collect(),
            material_offsets: packed.material_offsets.clone(),
            object_offsets: packed.object_offsets.clone(),
            textures: [d._depth_2d.clone(), d._depth_cube.clone()],
            views: [&d.sampled_2d, &d.sampled_cube]
                .into_iter()
                .chain(d.faces.iter())
                .cloned()
                .collect(),
            sampler: d.sampler.clone(),
            staging: self
                .capacity_slot
                .as_ref()
                .map(|s| s.inner.as_ref().unwrap().staging.clone()),
            cpu: [
                state.draws.as_ptr() as usize,
                packed.objects.as_ptr() as usize,
                packed.materials.as_ptr() as usize,
                state.draws[0].textures.as_ptr() as usize,
                state.draws[0].samplers.as_ptr() as usize,
            ],
        })
    }
    #[allow(
        dead_code,
        reason = "Retained exact prepared material scene source identity query"
    )]
    pub(crate) fn source(&self) -> &Arc<super::pbr::MaterialSceneSource3D> {
        &self.source
    }
    #[cfg(test)]
    pub(crate) fn observation(&self) -> Option<&MaterialObservation3D> {
        self.retained()?.uniforms.observation.as_ref()
    }
    pub(crate) fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &super::hdr::HdrScene3D,
        depth: &wgpu::TextureView,
    ) {
        if let Some(uploads) = self.uploads.take() {
            uploads.commit(&self.queue);
        }
        if !self.write_scene {
            return;
        }
        if let Some(state) = self.retained.as_ref() {
            let u = &state.uniforms;
            self.queue.write_buffer(&u.view, 0, &self.packed.view);
            self.queue.write_buffer(&u.objects, 0, &self.packed.objects);
            self.queue
                .write_buffer(&u.materials, 0, &self.packed.materials);
            if let Some(direct) = &state.direct {
                if self.write_shadows {
                    direct.upload(&self.queue);
                }
            }
            if let Some(indirect) = &state.indirect {
                self.queue
                    .write_buffer(&indirect.buffer, 0, &indirect.bytes);
            }
        }
        if self.write_shadows {
            encode_direct_shadows(self.retained.as_ref(), &self.packed, encoder);
        }
        encode_material_scene(self.retained.as_ref(), &self.packed, encoder, scene, depth);
    }
}

fn encode_material_scene(
    state: Option<&MaterialGpuRetention3D>,
    packed: &MaterialFrameBytes3D,
    encoder: &mut wgpu::CommandEncoder,
    scene: &super::hdr::HdrScene3D,
    depth: &wgpu::TextureView,
) {
    #[cfg(test)]
    if let Some(observation) = state.and_then(|s| s.uniforms.observation.as_ref()) {
        encoder.clear_buffer(&observation.values, 0, None);
    }
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("R024 actual typed geometry/material -> R027 private scene; no resolve"),
        color_attachments: &scene.attachments(),
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        multiview_mask: None,
        occlusion_query_set: None,
    });
    let Some(state) = state else {
        return;
    };
    let u = &state.uniforms;
    for (i, draw) in state.draws.iter().enumerate() {
        match &draw.pipeline {
            MaterialPipeline3D::Legacy { pipeline, .. } => {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &u.legacy_view, &[0]);
                pass.set_bind_group(1, &u.legacy_objects, &[packed.object_offsets[i]]);
            }
            MaterialPipeline3D::Pbr(p) => {
                pass.set_pipeline(&p.pipeline);
                pass.set_bind_group(0, u.pbr_view.as_ref().expect("PBR view binding"), &[0]);
                pass.set_bind_group(
                    1,
                    u.pbr_objects.as_ref().expect("PBR object binding"),
                    &[packed.object_offsets[i]],
                );
                if let Some(indirect) = &state.indirect {
                    pass.set_bind_group(3, &indirect.binding, &[i as u32 * indirect.stride]);
                } else if let Some(direct) = &state.direct {
                    pass.set_bind_group(3, &direct.binding, &[]);
                }
                #[cfg(test)]
                if let Some(observation) = &u.observation {
                    if state.direct.is_some() {
                        pass.set_bind_group(0, &observation.binding, &[0]);
                    } else {
                        pass.set_bind_group(3, &observation.binding, &[]);
                    }
                }
            }
        }
        pass.set_bind_group(2, &draw.binding, &[packed.material_offsets[i]]);
        match &draw.geometry {
            MaterialGeometry3D::Legacy(g) => {
                pass.set_vertex_buffer(0, g.vertex_buffer().slice(..));
                if let Some(index) = g.index_buffer() {
                    let format = match g.index_format() {
                        CanonicalIndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
                        CanonicalIndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
                        CanonicalIndexFormat::Unindexed => unreachable!("indexed source"),
                    };
                    pass.set_index_buffer(index.slice(..), format);
                    pass.draw_indexed(0..g.index_count(), 0, 0..1);
                } else {
                    pass.draw(0..g.vertex_count(), 0..1);
                }
            }
            MaterialGeometry3D::Pbr { gpu, .. } => {
                pass.set_vertex_buffer(0, gpu.vertex.slice(..));
                if let Some(face) = &gpu.face {
                    pass.set_vertex_buffer(1, face.slice(..));
                }
                if let (Some(index), Some(format)) = (&gpu.index, gpu.index_format) {
                    pass.set_index_buffer(index.slice(..), format);
                    pass.draw_indexed(0..gpu.index_count, 0, 0..1);
                } else {
                    pass.draw(0..gpu.vertex_count, 0..1);
                }
            }
        }
    }
}

pub(crate) enum MaterialScenePreparation3D {
    New(PreparedMaterialScene3D),
    Dynamic {
        source: Arc<super::pbr::MaterialSceneSource3D>,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        packed: MaterialFrameBytes3D,
        view_changed: bool,
        objects_changed: bool,
        write_scene: bool,
        upload_bytes: u64,
    },
}

pub(super) fn pack_material_view(
    source: &super::pbr::MaterialSceneSource3D,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    width: u32,
    height: u32,
) -> Result<super::appearance::ViewBlockBytes, super::pbr::PbrError3D> {
    if let Some(direct) = &source.direct {
        return super::direct_shadow::metric_view(
            view,
            origin,
            direct.meters_per_scene_unit,
            width,
            height,
        );
    }
    let projection = view.prepared_matrices(origin, width, height)?;
    let camera = view.pose().translation().render_relative_to(origin)?;
    Ok(super::appearance::ViewBlockBytes::new(
        projection.view_projection().columns(),
        [camera.x(), camera.y(), camera.z()],
    ))
}

impl MaterialScenePreparation3D {
    /// The current frame has already passed full qualification and all of its
    /// resource reservations. Seed only this new owner's basis for a successor.
    pub(super) fn seed_direct_slots(
        &mut self,
        context: &Context3D,
        #[cfg(any(test, feature = "test-harness"))] timing: Option<&mut PbrPreparationTiming3D>,
    ) {
        let Self::New(next) = self else { return };
        if next.packed().object_binding_bytes != 112 {
            return;
        }
        let Some(direct) = next
            .retained
            .as_mut()
            .and_then(|retained| retained.direct.as_mut())
            .and_then(Arc::get_mut)
        else {
            return;
        };
        #[cfg(any(test, feature = "test-harness"))]
        let start = timing.as_ref().map(|_| std::time::Instant::now());
        let Some((source, charge, _bytes)) = direct
            .qualified_source
            .source
            .seed_slots(context, RetainedQualifiedDirectSource3D::holder_bytes())
        else {
            return;
        };
        direct.qualified_source = Arc::new(RetainedQualifiedDirectSource3D {
            source,
            _cpu: charge,
        });
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (timing, start) {
            t.direct_slot_seed = start.elapsed();
            t.direct_slot_seed_cpu_bytes = _bytes;
        }
    }

    pub(crate) fn force_scene_write(&mut self) {
        match self {
            Self::New(next) => next.write_scene = true,
            Self::Dynamic { write_scene, .. } => *write_scene = true,
        }
    }

    /// HAL374 captures actual retained source buffers/slots, never ordinals or a
    /// second admission path. The caller encodes the replay in this SAME encoder.
    pub(crate) fn outline_target(
        &self,
        current: Option<&PreparedMaterialScene3D>,
        context: &Context3D,
        selection: &super::PbrOutlineSelection3D,
        extent: (u32, u32),
        previous: Option<&super::outline::OutlineGpuTarget>,
    ) -> super::PbrSourceResult3D<super::outline::OutlineGpuTarget> {
        use super::PbrSourceError3D as E;
        let (source, state, packed, upload_bytes) = match self {
            Self::New(next) => (
                &next.source,
                next.retained(),
                next.packed(),
                next.upload_bytes,
            ),
            Self::Dynamic {
                source,
                packed,
                upload_bytes,
                ..
            } => (
                source,
                current.and_then(PreparedMaterialScene3D::retained),
                packed,
                *upload_bytes,
            ),
        };
        if !selection.matches_source(source) {
            return Err(E::Association("HAL374 exact scene capture mismatch"));
        }
        selection.current(context)?;
        let state = state.ok_or(E::Association("HAL374 retained PBR draws unavailable"))?;
        if state.draws.len() != source.snapshot.packets.len()
            || packed.object_offsets.len() != state.draws.len()
        {
            return Err(E::Association(
                "HAL374 complete retained draw mapping mismatch",
            ));
        }
        if packed.object_binding_bytes == 128
            && packed.object_offsets.iter().any(|&at| {
                let start = at as usize + 112;
                packed
                    .objects
                    .get(start..start + 4)
                    .is_none_or(|b| f32::from_ne_bytes(b.try_into().expect("scalar width")) != 1.0)
            })
        {
            return Err(E::Association(
                "HAL374 partial HAL372 discard has no opaque replay parity",
            ));
        }
        let bytes = state
            .draws
            .len()
            .checked_mul(std::mem::size_of::<super::outline::OutlineDraw>())
            .and_then(|b| b.checked_add(3 * std::mem::size_of::<wgpu::BindGroup>()))
            .ok_or(E::Association("HAL374 retained capture byte overflow"))?;
        let charge = context.reserve_dynamic_storage(0, 0, 0, bytes as u64)?;
        let mut draws = Vec::new();
        draws
            .try_reserve_exact(state.draws.len())
            .map_err(|_| E::Association("HAL374 retained capture capacity"))?;
        for (slot, (draw, packet)) in state
            .draws
            .iter()
            .zip(source.snapshot.packets.iter())
            .enumerate()
        {
            let (
                MaterialGeometry3D::Pbr { gpu, _source },
                MaterialPipeline3D::Pbr(_),
                super::snapshot::CommittedMaterialPacket3D::PbrMaterial {
                    geometry,
                    material,
                    raster,
                    ..
                },
            ) = (&draw.geometry, &draw.pipeline, packet)
            else {
                return Err(E::Association(
                    "HAL374 complete opaque replay excludes legacy packets",
                ));
            };
            if _source.handle != geometry.handle
                || !Arc::ptr_eq(&_source.data, &geometry.data)
                || draw._material.as_ref().is_none_or(|m| {
                    m.handle != material.handle || !Arc::ptr_eq(&m.data, &material.data)
                })
                || material.data.source.alpha_mode != super::pbr::PbrAlphaMode3D::Opaque
                || geometry.data.source.certified_input().is_none()
            {
                return Err(E::Association(
                    "HAL374 original slot/geometry/material/certificate mapping mismatch",
                ));
            }
            draws.push(super::outline::OutlineDraw {
                geometry: gpu.clone(),
                source: geometry.clone(),
                object_offset: packed.object_offsets[slot],
                cull: raster.culls_back_faces(),
                edge_vertices: 0..0,
            });
        }
        super::outline::OutlineGpuTarget::new(
            context,
            selection,
            extent,
            &packed.view,
            &packed.objects,
            &state.uniforms.view,
            &state.uniforms.objects,
            draws,
            charge,
            previous,
            upload_bytes,
        )
    }

    pub(crate) fn encoder_completion(&self) -> Option<Arc<super::hdr::HdrWriterCompletion3D>> {
        match self {
            Self::New(next) => next.encoder_completion(),
            Self::Dynamic { .. } => None,
        }
    }

    pub(crate) fn encode_encoder_bound(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &super::hdr::HdrScene3D,
        depth: &wgpu::TextureView,
        tone: &wgpu::Buffer,
        tone_bytes: &[u8; 16],
    ) {
        let Self::New(next) = self else {
            unreachable!("selected capacity preparation")
        };
        next.capacity_slot
            .as_ref()
            .expect("selected capacity slot")
            .encode(encoder, scene, depth, tone, tone_bytes);
    }

    pub(crate) fn encode(
        &mut self,
        current: Option<&PreparedMaterialScene3D>,
        encoder: &mut wgpu::CommandEncoder,
        scene: &super::hdr::HdrScene3D,
        depth: &wgpu::TextureView,
    ) {
        match self {
            Self::New(next) => next.encode(encoder, scene, depth),
            Self::Dynamic {
                packed,
                view_changed,
                objects_changed,
                write_scene,
                ..
            } => {
                if !*write_scene {
                    return;
                }
                let current = current.expect("validated retained material target");
                if let Some(state) = current.retained() {
                    if *view_changed {
                        current
                            .queue
                            .write_buffer(&state.uniforms.view, 0, &packed.view);
                    }
                    if *objects_changed {
                        current
                            .queue
                            .write_buffer(&state.uniforms.objects, 0, &packed.objects);
                    }
                }
                encode_material_scene(current.retained(), packed, encoder, scene, depth);
            }
        }
    }
    pub(crate) fn commit(self, target: &mut Option<PreparedMaterialScene3D>) {
        match self {
            Self::New(next) => {
                *target = Some(next);
            }
            Self::Dynamic {
                source,
                view,
                origin,
                packed,
                upload_bytes,
                ..
            } => {
                let current = target.as_mut().expect("retained material target");
                current.source = source;
                current.view = view;
                current.origin = origin;
                current.packed = packed;
                current.upload_bytes = upload_bytes;
            }
        }
    }
}

fn material_program(
    context: &Context3D,
    profile: super::indirect::Presence3D,
    sampling: SamplingFamily3D,
    reveal: bool,
) -> Result<Arc<PbrProgram3D>, super::pbr::PbrError3D> {
    if reveal {
        return context.reveal_program(profile, sampling);
    }
    if sampling == SamplingFamily3D::V2 {
        return context.mip_program(profile);
    }
    if profile.indirect() {
        return context.indirect_program(profile);
    }
    if profile.direct() {
        return context.direct_program();
    }
    let state = context.pbr_state()?;
    let mut p = state
        .program
        .lock()
        .map_err(|_| super::pbr::PbrError3D::Lock("PBR module cache"))?;
    if p.is_none() {
        *p = Some(Arc::new(PbrProgram3D::new(context)?));
    }
    Ok(p.as_ref().unwrap().clone())
}

#[cfg(test)]
fn material_uv_role_extent(material: &super::pbr::PbrMaterialLease3D, shadow: bool) -> [u32; 2] {
    if material.data.source.sampling != SamplingFamily3D::V2 {
        return [0; 2];
    }
    let textures = if shadow {
        if material.data.source.alpha_mode != super::pbr::PbrAlphaMode3D::Mask
            || material.data.source.roles[0].is_none()
        {
            return [0; 2];
        }
        &material.data.textures[..1]
    } else {
        &material.data.textures[..]
    };
    let mut extent = [0; 2];
    for texture in textures {
        if let MaterialTextureLease3D::V2(t) = texture {
            extent[0] = extent[0].max(t.source.levels[0].width);
            extent[1] = extent[1].max(t.source.levels[0].height);
        }
    }
    extent
}

/// Explicit offline GPU-arithmetic model diagnostic; never production admission.
#[cfg(test)]
fn qualify_material_uvs(
    source: &super::pbr::MaterialSceneSource3D,
    packed: &MaterialFrameBytes3D,
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    shadow_face: Option<u32>,
) -> Result<(), super::pbr::PbrError3D> {
    for (i, packet) in source.snapshot.packets.iter().enumerate() {
        let super::snapshot::CommittedMaterialPacket3D::PbrMaterial {
            geometry,
            material,
            ordinal,
            ..
        } = packet
        else {
            continue;
        };
        let role_extent = material_uv_role_extent(material, shadow_face.is_some());
        if role_extent == [0; 2] {
            continue;
        }
        let at = packed.object_offsets[i] as usize;
        let object = bytemuck::pod_read_unaligned(&packed.objects[at..at + 64]);
        super::mip_sampling::qualify_uv_domain(
            &geometry.data.source,
            object,
            projection,
            extent,
            role_extent,
            super::mip_sampling::UvDomainContext3D {
                packet: i,
                ordinal: *ordinal,
                shadow_face,
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn qualify_material_uvs_for_test(
    context: &Context3D,
    source: &super::pbr::MaterialSceneSource3D,
    view: ViewDescriptor3,
    projection: [[f32; 4]; 4],
    shadow_face: Option<u32>,
) -> Result<(), super::pbr::PbrError3D> {
    let packed = material_frame_bytes(
        context,
        source,
        view,
        view.pose().translation(),
        32,
        32,
        None,
        None,
        None,
    )?;
    qualify_material_uvs(
        source,
        &packed,
        projection,
        if shadow_face.is_some() {
            (1024, 1024)
        } else {
            (32, 32)
        },
        shadow_face,
    )
}

fn refresh_capacity_frame(
    context: &Context3D,
    source: &super::pbr::MaterialSceneSource3D,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    extent: (u32, u32),
    inner: &mut MaterialCapacityInner3D,
    captured: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
    #[cfg(any(test, feature = "test-harness"))] mut timing: Option<&mut PbrPreparationTiming3D>,
    #[cfg(any(test, feature = "test-harness"))] mut coarse: Option<
        &mut super::viewport::PbrCoarseTiming3D,
    >,
) -> Result<u64, super::pbr::PbrError3D> {
    use super::pbr::PbrError3D;
    #[cfg(any(test, feature = "test-harness"))]
    debug_assert!(timing.is_none() || coarse.is_none());
    #[cfg(any(test, feature = "test-harness"))]
    let frame_start = timing.as_ref().map(|_| std::time::Instant::now());
    #[cfg(any(test, feature = "test-harness"))]
    let validation_start = timing.as_ref().map(|_| std::time::Instant::now());
    let valid = super::pbr::validate_material_scene_source(context, source, captured);
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(t), Some(start)) = (timing.as_deref_mut(), validation_start) {
        t.frame_validation += start.elapsed();
    }
    valid?;
    let direct_source = source.direct.as_ref().expect("selected D source");
    let vb = super::direct_shadow::metric_view(
        view,
        origin,
        direct_source.meters_per_scene_unit,
        extent.0,
        extent.1,
    )?;
    inner.packed.view.fill(0);
    inner.packed.view[..80].copy_from_slice(bytemuck::bytes_of(&vb));
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(t), Some(start)) = (timing.as_deref_mut(), frame_start) {
        t.frame_total += start.elapsed();
    }
    #[cfg(any(test, feature = "test-harness"))]
    let direct_start = timing.as_ref().map(|_| std::time::Instant::now());
    let gpu = context.gpu_resources()?;
    if !gpu.private_hdr_owner_live() {
        return Err(PbrError3D::Association("renderer retired"));
    }
    let d = Arc::get_mut(
        inner
            .retained
            .direct
            .as_mut()
            .expect("selected direct target"),
    )
    .ok_or(PbrError3D::Association(
        "capacity shadow owner remains shared",
    ))?;
    #[cfg(any(test, feature = "test-harness"))]
    let body_before = timing
        .as_ref()
        .map_or(std::time::Duration::ZERO, |t| t.direct_projection_body);
    #[cfg(any(test, feature = "test-harness"))]
    let projection_start = timing.as_ref().map(|_| std::time::Instant::now());
    #[cfg(any(test, feature = "test-harness"))]
    let coarse_direct_start = coarse.as_ref().map(|_| std::time::Instant::now());
    let (frame, changed_basis) = direct_source.prepare_frame_with_source_proof(
        source,
        view,
        origin,
        extent.0,
        extent.1,
        Some(&d.qualified_source.source),
        captured,
        #[cfg(any(test, feature = "test-harness"))]
        if coarse.is_some() {
            None
        } else {
            timing.as_deref_mut()
        },
    )?;
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(t), Some(start)) = (coarse.as_deref_mut(), coarse_direct_start) {
        t.direct_projection += start.elapsed();
    }
    if changed_basis.is_some() {
        return Err(PbrError3D::Association(
            "retained source qualification changed",
        ));
    }
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(t), Some(start)) = (timing.as_deref_mut(), projection_start) {
        let elapsed = start.elapsed();
        t.direct_projection += elapsed;
        t.direct_projection_wrapper += elapsed
            .checked_sub(t.direct_projection_body - body_before)
            .expect("current direct phases fit enclosing clock");
        t.direct_projection_partition_complete = true;
        t.direct_source_qualification_reused = true;
        t.shadow_faces += frame.active_faces.len();
    }
    #[cfg(any(test, feature = "test-harness"))]
    if let Some(t) = coarse.as_deref_mut() {
        t.shadow_faces += frame.active_faces.len();
    }
    let draws = (frame.active_faces.len() as u64)
        .checked_mul(source.snapshot.packets.len() as u64)
        .ok_or(PbrError3D::Association("shadow face/caster work overflow"))?;
    validate_budget(
        PolicyBudget3D::DrawPacketsPerSnapshot,
        draws,
        u64::from(context.limits().policy.max_draw_packets_per_snapshot),
    )?;
    d.view_bytes.fill(0);
    for &face in &frame.active_faces {
        let block = ViewBlockBytes::new(frame.transforms.matrices[face], [0.0; 3]);
        let at = face * d.view_stride as usize;
        d.view_bytes[at..at + 80].copy_from_slice(bytemuck::bytes_of(&block));
    }
    d.frame = frame;
    let bytes = 16 + inner.packed.view.len() as u64 + 2592 + d.view_bytes.len() as u64;
    inner.update_ledger(false);
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(t), Some(start)) = (timing.as_deref_mut(), direct_start) {
        t.direct_total += start.elapsed();
    }
    let mut upload = context.begin_preparation_upload_budget();
    upload.consume(bytes)?;
    validate_budget(
        PolicyBudget3D::DynamicUploadBytesPerFrame,
        bytes,
        context.limits().policy.max_dynamic_upload_bytes_per_frame,
    )?;
    Ok(bytes)
}

pub(crate) fn prepare_material_scene_with_capacity(
    context: &Context3D,
    source: Arc<super::pbr::MaterialSceneSource3D>,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    extent: (u32, u32),
    current: &mut Option<PreparedMaterialScene3D>,
    capacity: &mut Option<MaterialCapacity3D>,
    capacity_budget: &mut Option<Arc<MaterialCapacityBudget3D>>,
    captured: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
    reveal: Option<&super::object_reveal::ObjectRevealFrame3D>,
    committed_predecessor: Option<&Arc<super::pbr::MaterialSceneSource3D>>,
    #[cfg(any(test, feature = "test-harness"))] mut timing: Option<&mut PbrPreparationTiming3D>,
    #[cfg(any(test, feature = "test-harness"))] mut coarse: Option<
        &mut super::viewport::PbrCoarseTiming3D,
    >,
) -> Result<MaterialScenePreparation3D, super::pbr::PbrError3D> {
    #[cfg(any(test, feature = "test-harness"))]
    debug_assert!(timing.is_none() || coarse.is_none());
    // Object112 capacity slots cannot be rebound as Object128. Existing current
    // and in-flight slot Arcs keep their own queue-safe leases after pool release.
    if reveal.is_some()
        || current
            .as_ref()
            .is_some_and(|old| old.packed().object_binding_bytes == 128)
    {
        *capacity = None;
    }
    let eligible = reveal.is_none() && current.as_ref().is_some_and(|old| {
        old.packed().object_binding_bytes == 112 &&
        old.view != view && old.view.pose() == view.pose()
            && matches!((old.view.projection(), view.projection()),
                (super::Projection3::Perspective(a), super::Projection3::Perspective(b)) if a.near().to_bits() == b.near().to_bits() && a.far() == b.far())
            && old.origin == origin && old.extent == extent
            && source.direct.is_some() && source.indirect.is_none()
            && Arc::ptr_eq(&old.source.snapshot, &source.snapshot)
            && Arc::ptr_eq(&old.source.model, &source.model)
            && old.source.samples == source.samples && old.source.indirect.is_none()
            && matches!((&old.source.direct, &source.direct), (Some(a), Some(b)) if Arc::ptr_eq(a, b))
            && old.source.hdr.identity == source.hdr.identity && old.source.hdr.revision == source.hdr.revision
            && old.source.hdr.exposure_ev.to_bits() == source.hdr.exposure_ev.to_bits()
            && old.source.hdr.maximum_rgb.map(f32::to_bits) == source.hdr.maximum_rgb.map(f32::to_bits)
            && {
                #[cfg(test)]
                { old.source.observe == source.observe }
                #[cfg(not(test))]
                { true }
            }
    });
    if capacity
        .as_ref()
        .is_some_and(|pool| !pool.matches(&source, origin, extent))
    {
        *capacity = None;
    }
    if eligible {
        if let Some(pool) = capacity.as_mut() {
            #[cfg(any(test, feature = "test-harness"))]
            let slot_count = pool.slots.len();
            for slot in &mut pool.slots {
                if Arc::strong_count(slot) != 1 || !slot.available {
                    continue;
                }
                let unique = Arc::get_mut(slot).expect("exclusive free capacity");
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(t) = timing.as_deref_mut() {
                    t.attempts += 1;
                    t.direct_present = true;
                    t.same_snapshot = true;
                    t.view_changed = true;
                }
                let upload_bytes = refresh_capacity_frame(
                    context,
                    &source,
                    view,
                    origin,
                    extent,
                    unique.inner.as_mut().unwrap(),
                    captured,
                    #[cfg(any(test, feature = "test-harness"))]
                    timing.as_deref_mut(),
                    #[cfg(any(test, feature = "test-harness"))]
                    coarse.as_deref_mut(),
                )?;
                unique.available = false;
                unique.completion.reset();
                #[cfg(any(test, feature = "test-harness"))]
                {
                    pool.checkouts += 1;
                }
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(t) = timing.as_deref_mut() {
                    t.branch = "New";
                    t.capacity_reused = true;
                    t.capacity_slots = slot_count;
                    t.capacity_ready_revision = pool.ready_revision;
                }
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(t) = coarse.as_deref_mut() {
                    t.capacity_reused = true;
                }
                return Ok(MaterialScenePreparation3D::New(PreparedMaterialScene3D {
                    source,
                    view,
                    origin,
                    extent,
                    retained: None,
                    packed: MaterialFrameBytes3D::empty(),
                    queue: context.gpu_resources()?.queue,
                    write_scene: true,
                    write_shadows: true,
                    uploads: None,
                    capacity_slot: Some(slot.clone()),
                    upload_bytes,
                }));
            }
        }
    }
    let mut prepared = prepare_material_scene_with_reveal(
        context,
        source.clone(),
        view,
        origin,
        extent,
        current.as_ref(),
        captured,
        reveal,
        committed_predecessor,
        #[cfg(any(test, feature = "test-harness"))]
        timing.as_deref_mut(),
    )?;
    if eligible {
        if let MaterialScenePreparation3D::New(next) = &mut prepared {
            if next.uploads.as_ref().is_some_and(|uploads| {
                uploads.has_no_new_uploads(current.as_ref().expect("eligible current source"))
            }) {
                if capacity.is_none() {
                    if capacity_budget.is_none() {
                        *capacity_budget = Some(MaterialCapacityBudget3D::new(context)?);
                    }
                    *capacity = Some(MaterialCapacity3D::new(
                        context,
                        source,
                        origin,
                        extent,
                        capacity_budget.as_ref().unwrap().clone(),
                    )?);
                }
                let pool = capacity.as_mut().unwrap();
                pool.retain(
                    context,
                    current.as_mut().expect("eligible committed source"),
                    true,
                    #[cfg(any(test, feature = "test-harness"))]
                    timing.as_deref_mut(),
                )?;
                pool.retain(
                    context,
                    next,
                    false,
                    #[cfg(any(test, feature = "test-harness"))]
                    timing.as_deref_mut(),
                )?;
                next.uploads = None;
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(t) = timing {
                    t.capacity_slots = pool.slots.len();
                    t.capacity_ready_revision = pool.ready_revision;
                }
            }
        }
    }
    Ok(prepared)
}

#[cfg(test)]
pub(crate) fn prepare_material_scene(
    context: &Context3D,
    source: Arc<super::pbr::MaterialSceneSource3D>,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    extent: (u32, u32),
    current: Option<&PreparedMaterialScene3D>,
    captured: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
    #[cfg(any(test, feature = "test-harness"))] timing: Option<&mut PbrPreparationTiming3D>,
) -> Result<MaterialScenePreparation3D, super::pbr::PbrError3D> {
    prepare_material_scene_with_reveal(
        context,
        source,
        view,
        origin,
        extent,
        current,
        captured,
        None,
        None,
        #[cfg(any(test, feature = "test-harness"))]
        timing,
    )
}

fn prepare_material_scene_with_reveal(
    context: &Context3D,
    source: Arc<super::pbr::MaterialSceneSource3D>,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    extent: (u32, u32),
    current: Option<&PreparedMaterialScene3D>,
    captured: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
    reveal: Option<&super::object_reveal::ObjectRevealFrame3D>,
    committed_predecessor: Option<&Arc<super::pbr::MaterialSceneSource3D>>,
    #[cfg(any(test, feature = "test-harness"))] mut timing: Option<&mut PbrPreparationTiming3D>,
) -> Result<MaterialScenePreparation3D, super::pbr::PbrError3D> {
    use super::pbr::PbrError3D;
    use super::snapshot::CommittedMaterialPacket3D;
    super::hdr::scene_capabilities(&context.gpu_resources()?.device, source.samples)
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    #[cfg(any(test, feature = "test-harness"))]
    if let Some(t) = timing.as_deref_mut() {
        t.attempts += 1;
        t.direct_present = source.direct.is_some();
        t.same_snapshot =
            current.is_some_and(|old| Arc::ptr_eq(&old.source.snapshot, &source.snapshot));
        t.view_changed = current.is_some_and(|old| old.view != view);
    }
    #[cfg(any(test, feature = "test-harness"))]
    let frame_start = timing.as_ref().map(|_| std::time::Instant::now());
    let packed = material_frame_bytes(
        context,
        &source,
        view,
        origin,
        extent.0,
        extent.1,
        reveal,
        captured,
        #[cfg(any(test, feature = "test-harness"))]
        timing.as_deref_mut(),
    );
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(t), Some(start)) = (timing.as_deref_mut(), frame_start) {
        t.frame_total += start.elapsed();
    }
    let packed = packed?;
    let gpu = context.gpu_resources()?;
    if !gpu.private_hdr_owner_live() {
        return Err(PbrError3D::Association("renderer retired"));
    }
    if source.snapshot.packets.is_empty() && source.direct.is_none() && source.indirect.is_none() {
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(t) = timing.as_deref_mut() {
            t.branch = "New(empty)";
        }
        return Ok(MaterialScenePreparation3D::New(PreparedMaterialScene3D {
            source,
            view,
            origin,
            extent,
            retained: None,
            packed,
            queue: gpu.queue.clone(),
            write_scene: true,
            write_shadows: false,
            uploads: None,
            capacity_slot: None,
            upload_bytes: 16,
        }));
    }
    let mut upload = context.begin_preparation_upload_budget();
    upload.consume(16)?; // same frame's R027 Tone16
    let p = context.limits().policy;
    if let Some(old) = current.filter(|old| {
        Arc::ptr_eq(&old.source.snapshot, &source.snapshot)
            && old.packed().object_binding_bytes == packed.object_binding_bytes
            && ((old.source.direct.is_none() && source.direct.is_none())
                || (packed.object_binding_bytes == 128 && old.view == view && old.origin == origin
                    && Arc::ptr_eq(&old.source.model, &source.model)
                    && matches!((&old.source.direct, &source.direct), (Some(a), Some(b)) if Arc::ptr_eq(a,b))))
            && match (&old.source.indirect, &source.indirect) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
            && old.extent == extent
            && old.source.samples == source.samples
            && {
                #[cfg(test)]
                {
                    old.source.observe == source.observe
                }
                #[cfg(not(test))]
                {
                    true
                }
            }
    }) {
        let view_changed = old.packed().view != packed.view;
        let objects_changed = old.packed().objects != packed.objects;
        let bytes = u64::from(view_changed) * packed.view.len() as u64
            + u64::from(objects_changed) * packed.objects.len() as u64;
        upload.consume(bytes)?;
        validate_budget(
            PolicyBudget3D::DynamicUploadBytesPerFrame,
            bytes
                .checked_add(16)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "charging PBR dynamic writes",
                })?,
            p.max_dynamic_upload_bytes_per_frame,
        )?;
        let write_scene = view_changed || objects_changed;
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(t) = timing.as_deref_mut() {
            t.branch = "Dynamic";
        }
        return Ok(MaterialScenePreparation3D::Dynamic {
            source,
            view,
            origin,
            packed,
            view_changed,
            objects_changed,
            write_scene,
            upload_bytes: p.max_upload_bytes_per_frame - upload.remaining_bytes(),
        });
    }
    let scalar_bytes = (packed.view.len() as u64)
        .checked_add(packed.objects.len() as u64)
        .and_then(|n| n.checked_add(packed.materials.len() as u64))
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "charging PBR uniform arenas",
        })?;
    upload.consume(scalar_bytes)?;
    validate_budget(
        PolicyBudget3D::DynamicUploadBytesPerFrame,
        scalar_bytes
            .checked_add(16)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "charging PBR initial uniform writes",
            })?,
        p.max_dynamic_upload_bytes_per_frame,
    )?;
    let raster = context.raster_gpu()?;
    let has_pbr = source.direct.is_some()
        || source.indirect.is_some()
        || source
            .snapshot
            .packets
            .iter()
            .any(|p| matches!(p, CommittedMaterialPacket3D::PbrMaterial { .. }));
    let profile = source.indirect.as_ref().map_or(
        super::indirect::Presence3D(u8::from(source.direct.is_some())),
        |s| s.presence(source.direct.is_some()),
    );
    let mut programs = Vec::with_capacity(2);
    if has_pbr {
        if source.snapshot.sampling_families[0] || !source.snapshot.sampling_families[1] {
            programs.push(material_program(
                context,
                profile,
                SamplingFamily3D::V1,
                packed.object_binding_bytes == 128,
            )?);
        }
        if source.snapshot.sampling_families[1] {
            programs.push(material_program(
                context,
                profile,
                SamplingFamily3D::V2,
                packed.object_binding_bytes == 128,
            )?);
        }
    }
    let program = programs.first().cloned();
    let observer_bytes = 0u64;
    #[cfg(test)]
    let mut observer_bytes = observer_bytes;
    #[cfg(test)]
    if source.observe && has_pbr {
        let row_bytes = if source.direct.is_some() { 320 } else { 176 };
        observer_bytes = u64::from(extent.0)
            .checked_mul(u64::from(extent.1))
            .and_then(|n| n.checked_mul(row_bytes))
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "sizing exact numeric observation",
            })?;
        if observer_bytes > u64::from(gpu.device.limits().max_storage_buffer_binding_size) {
            return Err(PbrError3D::Association(
                "numeric observation exceeds actual storage binding limit",
            ));
        }
        upload.consume(16)?;
        validate_budget(
            PolicyBudget3D::DynamicUploadBytesPerFrame,
            scalar_bytes
                .checked_add(32)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "charging complete observed target writes",
                })?,
            p.max_dynamic_upload_bytes_per_frame,
        )?;
    }
    let storage_bytes = scalar_bytes
        .checked_add(observer_bytes)
        .and_then(|n| n.checked_add(if observer_bytes == 0 { 0 } else { 16 }))
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "charging complete material target storage",
        })?;
    validate_budget(
        PolicyBudget3D::DynamicStorageBytesPerTarget,
        storage_bytes,
        p.max_dynamic_storage_bytes_per_target,
    )?;
    let packing_reserve =
        scalar_bytes
            .checked_mul(2)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "charging retained material packing/custody",
            })?;
    let retained_cpu = (source.snapshot.packets.len() as u64)
        .checked_mul(
            (std::mem::size_of::<MaterialDraw3D>()
                + 5 * (std::mem::size_of::<MaterialTextureGpu3D>()
                    + std::mem::size_of::<MaterialSamplerGpu3D>())) as u64,
        )
        .and_then(|n| packing_reserve.checked_add(n))
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "charging retained material packing/custody",
        })?;
    let charge = context.reserve_dynamic_storage(
        storage_bytes,
        source.snapshot.packets.len() as u64,
        1,
        retained_cpu,
    )?;
    let mut uploads = MaterialSceneUploads3D {
        geometry: Vec::new(),
        legacy_geometry: Vec::new(),
        textures: Vec::new(),
        environment: Vec::new(),
        mips: Vec::new(),
        #[cfg(test)]
        header: None,
    };
    let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    #[cfg(any(test, feature = "test-harness"))]
    let creations = GpuCreations3D::default();
    let built = (|| -> Result<(MaterialGpuRetention3D, MaterialSceneUploads3D), PbrError3D> {
        // Every allocation below is uninitialized. No Queue write occurs before this
        // complete candidate, all reservations and its Device error scopes succeed.
        #[cfg(any(test, feature = "test-harness"))]
        let group2_table_start = timing
            .as_ref()
            .filter(|_| has_pbr)
            .map(|_| std::time::Instant::now());
        let (mut material_groups, _group2_bytes) = if has_pbr {
            material_group2_table(&source.snapshot.packets, &packed, packing_reserve)
        } else {
            (Vec::new(), 0)
        };
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (timing.as_deref_mut(), group2_table_start) {
            t.material_group2_scratch_bytes = t.material_group2_scratch_bytes.max(_group2_bytes);
            t.material_group2_lookup_cpu += start.elapsed();
        }
        let create_buffer = |label, size, usage| {
            #[cfg(any(test, feature = "test-harness"))]
            creations.bump(0);
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let make_uniform = |label, bytes: &[u8]| {
            create_buffer(
                label,
                bytes.len() as u64,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            )
        };
        let view_buffer = make_uniform("PBR target View80 arena", &packed.view);
        let objects = make_uniform(
            if packed.object_binding_bytes == 128 {
                "PBR target Object128Reveal arena"
            } else {
                "PBR target Object112 arena"
            },
            &packed.objects,
        );
        let materials = make_uniform("PBR target Material64/legacy16 arena", &packed.materials);
        let bind_uniform = |layout: &wgpu::BindGroupLayout, buffer: &wgpu::Buffer, size: u64| {
            #[cfg(any(test, feature = "test-harness"))]
            creations.bump(1);
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("exact target-local uniform binding"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: NonZeroU64::new(size),
                    }),
                }],
            })
        };
        let legacy_view = bind_uniform(&raster.layouts.view, &view_buffer, 80);
        let legacy_objects = bind_uniform(&raster.layouts.object, &objects, 112);
        let pbr_view = program
            .as_ref()
            .map(|p| bind_uniform(&p.layouts[0], &view_buffer, 80));
        let pbr_objects = program
            .as_ref()
            .map(|p| bind_uniform(&p.layouts[1], &objects, packed.object_binding_bytes));
        #[cfg(test)]
        let observation = if observer_bytes > 0 {
            let values = create_buffer(
                "actual per-covered-fragment numeric observations",
                observer_bytes,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            );
            let header = create_buffer(
                "actual target observation extent",
                16,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let entries = if source.direct.is_some() {
                vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &view_buffer,
                            offset: 0,
                            size: NonZeroU64::new(80),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: values.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: header.as_entire_binding(),
                    },
                ]
            } else {
                vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: values.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: header.as_entire_binding(),
                    },
                ]
            };
            creations.bump(1);
            let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("test observer only"),
                layout: &program
                    .as_ref()
                    .expect("observed PBR program")
                    .observation_layout,
                entries: &entries,
            });
            uploads.header = Some((header.clone(), [extent.0, extent.1, 0, 0]));
            Some(MaterialObservation3D {
                values,
                _header: header,
                binding,
                bytes: observer_bytes,
            })
        } else {
            None
        };
        #[cfg(any(test, feature = "test-harness"))]
        let direct_start = timing.as_ref().map(|_| std::time::Instant::now());
        let reusable_direct = current
            .filter(|old| {
                old.extent == extent
                    && old.view == view
                    && old.origin == origin
                    && old.source.samples == source.samples
                    && Arc::ptr_eq(&old.source.snapshot, &source.snapshot)
                    && Arc::ptr_eq(&old.source.model, &source.model)
                    && match (&old.source.direct, &source.direct) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        _ => false,
                    }
            })
            .and_then(PreparedMaterialScene3D::retained)
            .and_then(|s| s.direct.clone());
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(t) = timing.as_deref_mut() {
            t.direct_reused = reusable_direct.is_some();
        }
        let direct = if reusable_direct.is_some() {
            reusable_direct
        } else if let Some(direct) = &source.direct {
            let predecessor_scene = current
                .filter(|old| {
                    reveal.is_none()
                        && committed_predecessor
                            .is_some_and(|source| Arc::ptr_eq(source, &old.source))
                        && old.extent == extent
                        && old.packed().object_binding_bytes == 112
                        && super::direct_shadow::same_view_bits(old.view, view)
                })
                .map(|old| old.source.as_ref());
            Some(Arc::new(DirectTarget3D::new(
                context,
                &source,
                direct,
                view,
                origin,
                extent,
                &programs,
                &mut upload,
                storage_bytes,
                current
                    .filter(|old| old.extent == extent && old.source.samples == source.samples)
                    .and_then(PreparedMaterialScene3D::retained)
                    .and_then(|old| old.direct.as_deref()),
                predecessor_scene,
                captured,
                #[cfg(any(test, feature = "test-harness"))]
                timing.as_deref_mut(),
            )?))
        } else {
            None
        };
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (timing.as_deref_mut(), direct_start) {
            t.direct_total += start.elapsed();
        }
        let indirect = if let Some(indirect) = &source.indirect {
            let direct_bytes = direct
                .as_ref()
                .map_or(0, |d| d.light.size() + d.transforms.size() + d.views.size());
            let (state, pending) = IndirectTarget3D::new(
                context,
                indirect,
                &source,
                program.as_ref().expect("E/G program"),
                direct.as_deref(),
                &mut upload,
                storage_bytes
                    .checked_add(direct_bytes)
                    .ok_or(PbrError3D::Association("complete D/E/G storage overflow"))?,
            )?;
            uploads.environment = pending;
            Some(state)
        } else {
            None
        };
        let mut texture_map = std::collections::HashMap::new();
        let mut sampler_map = std::collections::HashMap::new();
        let mut geometry_map = std::collections::HashMap::new();
        let mut legacy_geometry_map = std::collections::HashMap::new();
        let mut draws = Vec::<MaterialDraw3D>::new();
        for packet in source.snapshot.packets.iter() {
            match packet {
                CommittedMaterialPacket3D::PbrMaterial {
                    geometry,
                    material,
                    raster: class,
                    ..
                } => {
                    let key = geometry.handle.0.slot_generation();
                    let mesh = if let Some(mesh) = geometry_map.get(&key) {
                        Arc::clone(mesh)
                    } else {
                        let cached = if source.direct.is_some() {
                            geometry
                                .data
                                .direct_gpu
                                .lock()
                                .map_err(|_| PbrError3D::Lock("D geometry cache"))?
                                .as_ref()
                                .and_then(std::sync::Weak::upgrade)
                        } else {
                            geometry
                                .data
                                .gpu
                                .lock()
                                .map_err(|_| PbrError3D::Lock("PBR geometry cache"))?
                                .clone()
                        };
                        let mesh = if let Some(mesh) = cached {
                            mesh
                        } else {
                            let expanded = source.direct.is_some();
                            let (pre_staging, pre_residency) = if expanded {
                                let count = geometry.data.source.element_count() as u64;
                                let bytes = count.checked_mul(64).ok_or(
                                    PbrError3D::Association("D expanded geometry bytes overflow"),
                                )?;
                                validate_budget(
                                    PolicyBudget3D::MeshVertices,
                                    count,
                                    u64::from(p.max_vertices_per_mesh),
                                )?;
                                validate_budget(
                                    PolicyBudget3D::MeshBytes,
                                    bytes,
                                    p.max_mesh_bytes,
                                )?;
                                if count * 52 > context.limits().hard.max_buffer_size
                                    || count * 12 > context.limits().hard.max_buffer_size
                                {
                                    return Err(PbrError3D::Association(
                                        "D expanded buffer exceeds Device limit",
                                    ));
                                }
                                upload.consume(bytes)?;
                                let cpu = bytes
                                    .checked_mul(2)
                                    .and_then(|n| {
                                        n.checked_add(
                                            (geometry.data.source.vertices.len() as u64)
                                                .checked_mul(52)?,
                                        )
                                    })
                                    .ok_or(PbrError3D::Association(
                                        "D packing/staging size overflow",
                                    ))?;
                                (
                                    Some(context.reserve_dynamic_storage(0, 0, 0, cpu)?),
                                    Some(context.reserve_controlled_geometry_bytes(bytes)?),
                                )
                            } else {
                                (None, None)
                            };
                            let (vertices, faces) = if expanded {
                                let count = geometry.data.source.element_count() as u64;
                                validate_budget(
                                    PolicyBudget3D::MeshVertices,
                                    count,
                                    u64::from(p.max_vertices_per_mesh),
                                )?;
                                let (v, f) = geometry.data.source.pack_direct()?;
                                (
                                    bytemuck::cast_slice(&v).to_vec(),
                                    bytemuck::cast_slice(&f).to_vec(),
                                )
                            } else {
                                (
                                    bytemuck::cast_slice(&geometry.data.source.pack()?).to_vec(),
                                    Vec::new(),
                                )
                            };
                            let (mut indices, index_format) = if expanded {
                                (Vec::new(), None)
                            } else {
                                match &geometry.data.source.indices {
                                    super::GeometryIndices::Unindexed => (Vec::new(), None),
                                    super::GeometryIndices::U16(v) => (
                                        bytemuck::cast_slice(v.as_ref()).to_vec(),
                                        Some(wgpu::IndexFormat::Uint16),
                                    ),
                                    super::GeometryIndices::U32(v) => (
                                        bytemuck::cast_slice(v.as_ref()).to_vec(),
                                        Some(wgpu::IndexFormat::Uint32),
                                    ),
                                }
                            };
                            let padded =
                                indices.len().checked_add(3).ok_or(PbrError3D::Association(
                                    "PBR index alignment overflow",
                                ))? / 4
                                    * 4;
                            indices.resize(padded, 0);
                            let bytes = (vertices.len() as u64)
                                .checked_add(indices.len() as u64)
                                .and_then(|n| n.checked_add(faces.len() as u64))
                                .ok_or(Context3DError::ArithmeticOverflow {
                                    operation: "charging PBR mesh storage",
                                })?;
                            validate_budget(PolicyBudget3D::MeshBytes, bytes, p.max_mesh_bytes)?;
                            if !expanded {
                                upload.consume(bytes)?;
                            }
                            let staging = match pre_staging {
                                Some(s) => s,
                                None => context.reserve_dynamic_storage(0, 0, 0, bytes)?,
                            };
                            let reserved = match pre_residency {
                                Some(r) => r,
                                None => context.reserve_controlled_geometry_bytes(bytes)?,
                            };
                            let vertex = create_buffer(
                                "R024 exact52 vertex stream",
                                vertices.len() as u64,
                                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            );
                            let face = if faces.is_empty() {
                                None
                            } else {
                                Some(create_buffer(
                                    "R025 source-winding face12 stream",
                                    faces.len() as u64,
                                    wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                                ))
                            };
                            let index = if indices.is_empty() {
                                None
                            } else {
                                Some(create_buffer(
                                    "R024 qualified indices",
                                    indices.len() as u64,
                                    wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                                ))
                            };
                            let mesh = Arc::new(PbrGeometryGpu3D {
                                vertex,
                                face,
                                index,
                                vertex_count: if expanded {
                                    geometry.data.source.element_count() as u32
                                } else {
                                    geometry.data.source.vertices.len() as u32
                                },
                                index_count: if index_format.is_some() {
                                    geometry.data.source.element_count() as u32
                                } else {
                                    0
                                },
                                index_format,
                                _charge: reserved.commit(),
                            });
                            uploads.geometry.push(PbrGeometryUpload3D {
                                source: geometry.data.clone(),
                                gpu: mesh.clone(),
                                vertices,
                                indices,
                                faces,
                                _staging: staging,
                            });
                            mesh
                        };
                        geometry_map.insert(key, mesh.clone());
                        mesh
                    };
                    let mut textures = Vec::new();
                    let mut samplers = Vec::new();
                    for i in 0..5 {
                        let t = &material.data.textures[i];
                        let tk = (
                            matches!(t, MaterialTextureLease3D::V2(_)),
                            t.handle().slot(),
                            t.handle().generation(),
                        );
                        if !texture_map.contains_key(&tk) {
                            match t {
                                MaterialTextureLease3D::V1(t) => {
                                    let plan =
                                        context.prepare_controlled_texture(t, &mut upload)?;
                                    texture_map.insert(
                                        tk,
                                        MaterialTextureGpu3D::V1(plan.lease.retained_clone()),
                                    );
                                    uploads.textures.push(plan);
                                }
                                MaterialTextureLease3D::V2(t) => {
                                    let (gpu, plan) = context.prepare_mip_chain(t, &mut upload)?;
                                    texture_map.insert(tk, MaterialTextureGpu3D::V2(gpu));
                                    if let Some(plan) = plan {
                                        uploads.mips.push(plan);
                                    }
                                }
                            }
                        }
                        textures.push(texture_map[&tk].retained_clone());
                        let s = &material.data.samplers[i];
                        let sk = (
                            matches!(s, MaterialSamplerLease3D::V2(_)),
                            s.handle().slot(),
                            s.handle().generation(),
                        );
                        if !sampler_map.contains_key(&sk) {
                            let gpu = match (s, t) {
                                (MaterialSamplerLease3D::V1(s), _) => {
                                    MaterialSamplerGpu3D::V1(context.materialize_sampler_lease(s)?)
                                }
                                (MaterialSamplerLease3D::V2(s), MaterialTextureLease3D::V2(t)) => {
                                    MaterialSamplerGpu3D::V2(context.prepare_mip_sampler(s, t)?)
                                }
                                _ => {
                                    return Err(PbrError3D::Association(
                                        "mismatched typed texture/sampler family",
                                    ));
                                }
                            };
                            sampler_map.insert(sk, gpu);
                        }
                        samplers.push(sampler_map[&sk].retained_clone());
                    }
                    let program = programs
                        .iter()
                        .find(|p| p.sampling == material.data.source.sampling)
                        .expect("typed sampling program");
                    let layout = &program.layouts[2];
                    // The lookup is sealed to this builder's admitted Context and
                    // newly created materials buffer at base 0, size 64. A hit
                    // also proves the exact layout and resolved ordered resources
                    // of the same immutable typed material generation.
                    #[cfg(any(test, feature = "test-harness"))]
                    let lookup_start = timing
                        .as_ref()
                        .filter(|_| !material_groups.is_empty())
                        .map(|_| std::time::Instant::now());
                    let generation = material.handle.0.slot_generation();
                    let shared = material_groups
                        .binary_search_by_key(&generation, |entry| entry.generation)
                        .ok()
                        .and_then(|i| {
                            let entry = &mut material_groups[i];
                            let Some((index, first_layout)) = entry.first_draw else {
                                entry.first_draw = Some((draws.len(), layout));
                                return None;
                            };
                            let first = &draws[index];
                            let admitted = first._material.as_ref().expect("PBR material");
                            (first_layout == layout
                                && admitted.handle == material.handle
                                && Arc::ptr_eq(&admitted.data, &material.data)
                                && (0..5).all(|role| {
                                    first.textures[role].view() == textures[role].view()
                                        && first.samplers[role].sampler()
                                            == samplers[role].sampler()
                                }))
                            .then(|| first.binding.clone())
                        });
                    #[cfg(any(test, feature = "test-harness"))]
                    if let (Some(t), Some(start)) = (timing.as_deref_mut(), lookup_start) {
                        t.material_group2_lookup_cpu += start.elapsed();
                        t.material_group2_hits += usize::from(shared.is_some());
                    }
                    let binding = shared.unwrap_or_else(|| {
                        let mut entries = vec![wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &materials,
                                offset: 0,
                                size: NonZeroU64::new(64),
                            }),
                        }];
                        for i in 0..5 {
                            entries.push(wgpu::BindGroupEntry {
                                binding: i as u32 + 1,
                                resource: wgpu::BindingResource::TextureView(textures[i].view()),
                            });
                        }
                        for i in 0..5 {
                            entries.push(wgpu::BindGroupEntry {
                                binding: i as u32 + 6,
                                resource: wgpu::BindingResource::Sampler(samplers[i].sampler()),
                            });
                        }
                        #[cfg(any(test, feature = "test-harness"))]
                        creations.bump(1);
                        #[cfg(any(test, feature = "test-harness"))]
                        let group2_start = timing.as_ref().map(|_| std::time::Instant::now());
                        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("typed immutable PBR roles"),
                            layout,
                            entries: &entries,
                        });
                        #[cfg(any(test, feature = "test-harness"))]
                        if let (Some(t), Some(start)) = (timing.as_deref_mut(), group2_start) {
                            t.material_group2_creations += 1;
                            t.material_group2_create_cpu += start.elapsed();
                        }
                        binding
                    });
                    let observe = {
                        #[cfg(test)]
                        {
                            source.observe
                        }
                        #[cfg(not(test))]
                        {
                            false
                        }
                    };
                    let pipeline = program.pipeline(
                        context,
                        source.samples,
                        class.culls_back_faces(),
                        observe,
                    )?;
                    draws.push(MaterialDraw3D {
                        geometry: MaterialGeometry3D::Pbr {
                            gpu: mesh,
                            _source: geometry.clone(),
                        },
                        pipeline: MaterialPipeline3D::Pbr(pipeline),
                        binding,
                        _material: Some(material.clone()),
                        textures,
                        samplers,
                    });
                }
                CommittedMaterialPacket3D::LegacyAppearance {
                    geometry,
                    appearance,
                    raster: class,
                    ..
                } => {
                    let key = (geometry.handle().slot(), geometry.handle().generation());
                    if !legacy_geometry_map.contains_key(&key) {
                        let plan =
                            context.prepare_controlled_legacy_geometry(geometry, &mut upload)?;
                        legacy_geometry_map.insert(key, plan.lease.retained_clone());
                        uploads.legacy_geometry.push(plan);
                    }
                    let mesh = legacy_geometry_map[&key].retained_clone();
                    let module = context
                        .admit_appearance_lease(appearance, geometry.descriptor().semantics())?;
                    let mut key =
                        PipelineKey::new(&module, *class, super::hdr::SCENE_FORMAT, source.samples);
                    key.color_target = ColorTargetLayout3D::HdrMrt4;
                    let request = PipelineRequest {
                        key,
                        appearance: &module,
                    };
                    let pipeline = raster
                        .prepare_pipelines(context, &[request], p.max_pipeline_variants)?
                        .remove(0);
                    let mut textures = Vec::new();
                    let mut samplers = Vec::new();
                    if let (Some(t), Some(s)) = (appearance.texture(), appearance.sampler()) {
                        let tk = (false, t.handle().slot(), t.handle().generation());
                        if !texture_map.contains_key(&tk) {
                            let plan = context.prepare_controlled_texture(t, &mut upload)?;
                            texture_map
                                .insert(tk, MaterialTextureGpu3D::V1(plan.lease.retained_clone()));
                            uploads.textures.push(plan);
                        }
                        textures.push(texture_map[&tk].retained_clone());
                        let sk = (false, s.handle().slot(), s.handle().generation());
                        if !sampler_map.contains_key(&sk) {
                            sampler_map.insert(
                                sk,
                                MaterialSamplerGpu3D::V1(context.materialize_sampler_lease(s)?),
                            );
                        }
                        samplers.push(sampler_map[&sk].retained_clone());
                    }
                    let texture = textures.first().map_or(&raster.fallback_view, |t| t.view());
                    let sampler = samplers
                        .first()
                        .map_or(&raster.fallback_sampler, |s| s.sampler());
                    #[cfg(any(test, feature = "test-harness"))]
                    creations.bump(1);
                    let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("unchanged legacy appearance in selected HDR source"),
                        layout: &raster.layouts.appearance,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &materials,
                                    offset: 0,
                                    size: NonZeroU64::new(16),
                                }),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(texture),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(sampler),
                            },
                        ],
                    });
                    draws.push(MaterialDraw3D {
                        geometry: MaterialGeometry3D::Legacy(mesh),
                        pipeline: MaterialPipeline3D::Legacy {
                            pipeline,
                            _module: module,
                        },
                        binding,
                        _material: None,
                        textures,
                        samplers,
                    });
                }
            }
        }
        drop(material_groups);
        let uniforms = MaterialUniforms3D {
            view: view_buffer,
            objects,
            materials,
            legacy_view,
            legacy_objects,
            pbr_view,
            pbr_objects,
            _charge: charge,
            #[cfg(test)]
            observation,
        };
        Ok((
            MaterialGpuRetention3D {
                uniforms,
                draws,
                _programs: programs.clone(),
                direct,
                indirect,
            },
            uploads,
        ))
    })();
    let validation = pollster::block_on(validation_scope.pop());
    let allocation = pollster::block_on(allocation_scope.pop());
    if let Some(error) = validation.or(allocation) {
        return Err(PbrError3D::Shader(error.to_string()));
    }
    #[cfg(any(test, feature = "test-harness"))]
    creations.add_to(timing.as_deref_mut());
    let (retained, uploads) = built?;
    #[cfg(any(test, feature = "test-harness"))]
    if let Some(t) = timing {
        t.branch = "New";
        t.uniform_buffers += 3;
        t.geometry_plans += uploads.geometry.len() + uploads.legacy_geometry.len();
        t.texture_plans += uploads.textures.len();
        t.mip_plans += uploads.mips.len();
    }
    let write_shadows = retained.direct.as_ref().is_some_and(|d| {
        !current
            .and_then(PreparedMaterialScene3D::retained)
            .and_then(|s| s.direct.as_ref())
            .is_some_and(|old| Arc::ptr_eq(d, old))
    });
    Ok(MaterialScenePreparation3D::New(PreparedMaterialScene3D {
        source,
        view,
        origin,
        extent,
        packed,
        queue: gpu.queue.clone(),
        write_scene: true,
        write_shadows,
        retained: Some(retained),
        uploads: Some(uploads),
        capacity_slot: None,
        upload_bytes: p.max_upload_bytes_per_frame - upload.remaining_bytes(),
    }))
}

impl PbrProgram3D {
    pub(super) fn shadow_pipeline(
        &self,
        context: &Context3D,
        cull: bool,
        cube: bool,
    ) -> Result<Arc<PbrPipeline3D>, super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        let mut cache = self
            .shadow_pipelines
            .lock()
            .map_err(|_| PbrError3D::Lock("shadow pipeline cache"))?;
        if let Some(p) = cache.get(&(cull, cube)) {
            return Ok(p.clone());
        }
        let mut charge = context
            .raster_gpu()?
            .reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
        let gpu = context.gpu_resources()?;
        const PRIMARY: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Unorm8x4,4=>Float32x4];
        const FACE: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 5,
        }];
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("R025 actual source geometry/Mask reverse-Z depth"),
                layout: self.shadow_layout.as_ref(),
                vertex: wgpu::VertexState {
                    module: &self.module,
                    entry_point: Some("pbr_vertex"),
                    buffers: &[
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 52,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &PRIMARY,
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 12,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &FACE,
                        }),
                    ],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.module,
                    entry_point: Some("shadow_fragment"),
                    targets: &[],
                    compilation_options: Default::default(),
                }),
                // Native cube U/V orientation reverses projected winding relative
                // to the ordinary camera. Compensate the raster classification,
                // retaining the SAME original source front/cull semantics.
                primitive: wgpu::PrimitiveState {
                    front_face: if cube {
                        wgpu::FrontFace::Cw
                    } else {
                        wgpu::FrontFace::Ccw
                    },
                    cull_mode: if cull { Some(wgpu::Face::Back) } else { None },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        if let Some(e) = pollster::block_on(validation_scope.pop()) {
            return Err(PbrError3D::Shader(e.to_string()));
        }
        charge.commit();
        let p = Arc::new(PbrPipeline3D {
            pipeline,
            _charge: charge,
        });
        cache.insert((cull, cube), p.clone());
        Ok(p)
    }
    pub(super) fn new(context: &Context3D) -> Result<Self, super::pbr::PbrError3D> {
        Self::new_family(context, false)
    }
    pub(super) fn new_family(
        context: &Context3D,
        direct_enabled: bool,
    ) -> Result<Self, super::pbr::PbrError3D> {
        Self::new_profile(
            context,
            super::indirect::Presence3D(u8::from(direct_enabled)),
        )
    }
    pub(super) fn new_profile(
        context: &Context3D,
        profile: super::indirect::Presence3D,
    ) -> Result<Self, super::pbr::PbrError3D> {
        Self::new_sampling_profile(context, profile, SamplingFamily3D::V1)
    }
    pub(super) fn new_sampling_profile(
        context: &Context3D,
        profile: super::indirect::Presence3D,
        sampling: SamplingFamily3D,
    ) -> Result<Self, super::pbr::PbrError3D> {
        Self::new_object_profile(context, profile, sampling, 112)
    }

    pub(super) fn new_reveal_sampling_profile(
        context: &Context3D,
        profile: super::indirect::Presence3D,
        sampling: SamplingFamily3D,
    ) -> Result<Self, super::pbr::PbrError3D> {
        Self::new_object_profile(context, profile, sampling, 128)
    }

    fn new_object_profile(
        context: &Context3D,
        profile: super::indirect::Presence3D,
        sampling: SamplingFamily3D,
        object_binding_bytes: u64,
    ) -> Result<Self, super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        if profile.0 >= 8 {
            return Err(PbrError3D::Association("unknown controlled D/E/G profile"));
        }
        let direct_enabled = profile.direct();
        let gpu = context.gpu_resources()?;
        let limits = gpu.device.limits();
        if limits.max_bind_groups < 3
            || limits.max_bindings_per_bind_group < 11
            || limits.max_sampled_textures_per_shader_stage < 5
            || limits.max_samplers_per_shader_stage < 5
            || limits.max_uniform_buffers_per_shader_stage < 3
            || limits.max_uniform_buffer_binding_size < object_binding_bytes
            || limits.max_vertex_attributes < 5
            || limits.max_vertex_buffer_array_stride < 52
            || limits.max_inter_stage_shader_variables < 6 // Five locations and front-facing.
        {
            return Err(PbrError3D::Association(
                "Device cannot admit exact R024 ABI/stage limits",
            ));
        }
        if direct_enabled
            && (limits.max_bind_groups < 4
                || limits.max_vertex_buffers < 2
                || limits.max_vertex_attributes < 6
                || limits.max_inter_stage_shader_variables < 7 // Also carries the face normal.
                || limits.max_sampled_textures_per_shader_stage < 7
                || limits.max_samplers_per_shader_stage < 7
                || limits.max_uniform_buffers_per_shader_stage < 5
                || limits.max_uniform_buffer_binding_size < 1984
                || limits.max_texture_array_layers < 24
                || limits.max_texture_dimension_2d < 1024)
        {
            return Err(PbrError3D::Direct {
                field: "Device limits",
                cause: "exact four-group/six-attribute/19-component/depth-array R025 family unsupported",
            });
        }
        if profile.indirect()
            && (limits.max_bind_groups < 4
                || limits.max_sampled_textures_per_shader_stage
                    < 5 + u32::from(direct_enabled) * 2 + u32::from(profile.environment()) * 3
                || limits.max_samplers_per_shader_stage
                    < 5 + u32::from(direct_enabled) * 2 + u32::from(profile.environment())
                || limits.max_uniform_buffers_per_shader_stage < 4 + u32::from(direct_enabled) * 2
                || (profile.environment()
                    && (limits.max_texture_array_layers < 48
                        || limits.max_texture_dimension_2d < 128)))
        {
            return Err(PbrError3D::Indirect {
                field: "Device limits",
                cause: "exact selected four-group E/G stage/array layout unavailable",
            });
        }
        let reservation = context.reserve_controlled_shader_module()?;
        let source = super::object_reveal::controlled_material_source(
            profile,
            sampling,
            object_binding_bytes == 128,
        )?;
        #[cfg(test)]
        let mut source = source;
        #[cfg(test)]
        if !direct_enabled && !profile.indirect() {
            if sampling == SamplingFamily3D::V2 {
                source.push_str(
                    &PBR_OBSERVATION_WGSL
                        .replace(
                            "let s=surface(i);",
                            "let sampled=sample_surface(i);let s=sampled.value;",
                        )
                        .replace(
                            "vec4(i.tangent.xyz,length(i.tangent.xyz)),vec4(i.uv0,0.0,1.0)",
                            "vec4(sampled.dx,sampled.dy),vec4(i.uv0,s.normal_sample.z,1.0)",
                        ),
                );
            } else {
                source.push_str(PBR_OBSERVATION_WGSL);
            }
        }
        #[cfg(test)]
        if direct_enabled && !profile.indirect() {
            source.push_str(super::direct_shadow::PROOF_WGSL);
        }
        #[cfg(test)]
        if profile.indirect() && !direct_enabled {
            source.push_str(super::indirect::PROBE_WGSL);
        }
        let mut ir = naga::front::wgsl::parse_str(&source)
            .map_err(|e| PbrError3D::Shader(e.emit_to_string(&source)))?;
        super::hdr::scene_fragment_outputs(
            &mut ir,
            &[
                "pbr_fragment",
                "direct_fragment",
                "indirect_fragment",
                "pbr_observe_fragment",
                "direct_observe_fragment",
            ],
        )
        .map_err(PbrError3D::Shader)?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            if direct_enabled || profile.environment() {
                naga::valid::Capabilities::CUBE_ARRAY_TEXTURES
            } else {
                naga::valid::Capabilities::empty()
            },
        )
        .validate(&ir)
        .map_err(|e| PbrError3D::Shader(e.emit_to_string(&source)))?;
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Halcyon private controlled R024 PBR V1"),
                source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(ir)),
            });
        let uniform = |binding, visibility, size| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: NonZeroU64::new(size),
            },
            count: None,
        };
        let view = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("R024 View80"),
                entries: &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT, 80)],
            });
        let object = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(if object_binding_bytes == 128 {
                    "HAL372 Object128Reveal VERTEX FRAGMENT"
                } else {
                    "R024 Object112 VERTEX FRAGMENT"
                }),
                entries: &[uniform(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    object_binding_bytes,
                )],
            });
        let mut entries = vec![uniform(0, wgpu::ShaderStages::FRAGMENT, 64)];
        for binding in 1..=5 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        for binding in 6..=10 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let material = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("R024 Material64 five roles"),
                entries: &entries,
            });
        let layouts = [view, object, material];
        let direct_layout = if direct_enabled {
            Some(direct_bind_layout(&gpu.device))
        } else {
            None
        };
        let color_layout = if profile.indirect() {
            Some(indirect_bind_layout(&gpu.device, profile))
        } else {
            direct_layout.clone()
        };
        let mut group_layouts = vec![Some(&layouts[0]), Some(&layouts[1]), Some(&layouts[2])];
        if let Some(d) = &color_layout {
            group_layouts.push(Some(d));
        }
        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("R024 exact three groups; group3 absent"),
                bind_group_layouts: &group_layouts,
                immediate_size: 0,
            });
        let shadow_layout = if direct_enabled {
            Some(
                gpu.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(
                            "R025 shadow depth reads material/source only, never writable depth",
                        ),
                        bind_group_layouts: &[Some(&layouts[0]), Some(&layouts[1]), Some(&layouts[2])],
                        immediate_size: 0,
                    }),
            )
        } else {
            None
        };
        #[cfg(test)]
        let observation_layout = if direct_enabled {
            gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{label:Some("test-only D observations beside View80; production group0 remains View80 only"),entries:&[
                uniform(0,wgpu::ShaderStages::VERTEX_FRAGMENT,80),
                wgpu::BindGroupLayoutEntry{binding:2,visibility:wgpu::ShaderStages::FRAGMENT,ty:wgpu::BindingType::Buffer{ty:wgpu::BufferBindingType::Storage{read_only:false},has_dynamic_offset:false,min_binding_size:None},count:None},
                wgpu::BindGroupLayoutEntry{binding:4,visibility:wgpu::ShaderStages::FRAGMENT,ty:wgpu::BindingType::Buffer{ty:wgpu::BufferBindingType::Uniform,has_dynamic_offset:false,min_binding_size:NonZeroU64::new(16)},count:None},
            ]})
        } else {
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(
                        "test observation only; absent from production R024 entry interface",
                    ),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: NonZeroU64::new(16),
                            },
                            count: None,
                        },
                    ],
                })
        };
        #[cfg(test)]
        let observation_groups = if let Some(direct) = &direct_layout {
            vec![Some(&observation_layout), Some(&layouts[1]), Some(&layouts[2]), Some(direct)]
        } else {
            vec![Some(&layouts[0]), Some(&layouts[1]), Some(&layouts[2]), Some(&observation_layout)]
        };
        #[cfg(test)]
        let observation_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("private numeric observation of exact PBR routines"),
                    bind_group_layouts: &observation_groups,
                    immediate_size: 0,
                });
        let validation = pollster::block_on(validation_scope.pop());
        let allocation = pollster::block_on(allocation_scope.pop());
        if let Some(error) = validation.or(allocation) {
            return Err(PbrError3D::Shader(error.to_string()));
        }
        Ok(Self {
            module,
            layouts,
            layout,
            pipelines: Mutex::new(std::collections::HashMap::new()),
            direct_layout,
            color_layout,
            profile,
            sampling,
            #[cfg(test)]
            object_binding_bytes,
            shadow_layout,
            shadow_pipelines: Mutex::new(HashMap::new()),
            _module_charge: reservation.commit_one(),
            #[cfg(test)]
            observation_layout,
            #[cfg(test)]
            observation_pipeline_layout,
        })
    }

    pub(super) fn pipeline(
        &self,
        context: &Context3D,
        samples: u32,
        cull: bool,
        observe: bool,
    ) -> Result<Arc<PbrPipeline3D>, super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        if !matches!(samples, 1 | 4) {
            return Err(PbrError3D::Association("PBR sample count must be1 or4"));
        }
        #[cfg(not(test))]
        if observe {
            return Err(PbrError3D::Association("numeric observation is test-only"));
        }
        let mut pipelines = self
            .pipelines
            .lock()
            .map_err(|_| PbrError3D::Lock("PBR pipeline cache"))?;
        if let Some(p) = pipelines.get(&(samples, cull, observe)) {
            return Ok(p.clone());
        }
        let mut charge = context
            .raster_gpu()?
            .reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
        let gpu = context.gpu_resources()?;
        const ATTRIBUTES: [wgpu::VertexAttribute; 5] = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Unorm8x4,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 36,
                shader_location: 4,
            },
        ];
        let layout = &self.layout;
        #[cfg(test)]
        let mut layout = layout;
        let mut entry = if self.direct_layout.is_some() {
            "direct_fragment"
        } else {
            "pbr_fragment"
        };
        if self.profile.indirect() {
            entry = "indirect_fragment";
            if observe {
                return Err(PbrError3D::Association(
                    "R026 uses its separate exact numeric probe, not the R024 observation ABI",
                ));
            }
        }
        #[cfg(test)]
        if observe {
            layout = &self.observation_pipeline_layout;
            entry = if self.direct_layout.is_some() {
                "direct_observe_fragment"
            } else {
                "pbr_observe_fragment"
            };
        }
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let face_attributes = [wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 5,
        }];
        let mut vertex_buffers = vec![Some(wgpu::VertexBufferLayout {
            array_stride: 52,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        })];
        if self.direct_layout.is_some() {
            vertex_buffers.push(Some(wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &face_attributes,
            }));
        }
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Halcyon controlled PbrMetallicRoughnessRealtimeF32V1"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &self.module,
                    entry_point: Some("pbr_vertex"),
                    buffers: &vertex_buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.module,
                    entry_point: Some(entry),
                    targets: &super::hdr::scene_color_targets(),
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: if cull { Some(wgpu::Face::Back) } else { None },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });
        if let Some(e) = pollster::block_on(validation_scope.pop()) {
            return Err(PbrError3D::Shader(e.to_string()));
        }
        charge.commit();
        let result = Arc::new(PbrPipeline3D {
            pipeline,
            _charge: charge,
        });
        pipelines.insert((samples, cull, observe), result.clone());
        Ok(result)
    }
}

// Fixed R028 §12.4 instrumentation only. No production entry/layout is changed.
#[cfg(test)]
const R028_ISOLATED_WGSL: &str = r#"
fn isolated_color(s:Surface)->vec3<f32> {
 return (s.base + (s.normal_sample+vec3(1.0))*0.5
  + vec3(s.metallic,s.roughness,(s.metallic+s.roughness)*0.5)
  + vec3(s.ao) + s.emission)*0.2;
}
@fragment fn r028_isolated(i:FragmentIn)->@location(0) vec4<f32> {
 let sampled=sample_surface(i);
 return vec4(isolated_color(sampled.value),1.0);
}
struct IsolatedObservation {
 dxdy:vec4<f32>, uv:vec4<f32>, base:vec4<f32>, normal:vec4<f32>,
 mr_ao:vec4<f32>, emission:vec4<f32>, output_color:vec4<f32>
}
@group(3) @binding(0) var<storage,read_write> isolated_observations:array<IsolatedObservation>;
@fragment fn r028_isolated_observe(i:FragmentIn)->@location(0) vec4<f32> {
 let sampled=sample_surface(i); let s=sampled.value;
 let result=vec4(isolated_color(s),1.0);
 let p=vec2<u32>(i.clip.xy); let local=p%vec2(120u);
 let center=all(local==vec2(60u)); let other=all(local==vec2(30u));
 if center || other {
  let index=((p.y/120u)*16u+p.x/120u)*2u+select(1u,0u,center);
  isolated_observations[index]=IsolatedObservation(vec4(sampled.dx,sampled.dy),
   vec4(i.uv0,0.0,1.0),vec4(s.base,s.alpha),vec4(s.normal_sample,1.0),
   vec4(s.metallic,s.roughness,s.ao,1.0),vec4(s.emission,1.0),result);
 }
 return result;
}
"#;

#[cfg(test)]
struct R028IsolatedResources {
    scenes: [MaterialGpuRetention3D; 2],
    packed: [MaterialFrameBytes3D; 2],
    _module: wgpu::ShaderModule,
    pipeline: wgpu::RenderPipeline,
    observe_pipeline: wgpu::RenderPipeline,
    view: wgpu::TextureView,
    _texture: wgpu::Texture,
    observation_binding: wgpu::BindGroup,
    observations: wgpu::Buffer,
    observation_readback: wgpu::Buffer,
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    timing_readback: wgpu::Buffer,
    _module_charge: super::context::ShaderModuleResidency,
    _pipeline_charges: [ControlledPipelineResidency; 2],
    _storage: DynamicStorageLease3D,
    _target: super::context::TargetBudgetLease3D,
}

#[cfg(test)]
pub(crate) struct R028IsolatedSamplingForTest<'a> {
    context: &'a Context3D,
    gpu: GpuResources,
    resources: Option<R028IsolatedResources>,
    pub(crate) initial_texture_upload_batches: usize,
    pub(crate) initial_geometry_upload_batches: usize,
    pub(crate) initial_uniform_upload_bytes: u64,
    pub(crate) attachment_bytes: u64,
    pub(crate) timestamp_period_ns: f64,
    pub(crate) buffer_bytes: u64,
}

#[cfg(test)]
impl Context3D {
    pub(crate) fn r028_isolated_sampling_for_test(
        &self,
        sources: &[Arc<super::pbr::MaterialSceneSource3D>; 2],
        view: ViewDescriptor3,
        format_features: wgpu::TextureFormatFeatures,
    ) -> R028IsolatedSamplingForTest<'_> {
        R028IsolatedSamplingForTest::new(self, sources, view, format_features)
    }
}

#[cfg(test)]
impl Drop for R028IsolatedSamplingForTest<'_> {
    fn drop(&mut self) {
        if let Some(resources) = self.resources.take() {
            // Same existing Queue-safe custody pattern: no Context/Device/Queue
            // strong owner enters the completion closure.
            crate::renderer::on_submitted_work_done(&self.gpu.queue, move || drop(resources));
        }
    }
}

#[cfg(test)]
impl<'a> R028IsolatedSamplingForTest<'a> {
    pub(crate) fn new(
        context: &'a Context3D,
        sources: &[Arc<super::pbr::MaterialSceneSource3D>; 2],
        view: ViewDescriptor3,
        format_features: wgpu::TextureFormatFeatures,
    ) -> Self {
        const PIXELS: u64 = 1920 * 1080;
        const OBSERVATION_BYTES: u64 = 144 * 2 * 112;
        const QUERY_COUNT: u32 = (30 + 120) * 4;
        const QUERY_BYTES: u64 = QUERY_COUNT as u64 * 8;
        let gpu = context.gpu_resources().unwrap();
        assert_eq!(gpu.device.features(), wgpu::Features::TIMESTAMP_QUERY);
        let samples = sources[0].samples;
        assert!(matches!(samples, 1 | 4));
        assert_eq!(sources[1].samples, samples);
        assert!(
            format_features
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        );
        assert!(format_features.flags.sample_count_supported(samples));
        let limits = gpu.device.limits();
        assert!(limits.max_texture_dimension_2d >= 1920);
        assert!(QUERY_COUNT <= wgpu::QUERY_SET_MAX_QUERIES);
        assert!(QUERY_BYTES <= limits.max_buffer_size);
        assert!(OBSERVATION_BYTES <= u64::from(limits.max_storage_buffer_binding_size));
        let period = f64::from(gpu.queue.get_timestamp_period());
        assert!(period.is_finite() && period > 0.0);
        let attachment_bytes = PIXELS
            .checked_mul(8)
            .unwrap()
            .checked_mul(u64::from(samples))
            .unwrap();
        validate_budget(
            PolicyBudget3D::SingleTargetAttachmentBytes,
            attachment_bytes,
            context.limits().policy.max_single_target_attachment_bytes,
        )
        .unwrap();
        let target = context
            .reserve_target_budget(PIXELS, attachment_bytes)
            .unwrap();
        let shader = format!(
            "{}{}",
            super::mip_sampling::shader_source(),
            R028_ISOLATED_WGSL
        );
        let buffer_bytes = 2 * OBSERVATION_BYTES + 3 * QUERY_BYTES;
        let storage = context
            .reserve_dynamic_storage(
                buffer_bytes,
                0,
                1,
                2 * OBSERVATION_BYTES
                    + QUERY_BYTES
                    + shader.len() as u64
                    + std::mem::size_of::<R028IsolatedResources>() as u64,
            )
            .unwrap();
        let mut retained = Vec::new();
        let mut packed = Vec::new();
        let mut texture_uploads = 0;
        let mut geometry_uploads = 0;
        let mut uniform_upload_bytes = 0;
        for source in sources {
            source.validate(context).unwrap();
            assert!(source.direct.is_none() && source.indirect.is_none() && !source.observe);
            let MaterialScenePreparation3D::New(mut prepared) = prepare_material_scene(
                context,
                source.clone(),
                view,
                WorldPoint3::origin(),
                (1920, 1080),
                None,
                None,
                None,
            )
            .unwrap() else {
                panic!("isolated benchmark requires two complete prepared sources")
            };
            let state = prepared.retained.take().unwrap();
            assert_eq!(state.draws.len(), 1);
            let draw = &state.draws[0];
            let MaterialGeometry3D::Pbr { gpu: mesh, .. } = &draw.geometry else {
                panic!()
            };
            assert_eq!(mesh.vertex_count, 864);
            assert!(mesh.index.is_none() && mesh.face.is_none());
            let material = draw._material.as_ref().unwrap();
            assert_eq!(material.data.source.sampling, SamplingFamily3D::V2);
            assert!(material.data.source.roles.iter().all(Option::is_some));
            assert_eq!(material.data.source.base_rgba, [1.0; 4]);
            assert_eq!(material.data.source.emissive_rgb, [1.0; 3]);
            if let Some(uploads) = prepared.uploads.take() {
                texture_uploads += uploads.mips.len();
                geometry_uploads += uploads.geometry.len();
                uploads.commit(&gpu.queue);
            }
            // Same static writes as PreparedMaterialScene3D::encode, before
            // any observation/warmup. The timed path never writes uniforms.
            gpu.queue
                .write_buffer(&state.uniforms.view, 0, &prepared.packed.view);
            gpu.queue
                .write_buffer(&state.uniforms.objects, 0, &prepared.packed.objects);
            gpu.queue
                .write_buffer(&state.uniforms.materials, 0, &prepared.packed.materials);
            uniform_upload_bytes += (prepared.packed.view.len()
                + prepared.packed.objects.len()
                + prepared.packed.materials.len()) as u64;
            packed.push(MaterialFrameBytes3D {
                object_binding_bytes: prepared.packed.object_binding_bytes,
                view: prepared.packed.view.clone(),
                objects: prepared.packed.objects.clone(),
                materials: prepared.packed.materials.clone(),
                object_offsets: prepared.packed.object_offsets.clone(),
                material_offsets: prepared.packed.material_offsets.clone(),
            });
            retained.push(state);
        }
        assert_eq!(packed[0].view, packed[1].view);
        assert_eq!(packed[0].objects, packed[1].objects);
        assert_eq!(packed[0].materials, packed[1].materials);
        let scenes: [MaterialGpuRetention3D; 2] = retained.try_into().ok().unwrap();
        let packed: [MaterialFrameBytes3D; 2] = packed.try_into().ok().unwrap();
        let program = &scenes[0]._programs[0];
        let reservation = context.reserve_controlled_shader_module().unwrap();
        let ir = naga::front::wgsl::parse_str(&shader).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&ir)
        .unwrap();
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("R028 exact sample_surface isolated instrumentation"),
                source: wgpu::ShaderSource::Wgsl(shader.into()),
            });
        let module_charge = reservation.commit_one();
        let observation_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("R028 untimed source-stage samples only"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(OBSERVATION_BYTES),
                        },
                        count: None,
                    }],
                });
        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("R028 isolated sampling groups0–2"),
                bind_group_layouts: &[
                    Some(&program.layouts[0]),
                    Some(&program.layouts[1]),
                    Some(&program.layouts[2]),
                ],
                immediate_size: 0,
            });
        let observed_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("R028 untimed observations, absent from timed pipeline"),
                bind_group_layouts: &[
                    Some(&program.layouts[0]),
                    Some(&program.layouts[1]),
                    Some(&program.layouts[2]),
                    Some(&observation_layout),
                ],
                immediate_size: 0,
            });
        let create_pipeline = |layout: &wgpu::PipelineLayout, entry: &str| {
            let mut charge = context
                .raster_gpu()
                .unwrap()
                .reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)
                .unwrap();
            const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
                0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Unorm8x4, 4 => Float32x4
            ];
            let pipeline = gpu
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("R028 isolated identical five-role sampling pipeline"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("pbr_vertex"),
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: 52,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &ATTRIBUTES,
                        })],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(entry),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: samples,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview_mask: None,
                    cache: None,
                });
            charge.commit();
            (pipeline, charge)
        };
        let (pipeline, pipeline_charge) = create_pipeline(&layout, "r028_isolated");
        let (observe_pipeline, observe_charge) =
            create_pipeline(&observed_layout, "r028_isolated_observe");
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("R028 isolated1920x1080 Rgba16Float scratch; no depth/resolve/front"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target_view = texture.create_view(&Default::default());
        let buffer = |label, size, usage| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let observations = buffer(
            "R028 untimed288 source observations",
            OBSERVATION_BYTES,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        );
        let observation_readback = buffer(
            "R028 untimed observation readback",
            OBSERVATION_BYTES,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let observation_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("R028 untimed readback only"),
            layout: &observation_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: observations.as_entire_binding(),
            }],
        });
        let queries = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("R028600 real pass-boundary GPU timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERY_COUNT,
        });
        let resolve = buffer(
            "R028 timestamp resolve",
            QUERY_BYTES,
            wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        );
        let timing_readback = buffer(
            "R028 timestamp readback outside query windows",
            QUERY_BYTES,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        assert!(pollster::block_on(validation_scope.pop()).is_none());
        Self {
            context,
            gpu,
            initial_texture_upload_batches: texture_uploads,
            initial_geometry_upload_batches: geometry_uploads,
            initial_uniform_upload_bytes: uniform_upload_bytes,
            attachment_bytes,
            timestamp_period_ns: period,
            buffer_bytes,
            resources: Some(R028IsolatedResources {
                scenes,
                packed,
                _module: module,
                pipeline,
                observe_pipeline,
                view: target_view,
                _texture: texture,
                observation_binding,
                observations,
                observation_readback,
                queries,
                resolve,
                timing_readback,
                _module_charge: module_charge,
                _pipeline_charges: [pipeline_charge, observe_charge],
                _storage: storage,
                _target: target,
            }),
        }
    }

    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        variant: usize,
        observe: bool,
        first_query: Option<u32>,
    ) {
        let r = self.resources.as_ref().unwrap();
        let s = &r.scenes[variant];
        let packed = &r.packed[variant];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("R028 fixed raster/five texture samples/short ALU/attachment only"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: &r.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: first_query.map(|q| wgpu::RenderPassTimestampWrites {
                query_set: &r.queries,
                beginning_of_pass_write_index: Some(q),
                end_of_pass_write_index: Some(q + 1),
            }),
            multiview_mask: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(if observe {
            &r.observe_pipeline
        } else {
            &r.pipeline
        });
        pass.set_viewport(0.0, 0.0, 1920.0, 1080.0, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, 1920, 1080);
        pass.set_bind_group(0, s.uniforms.pbr_view.as_ref().unwrap(), &[0]);
        pass.set_bind_group(
            1,
            s.uniforms.pbr_objects.as_ref().unwrap(),
            &[packed.object_offsets[0]],
        );
        pass.set_bind_group(2, &s.draws[0].binding, &[packed.material_offsets[0]]);
        if observe {
            pass.set_bind_group(3, &r.observation_binding, &[]);
        }
        let MaterialGeometry3D::Pbr { gpu: mesh, .. } = &s.draws[0].geometry else {
            panic!()
        };
        pass.set_vertex_buffer(0, mesh.vertex.slice(..));
        pass.draw(0..864, 0..1);
    }

    fn read(&self, buffer: &wgpu::Buffer) -> Vec<u8> {
        let (send, receive) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap();
            });
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let bytes = buffer.slice(..).get_mapped_range().expect("mapped readback range").to_vec();
        buffer.unmap();
        bytes
    }

    pub(crate) fn observe(&self, variant: usize) -> Vec<[[f32; 4]; 7]> {
        let r = self.resources.as_ref().unwrap();
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&r.observations, 0, None);
        self.encode(&mut encoder, variant, true, None);
        encoder.copy_buffer_to_buffer(
            &r.observations,
            0,
            &r.observation_readback,
            0,
            144 * 2 * 112,
        );
        self.gpu.queue.submit([encoder.finish()]);
        self.read(&r.observation_readback)
            .chunks_exact(112)
            .map(bytemuck::pod_read_unaligned)
            .collect()
    }

    pub(crate) fn measure(&self) -> (Vec<u64>, [[f64; 120]; 2]) {
        let r = self.resources.as_ref().unwrap();
        let before = self.context.resource_usage().unwrap();
        let execute_pairs = |range: std::ops::Range<u32>| {
            for pair in range {
                let order = if pair % 2 == 0 { [0, 1] } else { [1, 0] };
                for (within, variant) in order.into_iter().enumerate() {
                    // Isolate pass windows. This CPU wait is synchronization,
                    // never a timer; only the two hardware samples give time.
                    let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
                    self.encode(
                        &mut encoder,
                        variant,
                        false,
                        Some(pair * 4 + within as u32 * 2),
                    );
                    self.gpu.queue.submit([encoder.finish()]);
                    self.gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                }
            }
        };
        execute_pairs(0..30);
        assert_eq!(self.context.resource_usage().unwrap(), before);
        execute_pairs(30..150);
        // All counters, including the last pass end, exist before the separate
        // untimed resolve/copy/readback submission.
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(&r.queries, 0..600, &r.resolve, 0);
        encoder.copy_buffer_to_buffer(&r.resolve, 0, &r.timing_readback, 0, 600 * 8);
        self.gpu.queue.submit([encoder.finish()]);
        let bytes = self.read(&r.timing_readback);
        let ticks: Vec<u64> = bytes
            .chunks_exact(8)
            .map(bytemuck::pod_read_unaligned)
            .collect();
        assert_eq!(ticks.len(), 600);
        let first_inversion = ticks.windows(2).enumerate().find(|(_, w)| w[1] < w[0]);
        assert!(
            first_inversion.is_none(),
            "nonmonotone GPU timestamps: first_inversion={first_inversion:?}; positive_pass_intervals={}/300; first_ticks={:?}; last_ticks={:?}; period_ns={}",
            ticks
                .chunks_exact(2)
                .filter(|w| w[0] > 0 && w[1] > w[0])
                .count(),
            &ticks[..8],
            &ticks[592..],
            self.timestamp_period_ns,
        );
        let mut result = [[0.0; 120]; 2];
        for pair in 0..150 {
            let order = if pair % 2 == 0 { [0, 1] } else { [1, 0] };
            for (within, variant) in order.into_iter().enumerate() {
                let start = ticks[pair * 4 + within * 2];
                let end = ticks[pair * 4 + within * 2 + 1];
                assert!(
                    start > 0 && end > start,
                    "missing/nonpositive hardware timestamp"
                );
                let ns = (end - start) as f64 * self.timestamp_period_ns;
                assert!(ns.is_finite() && ns > 0.0);
                if pair >= 30 {
                    result[variant][pair - 30] = ns;
                }
            }
        }
        assert_eq!(self.context.resource_usage().unwrap(), before);
        (ticks, result)
    }
}

#[cfg(test)]
const PBR_OBSERVATION_WGSL: &str = r#"
struct BrdfProbe { base:vec4<f32>, mr:vec4<f32>, N:vec4<f32>, V:vec4<f32>, L:vec4<f32> }
@group(3) @binding(2) var<storage,read> brdf_probe_input:array<BrdfProbe>;
@group(3) @binding(3) var<storage,read_write> brdf_probe_output:array<vec4<f32>>;
// Fixed test incident directions exercise the production evaluator without a
// production light binding, alternate BRDF or scene-radiance provider.
@compute @workgroup_size(1) fn pbr_brdf_probe(@builtin(global_invocation_id) id:vec3<u32>) {
 if id.x<arrayLength(&brdf_probe_input) {
  let p=brdf_probe_input[id.x];
  brdf_probe_output[id.x]=vec4(reflected_brdf(p.base.xyz,p.mr.x,p.mr.y,p.N.xyz,p.V.xyz,p.L.xyz),1.0);
 }
}
struct Observation { local_n:vec4<f32>, local_g:vec4<f32>, world_n:vec4<f32>, world_t:vec4<f32>,
 color:vec4<f32>, receiver:vec4<f32>, base_alpha:vec4<f32>, mr_ao:vec4<f32>, brdf:vec4<f32>,
 local_t:vec4<f32>, center_uv:vec4<f32> }
struct ObservationHeader { width:u32, height:u32, unused0:u32, unused1:u32 }
@group(3) @binding(0) var<storage,read_write> observations:array<Observation>;
@group(3) @binding(1) var<uniform> observation_header:ObservationHeader;
// This observer is never the production material entry. It observes the SAME
// vertex interface, surface/frame/BRDF functions, then emits the same emission.
@fragment fn pbr_observe_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
 let s=surface(i); if material.role_mode.y==1u && s.alpha<material.mr_ao_cutoff.w {discard;}
 let f=frame(i,s.normal_sample,front);
 let p=vec2<u32>(i.clip.xy); let index=p.y*observation_header.width+p.x;
 if p.x<observation_header.width && p.y<observation_header.height {
  observations[index]=Observation(vec4(f.local_n,length(f.local_n)),vec4(f.g,length(f.g)),
   vec4(f.N,1.0),vec4(f.T,1.0),i.color,vec4(i.position,1.0),vec4(s.base,s.alpha),
   vec4(s.metallic,s.roughness,s.ao,1.0),
   vec4(reflected_brdf(s.base,s.metallic,s.roughness,f.N,view.camera_relative-i.position,vec3(0.0,0.0,1.0)),1.0),
   vec4(i.tangent.xyz,length(i.tangent.xyz)),vec4(i.uv0,0.0,1.0));
 }
 return vec4(s.emission,1.0);
}
"#;

fn direct_comparison_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("R025 nearest single-Boolean GreaterEqual comparison"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        compare: Some(wgpu::CompareFunction::GreaterEqual),
        anisotropy_clamp: 1,
        ..Default::default()
    })
}

/// Actual R-025 comparison functions on a bounded test-owned depth source.
/// This observes the production nine-test routines, never CPU depth emulation.
#[cfg(test)]
pub(crate) fn direct_compare_probe_for_test(
    context: &Context3D,
    inputs: &[super::direct_shadow::DirectProbeInput3D],
    pattern: u32,
) -> Result<Vec<[[f32; 4]; 20]>, super::pbr::PbrError3D> {
    use super::direct_shadow::{LightBlock608, POOL_BYTES, ShadowBlock1984};
    use super::pbr::PbrError3D;
    use bytemuck::Zeroable;
    if inputs.is_empty() || pattern > 5 {
        return Err(PbrError3D::Association("fixed comparison fixture input"));
    }
    for i in inputs {
        if i.mode[0] > 2
            || (i.mode[0] == 0 && i.mode[1] >= 7)
            || (i.mode[0] == 1 && (i.mode[1] >= 4 || i.p[3] <= 0.0 || i.light[3] <= 0.01))
            || i.p
                .iter()
                .chain(i.geometric.iter())
                .chain(i.light.iter())
                .any(|v| !v.is_finite())
        {
            return Err(PbrError3D::Association("unqualified comparison probe"));
        }
    }
    let gpu = context.gpu_resources()?;
    let raster = context.raster_gpu()?;
    let program = PbrProgram3D::new_family(context, true)?;
    let align = u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment).max(16);
    let input_bytes = (inputs.len() as u64)
        .checked_mul(64)
        .ok_or(PbrError3D::Association("comparison input byte overflow"))?;
    let output_bytes = (inputs.len() as u64)
        .checked_mul(320)
        .ok_or(PbrError3D::Association("comparison output byte overflow"))?;
    if input_bytes > u64::from(gpu.device.limits().max_storage_buffer_binding_size)
        || output_bytes > u64::from(gpu.device.limits().max_storage_buffer_binding_size)
    {
        return Err(PbrError3D::Association("comparison Device storage limit"));
    }
    let storage = input_bytes + output_bytes * 2 + 2672 + 31 * align;
    validate_budget(
        PolicyBudget3D::DynamicStorageBytesPerTarget,
        storage,
        context.limits().policy.max_dynamic_storage_bytes_per_target,
    )?;
    validate_budget(
        PolicyBudget3D::SingleTargetAttachmentBytes,
        POOL_BYTES,
        context.limits().policy.max_single_target_attachment_bytes,
    )?;
    let _storage = context.reserve_dynamic_storage(
        storage,
        0,
        1,
        input_bytes + output_bytes + 31 * align + std::mem::size_of::<DirectTarget3D>() as u64,
    )?;
    let _attachments = context.reserve_target_budget(0, POOL_BYTES)?;
    let _textures = context.reserve_shadow_texture_bytes(POOL_BYTES)?;
    let mut upload = context.begin_preparation_upload_budget();
    upload.consume(input_bytes + 2672 + 31 * align)?;
    let mut fixture_charge =
        raster.reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
    let mut compute_charge =
        raster.reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
    let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let create_buffer = |label, size, usage| {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let input = create_buffer(
        "R025 immutable numeric probe source",
        input_bytes,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    );
    let output = create_buffer(
        "R025 real comparison observations",
        output_bytes,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    );
    let readback = create_buffer(
        "R025 observation readback",
        output_bytes,
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    );
    let light = create_buffer(
        "R025 probe Light608",
        608,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let transforms = create_buffer(
        "R025 probe Shadow1984",
        1984,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let view = create_buffer(
        "R025 probe source-qualified camera View80",
        80,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let config = create_buffer(
        "R025 bounded fixed-depth source configuration",
        31 * align,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let make_depth = |size, layers| {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("R025 full qualified comparison-fixture depth pool"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    };
    let depth_2d = make_depth(1024, 7);
    let depth_cube = make_depth(512, 24);
    let sampled_2d = depth_2d.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampled_cube = depth_cube.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::CubeArray),
        ..Default::default()
    });
    let sampler = direct_comparison_sampler(&gpu.device);
    let compute_direct = direct_bind_layout_stage(&gpu.device, wgpu::ShaderStages::COMPUTE);
    let direct_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("R025 exact production comparison resources at compute observation stage"),
        layout: &compute_direct,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: light.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: transforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&sampled_2d),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&sampled_cube),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    let io_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("test-only numeric observation buffers, absent from production entry"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(80),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
    let io = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &io_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: view.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let empty = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[],
        });
    let empty_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &empty,
        entries: &[],
    });
    let compute_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("actual R025 functions; four groups"),
            bind_group_layouts: &[Some(&io_layout), Some(&empty), Some(&empty), Some(&compute_direct)],
            immediate_size: 0,
        });
    let compute = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("R025 actual nine-Boolean production-routine observation"),
            layout: Some(&compute_layout),
            module: &program.module,
            entry_point: Some("comparison_probe"),
            compilation_options: Default::default(),
            cache: None,
        });
    let config_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(16),
                },
                count: None,
            }],
        });
    let config_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &config_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 3,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &config,
                offset: 0,
                size: NonZeroU64::new(16),
            }),
        }],
    });
    let fixture_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&config_layout)],
            immediate_size: 0,
        });
    let fixture = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bounded numeric depth source, not production shadow geometry"),
            layout: Some(&fixture_layout),
            vertex: wgpu::VertexState {
                module: &program.module,
                entry_point: Some("fixed_depth_vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &program.module,
                entry_point: Some("fixed_depth_fragment"),
                targets: &[],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
    let error = pollster::block_on(validation_scope.pop())
        .or(pollster::block_on(allocation_scope.pop()));
    if let Some(e) = error {
        return Err(PbrError3D::Shader(e.to_string()));
    }
    fixture_charge.commit();
    compute_charge.commit();
    let mut light_data = LightBlock608::zeroed();
    light_data.counts = [1, 4, 4, 0];
    if pattern >= 4 {
        let light = super::direct_shadow::Light64 {
            a: [0.0, 0.0, 2.0, 0.1],
            b: [1.0, 1.0, 1.0, 10.0],
            c: [0.0, 0.0, -1.0, 0.2f32.cos()],
            d: [0.7f32.cos(), 0.0, 0.0, 0.0],
        };
        light_data.counts = if pattern == 4 {
            [0, 1, 0, 0]
        } else {
            [0, 0, 1, 0]
        };
        light_data.points[0] = light;
        light_data.spots[0] = light;
    }
    let mut shadow_data = ShadowBlock1984::zeroed();
    for m in &mut shadow_data.matrices {
        *m = glam::Mat4::IDENTITY.to_cols_array_2d();
    }
    if pattern >= 4 {
        for m in &mut shadow_data.matrices {
            *m = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::splat(0.25),
                glam::Quat::IDENTITY,
                glam::Vec3::new(0.0, 0.0, 0.25),
            )
            .to_cols_array_2d();
        }
    }
    let mut config_bytes = vec![0u8; (31 * align) as usize];
    for i in 0..31 {
        let face = if i < 7 { i } else { (i - 7) % 6 };
        let c = [
            if i < 7 { 1024.0 } else { 512.0 },
            pattern as f32,
            0.1 + 0.1 * face as f32,
            0.0,
        ];
        let at = (i * align) as usize;
        config_bytes[at..at + 16].copy_from_slice(bytemuck::bytes_of(&c));
    }
    gpu.queue
        .write_buffer(&light, 0, bytemuck::bytes_of(&light_data));
    gpu.queue
        .write_buffer(&transforms, 0, bytemuck::bytes_of(&shadow_data));
    gpu.queue.write_buffer(
        &view,
        0,
        bytemuck::bytes_of(&ViewBlockBytes::new(
            glam::Mat4::IDENTITY.to_cols_array_2d(),
            [0.0, 0.0, 4.0],
        )),
    );
    gpu.queue
        .write_buffer(&input, 0, bytemuck::cast_slice(inputs));
    gpu.queue.write_buffer(&config, 0, &config_bytes);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    for i in 0..31u32 {
        let (texture, layer) = if i < 7 {
            (&depth_2d, i)
        } else {
            (&depth_cube, i - 7)
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test numeric depth source write"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&fixture);
        pass.set_bind_group(0, &config_binding, &[i * align as u32]);
        pass.draw(0..3, 0..1);
    }
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&compute);
        pass.set_bind_group(0, &io, &[]);
        pass.set_bind_group(1, &empty_binding, &[]);
        pass.set_bind_group(2, &empty_binding, &[]);
        pass.set_bind_group(3, &direct_binding, &[]);
        pass.dispatch_workgroups(inputs.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output_bytes);
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    rx.recv()
        .map_err(|e| PbrError3D::Shader(e.to_string()))?
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    let bytes = readback.slice(..).get_mapped_range()
        .map_err(|error| PbrError3D::Shader(error.to_string()))?;
    let result = bytes
        .chunks_exact(320)
        .map(bytemuck::pod_read_unaligned)
        .collect();
    drop(bytes);
    readback.unmap();
    Ok(result)
}

#[cfg(test)]
pub(crate) fn material_brdf_probe_for_test(
    context: &Context3D,
    inputs: &[[[f32; 4]; 5]],
) -> Result<Vec<[f32; 4]>, super::pbr::PbrError3D> {
    use super::pbr::PbrError3D;
    let state = context.pbr_state()?;
    let program = {
        let mut guard = state
            .program
            .lock()
            .map_err(|_| PbrError3D::Lock("PBR fixed program"))?;
        if guard.is_none() {
            *guard = Some(Arc::new(PbrProgram3D::new(context)?));
        }
        guard.as_ref().unwrap().clone()
    };
    let gpu = context.gpu_resources()?;
    let input_bytes = bytemuck::cast_slice(inputs);
    let output_bytes = (inputs.len() as u64)
        .checked_mul(16)
        .ok_or(PbrError3D::Association("probe extent overflow"))?;
    if inputs.is_empty()
        || input_bytes.len() as u64 > u64::from(gpu.device.limits().max_storage_buffer_binding_size)
        || output_bytes > u64::from(gpu.device.limits().max_storage_buffer_binding_size)
    {
        return Err(PbrError3D::Association(
            "probe exceeds actual Device domain",
        ));
    }
    for p in inputs {
        if !p.iter().flatten().all(|x| x.is_finite())
            || !p[0][..3].iter().all(|x| (0.0..=1.0).contains(x))
            || !(0.0..=1.0).contains(&p[1][0])
            || !(0.0..=1.0).contains(&p[1][1])
        {
            return Err(PbrError3D::Association("unqualified test-only BRDF source"));
        }
    }
    let mut charge = context
        .raster_gpu()?
        .reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
    let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let empty = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("no production light binding"),
            entries: &[],
        });
    let entries = [(2, true), (3, false)].map(|(binding, read_only)| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });
    let layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fixed test direction numeric probe"),
            entries: &entries,
        });
    let pl = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("test observation only"),
            bind_group_layouts: &[Some(&empty), Some(&empty), Some(&empty), Some(&layout)],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("actual R024 reflected_brdf"),
            layout: Some(&pl),
            module: &program.module,
            entry_point: Some("pbr_brdf_probe"),
            compilation_options: Default::default(),
            cache: None,
        });
    let make = |label, size, usage| {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let input = make(
        "qualified BRDF fixture",
        input_bytes.len() as u64,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    );
    let output = make(
        "actual BRDF numeric result",
        output_bytes,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    );
    let readback = make(
        "BRDF readback",
        output_bytes,
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    );
    let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 2,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let empty_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &empty,
        entries: &[],
    });
    if let Some(e) = pollster::block_on(validation_scope.pop()) {
        return Err(PbrError3D::Shader(e.to_string()));
    }
    charge.commit();
    gpu.queue.write_buffer(&input, 0, input_bytes);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        for i in 0..3 {
            pass.set_bind_group(i, &empty_binding, &[]);
        }
        pass.set_bind_group(3, &binding, &[]);
        pass.dispatch_workgroups(inputs.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output_bytes);
    gpu.queue.submit([encoder.finish()]);
    let (s, r) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |v| s.send(v).unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    r.recv()
        .map_err(|e| PbrError3D::Shader(e.to_string()))?
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    let bytes = readback.slice(..).get_mapped_range()
        .map_err(|error| PbrError3D::Shader(error.to_string()))?;
    let values = bytemuck::cast_slice::<u8, [f32; 4]>(&bytes).to_vec();
    drop(bytes);
    readback.unmap();
    Ok(values)
}

/// Isolated numeric observation of the real controlled indirect_color routine.
/// Raw textures here are explicitly ARITHMETIC operands, not admitted producer
/// assets or a scene/source certificate. Full geometry/Image tests use the
/// actual qualified upload/target path instead.
#[cfg(test)]
pub(crate) fn indirect_probe_for_test(
    context: &Context3D,
    profile: super::indirect::Presence3D,
    block: super::indirect::IndirectBlock112,
    operands: Option<&[Arc<[u16]>; 3]>,
    inputs: &[[[f32; 4]; 4]],
) -> Result<Vec<[f32; 4]>, super::pbr::PbrError3D> {
    use super::{
        indirect::{EnvironmentRole3D, decode_half},
        pbr::PbrError3D,
    };
    if profile.direct()
        || !profile.indirect()
        || profile.0 >= 8
        || profile.environment() != operands.is_some()
        || block.header
            != [
                u32::from(profile.environment()),
                u32::from(profile.gi()),
                0,
                0,
            ]
        || inputs.is_empty()
        || inputs.len() > 4096
    {
        return Err(PbrError3D::Association(
            "invalid bounded indirect observation",
        ));
    }
    for p in inputs {
        if p.iter().flatten().any(|v| !v.is_finite())
            || p[0].iter().any(|v| !(0.0..=1.0).contains(v))
            || !(0.0..=1.0).contains(&p[1][0])
            || !(0.0..=1.0).contains(&p[1][1])
        {
            return Err(PbrError3D::Association(
                "invalid indirect observation source",
            ));
        }
        if p[2][..3].iter().all(|v| *v == 0.0) {
            return Err(PbrError3D::Association("zero observation normal"));
        }
    }
    let roles = [
        EnvironmentRole3D::Diffuse,
        EnvironmentRole3D::Specular,
        EnvironmentRole3D::BrdfLut,
    ];
    if let Some(assets) = operands {
        for (role, bits) in roles.into_iter().zip(assets) {
            let (w, h, l, c) = role.dimensions();
            if bits.len() as u64 != u64::from(w) * u64::from(h) * u64::from(l) * u64::from(c) {
                return Err(PbrError3D::Association("arithmetic operand extent"));
            }
            for p in bits.chunks_exact(c as usize) {
                let mut v = [0.0f32; 4];
                for (v, b) in v.iter_mut().zip(p) {
                    *v = decode_half(*b);
                }
                if v.iter().any(|x| !x.is_finite() || *x < 0.0)
                    || (role == EnvironmentRole3D::BrdfLut
                        && f64::from(v[0]) + f64::from(v[1]) > 1.0)
                {
                    return Err(PbrError3D::Association("arithmetic operand domain"));
                }
            }
        }
    }
    let program = context.indirect_program(profile)?;
    let gpu = context.gpu_resources()?;
    let in_bytes = bytemuck::cast_slice(inputs);
    let out_bytes = inputs.len() as u64 * 16;
    let texture_bytes = if operands.is_some() {
        super::indirect::ENVIRONMENT_BYTES
    } else {
        0
    };
    let _textures_charge = if texture_bytes > 0 {
        Some(context.reserve_shadow_texture_bytes(texture_bytes)?)
    } else {
        None
    };
    let _storage = context.reserve_dynamic_storage(
        in_bytes.len() as u64 + out_bytes * 2 + 112,
        1,
        3,
        in_bytes.len() as u64 + out_bytes,
    )?;
    let mut upload = context.begin_preparation_upload_budget();
    upload.consume(texture_bytes + in_bytes.len() as u64 + 112)?;
    let mut pipeline_charge = context
        .raster_gpu()?
        .reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
    let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let empty = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[],
        });
    let io = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[(1, true), (2, false)].map(|(binding, read_only)| {
                wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            }),
        });
    let layout = indirect_bind_layout_stage(&gpu.device, profile, wgpu::ShaderStages::COMPUTE);
    let pl = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("numeric observation, production evaluator"),
            bind_group_layouts: &[Some(&io), Some(&empty), Some(&empty), Some(&layout)],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("R026 exact indirect_color observation"),
            layout: Some(&pl),
            module: &program.module,
            entry_point: Some("indirect_probe"),
            compilation_options: Default::default(),
            cache: None,
        });
    let make = |size, usage| {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let input = make(
        in_bytes.len() as u64,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    );
    let output = make(
        out_bytes,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    );
    let readback = make(
        out_bytes,
        wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
    );
    let uniform = make(
        112,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let mut textures = Vec::new();
    let mut views = Vec::new();
    if operands.is_some() {
        for role in roles {
            let (w, h, l, _) = role.dimensions();
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("ARITHMETIC operand, not producer admission"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: l,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: role.format(),
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            views.push(texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(role.view()),
                ..Default::default()
            }));
            textures.push(texture);
        }
    }
    let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 6,
        resource: uniform.as_entire_binding(),
    }];
    if operands.is_some() {
        for (binding, view) in (7..10).zip(&views) {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: 10,
            resource: wgpu::BindingResource::Sampler(&sampler),
        });
    }
    let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &entries,
    });
    let io_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &io,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let empty_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &empty,
        entries: &[],
    });
    if let Some(e) = pollster::block_on(validation_scope.pop()) {
        return Err(PbrError3D::Shader(e.to_string()));
    }
    pipeline_charge.commit();
    gpu.queue.write_buffer(&input, 0, in_bytes);
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::bytes_of(&block));
    if let Some(assets) = operands {
        for ((texture, role), bits) in textures.iter().zip(roles).zip(assets) {
            let (w, h, l, c) = role.dimensions();
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(bits),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * c * 2),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: l,
                },
            );
        }
    }
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &io_binding, &[]);
        pass.set_bind_group(1, &empty_binding, &[]);
        pass.set_bind_group(2, &empty_binding, &[]);
        pass.set_bind_group(3, &binding, &[0]);
        pass.dispatch_workgroups(inputs.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, out_bytes);
    gpu.queue.submit([encoder.finish()]);
    let (s, r) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |v| {
        let _ = s.send(v);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    r.recv()
        .map_err(|e| PbrError3D::Shader(e.to_string()))?
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    let bytes = readback.slice(..).get_mapped_range()
        .map_err(|error| PbrError3D::Shader(error.to_string()))?;
    let values = bytemuck::cast_slice::<u8, [f32; 4]>(&bytes).to_vec();
    drop(bytes);
    readback.unmap();
    Ok(values)
}

#[derive(Clone)]
#[allow(dead_code)]
struct UniformPackingProof {
    view: ViewBlockBytes,
    objects: Vec<ObjectBlockBytes>,
    appearances: Vec<AppearanceBlockBytes>,
    object_offsets: Vec<u32>,
    appearance_offsets: Vec<u32>,
}

struct PackedUniforms {
    view_bytes: Vec<u8>,
    object_bytes: Vec<u8>,
    appearance_bytes: Vec<u8>,
    object_offsets: Vec<u32>,
    appearance_offsets: Vec<u32>,
    object_scratch: Vec<ObjectBlockBytes>,
    object_changed: Vec<bool>,
    view_changed: bool,
    proof: UniformPackingProof,
}

impl PackedUniforms {
    fn new(
        context: &Context3D,
        snapshot: &RenderSnapshot,
        model: &ModelState,
        view: ViewDescriptor3,
        render_origin: WorldPoint3,
        width: u32,
        height: u32,
    ) -> Result<Self, HeadlessRaster3DError> {
        let abi = context.abi_layout();
        validate_model(snapshot, model)?;
        let packet_count = snapshot.draw_packets().len();
        let object_allocation_count = packet_count.max(1);
        let appearance_allocation_count = packet_count.max(1);
        let mut view_bytes = zeroed_uniform_bytes(
            abi.view_dynamic_stride(),
            "view",
            context.limits().hard.max_buffer_size,
        )?;
        let object_size = checked_uniform_arena_size(
            abi.object_dynamic_stride(),
            object_allocation_count,
            context.limits().hard.max_buffer_size,
        )?;
        let appearance_size = checked_uniform_arena_size(
            abi.appearance_dynamic_stride(),
            appearance_allocation_count,
            context.limits().hard.max_buffer_size,
        )?;
        let dynamic_storage_bytes = abi.view_dynamic_stride().checked_add(object_size).ok_or(
            Context3DError::ArithmeticOverflow {
                operation: "sizing retained 3D dynamic uniform storage",
            },
        )?;
        validate_budget(
            PolicyBudget3D::DynamicStorageBytesPerTarget,
            dynamic_storage_bytes,
            context.limits().policy.max_dynamic_storage_bytes_per_target,
        )?;
        validate_budget(
            PolicyBudget3D::DynamicUploadBytesPerFrame,
            dynamic_storage_bytes,
            context.limits().policy.max_dynamic_upload_bytes_per_frame,
        )?;
        let mut object_bytes =
            zeroed_uniform_bytes(object_size, "object", context.limits().hard.max_buffer_size)?;
        let mut appearance_bytes = zeroed_uniform_bytes(
            appearance_size,
            "appearance",
            context.limits().hard.max_buffer_size,
        )?;
        let mut object_offsets = Vec::new();
        let mut appearance_offsets = Vec::new();
        let mut object_blocks = Vec::new();
        let mut object_scratch = Vec::new();
        let mut object_changed = Vec::new();
        let mut appearance_blocks = Vec::new();
        object_offsets
            .try_reserve_exact(packet_count)
            .map_err(|_| HeadlessRaster3DError::UniformStorageUnavailable {
                block: "object offsets",
                bytes: object_size,
            })?;
        appearance_offsets
            .try_reserve_exact(packet_count)
            .map_err(|_| HeadlessRaster3DError::UniformStorageUnavailable {
                block: "appearance offsets",
                bytes: appearance_size,
            })?;
        object_blocks.try_reserve_exact(packet_count).map_err(|_| {
            HeadlessRaster3DError::UniformStorageUnavailable {
                block: "object proof",
                bytes: object_size,
            }
        })?;
        object_scratch
            .try_reserve_exact(packet_count)
            .map_err(|_| HeadlessRaster3DError::UniformStorageUnavailable {
                block: "object scratch",
                bytes: object_size,
            })?;
        object_changed
            .try_reserve_exact(packet_count)
            .map_err(|_| HeadlessRaster3DError::UniformStorageUnavailable {
                block: "object change flags",
                bytes: object_size,
            })?;
        appearance_blocks
            .try_reserve_exact(packet_count)
            .map_err(|_| HeadlessRaster3DError::UniformStorageUnavailable {
                block: "appearance proof",
                bytes: appearance_size,
            })?;

        let matrices = view.prepared_matrices(render_origin, width, height)?;
        let camera = view
            .pose()
            .translation()
            .render_relative_to(render_origin)?;
        let view_block = ViewBlockBytes::new(
            matrices.view_projection().columns(),
            [camera.x(), camera.y(), camera.z()],
        );
        view_bytes[..VIEW_BLOCK_SIZE as usize].copy_from_slice(bytemuck::bytes_of(&view_block));

        for (index, placement) in model.placements().iter().copied().enumerate() {
            let transform = placement.checked_transform(render_origin)?;
            let normal = transform.normal_to_world().columns();
            let object_block = ObjectBlockBytes::new(
                transform.model_to_render().columns(),
                [
                    [normal[0][0], normal[0][1], normal[0][2], 0.0],
                    [normal[1][0], normal[1][1], normal[1][2], 0.0],
                    [normal[2][0], normal[2][1], normal[2][2], 0.0],
                ],
            );
            let appearance_block = AppearanceBlockBytes::new(DEFAULT_COLOR_FACTOR);
            let object_offset = checked_dynamic_offset(abi.object_dynamic_stride(), index)?;
            let appearance_offset = checked_dynamic_offset(abi.appearance_dynamic_stride(), index)?;
            let object_start = object_offset as usize;
            let appearance_start = appearance_offset as usize;
            object_bytes[object_start..object_start + OBJECT_BLOCK_SIZE as usize]
                .copy_from_slice(bytemuck::bytes_of(&object_block));
            appearance_bytes[appearance_start..appearance_start + APPEARANCE_BLOCK_SIZE as usize]
                .copy_from_slice(bytemuck::bytes_of(&appearance_block));
            object_offsets.push(object_offset);
            appearance_offsets.push(appearance_offset);
            object_blocks.push(object_block);
            object_scratch.push(object_block);
            object_changed.push(false);
            appearance_blocks.push(appearance_block);
        }

        let proof = UniformPackingProof {
            view: view_block,
            objects: object_blocks,
            appearances: appearance_blocks,
            object_offsets: object_offsets.clone(),
            appearance_offsets: appearance_offsets.clone(),
        };
        Ok(Self {
            view_bytes,
            object_bytes,
            appearance_bytes,
            object_offsets,
            appearance_offsets,
            object_scratch,
            object_changed,
            view_changed: false,
            proof,
        })
    }

    fn rewrite_dynamic(
        &mut self,
        context: &Context3D,
        snapshot: &RenderSnapshot,
        model: &ModelState,
        view: ViewDescriptor3,
        render_origin: WorldPoint3,
        width: u32,
        height: u32,
    ) -> Result<u64, HeadlessRaster3DError> {
        validate_model(snapshot, model)?;
        let matrices = view.prepared_matrices(render_origin, width, height)?;
        let camera = view
            .pose()
            .translation()
            .render_relative_to(render_origin)?;
        let view_block = ViewBlockBytes::new(
            matrices.view_projection().columns(),
            [camera.x(), camera.y(), camera.z()],
        );
        for (index, placement) in model.placements().iter().copied().enumerate() {
            let transform = placement.checked_transform(render_origin)?;
            let normal = transform.normal_to_world().columns();
            let object_block = ObjectBlockBytes::new(
                transform.model_to_render().columns(),
                [
                    [normal[0][0], normal[0][1], normal[0][2], 0.0],
                    [normal[1][0], normal[1][1], normal[1][2], 0.0],
                    [normal[2][0], normal[2][1], normal[2][2], 0.0],
                ],
            );
            self.object_scratch[index] = object_block;
            self.object_changed[index] =
                bytemuck::bytes_of(&object_block) != bytemuck::bytes_of(&self.proof.objects[index]);
        }

        let view_changed = bytemuck::bytes_of(&view_block) != bytemuck::bytes_of(&self.proof.view);
        let mut upload_bytes = if view_changed { VIEW_BLOCK_SIZE } else { 0 };
        for changed in &self.object_changed {
            if *changed {
                upload_bytes = upload_bytes.checked_add(OBJECT_BLOCK_SIZE).ok_or(
                    Context3DError::ArithmeticOverflow {
                        operation: "counting changed 3D dynamic uniform upload bytes",
                    },
                )?;
            }
        }
        validate_budget(
            PolicyBudget3D::DynamicUploadBytesPerFrame,
            upload_bytes,
            context.limits().policy.max_dynamic_upload_bytes_per_frame,
        )?;

        self.view_changed = view_changed;
        if view_changed {
            self.view_bytes[..VIEW_BLOCK_SIZE as usize]
                .copy_from_slice(bytemuck::bytes_of(&view_block));
            self.proof.view = view_block;
        }
        for (index, object_block) in self.object_scratch.iter().copied().enumerate() {
            if !self.object_changed[index] {
                continue;
            }
            let object_start = self.object_offsets[index] as usize;
            self.object_bytes[object_start..object_start + OBJECT_BLOCK_SIZE as usize]
                .copy_from_slice(bytemuck::bytes_of(&object_block));
            self.proof.objects[index] = object_block;
        }
        Ok(upload_bytes)
    }

    fn dynamic_storage_bytes(&self) -> Result<u64, Context3DError> {
        u64::try_from(self.view_bytes.len())
            .ok()
            .and_then(|view| {
                u64::try_from(self.object_bytes.len())
                    .ok()
                    .and_then(|object| view.checked_add(object))
            })
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting retained 3D dynamic uniform storage bytes",
            })
    }

    fn upload_bytes(&self) -> Result<u64, Context3DError> {
        self.dynamic_storage_bytes()?
            .checked_add(u64::try_from(self.appearance_bytes.len()).map_err(|_| {
                Context3DError::ArithmeticOverflow {
                    operation: "counting 3D appearance uniform upload bytes",
                }
            })?)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting 3D uniform upload bytes",
            })
    }

    fn retained_cpu_staging_bytes(&self) -> Result<u64, Context3DError> {
        let mut bytes = 0_u64;
        for (count, element_size) in [
            (self.view_bytes.len(), std::mem::size_of::<u8>()),
            (self.object_bytes.len(), std::mem::size_of::<u8>()),
            (self.appearance_bytes.len(), std::mem::size_of::<u8>()),
            (self.object_offsets.len(), std::mem::size_of::<u32>()),
            (self.appearance_offsets.len(), std::mem::size_of::<u32>()),
            (
                self.object_scratch.len(),
                std::mem::size_of::<ObjectBlockBytes>(),
            ),
            (
                self.proof.objects.len(),
                std::mem::size_of::<ObjectBlockBytes>(),
            ),
            (
                self.proof.appearances.len(),
                std::mem::size_of::<AppearanceBlockBytes>(),
            ),
            (self.proof.object_offsets.len(), std::mem::size_of::<u32>()),
            (
                self.proof.appearance_offsets.len(),
                std::mem::size_of::<u32>(),
            ),
        ] {
            bytes = checked_accumulate_payload_bytes(bytes, count, element_size)?;
        }
        let changed_flag_bytes =
            self.object_changed
                .len()
                .checked_add(7)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "sizing retained 3D object change flags",
                })?
                / 8;
        bytes = checked_accumulate_payload_bytes(bytes, changed_flag_bytes, 1)?;
        bytes = bytes
            .checked_add(
                u64::try_from(std::mem::size_of::<ViewBlockBytes>()).map_err(|_| {
                    Context3DError::ArithmeticOverflow {
                        operation: "representing retained 3D view proof bytes",
                    }
                })?,
            )
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "counting retained 3D CPU staging bytes",
            })?;
        Ok(bytes)
    }
}

fn retained_prepared_cpu_bytes(
    packed: &PackedUniforms,
    packet_count: usize,
) -> Result<u64, Context3DError> {
    let mut bytes = packed.retained_cpu_staging_bytes()?;
    for element_size in [
        std::mem::size_of::<wgpu::BindGroup>(),
        std::mem::size_of::<Option<AppearanceTextureGpuLease>>(),
        std::mem::size_of::<Arc<wgpu::RenderPipeline>>(),
        std::mem::size_of::<GeometryGpuLease>(),
        std::mem::size_of::<ValidatedDraw>(),
    ] {
        bytes = checked_accumulate_payload_bytes(bytes, packet_count, element_size)?;
    }
    Ok(bytes)
}

fn checked_accumulate_payload_bytes(
    current: u64,
    count: usize,
    element_size: usize,
) -> Result<u64, Context3DError> {
    let count = u64::try_from(count).map_err(|_| Context3DError::ArithmeticOverflow {
        operation: "representing retained 3D CPU staging element count",
    })?;
    let element_size =
        u64::try_from(element_size).map_err(|_| Context3DError::ArithmeticOverflow {
            operation: "representing retained 3D CPU staging element size",
        })?;
    let added = count
        .checked_mul(element_size)
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "sizing retained 3D CPU staging payload",
        })?;
    current
        .checked_add(added)
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "counting retained 3D CPU staging bytes",
        })
}

fn validate_model(
    snapshot: &RenderSnapshot,
    model: &ModelState,
) -> Result<(), HeadlessRaster3DError> {
    if !model.matches_snapshot(snapshot) {
        return Err(ModelStateError::StructuralIdentityMismatch.into());
    }
    if snapshot.draw_packets().len() != model.placements().len() {
        return Err(HeadlessRaster3DError::SnapshotInvariant);
    }
    Ok(())
}

fn checked_uniform_arena_size(
    stride: u64,
    count: usize,
    maximum: u64,
) -> Result<u64, HeadlessRaster3DError> {
    let count = u64::try_from(count).map_err(|_| Context3DError::ArithmeticOverflow {
        operation: "representing 3D uniform allocation count",
    })?;
    let size = stride
        .checked_mul(count)
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "sizing 3D uniform arena",
        })?;
    if size > maximum {
        return Err(Context3DError::HardDeviceLimitExceeded {
            limit: super::HardLimit3D::BufferSize,
            requested: size,
            maximum,
        }
        .into());
    }
    Ok(size)
}

fn zeroed_uniform_bytes(
    size: u64,
    block: &'static str,
    maximum: u64,
) -> Result<Vec<u8>, HeadlessRaster3DError> {
    if size > maximum {
        return Err(Context3DError::HardDeviceLimitExceeded {
            limit: super::HardLimit3D::BufferSize,
            requested: size,
            maximum,
        }
        .into());
    }
    let size_usize = usize::try_from(size)
        .map_err(|_| HeadlessRaster3DError::UniformStorageUnavailable { block, bytes: size })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size_usize)
        .map_err(|_| HeadlessRaster3DError::UniformStorageUnavailable { block, bytes: size })?;
    bytes.resize(size_usize, 0);
    Ok(bytes)
}

fn checked_dynamic_offset(stride: u64, index: usize) -> Result<u32, HeadlessRaster3DError> {
    let index = u64::try_from(index).map_err(|_| Context3DError::ArithmeticOverflow {
        operation: "representing 3D uniform offset index",
    })?;
    let offset = stride
        .checked_mul(index)
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "computing 3D dynamic uniform offset",
        })?;
    u32::try_from(offset).map_err(|_| HeadlessRaster3DError::DynamicOffsetOutOfRange { offset })
}

#[derive(Clone, Copy)]
struct ValidatedTarget {
    extent: wgpu::Extent3d,
    format: wgpu::TextureFormat,
    sample_count: u32,
    active_target_pixels: u64,
    attachment_bytes: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Viewport3DRasterTarget {
    validated: ValidatedTarget,
}

impl Viewport3DRasterTarget {
    pub(super) const fn extent(self) -> wgpu::Extent3d {
        self.validated.extent
    }

    pub(super) const fn format(self) -> wgpu::TextureFormat {
        self.validated.format
    }

    pub(super) const fn sample_count(self) -> u32 {
        self.validated.sample_count
    }

    pub(super) const fn active_target_pixels(self) -> u64 {
        self.validated.active_target_pixels
    }

    pub(super) const fn attachment_bytes(self) -> u64 {
        self.validated.attachment_bytes
    }
}

pub(super) fn validate_viewport_raster_target(
    context: &Context3D,
    width: u32,
    height: u32,
    sample_count: u32,
) -> Result<Viewport3DRasterTarget, HeadlessRaster3DError> {
    validate_target(context, width, height, sample_count, 2)
        .map(|validated| Viewport3DRasterTarget { validated })
}

fn validate_target(
    context: &Context3D,
    width: u32,
    height: u32,
    sample_count: u32,
    color_attachment_copies: u32,
) -> Result<ValidatedTarget, HeadlessRaster3DError> {
    if width == 0 || height == 0 {
        return Err(HeadlessRaster3DError::InactiveExtent { width, height });
    }
    if !matches!(sample_count, 1 | 4) {
        return Err(HeadlessRaster3DError::UnsupportedSampleCount {
            requested: sample_count,
        });
    }
    let requested_dimension = width.max(height);
    if requested_dimension > context.limits().hard.max_texture_dimension_2d {
        return Err(Context3DError::HardDeviceLimitExceeded {
            limit: super::HardLimit3D::TextureDimension2D,
            requested: u64::from(requested_dimension),
            maximum: u64::from(context.limits().hard.max_texture_dimension_2d),
        }
        .into());
    }
    validate_budget(
        PolicyBudget3D::TargetDimension2D,
        u64::from(requested_dimension),
        u64::from(context.limits().policy.max_target_dimension_2d),
    )?;

    let format = context.surface_format();
    let required_usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
    let color_features = format.guaranteed_format_features(wgpu::Features::empty());
    if !color_features.allowed_usages.contains(required_usage)
        || !color_features.flags.sample_count_supported(sample_count)
        || (sample_count == 4
            && !color_features
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE))
    {
        return Err(HeadlessRaster3DError::UnsupportedTargetFormat {
            format,
            sample_count,
        });
    }
    let depth_features = DEPTH_FORMAT.guaranteed_format_features(wgpu::Features::empty());
    if !depth_features
        .allowed_usages
        .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        || !depth_features.flags.sample_count_supported(sample_count)
    {
        return Err(HeadlessRaster3DError::UnsupportedDepthFormat { sample_count });
    }
    if format.block_dimensions() != (1, 1) {
        return Err(HeadlessRaster3DError::UnsupportedTargetFormat {
            format,
            sample_count,
        });
    }
    let color_bytes = u64::from(format.block_copy_size(None).ok_or(
        HeadlessRaster3DError::UnsupportedTargetFormat {
            format,
            sample_count,
        },
    )?);
    let pixels = u64::from(width).checked_mul(u64::from(height)).ok_or(
        Context3DError::ArithmeticOverflow {
            operation: "counting 3D target pixels",
        },
    )?;
    validate_budget(
        PolicyBudget3D::ActiveTargetPixels,
        pixels,
        context.limits().policy.max_active_target_pixels,
    )?;
    let single_color =
        pixels
            .checked_mul(color_bytes)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "sizing 3D color attachment",
            })?;
    let depth = pixels
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_mul(u64::from(sample_count)))
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "sizing 3D depth attachment",
        })?;
    let multisample_color = if sample_count == 4 {
        single_color
            .checked_mul(4)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "sizing 3D multisample color attachment",
            })?
    } else {
        0
    };
    for attachment in [single_color, depth, multisample_color] {
        validate_budget(
            PolicyBudget3D::SingleTargetAttachmentBytes,
            attachment,
            context.limits().policy.max_single_target_attachment_bytes,
        )?;
    }
    let retained_color = single_color
        .checked_mul(u64::from(color_attachment_copies))
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "sizing retained 3D color attachments",
        })?;
    let total = retained_color
        .checked_add(depth)
        .and_then(|bytes| bytes.checked_add(multisample_color))
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "sizing total 3D target attachments",
        })?;
    validate_budget(
        PolicyBudget3D::TotalAttachmentBytes,
        total,
        context.limits().policy.max_total_attachment_bytes,
    )?;

    Ok(ValidatedTarget {
        extent: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        format,
        sample_count,
        active_target_pixels: pixels,
        attachment_bytes: total,
    })
}

struct UniformBindings {
    view: wgpu::BindGroup,
    object: wgpu::BindGroup,
    appearances: Vec<wgpu::BindGroup>,
    view_buffer: wgpu::Buffer,
    object_buffer: wgpu::Buffer,
    _appearance_buffer: wgpu::Buffer,
}

impl UniformBindings {
    fn new(
        gpu: &GpuState3D,
        packed: &PackedUniforms,
        appearance_resources: &[Option<AppearanceTextureGpuLease>],
    ) -> Result<Self, Context3DError> {
        let view_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Halcyon 3D View Uniform Arena"),
                contents: &packed.view_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let object_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Halcyon 3D Object Uniform Arena"),
                contents: &packed.object_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let appearance_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Halcyon 3D Appearance Uniform Arena"),
                contents: &packed.appearance_bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let view = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Halcyon 3D View Bind Group"),
            layout: &gpu.layouts.view,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &view_buffer,
                    offset: 0,
                    size: NonZeroU64::new(VIEW_BLOCK_SIZE),
                }),
            }],
        });
        let object = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Halcyon 3D Object Bind Group"),
            layout: &gpu.layouts.object,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &object_buffer,
                    offset: 0,
                    size: NonZeroU64::new(OBJECT_BLOCK_SIZE),
                }),
            }],
        });
        let mut appearances = Vec::new();
        appearances
            .try_reserve_exact(appearance_resources.len())
            .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
        for resource in appearance_resources {
            let (texture_view, sampler) = match resource {
                Some(resource) => (resource.texture_view(), resource.sampler()),
                None => (&gpu.fallback_view, &gpu.fallback_sampler),
            };
            appearances.push(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Halcyon 3D Appearance Bind Group"),
                layout: &gpu.layouts.appearance,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &appearance_buffer,
                            offset: 0,
                            size: NonZeroU64::new(APPEARANCE_BLOCK_SIZE),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            }));
        }
        #[cfg(test)]
        {
            super::audit::record_gpu_buffers(3);
            super::audit::record_dynamic_resources(2, 2);
        }
        Ok(Self {
            view,
            object,
            appearances,
            view_buffer,
            object_buffer,
            _appearance_buffer: appearance_buffer,
        })
    }

    fn write_dynamic(&self, gpu: &GpuState3D, packed: &PackedUniforms) {
        // wgpu copies each write into staging and prepends its transfer to the next submission.
        // One Queue executes submissions in order, so prior draws finish before this transfer and
        // the following draw observes it. One retained dynamic slot per target is therefore safe.
        if packed.view_changed {
            gpu.queue.write_buffer(
                &self.view_buffer,
                0,
                &packed.view_bytes[..VIEW_BLOCK_SIZE as usize],
            );
        }
        for (index, changed) in packed.object_changed.iter().copied().enumerate() {
            if !changed {
                continue;
            }
            let object_start = packed.object_offsets[index] as usize;
            gpu.queue.write_buffer(
                &self.object_buffer,
                u64::from(packed.object_offsets[index]),
                &packed.object_bytes[object_start..object_start + OBJECT_BLOCK_SIZE as usize],
            );
        }
    }
}

pub(super) struct PreparedRaster3D {
    _raster_gpu: Arc<GpuState3D>,
    dynamic_storage: Option<DynamicStorageLease3D>,
    snapshot: RenderSnapshot,
    target: ValidatedTarget,
    packed: PackedUniforms,
    uniform_bindings: UniformBindings,
    _appearance_resources: Vec<Option<AppearanceTextureGpuLease>>,
    pipelines: Vec<Arc<wgpu::RenderPipeline>>,
    geometries: Vec<GeometryGpuLease>,
    draws: Vec<ValidatedDraw>,
}

impl PreparedRaster3D {
    pub(super) fn matches_structure(&self, snapshot: &RenderSnapshot) -> bool {
        self.snapshot.same_identity(snapshot)
    }

    pub(super) fn retire_after_submitted_work(mut self) {
        let Some(dynamic_storage) = self.dynamic_storage.take() else {
            return;
        };
        // The buffers may outlive their Rust handles while submitted work is in flight.
        // Keep their aggregate budget charged until the queue confirms completion.
        let queue = self._raster_gpu.queue.clone();
        crate::renderer::on_submitted_work_done(&queue, move || drop(dynamic_storage));
    }

    pub(super) fn update_dynamic(
        &mut self,
        context: &Context3D,
        model: &ModelState,
        view: ViewDescriptor3,
        render_origin: WorldPoint3,
    ) -> Result<(), HeadlessRaster3DError> {
        let bytes_written = self.packed.rewrite_dynamic(
            context,
            &self.snapshot,
            model,
            view,
            render_origin,
            self.target.extent.width,
            self.target.extent.height,
        )?;
        self.uniform_bindings
            .write_dynamic(&self._raster_gpu, &self.packed);
        #[cfg(test)]
        if bytes_written != 0 {
            super::audit::record_dynamic_write(bytes_written);
        }
        #[cfg(not(test))]
        let _ = bytes_written;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn dynamic_storage_bytes_for_test(&self) -> u64 {
        self.packed.dynamic_storage_bytes().unwrap_or(u64::MAX)
    }

    #[cfg(test)]
    pub(super) fn dynamic_bytes_for_test(&self) -> (Vec<u8>, Vec<u8>) {
        (
            self.packed.view_bytes.clone(),
            self.packed.object_bytes.clone(),
        )
    }

    pub(super) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        back_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        multisample_view: Option<&wgpu::TextureView>,
    ) {
        debug_assert_eq!(self.target.sample_count == 4, multisample_view.is_some());
        let color_view = multisample_view.unwrap_or(back_view);
        let resolve_target = multisample_view.map(|_| back_view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Halcyon 3D Offscreen Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: color_view,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            multiview_mask: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &self.uniform_bindings.view, &[0]);
        for (index, ((pipeline, geometry), draw)) in self
            .pipelines
            .iter()
            .zip(&self.geometries)
            .zip(&self.draws)
            .enumerate()
        {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(
                1,
                &self.uniform_bindings.object,
                &[self.packed.object_offsets[index]],
            );
            pass.set_bind_group(
                2,
                &self.uniform_bindings.appearances[index],
                &[self.packed.appearance_offsets[index]],
            );
            pass.set_vertex_buffer(0, geometry.vertex_buffer().slice(..));
            match draw {
                ValidatedDraw::Unindexed => pass.draw(0..geometry.vertex_count(), 0..1),
                ValidatedDraw::Indexed16(index_buffer) => {
                    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    pass.draw_indexed(0..geometry.index_count(), 0, 0..1);
                }
                ValidatedDraw::Indexed32(index_buffer) => {
                    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..geometry.index_count(), 0, 0..1);
                }
            }
        }
    }
}

pub(super) fn prepare_viewport_snapshot_raster(
    context: &Context3D,
    snapshot: &RenderSnapshot,
    model: &ModelState,
    view: ViewDescriptor3,
    render_origin: WorldPoint3,
    target: Viewport3DRasterTarget,
) -> Result<PreparedRaster3D, HeadlessRaster3DError> {
    prepare_snapshot_raster(
        context,
        snapshot,
        model,
        view,
        render_origin,
        target.validated,
    )
}

fn prepare_snapshot_raster(
    context: &Context3D,
    snapshot: &RenderSnapshot,
    model: &ModelState,
    view: ViewDescriptor3,
    render_origin: WorldPoint3,
    target: ValidatedTarget,
) -> Result<PreparedRaster3D, HeadlessRaster3DError> {
    if !context.same_resource_family(snapshot.context()) {
        return Err(Context3DError::ContextMismatch.into());
    }
    if snapshot.draw_packets().len() != snapshot.committed_packets().len() {
        return Err(HeadlessRaster3DError::SnapshotInvariant);
    }
    validate_model(snapshot, model)?;
    let packed = PackedUniforms::new(
        context,
        snapshot,
        model,
        view,
        render_origin,
        target.extent.width,
        target.extent.height,
    )?;
    let dynamic_storage_bytes = packed.dynamic_storage_bytes()?;
    let dynamic_object_capacity =
        u64::try_from(snapshot.draw_packets().len().max(1)).map_err(|_| {
            Context3DError::ArithmeticOverflow {
                operation: "representing retained 3D dynamic object capacity",
            }
        })?;
    let retained_cpu_bytes = retained_prepared_cpu_bytes(&packed, snapshot.draw_packets().len())?;
    let dynamic_storage = context.reserve_dynamic_storage(
        dynamic_storage_bytes,
        dynamic_object_capacity,
        1,
        retained_cpu_bytes,
    )?;
    let mut upload_budget = context.begin_preparation_upload_budget();
    upload_budget.consume(packed.upload_bytes()?)?;

    let mut appearances = Vec::new();
    appearances
        .try_reserve_exact(snapshot.committed_packets().len())
        .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
    for packet in snapshot.committed_packets() {
        appearances.push(context.admit_appearance_lease(
            packet.appearance(),
            packet.geometry().descriptor().semantics(),
        )?);
    }

    let mut pipeline_requests = Vec::new();
    pipeline_requests
        .try_reserve_exact(appearances.len())
        .map_err(|_| Context3DError::PipelineCacheUnavailable)?;
    for (descriptor, appearance) in snapshot.draw_packets().iter().zip(&appearances) {
        pipeline_requests.push(PipelineRequest {
            key: PipelineKey::new(
                appearance,
                descriptor.raster_mode(),
                target.format,
                target.sample_count,
            ),
            appearance,
        });
    }

    let raster_gpu = context.raster_gpu()?;
    let mut appearance_resources = Vec::new();
    appearance_resources
        .try_reserve_exact(appearances.len())
        .map_err(|_| Context3DError::AppearanceCacheUnavailable)?;
    for appearance in &appearances {
        appearance_resources
            .push(context.materialize_appearance_binding(appearance, &mut upload_budget)?);
    }
    let pipelines = raster_gpu.prepare_pipelines(
        context,
        &pipeline_requests,
        context.limits().policy.max_pipeline_variants,
    )?;

    let mut geometries = Vec::new();
    geometries
        .try_reserve_exact(snapshot.committed_packets().len())
        .map_err(|_| Context3DError::GeometryCacheUnavailable)?;
    for packet in snapshot.committed_packets() {
        geometries.push(context.materialize_geometry_lease(packet.geometry(), &mut upload_budget)?);
    }

    let mut draws = Vec::new();
    draws
        .try_reserve_exact(geometries.len())
        .map_err(|_| Context3DError::GeometryCacheUnavailable)?;
    for geometry in &geometries {
        let draw = match (geometry.index_format(), geometry.index_buffer()) {
            (CanonicalIndexFormat::Unindexed, None) => ValidatedDraw::Unindexed,
            (CanonicalIndexFormat::Uint16, Some(index_buffer)) => {
                ValidatedDraw::Indexed16(index_buffer.clone())
            }
            (CanonicalIndexFormat::Uint32, Some(index_buffer)) => {
                ValidatedDraw::Indexed32(index_buffer.clone())
            }
            _ => return Err(HeadlessRaster3DError::SnapshotInvariant),
        };
        draws.push(draw);
    }

    let uniform_bindings = UniformBindings::new(&raster_gpu, &packed, &appearance_resources)?;
    #[cfg(test)]
    {
        super::audit::record_structural_preparation();
        super::audit::record_dynamic_write(packed.dynamic_storage_bytes()?);
    }
    Ok(PreparedRaster3D {
        _raster_gpu: raster_gpu,
        dynamic_storage: Some(dynamic_storage),
        snapshot: snapshot.clone(),
        target,
        packed,
        uniform_bindings,
        _appearance_resources: appearance_resources,
        pipelines,
        geometries,
        draws,
    })
}

#[allow(dead_code)]
pub(crate) struct HeadlessRasterOutput {
    texture: wgpu::Texture,
    extent: [u32; 2],
    format: wgpu::TextureFormat,
    sample_count: u32,
    layout_contract: GpuLayoutContract3D,
    uniform_proof: UniformPackingProof,
}

impl HeadlessRasterOutput {
    #[cfg(test)]
    pub(crate) const fn extent(&self) -> [u32; 2] {
        self.extent
    }

    #[cfg(test)]
    pub(crate) const fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    #[cfg(test)]
    pub(crate) const fn sample_count(&self) -> u32 {
        self.sample_count
    }

    #[cfg(test)]
    pub(crate) const fn layout_contract(&self) -> GpuLayoutContract3D {
        self.layout_contract
    }

    #[cfg(test)]
    pub(crate) fn object_offsets(&self) -> &[u32] {
        &self.uniform_proof.object_offsets
    }

    #[cfg(test)]
    pub(crate) fn appearance_offsets(&self) -> &[u32] {
        &self.uniform_proof.appearance_offsets
    }

    #[cfg(test)]
    pub(crate) const fn view_block(&self) -> ViewBlockBytes {
        self.uniform_proof.view
    }

    #[cfg(test)]
    pub(crate) fn object_blocks(&self) -> &[ObjectBlockBytes] {
        &self.uniform_proof.objects
    }

    #[cfg(test)]
    pub(crate) fn appearance_blocks(&self) -> &[AppearanceBlockBytes] {
        &self.uniform_proof.appearances
    }

    #[cfg(test)]
    pub(crate) fn readback_rgba8(
        &self,
        gpu: &GpuResources,
    ) -> Result<Vec<[u8; 4]>, HeadlessRaster3DError> {
        if !matches!(
            self.format,
            wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            return Err(HeadlessRaster3DError::UnsupportedReadbackFormat {
                format: self.format,
            });
        }
        let unpadded = self.extent[0]
            .checked_mul(4)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "sizing 3D readback row",
            })?;
        let padded = unpadded
            .div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "aligning 3D readback row",
            })?;
        let buffer_size = u64::from(padded)
            .checked_mul(u64::from(self.extent[1]))
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "sizing 3D readback buffer",
            })?;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Halcyon 3D Headless Readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Halcyon 3D Headless Readback Encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.extent[1]),
                },
            },
            wgpu::Extent3d {
                width: self.extent[0],
                height: self.extent[1],
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(std::iter::once(encoder.finish()));

        use std::sync::atomic::{AtomicBool, Ordering};
        let done = Arc::new(AtomicBool::new(false));
        let succeeded = Arc::new(AtomicBool::new(false));
        let done_callback = done.clone();
        let succeeded_callback = succeeded.clone();
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            succeeded_callback.store(result.is_ok(), Ordering::Release);
            done_callback.store(true, Ordering::Release);
        });
        while !done.load(Ordering::Acquire) {
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        }
        if !succeeded.load(Ordering::Acquire) {
            return Err(HeadlessRaster3DError::ReadbackFailed);
        }
        let mapped = slice
            .get_mapped_range()
            .map_err(|_| HeadlessRaster3DError::ReadbackFailed)?;
        let pixel_count = usize::try_from(
            u64::from(self.extent[0])
                .checked_mul(u64::from(self.extent[1]))
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "counting 3D readback pixels",
                })?,
        )
        .map_err(|_| Context3DError::ArithmeticOverflow {
            operation: "representing 3D readback pixels",
        })?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(pixel_count)
            .map_err(|_| HeadlessRaster3DError::ReadbackFailed)?;
        for row in 0..self.extent[1] as usize {
            let row_start = row * padded as usize;
            for column in 0..self.extent[0] as usize {
                let offset = row_start + column * 4;
                let stored = [
                    mapped[offset],
                    mapped[offset + 1],
                    mapped[offset + 2],
                    mapped[offset + 3],
                ];
                let rgba = match self.format {
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                        [stored[2], stored[1], stored[0], stored[3]]
                    }
                    _ => stored,
                };
                pixels.push(rgba);
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }
}

#[allow(dead_code)]
pub(crate) fn render_headless_snapshot(
    context: &Context3D,
    snapshot: &RenderSnapshot,
    model: &ModelState,
    view: ViewDescriptor3,
    render_origin: WorldPoint3,
    extent: [u32; 2],
    sample_count: u32,
) -> Result<HeadlessRasterOutput, HeadlessRaster3DError> {
    let target = validate_target(context, extent[0], extent[1], sample_count, 1)?;
    let prepared = prepare_snapshot_raster(context, snapshot, model, view, render_origin, target)?;
    let raster_gpu = &prepared._raster_gpu;
    let back_texture = raster_gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Halcyon 3D Headless Single-Sample Color"),
        size: target.extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let back_view = back_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let multisample_texture = (target.sample_count == 4).then(|| {
        raster_gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Halcyon 3D Headless Private MSAA Color"),
            size: target.extent,
            mip_level_count: 1,
            sample_count: target.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: target.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    });
    let multisample_view = multisample_texture
        .as_ref()
        .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
    let depth_texture = raster_gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Halcyon 3D Headless Private Reverse-Z Depth"),
        size: target.extent,
        mip_level_count: 1,
        sample_count: target.sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = raster_gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Halcyon 3D Headless Raster Encoder"),
        });
    prepared.encode(
        &mut encoder,
        &back_view,
        &depth_view,
        multisample_view.as_ref(),
    );
    raster_gpu.queue.submit(std::iter::once(encoder.finish()));

    let uniform_proof = prepared.packed.proof.clone();
    let layout_contract = raster_gpu.layout_contract;
    prepared.retire_after_submitted_work();
    Ok(HeadlessRasterOutput {
        texture: back_texture,
        extent,
        format: target.format,
        sample_count: target.sample_count,
        layout_contract,
        uniform_proof,
    })
}

impl Context3D {
    #[cfg(test)]
    pub(crate) fn render_headless_snapshot_for_test(
        &self,
        snapshot: &RenderSnapshot,
        model: &ModelState,
        view: ViewDescriptor3,
        render_origin: WorldPoint3,
        extent: [u32; 2],
        sample_count: u32,
    ) -> Result<HeadlessRasterOutput, HeadlessRaster3DError> {
        render_headless_snapshot(
            self,
            snapshot,
            model,
            view,
            render_origin,
            extent,
            sample_count,
        )
    }

    #[cfg(test)]
    pub(crate) fn resident_pipeline_count_for_test(&self) -> Result<u32, Context3DError> {
        self.raster_gpu()?.pipeline_count()
    }
}
