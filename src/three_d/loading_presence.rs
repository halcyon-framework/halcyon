//! HAL381's source-associated private loading presentation. Producer provenance
//! is an explicit claim; this owner checks geometry/custody, never product facts.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::{GeometryIndices, GeometrySourceDescriptor, ModelPlacement3, WorldPoint3};

pub(crate) const NEUTRAL: [f32; 3] = [0.025, 0.031, 0.040];

/// Trusted disposition supplied by the existing application/motion owner.
/// No OS preference provider or implicit default is inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationMotionPolicy {
    pub allow_nonessential_motion: bool,
}

/// A checked materialization duration for one loading-presence occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadingPresenceTransitionDuration3D(u32);

impl LoadingPresenceTransitionDuration3D {
    pub const DEFAULT: Self = Self(160);

    pub fn try_from_millis(millis: u32) -> Result<Self, LoadingPresenceError3D> {
        match millis {
            80..=3_000 => Ok(Self(millis)),
            _ => Err(LoadingPresenceError3D::Source(
                "transition duration must be within 80..=3000 milliseconds",
            )),
        }
    }

    pub const fn as_millis(self) -> u32 {
        self.0
    }
}

/// Upstream-qualified provenance, not a HAL certificate of derivation. The
/// original digest and the PBR HDR identity are deliberately separate domains.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadingPresenceSource3D {
    pub original_binding: [u8; 32],
    pub source_revision: std::num::NonZeroU64,
    pub source_generation: std::num::NonZeroU64,
    pub original_root: Arc<str>,
    pub original_resource: Arc<str>,
    pub proxy_hash_algorithm: Arc<str>,
    pub proxy_byte_hash: Arc<[u8]>,
    pub qualification_receipt: Arc<str>,
    /// Original model-frame origin and the producer's actual source-to-proxy
    /// frame. Preparation applies its checked inverse before f32 lowering.
    pub model_frame_origin: WorldPoint3,
    pub source_to_proxy: ModelPlacement3,
}

#[derive(Debug, Clone)]
pub enum LoadingPresenceError3D {
    Source(&'static str),
    Capacity,
    Cancelled,
    Association(&'static str),
    Device(String),
    Resource(String),
    Context(super::Context3DError),
}

impl std::fmt::Display for LoadingPresenceError3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "loading presence: {self:?}")
    }
}
impl std::error::Error for LoadingPresenceError3D {}

impl From<super::Context3DError> for LoadingPresenceError3D {
    fn from(error: super::Context3DError) -> Self {
        Self::Context(error)
    }
}

fn resource_error(error: super::context::ControlledResourceError3D) -> LoadingPresenceError3D {
    match error {
        super::context::ControlledResourceError3D::Context(error) => error.into(),
        error => LoadingPresenceError3D::Resource(error.to_string()),
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProxyVertex {
    pub position: [f32; 3],
    pub e01: [f32; 3],
    pub e02: [f32; 3],
}
const _: () = assert!(std::mem::size_of::<ProxyVertex>() == 36);

pub(crate) struct ProxyCpu {
    pub provenance: LoadingPresenceSource3D,
    // Keep actual original referents, not a byte-equal reconstruction.
    pub _source: Arc<GeometrySourceDescriptor>,
    pub placement: ModelPlacement3,
    pub vertices: Arc<[ProxyVertex]>,
    pub extent: ([f32; 3], [f32; 3]),
    spectral_basis: [f32; 4],
    pub retained_bytes: u64,
    pub cancel: Arc<AtomicBool>,
}

/// HAL-sealed CPU-only input. Preparation must run on the caller's existing
/// CPU executor, before transfer to the Context. It is not full PBR admission.
pub struct LoadingPresenceProxyPrepared3D(pub(crate) Arc<ProxyCpu>);

/// Scoped immutable application/source claim shared with later full preparation.
/// CPU-only; this is neither an HDR identity nor a full PBR proof certificate.
#[derive(Clone)]
pub struct LoadingPresenceAssociation3D(Arc<ProxyCpu>);

/// CPU-only request authority captured by the UI before asynchronous full work.
/// It carries no Context, resource lease or mathematical certificate. Target
/// authority is established later by the exact target/front admission.
#[derive(Clone)]
pub struct LoadingPresenceRequest3D {
    context: super::context::ContextId,
    viewport: crate::renderer::CustomRenderPresentationId,
    epoch: u64,
    mount: u64,
    view: super::Viewport3DViewRevision,
    samples: u32,
    source: LoadingPresenceAssociation3D,
}
impl LoadingPresenceRequest3D {
    pub(crate) fn new(
        context: &super::Context3D,
        viewport: crate::renderer::CustomRenderPresentationId,
        epoch: u64,
        mount: u64,
        view: super::Viewport3DViewRevision,
        samples: u32,
        source: LoadingPresenceAssociation3D,
    ) -> Self {
        Self {
            context: context.context_identity(),
            viewport,
            epoch,
            mount,
            view,
            samples,
            source,
        }
    }
    pub(crate) fn matches(
        &self,
        context: &super::Context3D,
        viewport: crate::renderer::CustomRenderPresentationId,
        epoch: u64,
        mount: u64,
        view: super::Viewport3DViewRevision,
        samples: u32,
        proxy: &LoadingPresenceProxy3D,
    ) -> bool {
        self.context.same_as(&context.context_identity())
            && self.viewport == viewport
            && self.epoch == epoch
            && self.mount == mount
            && self.view == view
            && self.samples == samples
            && proxy.matches_association(&self.source)
    }
}

fn plus(a: usize, b: usize) -> Result<usize, LoadingPresenceError3D> {
    a.checked_add(b).ok_or(LoadingPresenceError3D::Capacity)
}

fn arc_bytes(bytes: usize, alignment: usize) -> Result<usize, LoadingPresenceError3D> {
    let data = std::alloc::Layout::from_size_align(bytes, alignment)
        .map_err(|_| LoadingPresenceError3D::Capacity)?;
    std::alloc::Layout::new::<[std::sync::atomic::AtomicUsize; 2]>()
        .extend(data)
        .map(|(layout, _)| layout.pad_to_align().size())
        .map_err(|_| LoadingPresenceError3D::Capacity)
}

fn checkpoint(cancel: &AtomicBool) -> Result<(), LoadingPresenceError3D> {
    if cancel.load(Ordering::Acquire) {
        Err(LoadingPresenceError3D::Cancelled)
    } else {
        Ok(())
    }
}

// A fixed stack identity set covers all original streams/provenance allocations.
// No dynamic accounting allocation is made before the caller budget check.
fn input_cost(
    source: &GeometrySourceDescriptor,
    claim: &LoadingPresenceSource3D,
) -> Result<usize, LoadingPresenceError3D> {
    let mut seen = [(0usize, 0usize); 11];
    let mut used = 0;
    let mut total = arc_bytes(
        std::mem::size_of::<GeometrySourceDescriptor>(),
        std::mem::align_of::<GeometrySourceDescriptor>(),
    )?;
    let mut add =
        |ptr: usize, bytes: usize, alignment: usize| -> Result<(), LoadingPresenceError3D> {
            if bytes == 0 || seen[..used].iter().any(|x| x.0 == ptr) {
                return Ok(());
            }
            seen[used] = (ptr, bytes);
            used += 1;
            total = plus(total, arc_bytes(bytes, alignment)?)?;
            Ok(())
        };
    let size = |n: usize, element: usize| {
        n.checked_mul(element)
            .ok_or(LoadingPresenceError3D::Capacity)
    };
    add(
        source.positions().as_ptr() as usize,
        std::mem::size_of_val(source.positions()),
        std::mem::align_of_val(source.positions()),
    )?;
    if let Some(v) = source.normals() {
        add(
            v.as_ptr() as usize,
            std::mem::size_of_val(v),
            std::mem::align_of_val(v),
        )?;
    }
    if let Some(v) = source.uvs() {
        add(
            v.as_ptr() as usize,
            std::mem::size_of_val(v),
            std::mem::align_of_val(v),
        )?;
    }
    if let Some(v) = source.colors() {
        add(
            v.as_ptr() as usize,
            std::mem::size_of_val(v),
            std::mem::align_of_val(v),
        )?;
    }
    match source.indices() {
        GeometryIndices::Unindexed => {}
        GeometryIndices::U16(v) => add(
            v.as_ptr() as usize,
            size(v.len(), 2)?,
            std::mem::align_of::<u16>(),
        )?,
        GeometryIndices::U32(v) => add(
            v.as_ptr() as usize,
            size(v.len(), 4)?,
            std::mem::align_of::<u32>(),
        )?,
    }
    for value in [
        &claim.original_root,
        &claim.original_resource,
        &claim.proxy_hash_algorithm,
        &claim.qualification_receipt,
    ] {
        add(value.as_ptr() as usize, value.len(), 1)?;
    }
    add(
        claim.proxy_byte_hash.as_ptr() as usize,
        claim.proxy_byte_hash.len(),
        1,
    )?;
    Ok(total)
}

impl LoadingPresenceProxyPrepared3D {
    /// `source` is in its producer-qualified proxy frame; the retained provenance
    /// carries its actual source-to-proxy mapping and original model origin.
    /// `placement` is the actual canonical object placement, not a fallback.
    /// Extents are independently derived over referenced corners and must be
    /// enclosed by the producer's claimed finite extents.
    pub fn prepare(
        source: Arc<GeometrySourceDescriptor>,
        provenance: LoadingPresenceSource3D,
        placement: ModelPlacement3,
        claimed_extent: ([f32; 3], [f32; 3]),
        maximum_cpu_bytes: usize,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, LoadingPresenceError3D> {
        checkpoint(&cancel)?;
        if provenance.original_root.is_empty()
            || provenance.original_resource.is_empty()
            || provenance.proxy_hash_algorithm.is_empty()
            || provenance.proxy_byte_hash.is_empty()
            || provenance.qualification_receipt.is_empty()
        {
            return Err(LoadingPresenceError3D::Source(
                "incomplete upstream provenance",
            ));
        }
        for axis in 0..3 {
            if !claimed_extent.0[axis].is_finite()
                || !claimed_extent.1[axis].is_finite()
                || claimed_extent.0[axis] > claimed_extent.1[axis]
            {
                return Err(LoadingPresenceError3D::Source(
                    "nonfinite/inverted proxy extent",
                ));
            }
        }
        let count = if source.indices().is_indexed() {
            source.indices().len()
        } else {
            source.positions().len()
        };
        if count == 0 {
            return Err(LoadingPresenceError3D::Source(
                "empty preview has no source-qualified surface",
            ));
        }
        let _ = u32::try_from(count).map_err(|_| LoadingPresenceError3D::Capacity)?;
        let vertex_bytes = count
            .checked_mul(36)
            .ok_or(LoadingPresenceError3D::Capacity)?;
        let input = input_cost(&source, &provenance)?;
        let fixed = plus(
            arc_bytes(
                std::mem::size_of::<ProxyCpu>(),
                std::mem::align_of::<ProxyCpu>(),
            )?,
            arc_bytes(
                std::mem::size_of::<AtomicBool>(),
                std::mem::align_of::<AtomicBool>(),
            )?,
        )?;
        let vertex_allocation = arc_bytes(vertex_bytes, std::mem::align_of::<ProxyVertex>())?;
        // Vec-to-Arc conversion can retain both allocations concurrently.
        let peak = plus(plus(input, fixed)?, plus(vertex_bytes, vertex_allocation)?)?;
        if peak > maximum_cpu_bytes {
            return Err(LoadingPresenceError3D::Capacity);
        }
        checkpoint(&cancel)?;
        let frame = provenance
            .source_to_proxy
            .checked_transform(provenance.model_frame_origin)
            .map_err(|_| LoadingPresenceError3D::Source("unrepresentable source-to-proxy frame"))?;
        let frame64 = frame
            .model_to_render()
            .columns()
            .map(|column| column.map(f64::from));
        let inverse_frame = glam::DMat4::from_cols_array_2d(&frame64).inverse();
        if !inverse_frame.is_finite() {
            return Err(LoadingPresenceError3D::Source(
                "noninvertible source-to-proxy frame",
            ));
        }
        let transform = placement
            .checked_transform(placement.translation())
            .map_err(|_| {
                LoadingPresenceError3D::Source("unrepresentable proxy object transform")
            })?;
        let matrix = glam::Mat4::from_cols_array_2d(&transform.model_to_render().columns());
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(count)
            .map_err(|_| LoadingPresenceError3D::Capacity)?;
        let mut lower = [f32::INFINITY; 3];
        let mut upper = [f32::NEG_INFINITY; 3];
        let index = |i: usize| -> usize {
            match source.indices() {
                GeometryIndices::Unindexed => i,
                GeometryIndices::U16(v) => usize::from(v[i]),
                GeometryIndices::U32(v) => v[i] as usize,
            }
        };
        for first in (0..count).step_by(3) {
            checkpoint(&cancel)?;
            let mut p = [glam::Vec3::ZERO; 3];
            for corner in 0..3 {
                let v = source.positions()[index(first + corner)];
                let model = inverse_frame.transform_point3(glam::DVec3::new(
                    f64::from(v.x()),
                    f64::from(v.y()),
                    f64::from(v.z()),
                ));
                p[corner] = model.as_vec3();
                if !model.is_finite() || !p[corner].is_finite() {
                    return Err(LoadingPresenceError3D::Source(
                        "nonfinite proxy frame lowering",
                    ));
                }
                for axis in 0..3 {
                    lower[axis] = lower[axis].min(p[corner][axis]);
                    upper[axis] = upper[axis].max(p[corner][axis]);
                }
            }
            let e01 = p[1] - p[0];
            let e02 = p[2] - p[0];
            let local = e01.cross(e02);
            let a = matrix.transform_vector3(e01);
            let b = matrix.transform_vector3(e02);
            let transformed = a.cross(b);
            if !e01.is_finite()
                || !e02.is_finite()
                || !local.is_finite()
                || !a.is_finite()
                || !b.is_finite()
                || !transformed.is_finite()
                || !local.length_squared().is_finite()
                || local.length_squared() <= 0.0
                || !transformed.length_squared().is_finite()
                || transformed.length_squared() <= 0.0
                || !transformed.normalize().is_finite()
            {
                return Err(LoadingPresenceError3D::Source(
                    "degenerate lowered/transformed proxy triangle",
                ));
            }
            for position in p {
                if !matrix.transform_point3(position).is_finite() {
                    return Err(LoadingPresenceError3D::Source(
                        "nonfinite transformed proxy vertex",
                    ));
                }
                vertices.push(ProxyVertex {
                    position: position.to_array(),
                    e01: e01.to_array(),
                    e02: e02.to_array(),
                });
            }
        }
        checkpoint(&cancel)?;
        for axis in 0..3 {
            if lower[axis] < claimed_extent.0[axis] || upper[axis] > claimed_extent.1[axis] {
                return Err(LoadingPresenceError3D::Source(
                    "proxy extent does not enclose actual source corners",
                ));
            }
        }
        let center: [f32; 3] = std::array::from_fn(|axis| {
            ((f64::from(lower[axis]) + f64::from(upper[axis])) * 0.5) as f32
        });
        let half_span = (0..3)
            .map(|axis| (f64::from(upper[axis]) - f64::from(lower[axis])) * 0.5)
            .fold(0.0_f64, f64::max) as f32;
        if !center.iter().all(|v| v.is_finite()) || !half_span.is_finite() || half_span <= 0.0 {
            return Err(LoadingPresenceError3D::Source(
                "unrepresentable spectral model basis",
            ));
        }
        let retained_bytes = u64::try_from(plus(plus(input, fixed)?, vertex_allocation)?)
            .map_err(|_| LoadingPresenceError3D::Capacity)?;
        let vertices = vertices.into();
        checkpoint(&cancel)?;
        Ok(Self(Arc::new(ProxyCpu {
            provenance,
            _source: source,
            placement,
            vertices,
            extent: (lower, upper),
            spectral_basis: [center[0], center[1], center[2], half_span],
            retained_bytes,
            cancel,
        })))
    }

    pub fn retained_cpu_bytes(&self) -> u64 {
        self.0.retained_bytes
    }
    pub fn triangle_count(&self) -> usize {
        self.0.vertices.len() / 3
    }
    pub fn source(&self) -> &LoadingPresenceSource3D {
        &self.0.provenance
    }
    pub fn extent(&self) -> ([f32; 3], [f32; 3]) {
        self.0.extent
    }
    pub fn association(&self) -> LoadingPresenceAssociation3D {
        LoadingPresenceAssociation3D(self.0.clone())
    }
}

pub(super) struct ProxySourceRecord {
    pub cpu: Arc<ProxyCpu>,
}

struct ProxyOwner {
    context: super::Context3D,
    key: super::context::ControlledResourceKey3D,
    lease: super::context::ControlledResourceLease3D,
    source: Arc<ProxySourceRecord>,
}

impl Drop for ProxyOwner {
    fn drop(&mut self) {
        // Actual outstanding leases keep the controlled record/CPU charge live.
        // A failed Context retirement is never represented as released capacity.
        if let Err(error) = self.context.retire_controlled_source(
            &self.key,
            super::context::ControlledResourceKind3D::LoadingPresenceProxy,
        ) {
            tracing::error!(%error,"HAL381 source retirement failed; retained charge is not declared free");
        }
    }
}

/// Opaque UI-owner resource; cannot cross into the source CPU executor.
#[derive(Clone)]
pub struct LoadingPresenceProxy3D {
    owner: std::rc::Rc<ProxyOwner>,
}

impl LoadingPresenceProxy3D {
    pub fn source(&self) -> &LoadingPresenceSource3D {
        &self.owner.source.cpu.provenance
    }

    pub(crate) fn current(&self, context: &super::Context3D) -> Result<(), LoadingPresenceError3D> {
        if self.owner.source.cpu.cancel.load(Ordering::Acquire) {
            return Err(LoadingPresenceError3D::Cancelled);
        }
        if !self.owner.context.same_resource_family(context) {
            return Err(LoadingPresenceError3D::Association(
                "proxy belongs to another Context",
            ));
        }
        context
            .lease_controlled_source(
                &self.owner.key,
                super::context::ControlledResourceKind3D::LoadingPresenceProxy,
            )
            .map_err(resource_error)?;
        context.gpu_resources()?;
        Ok(())
    }

    fn snapshot(
        &self,
        context: &super::Context3D,
    ) -> Result<(Arc<ProxyCpu>, super::context::ControlledResourceLease3D), LoadingPresenceError3D>
    {
        self.current(context)?;
        Ok((self.owner.source.cpu.clone(), self.owner.lease.clone()))
    }

    pub(crate) fn same_resource(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.owner, &other.owner)
    }

    pub(crate) fn matches_association(&self, association: &LoadingPresenceAssociation3D) -> bool {
        Arc::ptr_eq(&self.owner.source.cpu, &association.0)
    }
}

impl super::Context3D {
    /// Short borrowed transfer. The existing controlled owner checks all finite
    /// slot/generation/CPU limits BEFORE the factory clones the sealed input.
    /// No vertex conversion, source scan, upload or PBR proof occurs here.
    pub fn admit_loading_presence_proxy(
        &self,
        prepared: &LoadingPresenceProxyPrepared3D,
    ) -> Result<LoadingPresenceProxy3D, LoadingPresenceError3D> {
        checkpoint(&prepared.0.cancel)?;
        self.gpu_resources()?;
        let metadata = plus(
            arc_bytes(
                std::mem::size_of::<ProxySourceRecord>(),
                std::mem::align_of::<ProxySourceRecord>(),
            )?,
            arc_bytes(
                std::mem::size_of::<ProxyOwner>(),
                std::mem::align_of::<ProxyOwner>(),
            )?,
        )?;
        let bytes = prepared
            .retained_cpu_bytes()
            .checked_add(metadata as u64)
            .ok_or(LoadingPresenceError3D::Capacity)?;
        let (key, lease) = self
            .register_controlled_source_with(
                super::context::ControlledResourceKind3D::LoadingPresenceProxy,
                bytes,
                || {
                    Arc::new(ProxySourceRecord {
                        cpu: prepared.0.clone(),
                    })
                },
            )
            .map_err(resource_error)?;
        let source = lease
            .source::<ProxySourceRecord>()
            .expect("private typed proxy source factory and its atomic lease");
        Ok(LoadingPresenceProxy3D {
            owner: std::rc::Rc::new(ProxyOwner {
                context: self.clone(),
                key,
                lease,
                source,
            }),
        })
    }
}

pub(crate) struct PresenceGpuPrograms {
    pub preview: wgpu::RenderPipeline,
    pub waiting: wgpu::RenderPipeline,
    pub ready: wgpu::RenderPipeline,
    pub style_view: wgpu::BindGroupLayout,
    pub object: wgpu::BindGroupLayout,
    pub waiting_input: wgpu::BindGroupLayout,
    pub ready_input: wgpu::BindGroupLayout,
    _modules: [wgpu::ShaderModule; 3],
    _module_charge: super::context::ShaderModuleResidency,
    _pipeline_charge: [super::raster::ControlledPipelineResidency; 3],
}

fn uniform(binding: u32, bytes: u64, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: std::num::NonZeroU64::new(bytes),
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

pub(crate) fn composition_shader(samples: u32, ready: bool) -> String {
    let kind = if samples == 4 {
        "texture_multisampled_2d<f32>"
    } else {
        "texture_2d<f32>"
    };
    let full = if ready {
        "@group(0) @binding(1) var full: texture_2d<f32>;"
    } else {
        ""
    };
    let output = if ready {
        "let f=textureLoad(full,q,0); let h=f.rgb+(1.0-f.a)*u.background.rgb; return vec4((1.0-u.weight.x)*g+u.weight.x*h,1.0);"
    } else {
        "return vec4(g,1.0);"
    };
    format!(
        r#"
struct Uniform {{ background:vec4<f32>, weight:vec4<f32> }}
@group(0) @binding(0) var proxy: {kind};
{full}
@group(0) @binding(2) var<uniform> u:Uniform;
const N: u32 = {samples}u;
// One representative per nonzero integer-distance shell. Rotations and
// reflections generate the complete disk with squared distance below 25.
const SHELLS = array<vec2<i32>,12>(
 vec2(1,0),vec2(1,1),vec2(2,0),vec2(2,1),vec2(2,2),vec2(3,0),
 vec2(3,1),vec2(3,2),vec2(4,0),vec2(4,1),vec2(3,3),vec2(4,2));
fn proxy_sample(q: vec2<i32>, i: i32) -> vec4<f32> {{
 return textureLoad(proxy,q,{index});
}}
fn coverage(q: vec2<i32>, extent: vec2<i32>) -> u32 {{
 if any(q < vec2(0)) || any(q >= extent) {{ return 0u; }}
 var count = 0u;
 for (var i=0; i<i32(N); i+=1) {{
  count += select(0u, 1u, proxy_sample(q,i).a > 0.0);
 }}
 return count;
}}
fn shell_coverage(q: vec2<i32>, extent: vec2<i32>, d: vec2<i32>) -> u32 {{
 var a = d;
 var b = d.yx;
 let reflected = d.y != 0 && d.y != d.x;
 var maximum = 0u;
 for (var rotation=0; rotation<4; rotation+=1) {{
  maximum = max(maximum, coverage(q+a,extent));
  if maximum == N {{ return maximum; }}
  if reflected {{
   maximum = max(maximum, coverage(q+b,extent));
   if maximum == N {{ return maximum; }}
   b = vec2(-b.y,b.x);
  }}
  a = vec2(-a.y,a.x);
 }}
 return maximum;
}}
fn spectral_contour(q: vec2<i32>, count: u32, base: vec3<f32>) -> vec3<f32> {{
 if count == N {{ return base; }}
 let extent = vec2<i32>(textureDimensions(proxy));
 var core = 0.0;
 var halo = 0.0;
 for (var shell=0u; shell<12u; shell+=1u) {{
  let d = SHELLS[shell];
  let adjacent = shell_coverage(q,extent,d);
  if adjacent > count {{
   let delta = f32(adjacent-count)*u.weight.y;
   let distance = sqrt(f32(d.x*d.x+d.y*d.y));
   core = max(core, delta*(1.0-smoothstep(1.0,2.0,distance)));
   halo = max(halo, delta*(1.0-smoothstep(1.0,5.0,distance)));
  }}
  // A full shell at this distance dominates all farther monotone weights.
  if adjacent == N {{ break; }}
 }}
 return clamp(mix(base,vec3(0.58,0.78,0.88),0.24*core+0.08*halo),vec3(0.0),vec3(1.0));
}}
@vertex fn vs(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {{
 let p=array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));
 return vec4(p[i],0.0,1.0);
}}
@fragment fn fs(@builtin(position) p:vec4<f32>) -> @location(0) vec4<f32> {{
 let q=vec2<i32>(p.xy); var base=vec3<f32>(0.0); var count=0u;
 for(var i=0;i<i32(N);i+=1) {{
  let c=proxy_sample(q,i);
  base+=c.rgb+(1.0-c.a)*u.background.rgb;
  count+=select(0u,1u,c.a>0.0);
 }}
 let g=spectral_contour(q,count,base*u.weight.y);
 {output}
}}
"#,
        index = if samples == 4 { "i" } else { "0" }
    )
}

impl PresenceGpuPrograms {
    pub(crate) fn new(
        context: &super::Context3D,
        samples: u32,
    ) -> Result<Self, LoadingPresenceError3D> {
        if !matches!(samples, 1 | 4) {
            return Err(LoadingPresenceError3D::Association(
                "invalid preview sample count",
            ));
        }
        let gpu = context.gpu_resources()?;
        let limits = gpu.device.limits();
        let color =
            wgpu::TextureFormat::Rgba16Float.guaranteed_format_features(gpu.device.features());
        let depth =
            wgpu::TextureFormat::Depth32Float.guaranteed_format_features(gpu.device.features());
        if !color
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
            || !depth
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            || !color.flags.sample_count_supported(samples)
            || !depth.flags.sample_count_supported(samples)
            || limits.max_bind_groups < 2
            || limits.max_vertex_attributes < 3
            || limits.max_vertex_buffer_array_stride < 36
            || limits.max_inter_stage_shader_variables < 3
            || limits.max_uniform_buffer_binding_size < 112
            || limits.max_uniform_buffers_per_shader_stage < 3
            || limits.max_sampled_textures_per_shader_stage < 2
        {
            return Err(LoadingPresenceError3D::Device(
                "preview format/sample/layout capability".into(),
            ));
        }
        let raster = context.raster_gpu()?;
        let mut pipelines = [
            raster.reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?,
            raster.reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?,
            raster.reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?,
        ];
        let reservation = context.reserve_controlled_shader_modules(3)?;
        let sources = [
            PREVIEW_SHADER.to_owned(),
            composition_shader(samples, false),
            composition_shader(samples, true),
        ];
        let mut ir = Vec::with_capacity(3);
        for source in &sources {
            let module = naga::front::wgsl::parse_str(source)
                .map_err(|e| LoadingPresenceError3D::Device(e.emit_to_string(source)))?;
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .map_err(|e| LoadingPresenceError3D::Device(e.emit_to_string(source)))?;
            ir.push(module);
        }
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let modules: [wgpu::ShaderModule; 3] = std::array::from_fn(|_| {
            gpu.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("HAL381 private source-qualified program"),
                    source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(ir.remove(0))),
                })
        });
        let style_view = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HAL381 Style64 and semantic View80"),
                entries: &[
                    uniform(0, 64, wgpu::ShaderStages::VERTEX_FRAGMENT),
                    uniform(1, 80, wgpu::ShaderStages::VERTEX_FRAGMENT),
                ],
            });
        let object = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HAL381 semantic Object112"),
                entries: &[uniform(0, 112, wgpu::ShaderStages::VERTEX)],
            });
        let waiting_input = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HAL381 waiting: no full front binding"),
                entries: &[
                    texture(0, samples),
                    uniform(2, 32, wgpu::ShaderStages::FRAGMENT),
                ],
            });
        let ready_input = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("HAL381 registered pinned full front"),
                entries: &[
                    texture(0, samples),
                    texture(1, 1),
                    uniform(2, 32, wgpu::ShaderStages::FRAGMENT),
                ],
            });
        let make =
            |module: &wgpu::ShaderModule, layouts: &[Option<&wgpu::BindGroupLayout>], geometry: bool| {
                let layout = gpu
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("HAL381 distinct private layout"),
                        bind_group_layouts: layouts,
                        immediate_size: 0,
                    });
                let attributes = wgpu::vertex_attr_array![0=>Float32x3, 1=>Float32x3, 2=>Float32x3];
                let buffer = Some(wgpu::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                });
                gpu.device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("HAL381 private viewport evaluator"),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState {
                            module,
                            entry_point: Some("vs"),
                            compilation_options: Default::default(),
                            buffers: if geometry {
                                std::slice::from_ref(&buffer)
                            } else {
                                &[]
                            },
                        },
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleList,
                            ..Default::default()
                        },
                        depth_stencil: geometry.then_some(wgpu::DepthStencilState {
                            format: wgpu::TextureFormat::Depth32Float,
                            depth_write_enabled: Some(true),
                            depth_compare: Some(wgpu::CompareFunction::Greater),
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: wgpu::MultisampleState {
                            count: if geometry { samples } else { 1 },
                            ..Default::default()
                        },
                        fragment: Some(wgpu::FragmentState {
                            module,
                            entry_point: Some("fs"),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    })
            };
        let preview = make(&modules[0], &[Some(&style_view), Some(&object)], true);
        let waiting = make(&modules[1], &[Some(&waiting_input)], false);
        let ready = make(&modules[2], &[Some(&ready_input)], false);
        let validation = pollster::block_on(validation_scope.pop());
        let allocation = pollster::block_on(allocation_scope.pop());
        if let Some(error) = validation.or(allocation) {
            return Err(LoadingPresenceError3D::Device(error.to_string()));
        }
        let _module_charge = reservation.commit_all();
        for p in &mut pipelines {
            p.commit();
        }
        Ok(Self {
            preview,
            waiting,
            ready,
            style_view,
            object,
            waiting_input,
            ready_input,
            _modules: modules,
            _module_charge,
            _pipeline_charge: pipelines,
        })
    }
}

pub(crate) const PREVIEW_SHADER: &str = r#"
struct Style { base:vec4<f32>, rim:vec4<f32>, opacity:vec4<f32>, phase:vec4<f32> }
struct View { view_projection:mat4x4<f32>, camera_relative:vec3<f32>, padding:f32 }
struct Object { model_relative:mat4x4<f32>, normal_to_world:mat3x3<f32> }
@group(0) @binding(0) var<uniform> style:Style;
@group(0) @binding(1) var<uniform> view:View;
@group(1) @binding(0) var<uniform> object:Object;
struct Out {
 @builtin(position) clip:vec4<f32>,
 @location(0) point:vec3<f32>,
 @location(1) @interpolate(flat) geometric_normal:vec3<f32>,
 @location(2) spectral_point:vec3<f32>,
}
@vertex fn vs(@location(0) p:vec3<f32>, @location(1) e01:vec3<f32>, @location(2) e02:vec3<f32>) -> Out {
 var o:Out;
 let r=object.model_relative*vec4(p,1.0);
 o.clip=view.view_projection*r; o.point=r.xyz;
 let a=(object.model_relative*vec4(e01,0.0)).xyz;
 let b=(object.model_relative*vec4(e02,0.0)).xyz;
 o.geometric_normal=normalize(cross(a,b));
 let center=vec3(style.base.w,style.rim.w,style.phase.y);
 o.spectral_point=clamp((p-center)/style.phase.z,vec3(-1.0),vec3(1.0));
 return o;
}
fn lattice(cell:vec3<i32>) -> f32 {
 let c=bitcast<vec3<u32>>(cell);
 var h=(c.x*0x8da6b343u)^(c.y*0xd8163841u)^(c.z*0xcb1ab31fu);
 h=(h^(h>>16u))*0x7feb352du;
 h=(h^(h>>15u))*0x846ca68bu;
 return f32((h^(h>>16u))>>8u)/16777215.0;
}
fn value_noise(p:vec3<f32>) -> f32 {
 let cell=vec3<i32>(floor(p));
 let f=fract(p);
 let u=f*f*(3.0-2.0*f);
 let a=mix(lattice(cell),lattice(cell+vec3(1,0,0)),u.x);
 let b=mix(lattice(cell+vec3(0,1,0)),lattice(cell+vec3(1,1,0)),u.x);
 let c=mix(lattice(cell+vec3(0,0,1)),lattice(cell+vec3(1,0,1)),u.x);
 let d=mix(lattice(cell+vec3(0,1,1)),lattice(cell+vec3(1,1,1)),u.x);
 return mix(mix(a,b,u.y),mix(c,d,u.y),u.z);
}
@fragment fn fs(i:Out) -> @location(0) vec4<f32> {
 let v=normalize(view.camera_relative-i.point);
 let x=clamp(abs(dot(normalize(i.geometric_normal),v)),0.0,1.0);
 let h=clamp((x-style.opacity.z)/(style.opacity.w-style.opacity.z),0.0,1.0);
 let e=1.0-h*h*(3.0-2.0*h);
 let offset=vec3(sin(style.phase.x),cos(style.phase.x)-1.0,0.0)/16.0;
 let p=clamp(i.spectral_point,vec3(-1.0),vec3(1.0))+offset;
 let noise=clamp((2.0*value_noise(p*3.0)+value_noise(p*8.0))/3.0,0.0,1.0);
 let a=clamp(style.opacity.x+style.opacity.y*e+0.035*(noise-0.5),0.08,0.24);
 let c=mix(style.base.rgb,style.rim.rgb,e)*(0.94+0.12*noise);
 return vec4(clamp(a*c,vec3(0.0),vec3(1.0)),a);
}
"#;

struct PresenceGpuWork {
    programs: PresenceGpuPrograms,
    terminal: Arc<crate::renderer::wgpu_renderer::PresenceTerminalPipeline>,
    _proxy: wgpu::Texture,
    proxy_view: wgpu::TextureView,
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    _output: wgpu::Texture,
    vertex: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    style_view: wgpu::BindGroup,
    object: wgpu::BindGroup,
    waiting: wgpu::BindGroup,
    stride: u64,
    cpu: Arc<ProxyCpu>,
    _source: super::context::ControlledResourceLease3D,
    _attachments: super::context::TargetBudgetLease3D,
    active: Arc<super::context::TargetBudgetLease3D>,
    _dynamic: super::context::DynamicStorageLease3D,
    _geometry: super::context::GeometryGpuResidency,
}

pub(crate) struct PresenceGpuTarget {
    context: super::Context3D,
    gpu: crate::renderer::wgpu_renderer::GpuResources,
    work: Arc<PresenceGpuWork>,
    pub output: crate::renderer::wgpu_renderer::PresenceOutput,
    pub extent: (u32, u32),
    pub samples: u32,
    pub attachment_bytes: u64,
    revision: Option<u64>,
}

pub(crate) struct PresenceTerminalFactory {
    gpu: crate::renderer::wgpu_renderer::GpuResources,
    terminal: Arc<crate::renderer::wgpu_renderer::PresenceTerminalPipeline>,
    cpu: Arc<ProxyCpu>,
    source: super::context::ControlledResourceLease3D,
}

/// Queued F reads need its physical pin and immutable attribution, not another
/// live Context. MaterialSnapshot3D owns a Context; retaining a complete Scene
/// in the Queue callback would make Queue -> Scene -> Context -> Queue a cycle.
/// The UI's pin_tuple remains the strong current-source owner. This private
/// record preserves exact Arc identity (including after retirement), source
/// generations/claim and the real charged CPU claim lease through old reads;
/// it cannot admit, renew or publish a source after its UI owner disappears.
struct PresenceFrontSource {
    _identity: std::sync::Weak<super::pbr::MaterialSceneSource3D>,
    _context: super::context::ContextId,
    _request: super::Viewport3DRequestSerial,
    _target: super::Viewport3DTargetCompatibilityKey,
    _view_revision: super::Viewport3DViewRevision,
    _view: super::ViewDescriptor3,
    _structural: super::StructuralRevision,
    _model: super::ModelRevision,
    _original: Arc<ProxyCpu>,
    _source: super::context::ControlledResourceLease3D,
}
impl PresenceFrontSource {
    fn new(
        context: &super::Context3D,
        tuple: &super::viewport::MaterialViewport3DTuple,
        cpu: Arc<ProxyCpu>,
        source: super::context::ControlledResourceLease3D,
    ) -> Self {
        Self {
            _identity: Arc::downgrade(tuple.source()),
            _context: context.context_identity(),
            _request: tuple.request_serial(),
            _target: tuple.target_key(),
            _view_revision: tuple.view_revision(),
            _view: tuple.view(),
            _structural: tuple.structural_revision(),
            _model: tuple.model_revision(),
            _original: cpu,
            _source: source,
        }
    }
}
impl PresenceTerminalFactory {
    pub(crate) fn output(
        &self,
        context: &super::Context3D,
        pin: &crate::renderer::custom_render_pass::HdrFrontPin,
        source: super::viewport::MaterialViewport3DTuple,
    ) -> Result<crate::renderer::wgpu_renderer::PresenceOutput, LoadingPresenceError3D> {
        let charge = context.reserve_dynamic_storage(
            0,
            0,
            1,
            crate::renderer::wgpu_renderer::GpuResources::loading_presence_metadata_bytes()
                + std::mem::size_of::<PresenceFrontSource>() as u64,
        )?;
        let id = crate::utils::id::WidgetInternalId::new().raw();
        let source =
            PresenceFrontSource::new(context, &source, self.cpu.clone(), self.source.clone());
        let retained: Arc<crate::renderer::gpu_custody::GpuCharge> =
            Arc::new((pin.read_lease(), self.terminal.clone(), source, charge));
        self.gpu
            .loading_presence_output(
                id,
                pin.view().clone(),
                retained,
                Some(self.terminal.clone()),
            )
            .map_err(|s| LoadingPresenceError3D::Resource(s.into()))
    }
}

impl PresenceGpuTarget {
    pub(crate) fn new(
        context: &super::Context3D,
        proxy: &LoadingPresenceProxy3D,
        width: u32,
        height: u32,
        samples: u32,
        active: Option<Arc<super::context::TargetBudgetLease3D>>,
    ) -> Result<Self, LoadingPresenceError3D> {
        use wgpu::util::DeviceExt;
        let (cpu, source) = proxy.snapshot(context)?;
        let gpu = context.gpu_resources()?;
        let limits = gpu.device.limits();
        let policy = context.limits().policy;
        if width == 0
            || height == 0
            || width.max(height) > limits.max_texture_dimension_2d
            || width.max(height) > policy.max_target_dimension_2d
            || !matches!(samples, 1 | 4)
        {
            return Err(LoadingPresenceError3D::Capacity);
        }
        let pixels = u64::from(width) * u64::from(height);
        // One central read and 68 admitted neighbors per sample, plus F when ready.
        pixels
            .checked_mul(69 * u64::from(samples) + 1)
            .ok_or(LoadingPresenceError3D::Capacity)?;
        let bytes = pixels
            .checked_mul(12 * u64::from(samples) + 8)
            .ok_or(LoadingPresenceError3D::Capacity)?;
        let full = pixels
            .checked_mul(20 * u64::from(samples) + 16)
            .ok_or(LoadingPresenceError3D::Capacity)?;
        if bytes
            .checked_add(full)
            .is_none_or(|n| n > policy.max_single_target_attachment_bytes)
        {
            return Err(LoadingPresenceError3D::Capacity);
        }
        if active
            .as_ref()
            .is_some_and(|a| !a.active_for(context, pixels))
        {
            return Err(LoadingPresenceError3D::Association(
                "foreign active-target credit",
            ));
        }
        let active = match active {
            Some(a) => a,
            None => Arc::new(context.reserve_target_budget(pixels, 0)?),
        };
        let attachments = context.reserve_target_budget(0, bytes)?;
        let stride = u64::from(limits.min_uniform_buffer_offset_alignment)
            .max(112)
            .next_power_of_two();
        let uniform_bytes = stride
            .checked_mul(4)
            .ok_or(LoadingPresenceError3D::Capacity)?;
        let vertex_bytes = std::mem::size_of_val(&*cpu.vertices) as u64;
        if vertex_bytes > policy.max_mesh_bytes
            || vertex_bytes > policy.max_upload_bytes_per_frame
            || vertex_bytes > limits.max_buffer_size
            || uniform_bytes > limits.max_buffer_size
            || uniform_bytes > policy.max_dynamic_storage_bytes_per_target
            || 288 > policy.max_dynamic_upload_bytes_per_frame
            || 288 > policy.max_upload_bytes_per_frame
        {
            return Err(LoadingPresenceError3D::Capacity);
        }
        let dynamic = context.reserve_dynamic_storage(
            uniform_bytes,
            1,
            4,
            uniform_bytes
                + std::mem::size_of::<PresenceGpuWork>() as u64
                + crate::renderer::wgpu_renderer::GpuResources::loading_presence_metadata_bytes(),
        )?;
        let geometry = context.reserve_controlled_geometry_bytes(vertex_bytes)?;
        let programs = PresenceGpuPrograms::new(context, samples)?;
        let mut terminal_pipeline = context
            .raster_gpu()?
            .reserve_controlled_pipeline(policy.max_pipeline_variants)?;
        let terminal_module = context.reserve_controlled_shader_module()?;
        let terminal = Arc::new(
            gpu.loading_presence_terminal_pipeline(|| {
                terminal_pipeline.commit();
                Box::new((terminal_module.commit_one(), terminal_pipeline))
            })
            .map_err(|s| LoadingPresenceError3D::Device(s.into()))?,
        );
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let texture = |format, samples, usage, label| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
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
        let proxy_texture = texture(
            wgpu::TextureFormat::Rgba16Float,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "HAL381 private proxy",
        );
        let proxy_view = proxy_texture.create_view(&Default::default());
        let depth = texture(
            wgpu::TextureFormat::Depth32Float,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            "HAL381 proxy self-depth",
        );
        let depth_view = depth.create_view(&Default::default());
        let output_texture = texture(
            wgpu::TextureFormat::Rgba16Float,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            "HAL381 single presentation output",
        );
        let output_view = output_texture.create_view(&Default::default());
        let vertex = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HAL381 source-derived expanded triangles"),
                contents: bytemuck::cast_slice(&cpu.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let uniforms = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HAL381 aligned private uniforms"),
            size: uniform_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let buffer = |offset, size| {
            wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &uniforms,
                offset,
                size: std::num::NonZeroU64::new(size),
            })
        };
        let style_view = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HAL381 Style64 + View80"),
            layout: &programs.style_view,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer(0, 64),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffer(stride, 80),
                },
            ],
        });
        let object = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HAL381 actual Object112"),
            layout: &programs.object,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer(stride * 2, 112),
            }],
        });
        let waiting = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HAL381 waiting: no full-front binding"),
            layout: &programs.waiting_input,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&proxy_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer(stride * 3, 32),
                },
            ],
        });
        let validation = pollster::block_on(validation_scope.pop());
        let allocation = pollster::block_on(allocation_scope.pop());
        if let Some(e) = validation.or(allocation) {
            return Err(LoadingPresenceError3D::Device(e.to_string()));
        }
        let work = Arc::new(PresenceGpuWork {
            programs,
            terminal,
            _proxy: proxy_texture,
            proxy_view,
            _depth: depth,
            depth_view,
            _output: output_texture,
            vertex,
            uniforms,
            style_view,
            object,
            waiting,
            stride,
            cpu,
            _source: source,
            _attachments: attachments,
            active,
            _dynamic: dynamic,
            _geometry: geometry.commit(),
        });
        let output = gpu
            .loading_presence_output(
                crate::utils::id::WidgetInternalId::new().raw(),
                output_view,
                work.clone(),
                None,
            )
            .map_err(|s| LoadingPresenceError3D::Resource(s.into()))?;
        Ok(Self {
            context: context.clone(),
            gpu,
            work,
            output,
            extent: (width, height),
            samples,
            attachment_bytes: bytes,
            revision: None,
        })
    }

    pub(crate) fn active_credit(&self) -> Arc<super::context::TargetBudgetLease3D> {
        self.work.active.clone()
    }
    #[cfg(test)]
    pub(crate) fn output_texture(&self) -> &wgpu::Texture {
        &self.work._output
    }

    pub(crate) fn terminal_factory(&self) -> PresenceTerminalFactory {
        PresenceTerminalFactory {
            gpu: self.gpu.clone(),
            terminal: self.work.terminal.clone(),
            cpu: self.work.cpu.clone(),
            source: self.work._source.clone(),
        }
    }

    pub(crate) fn render(
        &mut self,
        view: super::ViewDescriptor3,
        phase: f32,
        weight: f32,
        revision: u64,
        pin: Option<&crate::renderer::custom_render_pass::HdrFrontPin>,
        source: Option<super::viewport::MaterialViewport3DTuple>,
    ) -> Result<bool, LoadingPresenceError3D> {
        if self.revision == Some(revision) {
            return Ok(false);
        }
        checkpoint(&self.work.cpu.cancel)?;
        if !self.output.can_write() {
            return Ok(false);
        }
        let capture = self.context.reserve_dynamic_storage(
            0,
            0,
            0,
            (std::mem::size_of::<(
                Arc<PresenceGpuWork>,
                Option<Arc<crate::renderer::custom_render_pass::HdrFrontReadLease>>,
                Option<wgpu::BindGroup>,
                Option<PresenceFrontSource>,
                super::context::DynamicStorageLease3D,
            )>() + 2 * std::mem::size_of::<usize>()) as u64,
        )?;
        let origin = view.pose().translation();
        let matrices = view
            .prepared_matrices(origin, self.extent.0, self.extent.1)
            .map_err(|_| LoadingPresenceError3D::Source("proxy View80 transform"))?;
        let camera = view
            .pose()
            .translation()
            .render_relative_to(origin)
            .map_err(|_| LoadingPresenceError3D::Source("proxy camera-relative frame"))?;
        let transform = self
            .work
            .cpu
            .placement
            .checked_transform(origin)
            .map_err(|_| LoadingPresenceError3D::Source("proxy Object112 transform"))?;
        let n = transform.normal_to_world().columns();
        let object = super::appearance::ObjectBlockBytes::new(
            transform.model_to_render().columns(),
            [
                [n[0][0], n[0][1], n[0][2], 0.0],
                [n[1][0], n[1][1], n[1][2], 0.0],
                [n[2][0], n[2][1], n[2][2], 0.0],
            ],
        );
        let view = super::appearance::ViewBlockBytes::new(
            matrices.view_projection().columns(),
            [camera.x(), camera.y(), camera.z()],
        );
        let [x, y, z, half_span] = self.work.cpu.spectral_basis;
        let style: [[f32; 4]; 4] = [
            [0.10, 0.22, 0.29, x],
            [0.40, 0.68, 0.81, y],
            [0.10, 0.10, 0.20, 0.65],
            [phase, z, half_span, 0.0],
        ];
        let compose: [[f32; 4]; 2] = [
            [NEUTRAL[0], NEUTRAL[1], NEUTRAL[2], 1.0],
            [weight, 1.0 / self.samples as f32, 0.0, 0.0],
        ];
        let w = &self.work;
        self.gpu
            .queue
            .write_buffer(&w.uniforms, 0, bytemuck::bytes_of(&style));
        self.gpu
            .queue
            .write_buffer(&w.uniforms, w.stride, bytemuck::bytes_of(&view));
        self.gpu
            .queue
            .write_buffer(&w.uniforms, w.stride * 2, bytemuck::bytes_of(&object));
        self.gpu
            .queue
            .write_buffer(&w.uniforms, w.stride * 3, bytemuck::bytes_of(&compose));
        let ready = pin.map(|pin| {
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("HAL381 actual pinned full front"),
                    layout: &w.programs.ready_input,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&w.proxy_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(pin.view()),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &w.uniforms,
                                offset: w.stride * 3,
                                size: std::num::NonZeroU64::new(32),
                            }),
                        },
                    ],
                })
        });
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("HAL381 bounded viewport presentation"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HAL381 real proxy self-occlusion"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &w.proxy_view,
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
            pass.set_pipeline(&w.programs.preview);
            pass.set_bind_group(0, &w.style_view, &[]);
            pass.set_bind_group(1, &w.object, &[]);
            pass.set_vertex_buffer(0, w.vertex.slice(..));
            pass.draw(0..w.cpu.vertices.len() as u32, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HAL381 G/H one private output"),
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
            pass.set_pipeline(if ready.is_some() {
                &w.programs.ready
            } else {
                &w.programs.waiting
            });
            pass.set_bind_group(0, ready.as_ref().unwrap_or(&w.waiting), &[]);
            pass.draw(0..3, 0..1);
        }
        let source = source
            .as_ref()
            .map(|s| PresenceFrontSource::new(&self.context, s, w.cpu.clone(), w._source.clone()));
        let retained: Arc<crate::renderer::gpu_custody::GpuCharge> = Arc::new((
            w.clone(),
            pin.map(|p| p.read_lease()),
            ready,
            source,
            capture,
        ));
        if self
            .output
            .queue(encoder.finish(), retained)
            .map_err(|s| LoadingPresenceError3D::Resource(s.into()))?
        {
            self.revision = Some(revision);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::three_d::{LocalPoint3, Orientation3, PositiveScale3};
    use sha2::Digest;

    fn fixture() -> (
        Arc<GeometrySourceDescriptor>,
        LoadingPresenceSource3D,
        ModelPlacement3,
    ) {
        let p = |x, y, z| LocalPoint3::try_new(x, y, z).unwrap();
        // A real translated triangle, with an unused distant source vertex.
        // Referenced geometry, rather than an AABB, determines the preview.
        let original_points = [
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(990.0, 1000.0, 1000.0),
        ];
        let source = Arc::new(
            GeometrySourceDescriptor::try_new(
                original_points
                    .iter()
                    .map(|v| p(v.x() + 10.0, v.y(), v.z()))
                    .collect(),
                None,
                None,
                None,
                GeometryIndices::from_u16(vec![0, 1, 2]),
            )
            .unwrap(),
        );
        let origin = WorldPoint3::try_new(0.0, 0.0, 0.0).unwrap();
        let translation = WorldPoint3::try_new(10.0, 0.0, 0.0).unwrap();
        let placement =
            ModelPlacement3::try_new(origin, Orientation3::identity(), PositiveScale3::one())
                .unwrap();
        let frame =
            ModelPlacement3::try_new(translation, Orientation3::identity(), PositiveScale3::one())
                .unwrap();
        let mut hash = sha2::Sha256::new();
        for position in source.positions() {
            for v in [position.x(), position.y(), position.z()] {
                hash.update(v.to_bits().to_le_bytes());
            }
        }
        for index in [0u16, 1, 2] {
            hash.update(index.to_le_bytes());
        }
        let hash: [u8; 32] = hash.finalize().into();
        let mut original_hash = sha2::Sha256::new();
        for v in original_points {
            for c in [v.x(), v.y(), v.z()] {
                original_hash.update(c.to_bits().to_le_bytes());
            }
        }
        for i in [0u16, 1, 2] {
            original_hash.update(i.to_le_bytes());
        }
        let original_hash: [u8; 32] = original_hash.finalize().into();
        let provenance = LoadingPresenceSource3D {
            original_binding: original_hash,
            source_revision: std::num::NonZeroU64::new(1).unwrap(),
            source_generation: std::num::NonZeroU64::new(1).unwrap(),
            original_root: "HAL381 owned translated-triangle fixture".into(),
            original_resource: "fixture source vertex/index allocation".into(),
            proxy_hash_algorithm: "SHA-256 over fixture LE position/index bits".into(),
            proxy_byte_hash: Arc::from(hash),
            qualification_receipt:
                "HAL-owned exact triangle derivative; explicit frame; not FAB011 evidence".into(),
            model_frame_origin: origin,
            source_to_proxy: frame,
        };
        (source, provenance, placement)
    }

    #[test]
    fn hal381_cpu_proxy_uses_actual_source_frame_and_runs_off_ui() {
        fn cpu<T: Send + Sync>() {}
        cpu::<LoadingPresenceSource3D>();
        cpu::<LoadingPresenceProxyPrepared3D>();
        cpu::<LoadingPresenceAssociation3D>();
        cpu::<LoadingPresenceRequest3D>();
        let owner_thread = std::thread::current().id();
        let (source, claim, placement) = fixture();
        let original = source.clone();
        let (prepared, worker_thread) = std::thread::spawn(move || {
            let prepared = LoadingPresenceProxyPrepared3D::prepare(
                source,
                claim,
                placement,
                ([0.0; 3], [1.0, 1.0, 0.0]),
                usize::MAX,
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
            (prepared, std::thread::current().id())
        })
        .join()
        .unwrap();
        assert_ne!(owner_thread, worker_thread);
        assert!(Arc::ptr_eq(&original, &prepared.0._source));
        assert_eq!(prepared.triangle_count(), 1);
        assert_eq!(prepared.extent(), ([0.0; 3], [1.0, 1.0, 0.0]));
        for vertex in prepared.0.vertices.iter() {
            assert_eq!(vertex.e01, [1.0, 0.0, 0.0]);
            assert_eq!(vertex.e02, [0.0, 1.0, 0.0]);
        }
        assert_eq!(prepared.0.vertices[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(original.positions()[1].x(), 11.0);
        assert!(prepared.retained_cpu_bytes() > 108);
    }

    #[test]
    fn hal381_cpu_proxy_refuses_capacity_cancel_and_collapsed_geometry() {
        let (source, claim, placement) = fixture();
        let extent = ([0.0; 3], [1.0, 1.0, 0.0]);
        let before = Arc::strong_count(&source);
        assert!(matches!(
            LoadingPresenceProxyPrepared3D::prepare(
                source.clone(),
                claim.clone(),
                placement,
                extent,
                0,
                Arc::new(AtomicBool::new(false)),
            ),
            Err(LoadingPresenceError3D::Capacity)
        ));
        assert_eq!(Arc::strong_count(&source), before);
        assert!(matches!(
            LoadingPresenceProxyPrepared3D::prepare(
                source.clone(),
                claim.clone(),
                placement,
                extent,
                usize::MAX,
                Arc::new(AtomicBool::new(true)),
            ),
            Err(LoadingPresenceError3D::Cancelled)
        ));
        let p = |x| LocalPoint3::try_new(x, 0.0, 0.0).unwrap();
        let collapsed = Arc::new(
            GeometrySourceDescriptor::try_new(
                vec![p(10.0), p(11.0), p(12.0)],
                None,
                None,
                None,
                GeometryIndices::Unindexed,
            )
            .unwrap(),
        );
        assert!(matches!(
            LoadingPresenceProxyPrepared3D::prepare(
                collapsed,
                claim,
                placement,
                ([0.0; 3], [2.0, 0.0, 0.0]),
                usize::MAX,
                Arc::new(AtomicBool::new(false)),
            ),
            Err(LoadingPresenceError3D::Source(
                "degenerate lowered/transformed proxy triangle"
            ))
        ));
    }

    #[test]
    fn hal381_context_proxy_refusal_precedes_strong_capture() {
        let renderer =
            pollster::block_on(crate::renderer::wgpu_renderer::WgpuRenderer::new_headless())
                .unwrap();
        let mut policy = super::super::PolicyLimits3D::conservative();
        policy.max_cache_residency_bytes = 1;
        let context =
            super::super::Context3D::with_policy(renderer.gpu_resources(), policy).unwrap();
        let (source, claim, placement) = fixture();
        let prepared = LoadingPresenceProxyPrepared3D::prepare(
            source,
            claim,
            placement,
            ([0.0; 3], [1.0, 1.0, 0.0]),
            1 << 20,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let before = Arc::strong_count(&prepared.0);
        assert!(matches!(
            context.admit_loading_presence_proxy(&prepared),
            Err(LoadingPresenceError3D::Context(
                super::super::Context3DError::PolicyBudgetExceeded {
                    budget: super::super::PolicyBudget3D::CacheResidencyBytes,
                    ..
                }
            ))
        ));
        assert_eq!(Arc::strong_count(&prepared.0), before);
        assert_eq!(
            context
                .resource_usage()
                .unwrap()
                .retained_cpu_descriptor_bytes,
            0
        );
    }

    #[test]
    fn hal381_private_programs_validate_with_no_unready_front_binding() {
        let module = naga::front::wgsl::parse_str(PREVIEW_SHADER).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
        for samples in [1, 4] {
            for ready in [false, true] {
                let text = composition_shader(samples, ready);
                let module = naga::front::wgsl::parse_str(&text).unwrap();
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::empty(),
                )
                .validate(&module)
                .unwrap();
                assert_eq!(
                    module.global_variables.iter().any(|(_, v)| v
                        .binding
                        .as_ref()
                        .is_some_and(|b| b.group == 0 && b.binding == 1)),
                    ready
                );
            }
        }
    }

    fn gpu_proxy(
        context: &super::super::Context3D,
        points: Vec<[f32; 3]>,
    ) -> LoadingPresenceProxy3D {
        let origin = WorldPoint3::origin();
        let placement =
            ModelPlacement3::try_new(origin, Orientation3::identity(), PositiveScale3::one())
                .unwrap();
        let mut hash = sha2::Sha256::new();
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in &points {
            for c in 0..3 {
                hash.update(p[c].to_bits().to_le_bytes());
                lo[c] = lo[c].min(p[c]);
                hi[c] = hi[c].max(p[c]);
            }
        }
        let digest: [u8; 32] = hash.finalize().into();
        let claim = LoadingPresenceSource3D {
            original_binding: digest,
            source_revision: std::num::NonZeroU64::new(1).unwrap(),
            source_generation: std::num::NonZeroU64::new(1).unwrap(),
            original_root: "HAL381 owned depth fixture".into(),
            original_resource: "actual retained triangle array".into(),
            proxy_hash_algorithm: "SHA256 LE f32 position bits".into(),
            proxy_byte_hash: Arc::from(digest),
            qualification_receipt:
                "exact identity derivative of this owned original triangle array".into(),
            model_frame_origin: origin,
            source_to_proxy: placement,
        };
        let source = Arc::new(
            GeometrySourceDescriptor::try_new(
                points
                    .into_iter()
                    .map(|p| LocalPoint3::try_new(p[0], p[1], p[2]).unwrap())
                    .collect(),
                None,
                None,
                None,
                GeometryIndices::Unindexed,
            )
            .unwrap(),
        );
        let prepared = std::thread::spawn(move || {
            LoadingPresenceProxyPrepared3D::prepare(
                source,
                claim,
                placement,
                (lo, hi),
                1 << 20,
                Arc::new(AtomicBool::new(false)),
            )
        })
        .join()
        .unwrap()
        .unwrap();
        context.admit_loading_presence_proxy(&prepared).unwrap()
    }

    fn spectral_ring_points() -> Vec<[f32; 3]> {
        // The original source is an asymmetric planar ring with a real hole
        // and a recessed upper-right corner. Each proxy triangle is identical
        // to its original, including the shared planar edges.
        let mut points = Vec::new();
        for (x0, y0, x1, y1) in [
            (-0.78, -0.64, -0.24, 0.66),
            (0.17, -0.64, 0.74, 0.23),
            (0.17, 0.23, 0.42, 0.66),
            (-0.24, -0.64, 0.17, -0.19),
            (-0.24, 0.23, 0.17, 0.66),
        ] {
            let a = [x0, y0, 0.0];
            let b = [x1, y0, 0.0];
            let c = [x1, y1, 0.0];
            let d = [x0, y1, 0.0];
            points.extend([a, b, c, a, c, d]);
        }
        points
    }

    fn spectral_fixture_view() -> super::super::ViewDescriptor3 {
        use crate::three_d::{OrthographicProjection, RigidPose3, ViewDescriptor3};
        ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(2.0, 0.1, 10.0)
                .unwrap()
                .into(),
        )
    }

    #[test]
    fn hal381_profile_queue_completion_cost() {
        use crate::renderer::wgpu_renderer::WgpuRenderer;
        use crate::three_d::Context3D;
        use std::time::Instant;

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        assert!(gpu.device.features().is_empty());
        let adapter = renderer.headless_adapter_for_test().clone();
        assert_eq!(adapter.backend, wgpu::Backend::Metal);
        let context = Context3D::new(gpu.clone()).unwrap();
        let source = gpu_proxy(&context, spectral_ring_points());
        let (w, h) = (1600, 1200);
        for n in [1, 4] {
            let mut target = PresenceGpuTarget::new(&context, &source, w, h, n, None).unwrap();
            let mut encode = [0; 7];
            let mut completion = [0; 7];
            for frame in 0..10 {
                let start = Instant::now();
                assert!(
                    target
                        .render(
                            spectral_fixture_view(),
                            frame as f32 * 0.1,
                            0.0,
                            frame + 1,
                            None,
                            None
                        )
                        .unwrap()
                );
                let encoded = start.elapsed();
                renderer.drain_pending_image_registrations();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                if frame >= 3 {
                    encode[(frame - 3) as usize] = encoded.as_micros();
                    completion[(frame - 3) as usize] = start.elapsed().as_micros();
                }
            }
            encode.sort_unstable();
            completion.sort_unstable();
            eprintln!(
                "HAL381 {} {w}x{h} N{n}, 7 frames after 3 warmup, median encode {} us, encode+submit+GPU completion {} us, samples {completion:?}",
                adapter.name, encode[3], completion[3],
            );
            drop(target);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
    }

    fn texture_samples(
        target: &PresenceGpuTarget,
        view: &wgpu::TextureView,
        n: u32,
    ) -> Vec<[f32; 4]> {
        use wgpu::util::DeviceExt;
        let gpu = &target.gpu;
        let (w, h) = target.extent;
        let count = w * h * n;
        let bytes = u64::from(count) * 16;
        let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HAL381 bounded actual sample observation"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HAL381 test sample readback"),
                contents: &vec![0; bytes as usize],
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            });
        let text = format!(
            r#"
@group(0) @binding(0) var input:{};
@group(0) @binding(1) var<storage,read_write> output:array<vec4<f32>>;
@compute @workgroup_size(8,8,1) fn main(@builtin(global_invocation_id) p:vec3<u32>) {{
 if p.x>={w}u || p.y>={h}u || p.z>={n}u {{return;}}
 output[(p.y*{w}u+p.x)*{n}u+p.z]=textureLoad(input,vec2<i32>(p.xy),{});
}}
"#,
            if n == 4 {
                "texture_multisampled_2d<f32>"
            } else {
                "texture_2d<f32>"
            },
            if n == 4 { "i32(p.z)" } else { "0" }
        );
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("HAL381 test observes private FP16 samples"),
                source: wgpu::ShaderSource::Wgsl(text.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("HAL381 sample observation only"),
                layout: None,
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &binding, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), n);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &read, 0, bytes);
        gpu.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        read.slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let data = read.slice(..).get_mapped_range().expect("mapped readback range");
        let result = bytemuck::cast_slice::<u8, [f32; 4]>(&data).to_vec();
        drop(data);
        read.unmap();
        result
    }

    #[test]
    fn hal381_spectral_contour_follows_real_coverage_and_physical_pixels_n1_n4() {
        use crate::renderer::wgpu_renderer::WgpuRenderer;
        use crate::three_d::Context3D;

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        assert_eq!(
            renderer.headless_adapter_for_test().backend,
            wgpu::Backend::Metal
        );
        assert!(gpu.device.features().is_empty());
        let context = Context3D::new(gpu.clone()).unwrap();
        let source = gpu_proxy(&context, spectral_ring_points());
        let view = spectral_fixture_view();
        let smooth = |lo: f32, hi: f32, x: f32| {
            let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        for n in [1, 4] {
            // Same source and camera at 1x, 2x and a non-square resized target.
            for (width, height) in [(128, 128), (256, 256), (176, 144)] {
                let mut target =
                    PresenceGpuTarget::new(&context, &source, width, height, n, None).unwrap();
                assert!(target.render(view, 0.0, 0.0, 1, None, None).unwrap());
                renderer.drain_pending_image_registrations();
                let proxy = texture_samples(&target, &target.work.proxy_view, n);
                let output_view = target.output_texture().create_view(&Default::default());
                let output = texture_samples(&target, &output_view, 1);
                let coverage: Vec<_> = proxy
                    .chunks_exact(n as usize)
                    .map(|p| p.iter().filter(|s| s[3] > 0.0).count() as u32)
                    .collect();
                let at = |x, y| (y * width + x) as usize;
                let world = |x: f32, y: f32| {
                    at(
                        (width as f32 * 0.5 + x * height as f32 * 0.5) as u32,
                        (height as f32 * 0.5 - y * height as f32 * 0.5) as u32,
                    )
                };
                let c = |x: i32, y: i32| {
                    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                        0.0
                    } else {
                        coverage[at(x as u32, y as u32)] as f32 / n as f32
                    }
                };
                assert_eq!(coverage[world(0.0, 0.0)], 0, "real central hole");
                assert_eq!(coverage[world(0.58, 0.48)], 0, "real concave corner");
                let interior = world(-0.51, 0.0);
                assert_eq!(coverage[interior], n);
                assert_eq!(coverage[world(0.14, 0.0)], 0, "near inner hole edge");
                let partial = coverage
                    .iter()
                    .filter(|&&count| count > 0 && count < n)
                    .count();
                assert_eq!(partial > 0, n == 4);
                let covered = proxy.iter().filter(|s| s[3] > 0.0);
                let (a_min, a_max) = covered.fold((1.0_f32, 0.0_f32), |(min, max), s| {
                    (min.min(s[3]), max.max(s[3]))
                });
                assert!(a_min >= 0.08 && a_max <= 0.24 && a_max - a_min > 0.01);

                // Independent disk oracle over the observed private FP16
                // samples, including partially covered N4 destinations.
                for y in 0..height as i32 {
                    for x in 0..width as i32 {
                        let i = at(x as u32, y as u32);
                        let s = &proxy[i * n as usize..(i + 1) * n as usize];
                        let (mut core, mut halo) = (0.0_f32, 0.0_f32);
                        let center = c(x, y);
                        if center < 1.0 {
                            for dy in -4..=4 {
                                for dx in -4..=4 {
                                    let r2 = dx * dx + dy * dy;
                                    if (1..25).contains(&r2) {
                                        let d = (r2 as f32).sqrt();
                                        let delta = (c(x + dx, y + dy) - center).max(0.0);
                                        core = core.max(delta * (1.0 - smooth(1.0, 2.0, d)));
                                        halo = halo.max(delta * (1.0 - smooth(1.0, 5.0, d)));
                                    }
                                }
                            }
                        }
                        assert_eq!(output[i][3], 1.0);
                        let tint = 0.24 * core + 0.08 * halo;
                        for (channel, edge) in [0.58, 0.78, 0.88].into_iter().enumerate() {
                            let base = s
                                .iter()
                                .map(|s| s[channel] + (1.0 - s[3]) * NEUTRAL[channel])
                                .sum::<f32>()
                                / n as f32;
                            let expected = base + tint * (edge - base);
                            assert!(
                                (output[i][channel] - expected).abs() <= 2.0_f32.powi(-11),
                                "{width}x{height} N{n} ({x},{y}) c{channel}: {:?} != {expected}",
                                output[i]
                            );
                        }
                    }
                }
                let y = height / 2;
                let left = (0..width).find(|&x| coverage[at(x, y)] != 0).unwrap();
                let rim = output[at(left - 1, y)];
                let halo = output[at(left - 4, y)];
                let outside = output[at(left - 5, y)];
                assert!(rim[2] > output[interior][2] + 0.08);
                assert!(halo[2] > outside[2] + 0.0015);
                assert!((outside[2] - NEUTRAL[2]).abs() < 0.0001);
                // Locate the real inner edge without assuming an exact sample
                // pattern or a DPI-dependent projection rounding mode.
                let hole_right = (width / 2..width)
                    .find(|&x| coverage[at(x, y)] != 0)
                    .unwrap();
                assert_eq!(coverage[at(hole_right - 1, y)], 0);
                assert!(output[at(hole_right - 1, y)][2] > outside[2] + 0.05);
                assert_eq!(output[world(0.0, 0.0)], outside);
                eprintln!(
                    "HAL381 spectral {width}x{height} N{n}: interior={:?}, rim={rim:?}, halo4={halo:?}, outside5={outside:?}, private alpha=[{a_min}, {a_max}], partial_pixels={partial}",
                    output[interior],
                );
                if (width, height) == (128, 128) {
                    assert!(
                        target
                            .render(view, std::f32::consts::FRAC_PI_2, 0.0, 2, None, None)
                            .unwrap()
                    );
                    renderer.drain_pending_image_registrations();
                    let animated = texture_samples(&target, &target.work.proxy_view, n);
                    assert_ne!(animated, proxy, "only the appearance advances");
                    assert!(
                        animated
                            .iter()
                            .zip(&proxy)
                            .all(|(a, b)| (a[3] > 0.0) == (b[3] > 0.0))
                    );
                    assert!(target.render(view, 0.0, 0.0, 3, None, None).unwrap());
                    renderer.drain_pending_image_registrations();
                    assert_eq!(
                        texture_samples(&target, &target.work.proxy_view, n),
                        proxy,
                        "identical static phase returns identical samples"
                    );
                }
                drop(target);
                renderer.drain_pending_image_registrations();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
        }
    }

    #[test]
    fn hal381_actual_proxy_sample_coverage_and_reverse_z_self_occlusion_n1_n4() {
        use crate::three_d::{Context3D, OrthographicProjection, RigidPose3, ViewDescriptor3};
        for n in [1, 4] {
            let mut renderer =
                pollster::block_on(crate::renderer::wgpu_renderer::WgpuRenderer::new_headless())
                    .unwrap();
            let gpu = renderer.gpu_resources();
            assert!(gpu.device.features().is_empty());
            let context = Context3D::new(gpu.clone()).unwrap();
            let view = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(0.125, 0.25, 3.0).unwrap(),
                    Orientation3::identity(),
                ),
                OrthographicProjection::try_new(2.0, 0.1, 10.0)
                    .unwrap()
                    .into(),
            );
            let far = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
            let near = vec![[0.0, 0.0, -0.25], [0.5, 0.0, 2.25], [0.0, 1.0, -0.25]];
            let mut observations = Vec::new();
            for points in [
                far.clone(),
                far.iter().chain(&near).copied().collect(),
                near.iter().chain(&far).copied().collect(),
            ] {
                let proxy = gpu_proxy(&context, points);
                let mut target = PresenceGpuTarget::new(&context, &proxy, 64, 64, n, None).unwrap();
                assert!(target.render(view, 0.0, 0.0, 1, None, None).unwrap());
                renderer.drain_pending_image_registrations();
                let actual = texture_samples(&target, &target.work.proxy_view, n);
                let center = ((32 * 64 + 32) * n) as usize;
                observations.push(actual[center..center + n as usize].to_vec());
                if observations.len() == 1 {
                    let mut uncovered = 0;
                    let mut partial = 0;
                    let mut covered = 0;
                    for pixel in actual.chunks_exact(n as usize) {
                        let k = pixel.iter().filter(|s| s[3] != 0.0).count();
                        if k == 0 {
                            uncovered += 1;
                        } else if k == n as usize {
                            covered += 1;
                        } else {
                            partial += 1;
                        }
                        for sample in pixel {
                            if sample[3] == 0.0 {
                                assert_eq!(*sample, [0.0; 4]);
                            } else {
                                assert!(
                                    (0.08 - 2.0_f32.powi(-11)..=0.24 + 2.0_f32.powi(-11))
                                        .contains(&sample[3])
                                );
                                assert!(
                                    sample[..3]
                                        .iter()
                                        .all(|v| v.is_finite() && *v >= 0.0 && *v <= sample[3])
                                );
                            }
                        }
                    }
                    assert!(uncovered > 0 && covered > 0);
                    if n == 4 {
                        assert!(partial > 0, "actual four-sample silhouette coverage");
                    }
                }
                drop(target);
                renderer.drain_pending_image_registrations();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
            assert_eq!(
                observations[1], observations[2],
                "actual reverse-Z winner is independent of triangle order"
            );
            assert!(
                observations[1][0][0] > observations[0][0][0] + 0.04,
                "geometric rim identifies the nearer sloped surface"
            );
        }
    }
}
