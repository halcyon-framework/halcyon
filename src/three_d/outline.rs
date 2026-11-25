//! HAL374: explicit source occurrence selection and one private visible Outline.
//! Canonical PBR/MRT4 and semantic selection remain owned by their existing paths.

use std::num::NonZeroU64;
use std::sync::{Arc, Weak};

use super::{Context3D, PbrScene3D, PbrSourceError3D, PbrSourceResult3D};

pub(crate) const MAX_EXACT_TAG: usize = 16_777_216;

fn refused(cause: &'static str) -> PbrSourceError3D {
    PbrSourceError3D::Source {
        field: "HAL374 outline",
        cause,
    }
}

fn live_context(context: &Context3D) -> PbrSourceResult3D<()> {
    if let Some(failure) = context.device_failure() {
        return Err(PbrSourceError3D::Gpu(format!("{failure:?}")));
    }
    if !context.gpu_resources()?.private_hdr_owner_live() {
        return Err(PbrSourceError3D::Gpu(
            "HAL374 renderer owner retired".into(),
        ));
    }
    Ok(())
}

struct SelectionSource {
    context: super::context::ContextId,
    scene: Arc<super::pbr::MaterialSceneSource3D>,
    membership: Arc<Membership>,
}
struct Membership {
    slots: Vec<u32>,
    selected: Vec<u32>,
    _charge: super::context::DynamicStorageLease3D,
}

/// HAL-issued membership in exact packet occurrences of one admitted PBR scene.
/// The caller owns the semantic mapping; a geometry handle or ordinal is no ID.
#[derive(Clone)]
pub struct PbrOutlineSelection3D(Arc<SelectionSource>);

impl std::fmt::Debug for PbrOutlineSelection3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PbrOutlineSelection3D")
            .field("packet_slots", &self.0.membership.slots)
            .finish()
    }
}

impl PbrOutlineSelection3D {
    pub fn packet_slots(&self) -> &[u32] {
        &self.0.membership.slots
    }
    pub fn source(&self) -> PbrScene3D {
        PbrScene3D(self.0.scene.clone())
    }
    pub(crate) fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.scene, &other.0.scene)
            && self.0.membership.slots == other.0.membership.slots
    }
    pub(crate) fn matches_source(&self, source: &Arc<super::pbr::MaterialSceneSource3D>) -> bool {
        Arc::ptr_eq(&self.0.scene, source)
    }
    pub(crate) fn current(&self, context: &Context3D) -> PbrSourceResult3D<()> {
        live_context(context)?;
        if !self.0.context.same_as(&context.context_identity()) {
            return Err(super::Context3DError::ContextMismatch.into());
        }
        self.0.scene.snapshot.validate_live_resources(context)?;
        Ok(())
    }
}

/// Opaque receipt for a viewport's selected request, never authority to rebind it.
#[derive(Debug, Clone)]
pub struct PbrOutlineRequest3D {
    pub(crate) selection: PbrOutlineSelection3D,
    pub(crate) epoch: u64,
}
impl PbrOutlineRequest3D {
    pub fn selection(&self) -> &PbrOutlineSelection3D {
        &self.selection
    }
}

impl Context3D {
    /// Admit caller-selected occurrence slots under HAL374 §8.1.
    /// Does not choose semantic membership or allocate a render target.
    pub fn admit_pbr_outline_selection(
        &self,
        scene: &PbrScene3D,
        packet_slots: &[u32],
    ) -> PbrSourceResult3D<PbrOutlineSelection3D> {
        live_context(self)?;
        scene.0.validate(self)?;
        scene.0.snapshot.validate_live_resources(self)?;
        let n = scene.0.snapshot.packets.len();
        if n == 0 || n > MAX_EXACT_TAG || packet_slots.is_empty() || packet_slots.len() > n {
            return Err(refused("empty, oversized or inexact occurrence set"));
        }
        for packet in scene.0.snapshot.packets.iter() {
            let super::snapshot::CommittedMaterialPacket3D::PbrMaterial {
                geometry, material, ..
            } = packet
            else {
                return Err(refused("complete winner replay requires PBR packets only"));
            };
            if material.data.source.alpha_mode != super::pbr::PbrAlphaMode3D::Opaque
                || geometry.data.source.certified_input().is_none()
            {
                return Err(refused(
                    "complete winner replay requires certified opaque geometry/materials",
                ));
            }
        }
        let bytes = n
            .checked_add(packet_slots.len())
            .and_then(|v| v.checked_mul(4))
            .and_then(|v| {
                v.checked_add(
                    std::mem::size_of::<SelectionSource>()
                        + std::mem::size_of::<Membership>()
                        + 4 * std::mem::size_of::<usize>(),
                )
            })
            .ok_or_else(|| refused("selection storage overflow"))?;
        let charge = self.reserve_dynamic_storage(0, 0, 0, bytes as u64)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(packet_slots.len())
            .map_err(|_| refused("selection allocation"))?;
        slots.extend_from_slice(packet_slots);
        slots.sort_unstable();
        if slots.last().is_none_or(|&s| s as usize >= n) || slots.windows(2).any(|s| s[0] == s[1]) {
            return Err(refused(
                "foreign, duplicate or out-of-range occurrence slot",
            ));
        }
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(n)
            .map_err(|_| refused("membership allocation"))?;
        selected.resize(n, 0);
        for &slot in &slots {
            selected[slot as usize] = 1;
        }
        Ok(PbrOutlineSelection3D(Arc::new(SelectionSource {
            context: self.context_identity(),
            scene: scene.0.clone(),
            membership: Arc::new(Membership {
                slots,
                selected,
                _charge: charge,
            }),
        })))
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct EdgeVertex {
    a: [f32; 3],
    b: [f32; 3],
    corner: [f32; 2],
    slot: u32,
}
const _: [(); 36] = [(); std::mem::size_of::<EdgeVertex>()];

pub(super) struct OutlineDraw {
    pub geometry: Arc<super::raster::PbrGeometryGpu3D>,
    pub source: super::pbr::PbrGeometryLease3D,
    pub object_offset: u32,
    pub cull: bool,
    pub edge_vertices: std::ops::Range<u32>,
}

struct Programs {
    context: super::context::ContextId,
    view: wgpu::BindGroupLayout,
    object: wgpu::BindGroupLayout,
    tag: wgpu::BindGroupLayout,
    edge_input: wgpu::BindGroupLayout,
    compose_input: wgpu::BindGroupLayout,
    replay: [wgpu::RenderPipeline; 2],
    edge: wgpu::RenderPipeline,
    compose: wgpu::RenderPipeline,
    _modules: super::context::ShaderModuleResidency,
    _pipelines: Vec<super::raster::ControlledPipelineResidency>,
}

impl Programs {
    fn new(context: &Context3D, samples: u32) -> PbrSourceResult3D<Arc<Self>> {
        let gpu = context.gpu_resources()?;
        let device = &gpu.device;
        let limits = device.limits();
        if !matches!(samples, 1 | 4)
            || limits.max_bind_groups < 3
            || limits.max_vertex_attributes < 4
            || limits.max_vertex_buffer_array_stride < 36
            || limits.max_uniform_buffer_binding_size < 112
            || limits.max_uniform_buffers_per_shader_stage < 3
            || limits.max_sampled_textures_per_shader_stage < 2
            || limits.max_storage_buffers_per_shader_stage < 2
        {
            return Err(refused("private layout/sample capability"));
        }
        for (format, usage, count) in [
            (
                wgpu::TextureFormat::R32Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                samples,
            ),
            (
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                samples,
            ),
            (
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                samples,
            ),
            (
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                1,
            ),
        ] {
            let f = format.guaranteed_format_features(device.features());
            if !f.allowed_usages.contains(usage) || !f.flags.sample_count_supported(count) {
                return Err(refused("private format/sample capability"));
            }
        }
        let modules = context.reserve_controlled_shader_modules(3)?;
        let raster = context.raster_gpu()?;
        let mut pipelines = Vec::new();
        for _ in 0..4 {
            pipelines.push(
                raster
                    .reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?,
            );
        }
        let sources = [
            replay_shader(),
            edge_shader(samples),
            compose_shader(samples),
        ];
        for source in &sources {
            let ir = naga::front::wgsl::parse_str(source)
                .map_err(|e| PbrSourceError3D::Gpu(e.emit_to_string(source)))?;
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::MULTISAMPLED_SHADING,
            )
            .validate(&ir)
            .map_err(|e| PbrSourceError3D::Gpu(e.to_string()))?;
        }
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shaders: Vec<_> = sources
            .iter()
            .map(|source| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("HAL374 distinct private program"),
                    source: wgpu::ShaderSource::Wgsl(source.as_str().into()),
                })
            })
            .collect();
        let layout = |entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HAL374 private layout"),
                entries,
            })
        };
        let view = layout(&[uniform(0, 80, false)]);
        let object = layout(&[uniform(0, 112, true)]);
        let tag = layout(&[uniform(0, 16, true)]);
        let edge_input = layout(&[
            texture(0, samples),
            storage(1, true),
            uniform(2, 16, false),
            storage(3, false),
        ]);
        let compose_input = layout(&[texture(0, samples), texture(1, 1), storage(2, true)]);
        let replay_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HAL374 winner layout"),
            bind_group_layouts: &[Some(&view), Some(&object), Some(&tag)],
            immediate_size: 0,
        });
        let edge_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HAL374 edge layout"),
            bind_group_layouts: &[Some(&view), Some(&object), Some(&edge_input)],
            immediate_size: 0,
        });
        let compose_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HAL374 OVER-F layout"),
            bind_group_layouts: &[Some(&compose_input)],
            immediate_size: 0,
        });
        let replay = std::array::from_fn(|cull| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("HAL374 opaque winner replay"),
                layout: Some(&replay_layout),
                vertex: wgpu::VertexState {
                    module: &shaders[0],
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 52,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: (cull == 1).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shaders[0],
                    entry_point: Some("fragment"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::R32Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        let maximum = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        };
        let edge = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HAL374 analytic visible outer stroke MAX"), layout: Some(&edge_layout),
            vertex: wgpu::VertexState { module: &shaders[1], entry_point: Some("vertex"), compilation_options: Default::default(), buffers: &[
                Some(wgpu::VertexBufferLayout { array_stride: 36, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Uint32] }),
            ]},
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Greater), stencil: Default::default(), bias: Default::default() }),
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shaders[1], entry_point: Some("fragment"), compilation_options: Default::default(), targets: &[
                Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba16Float, blend: Some(wgpu::BlendState { color: maximum, alpha: maximum }), write_mask: wgpu::ColorWrites::ALL }),
            ]}), multiview_mask: None, cache: None,
        });
        let compose = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HAL374 display-space OVER original F"),
            layout: Some(&compose_layout),
            vertex: wgpu::VertexState {
                module: &shaders[2],
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shaders[2],
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        if let Some(e) = pollster::block_on(validation_scope.pop()) {
            return Err(PbrSourceError3D::Gpu(e.to_string()));
        }
        let pipelines = pipelines
            .into_iter()
            .map(|mut p| {
                p.commit();
                p
            })
            .collect();
        Ok(Arc::new(Self {
            context: context.context_identity(),
            view,
            object,
            tag,
            edge_input,
            compose_input,
            replay,
            edge,
            compose,
            _modules: modules.commit_all(),
            _pipelines: pipelines,
        }))
    }
}

fn uniform(binding: u32, size: u64, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: NonZeroU64::new(size),
        },
        count: None,
    }
}
fn texture(binding: u32, samples: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: samples == 4,
        },
        count: None,
    }
}
fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(4),
        },
        count: None,
    }
}

const TRANSFORM: &str = r#"
struct View { view_projection:mat4x4<f32>, camera_relative:vec3<f32>, pad:f32 }
struct Object { model_relative:mat4x4<f32>, normal_to_world:mat3x3<f32> }
@group(0) @binding(0) var<uniform> view:View;
@group(1) @binding(0) var<uniform> object:Object;
"#;
fn replay_shader() -> String {
    format!(
        "{TRANSFORM}\n{}",
        r#"
struct Tag { value:vec4<f32> }
@group(2) @binding(0) var<uniform> tag:Tag;
@vertex fn vertex(@location(0) position:vec3<f32>)->@builtin(position) vec4<f32> {
 let p=object.model_relative*vec4(position,1.0); return view.view_projection*p;
}
@fragment fn fragment()->@location(0) f32 {return tag.value.x;}
"#
    )
}

fn edge_shader(samples: u32) -> String {
    let ty = if samples == 4 {
        "texture_multisampled_2d<f32>"
    } else {
        "texture_2d<f32>"
    };
    let load = if samples == 4 {
        "textureLoad(tags,p,i32(s)).x"
    } else {
        "textureLoad(tags,p,0).x"
    };
    // N1's actual raster centre is already available without interpolation.
    // N4 requires the sample-qualified linear varying, not the pixel centroid.
    let physical = if samples == 1 {
        "i.position.xy"
    } else {
        "i.point"
    };
    format!(
        r#"{TRANSFORM}
@group(2) @binding(0) var tags:{ty};
@group(2) @binding(1) var<storage,read> selected:array<u32>;
struct Size {{ value:vec4<u32> }}
@group(2) @binding(2) var<uniform> size:Size;
@group(2) @binding(3) var<storage,read_write> invalid:atomic<u32>;
struct Edge {{
 @builtin(position) position:vec4<f32>,
 @location(0) @interpolate(flat) a:vec2<f32>,
 @location(1) @interpolate(flat) b:vec2<f32>,
 @location(2) @interpolate(flat) z:vec2<f32>,
 @location(3) @interpolate(flat) slot:u32,
 @location(4) @interpolate(linear,sample) point:vec2<f32>,
}}
@vertex fn vertex(@location(0) a:vec3<f32>,@location(1) b:vec3<f32>,
 @location(2) corner:vec2<f32>,@location(3) slot:u32)->Edge {{
 let p0=object.model_relative*vec4(a,1.0);let p1=object.model_relative*vec4(b,1.0);
 let c0=view.view_projection*p0;let c1=view.view_projection*p1;
 let q0=c0.xyz/c0.w;let q1=c1.xyz/c1.w;
 let extent=vec2<f32>(size.value.xy);
 let x0=(q0.xy*vec2(0.5,-0.5)+vec2(0.5))*extent;
 let x1=(q1.xy*vec2(0.5,-0.5)+vec2(0.5))*extent;
 let t=normalize(x1-x0);let n=vec2(-t.y,t.x);
 let p=mix(x0,x1,corner.x)+(2.0*corner.x-1.0)*2.0*t+2.0*corner.y*n;
 var o:Edge;o.position=vec4((p/extent-vec2(0.5))*vec2(2.0,-2.0),mix(q0.z,q1.z,corner.x),1.0);
 o.a=x0;o.b=x1;o.z=vec2(q0.z,q1.z);o.slot=slot;o.point=p;return o;
}}
fn decode(p:vec2<i32>,s:u32)->u32 {{
 let f={load};
 if f==0.0 {{return 0u;}}
 if !(f>0.0)||!(f<=f32(size.value.w)) {{atomicStore(&invalid,1u);return 0u;}}
 let tag=u32(f);if f32(tag)!=f {{atomicStore(&invalid,1u);return 0u;}}return tag;
}}
struct Stroke {{@location(0) color:vec4<f32>,@builtin(frag_depth) depth:f32}}
@fragment fn fragment(i:Edge,@builtin(sample_index) sample:u32)->Stroke {{
 let physical={physical};let p=vec2<i32>(floor(physical));
 let destination=decode(p,sample);
 if destination!=0u {{if selected[destination-1u]!=0u {{discard;}}}}
 var nearby=false;
 for(var y=-2;y<=2;y++){{for(var x=-2;x<=2;x++){{
  if x*x+y*y>4 {{continue;}}
  let q=p+vec2(x,y);if any(q<vec2(0))||any(q>=vec2<i32>(size.value.xy)) {{continue;}}
  for(var s=0u;s<{samples}u;s++){{if decode(q,s)==i.slot+1u {{nearby=true;}}}}
 }}}}
 if !nearby {{discard;}}
 let d=i.b-i.a;let t=clamp(dot(physical-i.a,d)/dot(d,d),0.0,1.0);
 let distance=length(physical-(i.a+t*d));if !(distance<2.0) {{discard;}}
 let h=clamp(distance-1.0,0.0,1.0);let a=1.0-h*h*(3.0-2.0*h);
 var o:Stroke;o.color=vec4(vec3(0.24,0.62,0.84)*a,a);o.depth=mix(i.z.x,i.z.y,t);return o;
}}
"#
    )
}

fn compose_shader(samples: u32) -> String {
    let ty = if samples == 4 {
        "texture_multisampled_2d<f32>"
    } else {
        "texture_2d<f32>"
    };
    let load = if samples == 4 {
        "textureLoad(stroke,p,i32(s))"
    } else {
        "textureLoad(stroke,p,0)"
    };
    format!(
        r#"
@group(0) @binding(0) var stroke:{ty};
@group(0) @binding(1) var front:texture_2d<f32>;
@group(0) @binding(2) var<storage,read> invalid:array<u32>;
@vertex fn vertex(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {{
 var p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));return vec4(p[i],0.,1.);
}}
@fragment fn fragment(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {{
 let p=vec2<i32>(position.xy);let f=textureLoad(front,p,0);
 if invalid[0]!=0u {{return f;}}
 var sum=vec4(0.0);for(var s=0u;s<{samples}u;s++){{sum+={load};}}
 let overlay=sum/{samples}.0;if overlay.a==0.0 {{return f;}}
 return overlay+(1.0-overlay.a)*f;
}}
"#
    )
}

struct ReplayCapture {
    view: wgpu::BindGroup,
    objects: wgpu::BindGroup,
    tags: wgpu::BindGroup,
    draws: Vec<OutlineDraw>,
    tag_stride: u32,
    _charge: super::context::DynamicStorageLease3D,
}

struct GpuWork {
    programs: Arc<Programs>,
    _tags: wgpu::Texture,
    tags_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    _stroke: wgpu::Texture,
    stroke_view: wgpu::TextureView,
    _output: wgpu::Texture,
    edges: wgpu::Buffer,
    invalid: wgpu::Buffer,
    _uniforms: wgpu::Buffer,
    _tags_uniform: wgpu::Buffer,
    _membership_buffer: wgpu::Buffer,
    edge_binding: wgpu::BindGroup,
    capture: std::sync::Mutex<Option<ReplayCapture>>,
    encoded: std::sync::atomic::AtomicBool,
    _membership: Arc<Membership>,
    _source: Weak<super::pbr::MaterialSceneSource3D>,
    _context: super::context::ContextId,
    _attachments: super::context::TargetBudgetLease3D,
    _geometry: super::context::GeometryGpuResidency,
    _storage: super::context::DynamicStorageLease3D,
}

pub(crate) struct OutlineGpuTarget {
    gpu: crate::renderer::wgpu_renderer::GpuResources,
    work: Arc<GpuWork>,
    pub(crate) output: crate::renderer::wgpu_renderer::PresenceOutput,
    #[allow(
        dead_code,
        reason = "Retained target extent and attachment accounting facts"
    )]
    pub(crate) extent: (u32, u32),
    pub(crate) samples: u32,
    #[allow(
        dead_code,
        reason = "Retained target extent and attachment accounting facts"
    )]
    pub(crate) attachment_bytes: u64,
    composed: bool,
}

impl Drop for OutlineGpuTarget {
    fn drop(&mut self) {
        // Actual private resources/charges, never Context/Device/Queue or Scene,
        // survive pending original-encoder writes and all following GPU reads.
        let work = self.work.clone();
        crate::renderer::on_submitted_work_done(&self.gpu.queue, move || drop(work));
    }
}

fn arithmetic() -> PbrSourceError3D {
    refused("checked private resource arithmetic overflow")
}
fn limit(
    context: &Context3D,
    kind: super::PolicyBudget3D,
    requested: u64,
    maximum: u64,
) -> PbrSourceResult3D<()> {
    super::pbr::budget(context, kind, requested, maximum)?;
    Ok(())
}

fn matrix(bytes: &[u8]) -> [[f32; 4]; 4] {
    std::array::from_fn(|c| {
        std::array::from_fn(|r| {
            f32::from_ne_bytes(
                bytes[(c * 4 + r) * 4..(c * 4 + r + 1) * 4]
                    .try_into()
                    .expect("checked fixed matrix"),
            )
        })
    })
}
fn multiply(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|r| ((m[0][r] * v[0] + m[1][r] * v[1]) + m[2][r] * v[2]) + m[3][r] * v[3])
}
fn projected(
    p: [f32; 3],
    object: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    extent: (u32, u32),
) -> PbrSourceResult3D<[f32; 2]> {
    let q = multiply(view, multiply(object, [p[0], p[1], p[2], 1.0]));
    if q.iter().any(|v| !v.is_finite()) || q[3] <= 0.0 || q[2] <= 0.0 || q[2] >= q[3] {
        return Err(refused(
            "near/far-plane or nonfinite source edge is outside qualified projection",
        ));
    }
    let point = [
        (q[0] / q[3] * 0.5 + 0.5) * extent.0 as f32,
        (0.5 - q[1] / q[3] * 0.5) * extent.1 as f32,
    ];
    if point.iter().any(|v| !v.is_finite()) {
        return Err(refused("nonfinite physical source edge"));
    }
    Ok(point)
}

impl OutlineGpuTarget {
    /// All source buffers are the actual canonical retained buffers. This capture
    /// is consumed in that canonical encoder, not used to synthesize a new scene.
    pub(super) fn new(
        context: &Context3D,
        selection: &PbrOutlineSelection3D,
        extent: (u32, u32),
        view_bytes: &[u8],
        object_bytes: &[u8],
        view_buffer: &wgpu::Buffer,
        objects_buffer: &wgpu::Buffer,
        mut draws: Vec<OutlineDraw>,
        capture_charge: super::context::DynamicStorageLease3D,
        previous: Option<&Self>,
        canonical_upload_bytes: u64,
    ) -> PbrSourceResult3D<Self> {
        use wgpu::util::DeviceExt;
        selection.current(context)?;
        let source = &selection.0.scene;
        let n = source.samples;
        let gpu = context.gpu_resources()?;
        let policy = context.limits().policy;
        let device_limits = gpu.device.limits();
        if extent.0 == 0
            || extent.1 == 0
            || extent.0.max(extent.1) > policy.max_target_dimension_2d
            || extent.0.max(extent.1) > device_limits.max_texture_dimension_2d
            || draws.len() != source.snapshot.packets.len()
            || view_bytes.len() < 80
        {
            return Err(refused("private target/frame shape mismatch"));
        }
        let pixels = u64::from(extent.0)
            .checked_mul(u64::from(extent.1))
            .ok_or_else(arithmetic)?;
        let attachment_bytes = pixels
            .checked_mul(16 * u64::from(n) + 8)
            .ok_or_else(arithmetic)?;
        let canonical = pixels
            .checked_mul(20 * u64::from(n) + 16)
            .ok_or_else(arithmetic)?;
        let combined = attachment_bytes
            .checked_add(canonical)
            .and_then(|b| {
                b.checked_add(if source.direct.is_some() {
                    super::direct_shadow::POOL_BYTES
                } else {
                    0
                })
            })
            .ok_or_else(arithmetic)?;
        limit(
            context,
            super::PolicyBudget3D::SingleTargetAttachmentBytes,
            combined,
            policy.max_single_target_attachment_bytes,
        )?;
        // Canonical HDR owns the active pixel credit. Only new private payload
        // is reserved here; old/in-flight generations keep their actual leases.
        let attachments = context.reserve_target_budget(0, attachment_bytes)?;
        let count = draws.len();
        let triangle_count = draws
            .iter()
            .enumerate()
            .filter(|(i, _)| selection.0.membership.selected[*i] != 0)
            .try_fold(0usize, |sum, (_, d)| {
                sum.checked_add(
                    d.source
                        .data
                        .source
                        .certified_input()
                        .map_or(0, |c| c.triangles.len()),
                )
                .ok_or_else(arithmetic)
            })?;
        let vertex_count = triangle_count.checked_mul(18).ok_or_else(arithmetic)?;
        let vertex_bytes = vertex_count
            .checked_mul(std::mem::size_of::<EdgeVertex>())
            .ok_or_else(arithmetic)? as u64;
        if vertex_count == 0 || vertex_count > u32::MAX as usize {
            return Err(refused("empty or oversized source edge set"));
        }
        let stride = u64::from(device_limits.min_uniform_buffer_offset_alignment)
            .max(16)
            .next_power_of_two();
        let tag_bytes = stride.checked_mul(count as u64).ok_or_else(arithmetic)?;
        let membership_bytes = (count as u64).checked_mul(4).ok_or_else(arithmetic)?;
        let storage_bytes = tag_bytes
            .checked_add(membership_bytes)
            .and_then(|b| b.checked_add(20))
            .ok_or_else(arithmetic)?;
        let uploads = storage_bytes
            .checked_add(vertex_bytes)
            .ok_or_else(arithmetic)?;
        limit(
            context,
            super::PolicyBudget3D::MeshBytes,
            vertex_bytes,
            policy.max_mesh_bytes,
        )?;
        let complete_upload = canonical_upload_bytes
            .checked_add(uploads)
            .ok_or_else(arithmetic)?;
        limit(
            context,
            super::PolicyBudget3D::UploadBytesPerFrame,
            complete_upload,
            policy.max_upload_bytes_per_frame,
        )?;
        let extra_draws = (count as u64)
            .checked_add(selection.packet_slots().len() as u64)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(arithmetic)?;
        limit(
            context,
            super::PolicyBudget3D::DrawPacketsPerSnapshot,
            extra_draws,
            u64::from(policy.max_draw_packets_per_snapshot),
        )?;
        limit(
            context,
            super::PolicyBudget3D::DynamicStorageBytesPerTarget,
            storage_bytes,
            policy.max_dynamic_storage_bytes_per_target,
        )?;
        if vertex_bytes > device_limits.max_buffer_size
            || tag_bytes > device_limits.max_buffer_size
            || membership_bytes > u64::from(device_limits.max_storage_buffer_binding_size)
            || tag_bytes > u64::from(u32::MAX)
        {
            return Err(refused("private buffer/offset Device limit"));
        }
        let cpu_bytes = vertex_bytes
            .checked_add(tag_bytes)
            .and_then(|b| b.checked_add((count as u64).checked_mul(72)?))
            .and_then(|b| {
                b.checked_add(std::mem::size_of::<GpuWork>() as u64
                + crate::renderer::wgpu_renderer::GpuResources::loading_presence_metadata_bytes())
            })
            .ok_or_else(arithmetic)?;
        let storage = context.reserve_dynamic_storage(storage_bytes, count as u64, 1, cpu_bytes)?;
        let geometry = context.reserve_controlled_geometry_bytes(vertex_bytes)?;
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| refused("bounded edge packing capacity"))?;
        let vp = matrix(view_bytes);
        let mut duplicates = Vec::new();
        duplicates
            .try_reserve_exact(count)
            .map_err(|_| refused("source ambiguity check capacity"))?;
        for (slot, draw) in draws.iter_mut().enumerate() {
            let offset = draw.object_offset as usize;
            let object = object_bytes
                .get(offset..offset + 64)
                .ok_or_else(|| refused("canonical Object112 offset"))?;
            let identity = Arc::as_ptr(&draw.source.data) as usize;
            duplicates.push((
                identity,
                <[u8; 64]>::try_from(object).expect("fixed object matrix"),
            ));
            let start = vertices.len() as u32;
            if selection.0.membership.selected[slot] != 0 {
                let input = draw
                    .source
                    .data
                    .source
                    .certified_input()
                    .ok_or_else(|| refused("missing original triangle proof"))?;
                let m = matrix(object);
                for triangle in 0..input.triangles.len() {
                    let mut points = [[0.0; 3]; 3];
                    for (corner, point) in points.iter_mut().enumerate() {
                        let element = triangle * 3 + corner;
                        let index = match &input.original.indices {
                            super::GeometryIndices::Unindexed => element,
                            super::GeometryIndices::U16(v) => usize::from(v[element]),
                            super::GeometryIndices::U32(v) => v[element] as usize,
                        };
                        *point = input.original.vertices[index].position;
                    }
                    for (a, b) in [
                        (points[0], points[1]),
                        (points[1], points[2]),
                        (points[2], points[0]),
                    ] {
                        let p = projected(a, m, vp, extent)?;
                        let q = projected(b, m, vp, extent)?;
                        let dx = q[0] - p[0];
                        let dy = q[1] - p[1];
                        let squared = dx * dx + dy * dy;
                        if !squared.is_finite() || squared == 0.0 {
                            return Err(refused(
                                "degenerate or overflowing projected original edge",
                            ));
                        }
                        for corner in [
                            [0., -1.],
                            [1., -1.],
                            [1., 1.],
                            [0., -1.],
                            [1., 1.],
                            [0., 1.],
                        ] {
                            vertices.push(EdgeVertex {
                                a,
                                b,
                                corner,
                                slot: slot as u32,
                            });
                        }
                    }
                }
            }
            draw.edge_vertices = start..vertices.len() as u32;
        }
        duplicates.sort_unstable();
        if duplicates.windows(2).any(|w| w[0] == w[1]) {
            return Err(refused(
                "duplicate geometry at the same actual transform has undefined depth ties",
            ));
        }
        let programs = match previous.filter(|p| {
            p.samples == n && p.work.programs.context.same_as(&context.context_identity())
        }) {
            Some(p) => p.work.programs.clone(),
            None => Programs::new(context, n)?,
        };
        let mut tag_data = Vec::new();
        tag_data
            .try_reserve_exact(tag_bytes as usize)
            .map_err(|_| refused("constant tag upload capacity"))?;
        tag_data.resize(tag_bytes as usize, 0);
        for slot in 0..count {
            tag_data[slot * stride as usize..slot * stride as usize + 4]
                .copy_from_slice(&((slot + 1) as f32).to_ne_bytes());
        }
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let tex = |format, samples, usage, label| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: extent.0,
                    height: extent.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let read_color =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let tags = tex(
            wgpu::TextureFormat::R32Float,
            n,
            read_color,
            "HAL374 exact occurrence winners",
        );
        let depth = tex(
            wgpu::TextureFormat::Depth32Float,
            n,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            "HAL374 original opaque replay depth",
        );
        let stroke = tex(
            wgpu::TextureFormat::Rgba16Float,
            n,
            read_color,
            "HAL374 outer stroke MAX",
        );
        let output = tex(
            wgpu::TextureFormat::Rgba16Float,
            1,
            read_color | wgpu::TextureUsages::COPY_SRC,
            "HAL374 exclusive OVER-F output",
        );
        let tags_view = tags.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let stroke_view = stroke.create_view(&Default::default());
        let output_view = output.create_view(&Default::default());
        let edges = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HAL374 36-byte original p0/p1/corner/slot edge vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let tags_uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HAL374 exact constant draw tags"),
                contents: &tag_data,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let membership = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HAL374 sealed source membership"),
                contents: bytemuck::cast_slice(&selection.0.membership.selected),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let size = [extent.0, extent.1, n, count as u32];
        let uniforms = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HAL374 actual target and exact tag range"),
                contents: bytemuck::bytes_of(&size),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let invalid = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HAL374 fail-closed decoded-tag witness"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = |layout: &wgpu::BindGroupLayout, buffer: &wgpu::Buffer, bytes| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("HAL374 actual canonical buffer capture"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: NonZeroU64::new(bytes),
                    }),
                }],
            })
        };
        let view_binding = binding(&programs.view, view_buffer, 80);
        let object_binding = binding(&programs.object, objects_buffer, 112);
        let tag_binding = binding(&programs.tag, &tags_uniform, 16);
        let edge_binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HAL374 same-frame winners and source group"),
            layout: &programs.edge_input,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tags_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: membership.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: invalid.as_entire_binding(),
                },
            ],
        });
        let validation = pollster::block_on(validation_scope.pop());
        let allocation = pollster::block_on(allocation_scope.pop());
        if let Some(error) = validation.or(allocation) {
            return Err(PbrSourceError3D::Gpu(error.to_string()));
        }
        let work = Arc::new(GpuWork {
            programs,
            _tags: tags,
            tags_view,
            _depth: depth,
            depth_view,
            _stroke: stroke,
            stroke_view,
            _output: output,
            edges,
            invalid,
            _uniforms: uniforms,
            _tags_uniform: tags_uniform,
            _membership_buffer: membership,
            edge_binding,
            capture: std::sync::Mutex::new(Some(ReplayCapture {
                view: view_binding,
                objects: object_binding,
                tags: tag_binding,
                draws,
                tag_stride: stride as u32,
                _charge: capture_charge,
            })),
            encoded: std::sync::atomic::AtomicBool::new(false),
            _membership: selection.0.membership.clone(),
            _source: Arc::downgrade(source),
            _context: context.context_identity(),
            _attachments: attachments,
            _geometry: geometry.commit(),
            _storage: storage,
        });
        let output = gpu
            .loading_presence_output(
                crate::utils::id::WidgetInternalId::new().raw(),
                output_view,
                work.clone(),
                None,
            )
            .map_err(|e| PbrSourceError3D::Gpu(e.into()))?;
        Ok(Self {
            gpu,
            work,
            output,
            extent,
            samples: n,
            attachment_bytes,
            composed: false,
        })
    }

    /// Consumes original-buffer capture inside the original PBR encoder. Any
    /// capture failure leaves no eligible overlay and cannot fail ordinary PBR.
    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        let capture = match self.work.capture.lock() {
            Ok(mut c) => c.take(),
            Err(_) => None,
        };
        let Some(capture) = capture else { return };
        let w = &self.work;
        encoder.clear_buffer(&w.invalid, 0, None);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HAL374 SAME encoder complete opaque occurrence replay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &w.tags_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &w.depth_view,
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
            pass.set_bind_group(0, &capture.view, &[]);
            for (slot, draw) in capture.draws.iter().enumerate() {
                pass.set_pipeline(&w.programs.replay[usize::from(draw.cull)]);
                pass.set_bind_group(1, &capture.objects, &[draw.object_offset]);
                pass.set_bind_group(2, &capture.tags, &[slot as u32 * capture.tag_stride]);
                pass.set_vertex_buffer(0, draw.geometry.vertex.slice(..));
                if let (Some(index), Some(format)) =
                    (&draw.geometry.index, draw.geometry.index_format)
                {
                    pass.set_index_buffer(index.slice(..), format);
                    pass.draw_indexed(0..draw.geometry.index_count, 0, 0..1);
                } else {
                    pass.draw(0..draw.geometry.vertex_count, 0..1);
                }
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HAL374 actual projected edges over READ-ONLY replay depth"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &w.stroke_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &w.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                multiview_mask: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&w.programs.edge);
            pass.set_bind_group(0, &capture.view, &[]);
            pass.set_bind_group(2, &w.edge_binding, &[]);
            pass.set_vertex_buffer(0, w.edges.slice(..));
            for draw in &capture.draws {
                if !draw.edge_vertices.is_empty() {
                    pass.set_bind_group(1, &capture.objects, &[draw.object_offset]);
                    pass.draw(draw.edge_vertices.clone(), 0..1);
                }
            }
        }
        // Captured base buffers/leases are no longer held by the private target.
        // The original PreparedMaterialScene's same-Queue retirement covers them.
        w.encoded.store(true, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn compose(
        &mut self,
        context: &Context3D,
        pin: &crate::renderer::custom_render_pass::HdrFrontPin,
        tuple: &super::viewport::MaterialViewport3DTuple,
    ) -> PbrSourceResult3D<bool> {
        if self.composed
            || !self.work.encoded.load(std::sync::atomic::Ordering::Acquire)
            || !self.output.can_write()
        {
            return Ok(false);
        }
        if !self.work._context.same_as(&context.context_identity())
            || context.device_failure().is_some()
            || !self.work._source.ptr_eq(&Arc::downgrade(tuple.source()))
            || pin
                .registered()
                .map_err(|e| PbrSourceError3D::Gpu(e.to_string()))?
                .is_none()
        {
            return Err(refused("unregistered or mismatched canonical frame"));
        }
        let capture = context.reserve_dynamic_storage(
            0,
            0,
            0,
            (std::mem::size_of::<OutlineFrontRead>()
                + std::mem::size_of::<wgpu::BindGroup>()
                + 4 * std::mem::size_of::<usize>()) as u64,
        )?;
        let binding = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("HAL374 registered immutable F and same-frame stroke"),
                layout: &self.work.programs.compose_input,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.work.stroke_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(pin.view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.work.invalid.as_entire_binding(),
                    },
                ],
            });
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("HAL374 qualified one-output composition"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HAL374 display-space MAX-stroke OVER original FP16 F"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: self.output.view(),
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
            pass.set_pipeline(&self.work.programs.compose);
            pass.set_bind_group(0, &binding, &[]);
            pass.draw(0..3, 0..1);
        }
        let read = OutlineFrontRead {
            _source: Arc::downgrade(tuple.source()),
            _context: context.context_identity(),
            _request: tuple.request_serial(),
            _target: tuple.target_key(),
            _view: tuple.view(),
            _revision: tuple.view_revision(),
        };
        let retained: Arc<dyn Send + Sync> =
            Arc::new((self.work.clone(), pin.read_lease(), binding, read, capture));
        if self
            .output
            .queue(encoder.finish(), retained)
            .map_err(|e| PbrSourceError3D::Gpu(e.into()))?
        {
            self.composed = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub(crate) fn composed(&self) -> bool {
        self.composed
    }
    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn output_texture(&self) -> &wgpu::Texture {
        &self.work._output
    }
    #[cfg(test)]
    pub(crate) fn winners(&self) -> &wgpu::Texture {
        &self.work._tags
    }
    #[cfg(test)]
    pub(crate) fn strokes(&self) -> &wgpu::Texture {
        &self.work._stroke
    }
}

struct OutlineFrontRead {
    _source: Weak<super::pbr::MaterialSceneSource3D>,
    _context: super::context::ContextId,
    _request: super::Viewport3DRequestSerial,
    _target: super::Viewport3DTargetCompatibilityKey,
    _view: super::ViewDescriptor3,
    _revision: super::Viewport3DViewRevision,
}

#[cfg(test)]
pub(crate) fn validate_programs_for_test(
    context: &Context3D,
    samples: u32,
) -> PbrSourceResult3D<()> {
    Programs::new(context, samples).map(drop)
}
