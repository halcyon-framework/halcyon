use super::graph::{Filter, PhysicalRole};
use super::{
    FrostedGlassCompileRefusal as Refusal, FrostedGlassParams, FrostedSurfacePlan,
    RendererCompileError,
};
use crate::compositor::SurfaceId;
use crate::renderer::render_graph::{AttachmentHandle, BufferHandle};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

const FREE: u8 = 0;
const PREPARED: u8 = 1;
const ENCODED: u8 = 2;
const SUBMITTED: u8 = 3;
const COMPLETE: u8 = 4;
const CANCELLED: u8 = 5;
const UNCONFIRMED: u8 = 6;
const SUBMISSION_UNCERTAIN: u8 = 7;

/// Only the queue callback can certify submitted completion. In particular,
/// automatic output Drop cannot certify disposal of a moved-out raw command.
pub(crate) struct FrostUse {
    state: Arc<AtomicU8>,
}
impl FrostUse {
    fn cancel_encoded(self) {
        let _ =
            self.state
                .compare_exchange(ENCODED, CANCELLED, Ordering::AcqRel, Ordering::Acquire);
    }
    fn begin_submission(&self) {
        assert_eq!(
            self.state.swap(SUBMISSION_UNCERTAIN, Ordering::AcqRel),
            ENCODED,
            "INV-385-CUSTODY: exact encoded command must be submitted once"
        );
    }
    fn submitted(self, queue: &wgpu::Queue) {
        assert_eq!(
            self.state.swap(SUBMITTED, Ordering::AcqRel),
            SUBMISSION_UNCERTAIN,
            "INV-385-CUSTODY: completion belongs to the actual queue handoff"
        );
        let state = self.state.clone();
        crate::renderer::on_submitted_work_done(queue, move || state.store(COMPLETE, Ordering::Release));
    }
    fn export(self) {
        self.state.store(UNCONFIRMED, Ordering::Release);
    }
}

/// HAL385 §9: the original executable and its exact use never separate while
/// reusable custody exists. No raw borrow, replacement slot, Clone or Deref.
pub(crate) struct EncodedTargetCommands {
    command: Option<wgpu::CommandBuffer>,
    frost_use: Option<FrostUse>,
    origin: Option<Arc<wgpu::Queue>>,
}
struct QueueHandoff {
    command: wgpu::CommandBuffer,
    usage: Option<FrostUse>,
}
impl EncodedTargetCommands {
    /// The only custody-bearing constructor consumes the actual target encoder.
    pub(crate) fn finish(
        encoder: wgpu::CommandEncoder,
        frost_use: Option<FrostUse>,
        origin: Arc<wgpu::Queue>,
    ) -> Self {
        Self {
            command: Some(encoder.finish()),
            frost_use,
            origin: Some(origin),
        }
    }
    /// Explicit ordinary/custom raw capability. No use can be attached later.
    pub(crate) fn unmanaged(command: wgpu::CommandBuffer) -> Self {
        Self {
            command: Some(command),
            frost_use: None,
            origin: None,
        }
    }
    pub(crate) fn cancel_encoding(encoder: wgpu::CommandEncoder, usage: Option<FrostUse>) {
        drop(encoder);
        if let Some(usage) = usage {
            usage.cancel_encoded();
        }
    }
    pub(in crate::renderer) fn validate_submission(
        &self,
        queue: &Arc<wgpu::Queue>,
    ) -> Result<(), crate::renderer::sdf_image_generation_custody::SdfImageCustodyError> {
        if self
            .origin
            .as_ref()
            .is_some_and(|origin| !Arc::ptr_eq(origin, queue))
        {
            return Err(
                crate::renderer::sdf_image_generation_custody::SdfImageCustodyError::QueueMismatch,
            );
        }
        Ok(())
    }
    pub(in crate::renderer) fn submit(
        self,
        queue: &Arc<wgpu::Queue>,
    ) -> Result<
        wgpu::SubmissionIndex,
        crate::renderer::sdf_image_generation_custody::SdfImageCustodyError,
    > {
        self.validate_submission(queue)?;
        let QueueHandoff { command, usage } = self.begin_handoff();
        // Unwinding or lost callback registration can never cancel/reclaim this
        // use: it became non-reclaimable before the command left our ownership.
        let index = queue.submit(std::iter::once(command));
        if let Some(usage) = usage {
            usage.submitted(queue);
        }
        Ok(index)
    }
    fn begin_handoff(mut self) -> QueueHandoff {
        let command = self
            .command
            .take()
            .expect("INV-385-CUSTODY: owned original command");
        let usage = self.frost_use.take();
        if let Some(usage) = &usage {
            usage.begin_submission();
        }
        QueueHandoff { command, usage }
    }
    pub(crate) fn into_raw(mut self) -> wgpu::CommandBuffer {
        if let Some(usage) = self.frost_use.take() {
            usage.export();
        }
        self.command
            .take()
            .expect("INV-385-CUSTODY: owned raw export")
    }
}
impl Drop for EncodedTargetCommands {
    fn drop(&mut self) {
        // Disposal order is evidence. A still-sealed unsubmitted command dies
        // before its own use can become reclaimable, including wrong-queue Drop.
        drop(self.command.take());
        if let Some(usage) = self.frost_use.take() {
            usage.cancel_encoded();
        }
    }
}
impl Drop for FrostUse {
    fn drop(&mut self) {
        let _ =
            self.state
                .compare_exchange(ENCODED, UNCONFIRMED, Ordering::AcqRel, Ordering::Acquire);
    }
}

pub(crate) struct FrostedPipelines {
    pub horizontal: wgpu::RenderPipeline,
    pub vertical: wgpu::RenderPipeline,
    pub replace: wgpu::RenderPipeline,
    pub terminal: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}
impl FrostedPipelines {
    fn group(
        &self,
        device: &wgpu::Device,
        uniform: &wgpu::Buffer,
        one: &wgpu::TextureView,
        two: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HAL385 declared reads"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(one),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(two),
                },
            ],
        })
    }
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals: &wgpu::BindGroupLayout,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HAL385 frozen inputs"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<super::FrostedGlassParams>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HAL385 direct filter ABI"),
            bind_group_layouts: &[Some(globals), Some(&layout)],
            immediate_size: 0,
        });
        let make = |label, source: String, target| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(super::shader_source(&source).into()),
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let horizontal = make(
            "HAL385 horizontal",
            include_str!("../shaders/frosted_glass_horizontal.wgsl").into(),
            wgpu::TextureFormat::Rgba16Float,
        );
        let vertical = make(
            "HAL385 vertical",
            include_str!("../shaders/frosted_glass_vertical.wgsl").into(),
            wgpu::TextureFormat::Rgba16Float,
        );
        let replace = make(
            "HAL385 replacement",
            format!(
                "{}\n{}",
                include_str!("../shaders/micrograin.wgsl"),
                include_str!("../shaders/frosted_glass_replace.wgsl")
            ),
            format,
        );
        let terminal = make(
            "HAL385 terminal",
            include_str!("../shaders/frosted_glass_terminal.wgsl").into(),
            format,
        );
        Self {
            horizontal,
            vertical,
            replace,
            terminal,
            layout,
        }
    }
}
/// Frozen Compile bindings, exposed only to constrained pass execution.
pub(crate) struct FrostedPassResources {
    pub pipelines: Arc<FrostedPipelines>,
    pub bloom: Option<Arc<crate::renderer::bloom::pipelines::BloomPipelines>>,
    pub textures: HashMap<AttachmentHandle, wgpu::Texture>,
    pub views: HashMap<AttachmentHandle, wgpu::TextureView>,
    pub uniforms: HashMap<BufferHandle, wgpu::Buffer>,
    pub groups: HashMap<BufferHandle, wgpu::BindGroup>,
}
struct Backing {
    key: Key,
    bytes: u64,
    state: Arc<AtomicU8>,
    accumulator: wgpu::Texture,
    accumulator_view: wgpu::TextureView,
    occurrences: Vec<[wgpu::Texture; 3]>,
    occurrence_views: Vec<[wgpu::TextureView; 3]>,
    occurrence_groups: Vec<[wgpu::BindGroup; 3]>,
    terminal_group: wgpu::BindGroup,
    uniforms: Vec<wgpu::Buffer>,
    pipelines: Arc<FrostedPipelines>,
    bloom_pipelines: Option<Arc<crate::renderer::bloom::pipelines::BloomPipelines>>,
    bloom: Option<crate::renderer::bloom::pipelines::BloomBacking>,
    frozen: Option<FrostedPassResources>,
    // This allocation may be used only after its own error scope completes.
    qualification: Arc<AtomicU8>,
}
const SCOPE_PENDING: u8 = 0;
const SCOPE_READY: u8 = 1;
const SCOPE_OOM: u8 = 2;
#[derive(PartialEq, Eq)]
struct Key {
    size: (u32, u32),
    format: wgpu::TextureFormat,
    rois: Vec<([u32; 2], [u32; 2])>,
    bloom: Option<[(u32, u32); 3]>,
}
#[derive(Default)]
pub(crate) struct FrostPool {
    entries: Vec<Backing>,
    prepared: HashMap<SurfaceId, usize>,
    pipelines: Option<Arc<FrostedPipelines>>,
    bloom_pipelines: Option<Arc<crate::renderer::bloom::pipelines::BloomPipelines>>,
    pipeline_fault: Arc<std::sync::atomic::AtomicBool>,
    pub allocations: u64,
}
impl FrostPool {
    pub fn begin_preparation(&mut self) {
        // A superseded browser scope cannot qualify new bindings for another
        // generation. Ready entries retain the pipelines that built their groups.
        if self.prepared.values().any(|&index|
            self.entries[index].qualification.load(Ordering::Acquire) == SCOPE_PENDING
        ) {
            self.pipelines = None;
            self.bloom_pipelines = None;
        }
        for &index in self.prepared.values() {
            let _ = self.entries[index].state.compare_exchange(
                PREPARED,
                CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
        self.prepared.clear();
        self.maintain();
    }
    pub fn maintain(&mut self) {
        if self.pipeline_fault.swap(false, Ordering::AcqRel) {
            self.pipelines = None;
            self.bloom_pipelines = None;
        }
        for b in &mut self.entries {
            let state = b.state.load(Ordering::Acquire);
            if state == COMPLETE || (state == CANCELLED && b.qualification.load(Ordering::Acquire) != SCOPE_PENDING) {
                b.frozen = None;
                b.state.store(FREE, Ordering::Release);
            }
        }
    }
    pub fn resident_bytes(&self) -> u64 {
        self.entries.iter().map(|b| b.bytes).sum()
    }
    #[cfg(test)]
    pub(crate) fn state_counts(&self) -> [usize; 8] {
        let mut result = [0; 8];
        for entry in &self.entries {
            result[entry.state.load(Ordering::Acquire) as usize] += 1;
        }
        result
    }
    #[cfg(test)]
    pub(crate) fn bloom_textures(&self, surface: SurfaceId) -> Option<[wgpu::Texture; 3]> {
        self.entries[*self.prepared.get(&surface)?].bloom.as_ref().map(|b| b.textures.clone())
    }
    pub fn retire_surface(&mut self, surface: SurfaceId) {
        if let Some(index) = self.prepared.remove(&surface) {
            let _ = self.entries[index].state.compare_exchange(
                PREPARED,
                CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
        self.maintain();
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        globals: &wgpu::BindGroupLayout,
        surface: SurfaceId,
        plan: &FrostedSurfacePlan,
        preparation: Option<&super::RendererPreparationIdentity>,
    ) -> Result<(), RendererCompileError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
        let allocation_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let result = self.prepare_inner(device, queue, globals, surface, plan, preparation);
        if pollster::block_on(allocation_scope.pop()).is_some() {
            self.pipelines = None;
            self.bloom_pipelines = None;
            if let Some(&index) = self.prepared.get(&surface) {
                self.entries[index].qualification.store(SCOPE_OOM, Ordering::Release);
            }
            return Err(crate::renderer::RendererSurfaceError::OutOfMemory.into());
        }
        result
        }
        #[cfg(target_arch = "wasm32")]
        self.prepare_inner(device, queue, globals, surface, plan, preparation)
    }
    fn prepare_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        globals: &wgpu::BindGroupLayout,
        surface: SurfaceId,
        plan: &FrostedSurfacePlan,
        _preparation: Option<&super::RendererPreparationIdentity>,
    ) -> Result<(), RendererCompileError> {
        use crate::renderer::bloom::pipelines::{BloomBacking, BloomPipelines};
        use crate::renderer::BloomCompileRefusal;
        let key = Key {
            size: plan.size,
            format: plan.format,
            rois: plan
                .occurrences
                .iter()
                .map(|o| {
                    (
                        [o.params.snapshot[2], o.params.snapshot[3]],
                        [o.params.filtered_domain[2], o.params.filtered_domain[3]],
                    )
                })
                .collect(),
            bloom: plan.bloom.as_ref().map(|b| b.regions().map(|[_, _, w, h]| (w, h))),
        };
        let limit = device.limits();
        if plan.size.0 > limit.max_texture_dimension_2d
            || plan.size.1 > limit.max_texture_dimension_2d
            || (limit.max_uniform_buffer_binding_size as usize) < std::mem::size_of::<super::FrostedGlassParams>()
        {
            return Err(if let Some(bloom) = &plan.bloom {
                RendererCompileError::Bloom { surface_id: surface, node_key: bloom.node_key, reason: BloomCompileRefusal::InvalidTarget }
            } else {
                super::error(surface, None, Refusal::InvalidTarget)
            });
        }
        self.maintain();
        let matching = self
            .entries
            .iter()
            .position(|b| b.state.load(Ordering::Acquire) == FREE && b.key == key
                && b.qualification.load(Ordering::Acquire) == SCOPE_READY);
        let index = if let Some(i) = matching {
            i
        } else {
            let required = self
                .resident_bytes()
                .checked_add(plan.bytes)
                .unwrap_or(u64::MAX);
            if required > super::RESIDENT_BUDGET {
                // Only free backing is evictable. Never wait or reclaim encoded,
                // submitted or unconfirmed work to make a budget appear smaller.
                for i in (0..self.entries.len()).rev() {
                    if self.entries[i].state.load(Ordering::Acquire) == FREE {
                        self.entries.remove(i);
                        for index in self.prepared.values_mut() {
                            if *index > i {
                                *index -= 1;
                            }
                        }
                    }
                }
            }
            if self
                .resident_bytes()
                .checked_add(plan.bytes)
                .is_none_or(|v| v > super::RESIDENT_BUDGET)
            {
                let required_bytes = self.resident_bytes().saturating_add(plan.bytes);
                let limit_bytes = super::RESIDENT_BUDGET;
                return Err(if let Some(bloom) = &plan.bloom {
                    RendererCompileError::Bloom {
                        surface_id: surface, node_key: bloom.node_key,
                        reason: BloomCompileRefusal::ResourceBudget { required_bytes, limit_bytes },
                    }
                } else {
                    super::error(surface, None, Refusal::ResourceBudget { required_bytes, limit_bytes })
                });
            }
            #[cfg(target_arch = "wasm32")]
            let allocation_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
            if self.pipelines.is_none() {
                self.pipelines = Some(Arc::new(FrostedPipelines::new(
                    device,
                    plan.format,
                    globals,
                )));
            }
            let texture = |size: (u32, u32), format, usage| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("HAL385 bounded backing"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            let accumulator = texture(
                plan.size,
                plan.format,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
            );
            let mut occurrences = Vec::new();
            for &(s, v) in &key.rois {
                occurrences.push([
                    texture(
                        (s[0], s[1]),
                        plan.format,
                        wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                    ),
                    texture(
                        (s[0], s[1]),
                        wgpu::TextureFormat::Rgba16Float,
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                    ),
                    texture(
                        (v[0], v[1]),
                        wgpu::TextureFormat::Rgba16Float,
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                    ),
                ]);
            }
            let uniforms: Vec<wgpu::Buffer> = (0..=plan.occurrences.len())
                .map(|_| {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("HAL385 immutable uniform"),
                        size: std::mem::size_of::<super::FrostedGlassParams>() as u64,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                })
                .collect();
            // All GPU views and bindings are part of the reusable backing.
            // A warmed Compile changes uniform bytes and logical handles only.
            let accumulator_view = accumulator.create_view(&Default::default());
            let occurrence_views: Vec<_> = occurrences
                .iter()
                .map(|t| {
                    [
                        t[0].create_view(&Default::default()),
                        t[1].create_view(&Default::default()),
                        t[2].create_view(&Default::default()),
                    ]
                })
                .collect();
            let pipelines = self.pipelines.as_ref().unwrap().clone();
            let occurrence_groups = occurrence_views
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    [
                        pipelines.group(device, &uniforms[i], &v[0], &v[0]),
                        pipelines.group(device, &uniforms[i], &v[1], &v[1]),
                        pipelines.group(device, &uniforms[i], &v[0], &v[2]),
                    ]
                })
                .collect();
            let terminal_group = pipelines.group(
                device,
                uniforms.last().unwrap(),
                &accumulator_view,
                &accumulator_view,
            );
            let bloom_pipelines = plan.bloom.as_ref().map(|_| {
                self.bloom_pipelines.get_or_insert_with(|| {
                    Arc::new(BloomPipelines::new(device, globals, plan.format))
                }).clone()
            });
            let bloom = plan.bloom.as_ref().zip(bloom_pipelines.as_deref())
                .map(|(plan, pipelines)| BloomBacking::new(device, pipelines, plan, &texture));
            let qualification = Arc::new(AtomicU8::new(SCOPE_READY));
            #[cfg(target_arch = "wasm32")]
            {
                qualification.store(SCOPE_PENDING, Ordering::Release);
                let preparation =
                    _preparation.expect("browser frosted allocation has a current preparation");
                preparation.add_scope();
                // Pop now, before any other allocation scope is opened. Polling
                // the resulting JS Promise belongs to the browser event loop.
                let scope = allocation_scope.pop();
                let qualification = qualification.clone();
                let preparation = preparation.clone();
                let pipeline_fault = self.pipeline_fault.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let out_of_memory = scope.await.is_some();
                    if out_of_memory { pipeline_fault.store(true, Ordering::Release); }
                    qualification.store(if out_of_memory { SCOPE_OOM } else { SCOPE_READY }, Ordering::Release);
                    preparation.finish_scope(out_of_memory);
                });
            }
            self.entries.push(Backing {
                key,
                bytes: plan.bytes,
                state: Arc::new(AtomicU8::new(FREE)),
                accumulator,
                accumulator_view,
                occurrences,
                occurrence_views,
                occurrence_groups,
                terminal_group,
                uniforms,
                pipelines,
                bloom_pipelines,
                bloom,
                frozen: None,
                qualification,
            });
            self.allocations += 1;
            self.entries.len() - 1
        };
        let b = &mut self.entries[index];
        b.state = Arc::new(AtomicU8::new(PREPARED));
        if let Some(bloom) = &plan.bloom {
            b.bloom.as_ref().expect("matched Bloom backing").write(queue, bloom);
        }
        for (i, o) in plan.occurrences.iter().enumerate() {
            queue.write_buffer(&b.uniforms[i], 0, bytemuck::bytes_of(&o.params));
        }
        queue.write_buffer(
            b.uniforms.last().unwrap(),
            0,
            bytemuck::bytes_of(&FrostedGlassParams::terminal(plan.size)),
        );
        let mut textures = HashMap::new();
        let mut views = HashMap::new();
        for (&handle, role) in &plan.roles {
            let (texture, view) = match *role {
                PhysicalRole::Accumulator => (&b.accumulator, &b.accumulator_view),
                PhysicalRole::Snapshot(i) => (&b.occurrences[i][0], &b.occurrence_views[i][0]),
                PhysicalRole::Horizontal(i) => (&b.occurrences[i][1], &b.occurrence_views[i][1]),
                PhysicalRole::Filtered(i) => (&b.occurrences[i][2], &b.occurrence_views[i][2]),
                PhysicalRole::Bloom(i) => {
                    let bloom = b.bloom.as_ref().expect("matched Bloom backing");
                    (&bloom.textures[i], &bloom.views[i])
                }
                PhysicalRole::Terminal => continue,
            };
            textures.insert(handle, texture.clone());
            views.insert(handle, view.clone());
        }
        let mut groups = HashMap::new();
        let mut uniforms = HashMap::new();
        for &(handle, filter, i) in &plan.groups {
            let (uniform, group) = match filter {
                Filter::Horizontal => (&b.uniforms[i], &b.occurrence_groups[i][0]),
                Filter::Vertical => (&b.uniforms[i], &b.occurrence_groups[i][1]),
                Filter::Replace => (&b.uniforms[i], &b.occurrence_groups[i][2]),
                Filter::Terminal => (b.uniforms.last().unwrap(), &b.terminal_group),
                Filter::Bloom(stage) => {
                    let bloom = b.bloom.as_ref().expect("matched Bloom backing");
                    (&bloom.uniforms[stage as usize], &bloom.groups[stage as usize])
                }
            };
            uniforms.insert(handle, uniform.clone());
            groups.insert(handle, group.clone());
        }
        b.frozen = Some(FrostedPassResources {
            pipelines: b.pipelines.clone(),
            bloom: b.bloom_pipelines.clone(),
            textures,
            views,
            uniforms,
            groups,
        });
        self.prepared.insert(surface, index);
        Ok(())
    }
    pub fn bind(
        &self,
        surface: SurfaceId,
        resources: &mut crate::renderer::render_graph::BoundResources,
    ) -> Option<FrostUse> {
        let b = &self.entries[*self.prepared.get(&surface)?];
        if b.qualification.load(Ordering::Acquire) != SCOPE_READY {
            return None;
        }
        let frozen = b.frozen.as_ref()?;
        b.state
            .compare_exchange(PREPARED, ENCODED, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        resources.attachments.extend(frozen.views.clone());
        resources.buffers.extend(frozen.uniforms.clone());
        resources.frosted = Some(FrostedPassResources {
            pipelines: frozen.pipelines.clone(),
            bloom: frozen.bloom.clone(),
            textures: frozen.textures.clone(),
            views: frozen.views.clone(),
            uniforms: frozen.uniforms.clone(),
            groups: frozen.groups.clone(),
        });
        Some(FrostUse {
            state: b.state.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frosted_glass_custody_drop_is_not_completion_or_raw_command_disposal() {
        let state = Arc::new(AtomicU8::new(ENCODED));
        drop(FrostUse {
            state: state.clone(),
        });
        assert_eq!(state.load(Ordering::Acquire), UNCONFIRMED);
        let state = Arc::new(AtomicU8::new(ENCODED));
        FrostUse {
            state: state.clone(),
        }
        .cancel_encoded();
        assert_eq!(state.load(Ordering::Acquire), CANCELLED);
        let state = Arc::new(AtomicU8::new(SUBMITTED));
        drop(FrostUse {
            state: state.clone(),
        });
        assert_eq!(
            state.load(Ordering::Acquire),
            SUBMITTED,
            "Drop cannot manufacture queue completion"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn frosted_glass_sealed_wrong_queue_uncertain_cancel_and_completion_metal() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::{
            math::RectF,
            paint::{Color as PaintColor, FrostedGlassAppearance, RoundedCorners},
            scene::{Color, SceneBuilder},
        };
        use crate::renderer::sdf_image_generation_custody::SdfImageCustodyError;
        use crate::renderer::{
            DrawPlanBuilder, mesh_cache::MeshCache, wgpu_renderer::WgpuRenderer,
        };
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).expect("real Metal");
        let other =
            pollster::block_on(WgpuRenderer::new_headless()).expect("second real queue owner");
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.adapter.get_info().backend, wgpu::Backend::Metal);
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 40.0, 32.0));
        let surface =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 40.0, 32.0), SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (40, 32), 1.0)
            .unwrap();
        let mut scene = SceneBuilder::new();
        scene.push_rect(RectF::new(0.0, 0.0, 40.0, 32.0), Color::WHITE, 0, None);
        scene
            .push_frosted_glass(
                RectF::new(1.0, 1.0, 30.0, 24.0),
                RoundedCorners::uniform(2.0),
                &FrostedGlassAppearance::try_new(1.0, PaintColor::WHITE, 0.8).unwrap(),
                10,
                None,
            )
            .unwrap();
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene.build())]),
                compositor.snapshot_multi(),
                &mut crate::renderer::mesh_cache::MeshCache::new(),
                |_| 1.0,
                0,
            )
            .unwrap();
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HAL385 real sealed owner"),
            size: wgpu::Extent3d {
                width: 40,
                height: 32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let original = renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .unwrap();
        let state = original.frost_use.as_ref().unwrap().state.clone();
        assert!(matches!(
            original.submit(&other.gpu_resources().queue),
            Err(SdfImageCustodyError::QueueMismatch)
        ));
        assert_eq!(
            state.load(Ordering::Acquire),
            CANCELLED,
            "wrong queue disposes only the held original without submission"
        );
        let warm = renderer.frosted_pool_for_test();
        assert_eq!(warm.0, 1);
        for _ in 0..4 {
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            let command = renderer
                .encode_compiled_target_for_test(&compiled, surface, &view)
                .unwrap();
            let state = command.frost_use.as_ref().unwrap().state.clone();
            renderer.submit_compiled_target_for_test(command);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(
                state.load(Ordering::Acquire),
                COMPLETE,
                "only actual original queue completion"
            );
            assert_eq!(
                renderer.frosted_pool_for_test(),
                warm,
                "warmed completed real backing allocations stabilize"
            );
        }
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let command = renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .unwrap();
        let state = command.frost_use.as_ref().unwrap().state.clone();
        drop(command);
        assert_eq!(
            state.load(Ordering::Acquire),
            CANCELLED,
            "sealed abandonment disposes the original before cancellation"
        );
        renderer.frosted_pool_for_test();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let command = renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .unwrap();
        let state = command.frost_use.as_ref().unwrap().state.clone();
        // Exercise the exact production handoff transition without inventing a
        // fake completion. Uncertain cleanup remains charged even if this test
        // can deliberately discard a still-local command afterward.
        let uncertain = command.begin_handoff();
        assert_eq!(state.load(Ordering::Acquire), SUBMISSION_UNCERTAIN);
        drop(uncertain);
        assert_eq!(state.load(Ordering::Acquire), SUBMISSION_UNCERTAIN);
        let next = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        assert!(
            renderer.frosted_pool_for_test().0 > warm.0,
            "real uncertain backing cannot become reusable"
        );
        drop(next);
    }
}
