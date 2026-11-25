//! Private R-027 output owner. Source radiance generation is deliberately outside
//! this module: an admitted producer writes the scene attachment, then this fixed
//! owner tones each sample and publishes through the existing CustomRenderPass.

use std::borrow::Cow;
use std::num::NonZeroU64;
use std::sync::Arc;

use crate::renderer::wgpu_renderer::GpuResources;
use crate::renderer::{CustomRenderFrame, CustomRenderPass, PhysicalRect};
use crate::utils::id::WidgetInternalId;

use super::context::{DynamicStorageLease3D, ShaderModuleResidency, TargetBudgetLease3D};
use super::raster::{ControlledPipelineResidency, DEPTH_FORMAT};
use super::{Context3D, Context3DError, PolicyBudget3D};

#[cfg(test)]
pub(crate) use super::raster::{
    MaterialScenePreparation3D, prepare_material_scene, prepare_material_scene_with_capacity,
};

pub(crate) const FINAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub(crate) const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
pub(crate) const SCENE_DEPTH_BYTES_PER_SAMPLE: u64 = 20;
pub(crate) const FINAL_PAIR_BYTES_PER_PIXEL: u64 = 16;

/// Fixed R027 scene ABI. The four views cannot be supplied from different targets.
pub(crate) struct HdrScene3D {
    _textures: [wgpu::Texture; 4],
    views: [wgpu::TextureView; 4],
}

impl HdrScene3D {
    pub(crate) fn attachments(&self) -> [Option<wgpu::RenderPassColorAttachment<'_>>; 4] {
        std::array::from_fn(|i| {
            Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: &self.views[i],
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })
        })
    }
}

pub(crate) fn scene_color_targets() -> [Option<wgpu::ColorTargetState>; 4] {
    std::array::from_fn(|_| {
        Some(wgpu::ColorTargetState {
            format: SCENE_FORMAT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })
    })
}

pub(crate) fn scene_capabilities(device: &wgpu::Device, samples: u32) -> Result<(), HdrError3D> {
    let limits = device.limits();
    let features = SCENE_FORMAT.guaranteed_format_features(device.features());
    if !matches!(samples, 1 | 4)
        || limits.max_color_attachments < 4
        || limits.max_color_attachment_bytes_per_sample < 16
        || limits.max_sampled_textures_per_shader_stage < 4
        || limits.max_bindings_per_bind_group < 5
        || !features
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        || !features.flags.sample_count_supported(samples)
    {
        return Err(HdrError3D::Target(
            "R027 four R32Float scene attachment/load limits",
        ));
    }
    Ok(())
}

/// Change only selected fragment return packing, preserving all source expressions,
/// discard/control flow, input interfaces and depth-only/diagnostic entry points.
pub(crate) fn scene_fragment_outputs(
    module: &mut naga::Module,
    entries: &[&str],
) -> Result<(), String> {
    use naga::{Binding, Expression, Scalar, Span, Statement, StructMember, Type, TypeInner};
    fn block(
        body: &mut naga::Block,
        expressions: &mut naga::Arena<Expression>,
        output: naga::Handle<Type>,
    ) {
        let old = std::mem::take(body);
        for (statement, span) in old.span_iter() {
            let mut statement = statement.clone();
            match &mut statement {
                Statement::Return { value: Some(value) } => {
                    let mut emitter = naga::proc::Emitter::default();
                    emitter.start(expressions);
                    let components = (0..4)
                        .map(|index| {
                            expressions.append(
                                Expression::AccessIndex {
                                    base: *value,
                                    index,
                                },
                                Span::UNDEFINED,
                            )
                        })
                        .collect();
                    *value = expressions.append(
                        Expression::Compose {
                            ty: output,
                            components,
                        },
                        Span::UNDEFINED,
                    );
                    body.extend(emitter.finish(expressions));
                }
                Statement::Block(b) => block(b, expressions, output),
                Statement::If { accept, reject, .. } => {
                    block(accept, expressions, output);
                    block(reject, expressions, output);
                }
                Statement::Switch { cases, .. } => {
                    for case in cases {
                        block(&mut case.body, expressions, output);
                    }
                }
                Statement::Loop {
                    body, continuing, ..
                } => {
                    block(body, expressions, output);
                    block(continuing, expressions, output);
                }
                _ => {}
            }
            body.push(statement, *span);
        }
    }
    let scalar = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar::F32),
        },
        Span::UNDEFINED,
    );
    let output = module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Struct {
                members: (0..4)
                    .map(|i| StructMember {
                        name: Some(["red", "green", "blue", "coverage"][i as usize].into()),
                        ty: scalar,
                        binding: Some(Binding::Location {
                            location: i,
                            interpolation: None,
                            sampling: None,
                            blend_src: None,
                            per_primitive: false,
                        }),
                        offset: i * 4,
                    })
                    .collect(),
                span: 16,
            },
        },
        Span::UNDEFINED,
    );
    let mut mapped = 0;
    for entry in &mut module.entry_points {
        if entry.stage != naga::ShaderStage::Fragment || !entries.contains(&entry.name.as_str()) {
            continue;
        }
        let result = entry
            .function
            .result
            .as_mut()
            .ok_or("missing R027 color output")?;
        if !matches!(
            module.types[result.ty].inner,
            TypeInner::Vector {
                size: naga::VectorSize::Quad,
                scalar: Scalar::F32
            }
        ) {
            return Err("R027 color output is not the original vec4<f32>".into());
        }
        result.ty = output;
        result.binding = None;
        block(
            &mut entry.function.body,
            &mut entry.function.expressions,
            output,
        );
        mapped += 1;
    }
    if mapped == 0 {
        Err("missing R027 source color entry point".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) trait HdrProofSceneWriter3D: Send + Sync {
    fn source(&self) -> &HdrSource3D;
    fn samples(&self) -> u32;
    fn gpu_resources(&self) -> &GpuResources;
    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &HdrScene3D,
        depth: &wgpu::TextureView,
    );
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HdrSource3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) exposure_ev: f32,
    /// Complete producer-certified component bounds, not a renderer estimate.
    pub(crate) maximum_rgb: [f32; 3],
}

impl HdrSource3D {
    pub(crate) fn exposure(&self) -> Result<f32, HdrError3D> {
        if self.identity == 0 || self.revision == 0 {
            return Err(HdrError3D::Source(
                "missing immutable source identity/revision",
            ));
        }
        if !self
            .maximum_rgb
            .iter()
            .all(|c| c.is_finite() && (0.0..=60000.0).contains(c))
        {
            return Err(HdrError3D::Source(
                "uncertified finite nonnegative radiance <=60000",
            ));
        }
        if !self.exposure_ev.is_finite() || !(-16.0..=16.0).contains(&self.exposure_ev) {
            return Err(HdrError3D::Source("exposure outside [-16,+16]"));
        }
        let e = self.exposure_ev.exp2();
        if !e.is_finite() || e <= 0.0 {
            return Err(HdrError3D::Source("unrepresentable exposure"));
        }
        Ok(e)
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum HdrError3D {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error("R-027 source: {0}")]
    Source(&'static str),
    #[error("R-027 target/capability: {0}")]
    Target(&'static str),
    #[error("R-027 final back buffer is retained by a registered presentation reader")]
    PinnedFront,
    #[error("R-027 controlled shader/device: {0}")]
    Shader(String),
}

/// Writer progress only. No stage certifies GPU completion or resource reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HdrSubmissionStage3D {
    BeforeWrites,
    WritesQueuedNoFrameSubmit,
    SubmittedSwappedUnpublished,
    Published,
}

#[derive(Default)]
pub(crate) struct HdrWriterCompletion3D {
    submitted: std::sync::atomic::AtomicBool,
    complete: std::sync::atomic::AtomicBool,
}

impl HdrWriterCompletion3D {
    pub(crate) fn submitted(&self) -> bool {
        self.submitted.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn complete(&self) -> bool {
        self.complete.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn reset(&self) {
        self.submitted
            .store(false, std::sync::atomic::Ordering::Release);
        self.complete
            .store(false, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn record(self: &Arc<Self>, queue: &wgpu::Queue) {
        self.submitted
            .store(true, std::sync::atomic::Ordering::Release);
        let completion = self.clone();
        crate::renderer::on_submitted_work_done(queue, move || {
            completion
                .complete
                .store(true, std::sync::atomic::Ordering::Release);
        });
    }
}

fn budget(kind: PolicyBudget3D, requested: u64, maximum: u64) -> Result<(), HdrError3D> {
    if requested > maximum {
        return Err(Context3DError::PolicyBudgetExceeded {
            budget: kind,
            requested,
            maximum,
        }
        .into());
    }
    Ok(())
}

pub(crate) struct ToneProgram3D {
    pub(crate) layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    _module: wgpu::ShaderModule,
    _module_residency: ShaderModuleResidency,
    _pipeline_residency: ControlledPipelineResidency,
    samples: u32,
}

impl ToneProgram3D {
    pub(crate) fn new(context: &Context3D, samples: u32) -> Result<Self, HdrError3D> {
        if !matches!(samples, 1 | 4) {
            return Err(HdrError3D::Target("scene sample count must be 1 or 4"));
        }
        let gpu = context.gpu_resources()?;
        scene_capabilities(&gpu.device, samples)?;
        if !gpu.private_hdr_owner_live() {
            return Err(HdrError3D::Target("renderer retired"));
        }
        let raster = context.raster_gpu()?;
        let mut pipeline_residency =
            raster.reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
        let reservation = context.reserve_controlled_shader_module()?;
        let source = tone_source(samples);
        let ir = naga::front::wgsl::parse_str(&source)
            .map_err(|e| HdrError3D::Shader(e.emit_to_string(&source)))?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&ir)
        .map_err(|e| HdrError3D::Shader(e.emit_to_string(&source)))?;
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Halcyon private R027 tone module"),
                source: wgpu::ShaderSource::Wgsl(Cow::Owned(source)),
            });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Halcyon R027 Tone16 + exact scene class"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(16),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: samples == 4,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: samples == 4,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: samples == 4,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: samples == 4,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Halcyon R027 private tone layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Halcyon R027 tone to linear FP16"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FINAL_FORMAT,
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
            });
        if let Some(error) = pollster::block_on(validation_scope.pop()) {
            return Err(HdrError3D::Shader(error.to_string()));
        }
        let module_residency = reservation.commit_one();
        pipeline_residency.commit();
        Ok(Self {
            layout,
            pipeline,
            _module: module,
            _module_residency: module_residency,
            _pipeline_residency: pipeline_residency,
            samples,
        })
    }
}

fn tone_source(samples: u32) -> String {
    let texture = if samples == 4 {
        "texture_multisampled_2d<f32>"
    } else {
        "texture_2d<f32>"
    };
    format!(
        r#"
@group(0) @binding(0) var<uniform> tone: vec4<f32>;
@group(0) @binding(1) var scene_r: {texture};
@group(0) @binding(2) var scene_g: {texture};
@group(0) @binding(3) var scene_b: {texture};
@group(0) @binding(4) var scene_coverage: {texture};
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
    let p = array<vec2<f32>,3>(vec2(-1.0,-1.0), vec2(3.0,-1.0), vec2(-1.0,3.0));
    return vec4(p[i],0.0,1.0);
}}
@fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {{
    let pixel = vec2<i32>(position.xy);
    var result = vec4<f32>(0.0);
    for (var i = 0; i < {samples}; i = i + 1) {{
        let c = vec4(textureLoad(scene_r, pixel, {sample_index}).x,
                     textureLoad(scene_g, pixel, {sample_index}).x,
                     textureLoad(scene_b, pixel, {sample_index}).x,
                     textureLoad(scene_coverage, pixel, {sample_index}).x);
        if c.a == 1.0 {{
            let exposed = tone.x * c.rgb;
            result += vec4(exposed / (vec3(1.0) + exposed), 1.0);
        }}
    }}
    return result / f32({samples});
}}
"#,
        sample_index = if samples == 4 { "i" } else { "0" }
    )
}

struct HdrWork3D {
    scene: HdrScene3D,
    depth: HdrDepth3D,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
    program: ToneProgram3D,
    _scene_charge: TargetBudgetLease3D,
    _uniform_charge: DynamicStorageLease3D,
}

enum HdrDepth3D {
    Attachment {
        _texture: wgpu::Texture,
        view: wgpu::TextureView,
        _charge: TargetBudgetLease3D,
    },
    Sampled(Arc<super::spatial_hud::HudDepth3D>),
}

impl HdrDepth3D {
    fn view(&self) -> &wgpu::TextureView {
        match self {
            Self::Attachment { view, .. } => view,
            Self::Sampled(depth) => depth.view(),
        }
    }
}

/// Nonoverlapping CPU encode and submission/swap stages of an opt-in HDR render.
/// Neither stage waits for GPU completion or observes native presentation.
#[cfg(any(test, feature = "test-harness"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HdrRenderTiming3D {
    pub encode_cpu: std::time::Duration,
    /// Queue submission, swap and queued registration; not GPU completion.
    pub submit_swap_cpu: std::time::Duration,
    pub complete: bool,
}

pub(crate) struct HdrOutput3D {
    pass: Option<CustomRenderPass>,
    work: Option<HdrWork3D>,
    active: Option<Arc<TargetBudgetLease3D>>,
    gpu: GpuResources,
    pub(crate) extent: (u32, u32),
    pub(crate) samples: u32,
    #[cfg(any(test, feature = "test-harness"))]
    timing: Option<HdrRenderTiming3D>,
}

impl HdrOutput3D {
    pub(crate) fn new(
        context: &Context3D,
        width: u32,
        height: u32,
        samples: u32,
        source: &HdrSource3D,
    ) -> Result<Self, HdrError3D> {
        Self::new_with_presence(context, width, height, samples, source, None, 0)
    }

    pub(super) fn new_with_presence(
        context: &Context3D,
        width: u32,
        height: u32,
        samples: u32,
        source: &HdrSource3D,
        active: Option<Arc<TargetBudgetLease3D>>,
        private_bytes: u64,
    ) -> Result<Self, HdrError3D> {
        source.exposure()?;
        if width == 0 || height == 0 || !matches!(samples, 1 | 4) {
            return Err(HdrError3D::Target("inactive extent or invalid samples"));
        }
        let gpu = context.gpu_resources()?;
        let policy = context.limits().policy;
        if !gpu.private_hdr_owner_live() {
            return Err(HdrError3D::Target("renderer retired"));
        }
        let dimension = width.max(height);
        if dimension > gpu.device.limits().max_texture_dimension_2d {
            return Err(HdrError3D::Target("Device texture extent"));
        }
        budget(
            PolicyBudget3D::TargetDimension2D,
            u64::from(dimension),
            u64::from(policy.max_target_dimension_2d),
        )?;
        let features = FINAL_FORMAT.guaranteed_format_features(gpu.device.features());
        scene_capabilities(&gpu.device, samples)?;
        if !features.allowed_usages.contains(
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        ) || !features
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
            || !features.flags.sample_count_supported(1)
            || !DEPTH_FORMAT
                .guaranteed_format_features(gpu.device.features())
                .flags
                .sample_count_supported(samples)
        {
            return Err(HdrError3D::Target(
                "required FP16 attachment/sample-load/filterable Image or depth capability",
            ));
        }
        let pixels = u64::from(width) * u64::from(height);
        let scene_bytes = pixels
            .checked_mul(SCENE_DEPTH_BYTES_PER_SAMPLE * u64::from(samples))
            .ok_or(HdrError3D::Target("scene byte overflow"))?;
        let final_bytes = pixels
            .checked_mul(FINAL_PAIR_BYTES_PER_PIXEL)
            .ok_or(HdrError3D::Target("final byte overflow"))?;
        let total = scene_bytes
            .checked_add(final_bytes)
            .ok_or(HdrError3D::Target("attachment byte overflow"))?;
        // The complete target is qualified once, before partition or allocation.
        budget(
            PolicyBudget3D::SingleTargetAttachmentBytes,
            total
                .checked_add(private_bytes)
                .ok_or(HdrError3D::Target("combined HAL381 byte overflow"))?,
            policy.max_single_target_attachment_bytes,
        )?;
        if active
            .as_ref()
            .is_some_and(|a| !a.active_for(context, pixels))
        {
            return Err(HdrError3D::Target("foreign HAL381 active-target credit"));
        }
        let full_charge =
            context.reserve_target_budget(if active.is_some() { 0 } else { pixels }, total)?;
        let uniform_bytes =
            u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment).max(16);
        if uniform_bytes > gpu.device.limits().max_buffer_size
            || gpu.device.limits().max_uniform_buffer_binding_size < 16
        {
            return Err(HdrError3D::Target("Tone16 Device buffer limits"));
        }
        budget(
            PolicyBudget3D::DynamicStorageBytesPerTarget,
            uniform_bytes,
            policy.max_dynamic_storage_bytes_per_target,
        )?;
        budget(
            PolicyBudget3D::DynamicUploadBytesPerFrame,
            16,
            policy.max_dynamic_upload_bytes_per_frame,
        )?;
        budget(
            PolicyBudget3D::UploadBytesPerFrame,
            16,
            policy.max_upload_bytes_per_frame,
        )?;
        let uniform_charge = context.reserve_dynamic_storage(
            uniform_bytes,
            0,
            1,
            16 + std::mem::size_of::<HdrWork3D>() as u64,
        )?;
        let program = ToneProgram3D::new(context, samples)?;
        let (new_active, mut scene_charge, final_charge) =
            full_charge.split_hdr(scene_bytes, final_bytes);
        let depth_charge = scene_charge.split_attachment(pixels * 4 * u64::from(samples));
        let active = active.unwrap_or_else(|| Arc::new(new_active));
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let textures = std::array::from_fn(|_| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Halcyon private R027 R/G/B/Coverage scene plane"),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: SCENE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        });
        let scene = HdrScene3D {
            views: std::array::from_fn(|i| textures[i].create_view(&Default::default())),
            _textures: textures,
        };
        let depth = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Halcyon private R027 reverse-Z depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Halcyon R027 Tone16 slot"),
            size: uniform_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Halcyon R027 tone source"),
            layout: &program.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform,
                        offset: 0,
                        size: NonZeroU64::new(16),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&scene.views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&scene.views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&scene.views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&scene.views[3]),
                },
            ],
        });
        let pass = CustomRenderPass::new_private_hdr_final(
            WidgetInternalId::new().raw(),
            gpu.clone(),
            PhysicalRect::new(0, 0, width, height),
            Box::new(final_charge),
        );
        let validation = pollster::block_on(validation_scope.pop());
        let allocation = pollster::block_on(allocation_scope.pop());
        let pass = pass.map_err(HdrError3D::Target)?;
        let result = Self {
            pass: Some(pass),
            active: Some(active),
            gpu,
            extent: (width, height),
            samples,
            #[cfg(any(test, feature = "test-harness"))]
            timing: None,
            work: Some(HdrWork3D {
                scene,
                depth: HdrDepth3D::Attachment {
                    _texture: depth,
                    view: depth_view,
                    _charge: depth_charge,
                },
                uniform,
                binding,
                program,
                _scene_charge: scene_charge,
                _uniform_charge: uniform_charge,
            }),
        };
        if let Some(error) = validation.or(allocation) {
            return Err(HdrError3D::Shader(error.to_string()));
        }
        Ok(result)
    }

    pub(super) fn hud_depth(&self) -> Option<&Arc<super::spatial_hud::HudDepth3D>> {
        match &self.work.as_ref()?.depth {
            HdrDepth3D::Sampled(depth) => Some(depth),
            HdrDepth3D::Attachment { .. } => None,
        }
    }

    pub(super) fn tone_storage_bytes(&self) -> u64 {
        self.work.as_ref().expect("live HDR work").uniform.size()
    }

    pub(super) fn qualify_hud_depth(
        &mut self,
        context: &Context3D,
        direct: bool,
    ) -> super::PbrSourceResult3D<()> {
        if self.hud_depth().is_none() {
            let depth =
                super::spatial_hud::HudDepth3D::new(context, self.extent, self.samples, direct)?;
            let work = self.work.as_mut().expect("live canonical HDR work");
            let previous = std::mem::replace(&mut work.depth, HdrDepth3D::Sampled(depth));
            // Only the new attachment is used after this point. The old target
            // attachment stays charged until the last earlier writer completes.
            crate::renderer::on_submitted_work_done(&self.gpu.queue, move || drop(previous));
        }
        Ok(())
    }

    pub(crate) fn texture_id(&self) -> u64 {
        self.pass.as_ref().expect("live HDR output").texture_id()
    }

    pub(super) fn active_credit(&self) -> Option<Arc<TargetBudgetLease3D>> {
        self.active.clone()
    }
    pub(crate) fn back_writable(&self) -> bool {
        self.pass.as_ref().is_some_and(|p| p.hdr_back_writable())
    }

    pub(crate) fn is_live(&self) -> bool {
        self.pass.is_some() && self.gpu.private_hdr_owner_live()
    }

    pub(crate) fn pin_front(
        &mut self,
    ) -> Result<crate::renderer::custom_render_pass::HdrFrontPin, HdrError3D> {
        self.pass
            .as_mut()
            .ok_or(HdrError3D::Target("retired output"))?
            .pin_hdr_front()
            .map_err(|_| {
                HdrError3D::Target(
                    "canonical front retirement or pending write prevents immutable pin",
                )
            })
    }

    pub(crate) fn matches_pin(
        &self,
        pin: &crate::renderer::custom_render_pass::HdrFrontPin,
    ) -> bool {
        self.is_live()
            && self
                .pass
                .as_ref()
                .is_some_and(|pass| pass.matches_hdr_pin(pin))
    }

    /// All qualification/reservation is complete before entering this producer
    /// callback. It writes only the selected scene/depth, never final or registry.
    pub(crate) fn render(
        &mut self,
        source: &HdrSource3D,
        write_scene: impl FnOnce(&mut wgpu::CommandEncoder, &HdrScene3D, &wgpu::TextureView),
    ) -> Result<(), HdrError3D> {
        self.render_with_submission_stage(source, write_scene)
            .map(|_| ())
            .map_err(|(error, _)| error)
    }

    pub(crate) fn render_with_submission_stage(
        &mut self,
        source: &HdrSource3D,
        write_scene: impl FnOnce(&mut wgpu::CommandEncoder, &HdrScene3D, &wgpu::TextureView),
    ) -> Result<HdrSubmissionStage3D, (HdrError3D, HdrSubmissionStage3D)> {
        self.render_staged(
            source,
            |encoder, scene, depth, _| write_scene(encoder, scene, depth),
            None,
            #[cfg(test)]
            || {},
            #[cfg(test)]
            || {},
        )
    }

    pub(crate) fn render_encoder_bound(
        &mut self,
        source: &HdrSource3D,
        completion: Arc<HdrWriterCompletion3D>,
        write_scene: impl FnOnce(
            &mut wgpu::CommandEncoder,
            &HdrScene3D,
            &wgpu::TextureView,
            &wgpu::Buffer,
            &[u8; 16],
        ),
    ) -> Result<HdrSubmissionStage3D, (HdrError3D, HdrSubmissionStage3D)> {
        self.render_staged(
            source,
            |encoder, scene, depth, tone| {
                let (destination, bytes) = tone.expect("selected encoder-bound tone");
                write_scene(encoder, scene, depth, destination, bytes);
            },
            Some(completion),
            #[cfg(test)]
            || {},
            #[cfg(test)]
            || {},
        )
    }

    #[cfg(test)]
    pub(crate) fn render_encoder_bound_interposed_for_test(
        &mut self,
        source: &HdrSource3D,
        completion: Arc<HdrWriterCompletion3D>,
        write_scene: impl FnOnce(
            &mut wgpu::CommandEncoder,
            &HdrScene3D,
            &wgpu::TextureView,
            &wgpu::Buffer,
            &[u8; 16],
        ),
        before_submit: impl FnOnce(),
        before_registration: impl FnOnce(),
    ) -> Result<HdrSubmissionStage3D, (HdrError3D, HdrSubmissionStage3D)> {
        self.render_staged(
            source,
            |encoder, scene, depth, tone| {
                let (destination, bytes) = tone.expect("selected encoder-bound tone");
                write_scene(encoder, scene, depth, destination, bytes);
            },
            Some(completion),
            before_submit,
            before_registration,
        )
    }

    #[cfg(test)]
    pub(crate) fn render_interposed_for_test(
        &mut self,
        source: &HdrSource3D,
        write_scene: impl FnOnce(&mut wgpu::CommandEncoder, &HdrScene3D, &wgpu::TextureView),
        before_submit: impl FnOnce(),
        before_registration: impl FnOnce(),
    ) -> Result<HdrSubmissionStage3D, (HdrError3D, HdrSubmissionStage3D)> {
        self.render_staged(
            source,
            |encoder, scene, depth, _| write_scene(encoder, scene, depth),
            None,
            before_submit,
            before_registration,
        )
    }

    fn render_staged(
        &mut self,
        source: &HdrSource3D,
        write_scene: impl FnOnce(
            &mut wgpu::CommandEncoder,
            &HdrScene3D,
            &wgpu::TextureView,
            Option<(&wgpu::Buffer, &[u8; 16])>,
        ),
        completion: Option<Arc<HdrWriterCompletion3D>>,
        #[cfg(test)] after_encode: impl FnOnce(),
        #[cfg(test)] after_submit: impl FnOnce(),
    ) -> Result<HdrSubmissionStage3D, (HdrError3D, HdrSubmissionStage3D)> {
        use HdrSubmissionStage3D::{
            BeforeWrites, Published, SubmittedSwappedUnpublished, WritesQueuedNoFrameSubmit,
        };
        #[cfg(any(test, feature = "test-harness"))]
        let timing_start = self.timing.as_ref().map(|_| std::time::Instant::now());
        let e = source.exposure().map_err(|error| (error, BeforeWrites))?;
        let pass = self
            .pass
            .as_mut()
            .ok_or((HdrError3D::Target("retired output"), BeforeWrites))?;
        let mut frame: CustomRenderFrame = pass
            .begin_frame(Some("Halcyon R027 scene -> tone"))
            .map_err(|error| (match error {
                crate::renderer::custom_render_pass::CustomRenderPassError::ContentPinned=>HdrError3D::PinnedFront,
                _=>HdrError3D::Target("retired renderer/target"),
            }, BeforeWrites))?;
        let work = self.work.as_ref().expect("live HDR work");
        let values = [e, 0.0, 0.0, 0.0];
        if completion.is_none() {
            self.gpu
                .queue
                .write_buffer(&work.uniform, 0, bytemuck::cast_slice(&values));
        }
        let (encoder, final_back) = frame.encoder_and_view();
        let tone_bytes = bytemuck::cast::<[f32; 4], [u8; 16]>(values);
        write_scene(
            encoder,
            &work.scene,
            work.depth.view(),
            completion.as_ref().map(|_| (&work.uniform, &tone_bytes)),
        );
        {
            let mut tone = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Halcyon R027 per-sample tone before coverage average"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: final_back,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                multiview_mask: None,
                occlusion_query_set: None,
            });
            tone.set_pipeline(&work.program.pipeline);
            tone.set_bind_group(0, &work.binding, &[]);
            tone.draw(0..3, 0..1);
        }
        #[cfg(any(test, feature = "test-harness"))]
        let before_submit = timing_start.map(|start| start.elapsed());
        #[cfg(test)]
        after_encode();
        let swaps_before = pass.swap_count();
        if pass.submit_and_swap_deferred_registration(frame).is_err() {
            debug_assert_eq!(pass.swap_count(), swaps_before);
            return Err((
                HdrError3D::Target("renderer retired before output publication"),
                WritesQueuedNoFrameSubmit,
            ));
        }
        debug_assert_eq!(pass.swap_count().wrapping_sub(swaps_before), 1);
        if let Some(completion) = completion {
            completion.record(&self.gpu.queue);
        }
        #[cfg(test)]
        after_submit();
        pass.queue_front_registration().map_err(|_| {
            (
                HdrError3D::Target("renderer retired before output publication"),
                SubmittedSwappedUnpublished,
            )
        })?;
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start), Some(before)) =
            (self.timing.as_mut(), timing_start, before_submit)
        {
            t.encode_cpu = before;
            t.submit_swap_cpu = start.elapsed() - before;
            t.complete = true;
        }
        Ok(Published)
    }

    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn time_next_render_for_test(&mut self) {
        self.timing = Some(HdrRenderTiming3D::default());
    }

    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn take_render_timing_for_test(&mut self) -> Option<HdrRenderTiming3D> {
        self.timing.take()
    }

    #[cfg(test)]
    pub(crate) fn front_for_test(&self) -> &wgpu::Texture {
        self.pass
            .as_ref()
            .unwrap()
            .front_texture_for_test()
            .unwrap()
    }

    #[cfg(test)]
    pub(crate) fn scene_planes_for_test(&self) -> &[wgpu::Texture; 4] {
        &self
            .work
            .as_ref()
            .expect("live complete scene")
            .scene
            ._textures
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn front_for_test_harness(&self) -> Result<&wgpu::Texture, HdrError3D> {
        self.pass
            .as_ref()
            .ok_or(HdrError3D::Target("retired output"))?
            .front_texture_for_test()
            .map_err(|_| HdrError3D::Target("withdrawn final front"))
    }
    #[cfg(test)]
    pub(crate) fn swaps_for_test(&self) -> u64 {
        self.pass.as_ref().unwrap().swap_count()
    }
}

impl Drop for HdrOutput3D {
    fn drop(&mut self) {
        // Logical activity ends now; scene work and final custody have different
        // completion owners. No callback retains a Device/Queue cycle.
        drop(self.active.take());
        if let Some(pass) = self.pass.take() {
            let _ = pass.destroy_deferred();
        }
        if let Some(work) = self.work.take() {
            crate::renderer::on_submitted_work_done(&self.gpu.queue, move || drop(work));
        }
    }
}
