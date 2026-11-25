//! HAL372's fixed whole-source presentation ABI and controlled shader transform.
//! Source qualification, time and target publication remain with their owners.

use super::context::{ControlledResourceLease3D, DynamicStorageLease3D, NormalCpuPendingCharge3D};
use super::material_api::{PbrScene3D, PbrSourceError3D};
use super::{
    Context3D, Context3DError, GeometryIndices, ModelPlacement3, PolicyBudget3D, WorldPoint3,
};
use super::{appearance::ObjectBlockBytes, pbr::PbrError3D};
use std::{
    fmt,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct CpuPacket {
    vertices: Arc<[super::geometry::PbrVertexSource3D]>,
    indices: GeometryIndices,
    placement: ModelPlacement3,
}

/// HAL-owned CPU capture; the caller's existing CPU executor runs `verify`.
/// No Context, GPU object or registered resource lease is reachable from this job.
pub struct PbrObjectRevealExtentCpuJob3D {
    packets: Vec<CpuPacket>,
    // Must be dropped after every worker-held source Arc, including cancellation.
    ticket: Arc<()>,
}

/// UI-only source association and original-source retention escrow.
/// ```compile_fail
/// use halcyon::three_d::PbrObjectRevealExtentPending3D;
/// fn send<T: Send>() {}
/// send::<PbrObjectRevealExtentPending3D>();
/// ```
pub struct PbrObjectRevealExtentPending3D {
    context: Context3D,
    source: PbrScene3D,
    ticket: Option<Arc<()>>,
    charge: Option<Box<NormalCpuPendingCharge3D>>,
    _ui: PhantomData<Rc<()>>,
}

impl Drop for PbrObjectRevealExtentPending3D {
    fn drop(&mut self) {
        drop(self.ticket.take());
        if let Some(charge) = self.charge.take() {
            self.context.retire_normal_cpu_charge(charge);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PbrObjectRevealExtentCpuError3D {
    #[error("PBR object reveal extent cancelled")]
    Cancelled,
    #[error(transparent)]
    Source(#[from] PbrSourceError3D),
}

/// One-use HAL-created source-world extent result; not caller-authored bounds.
pub struct PbrObjectRevealExtentCpuResult3D {
    minimum: [f64; 3],
    maximum: [f64; 3],
    axis: usize,
    ticket: Arc<()>,
}

struct Extent {
    source: PbrScene3D,
    minimum: [f64; 3],
    maximum: [f64; 3],
    axis: usize,
    _charge: DynamicStorageLease3D,
}

/// Opaque UI-owned, exact-source-qualified whole-scene extent.
#[derive(Clone)]
pub struct PbrObjectRevealExtent3D {
    inner: Arc<Extent>,
    _ui: PhantomData<Rc<()>>,
}

/// Explicit whole-current-source request; geometry/semantics remain unchanged.
#[derive(Debug, Clone)]
pub struct PbrObjectRevealRequest3D {
    pub source: PbrScene3D,
    pub request_revision: u64,
    pub duration: std::time::Duration,
    pub enabled: bool,
    pub extent: Option<PbrObjectRevealExtent3D>,
}

#[derive(Clone)]
pub(crate) struct ObjectRevealFrame3D {
    pub(crate) extent: PbrObjectRevealExtent3D,
    pub(crate) progress: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ObjectRevealTail3D {
    progress: f32,
    lower_axis: f32,
    span_axis: f32,
    axis: u32,
}

impl ObjectRevealTail3D {
    fn checked(
        progress: f32,
        lower_axis: f32,
        span_axis: f32,
        axis: u32,
    ) -> Result<Self, PbrError3D> {
        if !progress.is_finite()
            || !(0.0..=1.0).contains(&progress)
            || !lower_axis.is_finite()
            || !span_axis.is_finite()
            || span_axis <= 0.0
            || !(lower_axis + span_axis).is_finite()
            || axis > 2
        {
            return Err(PbrError3D::Association(
                "unqualified Object128 reveal parameters",
            ));
        }
        Ok(Self {
            progress,
            lower_axis,
            span_axis,
            axis,
        })
    }
}
const _: [(); 16] = [(); std::mem::size_of::<ObjectRevealTail3D>()];

impl fmt::Debug for PbrObjectRevealExtent3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PbrObjectRevealExtent3D")
            .field("source", &self.inner.source.identity())
            .field("axis", &self.inner.axis)
            .finish_non_exhaustive()
    }
}

fn refusal(cause: &'static str) -> PbrSourceError3D {
    PbrSourceError3D::Association(cause)
}
fn cpu_budget(requested: usize, maximum: usize) -> PbrSourceError3D {
    Context3DError::PolicyBudgetExceeded {
        budget: PolicyBudget3D::RetainedDynamicCpuBytes,
        requested: requested as u64,
        maximum: maximum as u64,
    }
    .into()
}
fn bytes(count: usize, size: usize) -> Result<usize, PbrSourceError3D> {
    count
        .checked_mul(size)
        .ok_or_else(|| refusal("reveal CPU size overflow"))
}
fn sum(a: usize, b: usize) -> Result<usize, PbrSourceError3D> {
    a.checked_add(b)
        .ok_or_else(|| refusal("reveal CPU size overflow"))
}
fn vector<T>(count: usize) -> Result<Vec<T>, PbrSourceError3D> {
    let mut value = Vec::new();
    value
        .try_reserve_exact(count)
        .map_err(|_| refusal("reveal CPU descriptor allocation unavailable"))?;
    Ok(value)
}

#[derive(Clone, Copy, Default)]
struct Allocation {
    address: usize,
    bytes: usize,
}

// Fixed-pass address radix sort: O(packet count), without source-array reads or
// a data-dependent hash table allocation on the UI owner.
fn allocation_bytes(
    values: &mut Vec<Allocation>,
    scratch: &mut Vec<Allocation>,
) -> Result<usize, PbrSourceError3D> {
    scratch.resize(values.len(), Allocation::default());
    for shift in (0..usize::BITS).step_by(8) {
        let mut buckets = [0usize; 256];
        for value in values.iter() {
            buckets[(value.address >> shift) & 255] += 1;
        }
        let mut start = 0;
        for count in &mut buckets {
            let n = *count;
            *count = start;
            start += n;
        }
        for value in values.iter() {
            let bucket = &mut buckets[(value.address >> shift) & 255];
            scratch[*bucket] = *value;
            *bucket += 1;
        }
        std::mem::swap(values, scratch);
    }
    let mut total = 0usize;
    let mut previous: Option<Allocation> = None;
    for &value in values.iter() {
        if previous.is_some_and(|p| p.address == value.address) {
            if previous.unwrap().bytes != value.bytes {
                return Err(refusal("reveal Arc allocation span mismatch"));
            }
            continue;
        }
        total = sum(total, sum(value.bytes, 2 * std::mem::size_of::<usize>())?)?;
        previous = Some(value);
    }
    Ok(total)
}

fn live_source(context: &Context3D, source: &PbrScene3D) -> Result<(), PbrSourceError3D> {
    if !context.same_resource_family(&source.0.snapshot.context) {
        return Err(Context3DError::ContextMismatch.into());
    }
    if context.device_failure().is_some() {
        return Err(refusal("reveal Device has failed"));
    }
    if !Arc::ptr_eq(&source.0.snapshot, &source.0.model.snapshot)
        || source.0.snapshot.packets.len() != source.0.model.placements.len()
    {
        return Err(refusal("reveal source/model association mismatch"));
    }
    Ok(())
}

impl Context3D {
    pub fn begin_pbr_object_reveal_extent(
        &self,
        source: &PbrScene3D,
        maximum_cpu_bytes: usize,
    ) -> Result<
        (
            PbrObjectRevealExtentCpuJob3D,
            PbrObjectRevealExtentPending3D,
        ),
        PbrSourceError3D,
    > {
        live_source(self, source)?;
        let count = source.0.snapshot.packets.len();
        if count == 0 {
            return Err(refusal("reveal requires drawable PBR triangles"));
        }
        let rows = bytes(count, 2)?;
        let persistent = sum(
            bytes(
                count,
                std::mem::size_of::<CpuPacket>() + std::mem::size_of::<ControlledResourceLease3D>(),
            )?,
            std::mem::size_of::<PbrObjectRevealExtentCpuJob3D>()
                + std::mem::size_of::<PbrObjectRevealExtentPending3D>()
                + std::mem::size_of::<PbrObjectRevealExtentCpuResult3D>()
                + std::mem::size_of::<NormalCpuPendingCharge3D>()
                + 2 * std::mem::size_of::<usize>(),
        )?;
        let scratch_bytes = bytes(rows, 2 * std::mem::size_of::<Allocation>())?;
        let peak = sum(persistent, scratch_bytes)?;
        if peak > maximum_cpu_bytes {
            return Err(cpu_budget(peak, maximum_cpu_bytes));
        }
        let ticket = Arc::new(());
        let mut charge =
            self.reserve_normal_cpu_charge(persistent as u64, Arc::downgrade(&ticket))?;
        let _scratch_charge = self.reserve_dynamic_storage(0, 0, 0, scratch_bytes as u64)?;
        let mut allocations = vector(rows)?;
        let mut scratch = vector(rows)?;
        // Only addresses/lengths are inspected before the budget admits captures.
        for packet in source.0.snapshot.packets.iter() {
            let super::snapshot::CommittedMaterialPacket3D::PbrMaterial {
                geometry, material, ..
            } = packet
            else {
                return Err(refusal("reveal requires a complete PBR-only source"));
            };
            if !geometry.ownership.belongs_to(self) || !material.belongs_to(self) {
                return Err(Context3DError::ContextMismatch.into());
            }
            let geometry = &geometry.data.source;
            let elements = if geometry.indices.is_indexed() {
                geometry.indices.len()
            } else {
                geometry.vertices.len()
            };
            if elements == 0 || elements % 3 != 0 {
                return Err(refusal("reveal requires complete PBR triangles"));
            }
            allocations.push(Allocation {
                address: Arc::as_ptr(&geometry.vertices) as *const () as usize,
                bytes: bytes(
                    geometry.vertices.len(),
                    std::mem::size_of::<super::geometry::PbrVertexSource3D>(),
                )?,
            });
            match &geometry.indices {
                GeometryIndices::Unindexed => {}
                GeometryIndices::U16(v) => allocations.push(Allocation {
                    address: Arc::as_ptr(v) as *const () as usize,
                    bytes: bytes(v.len(), 2)?,
                }),
                GeometryIndices::U32(v) => allocations.push(Allocation {
                    address: Arc::as_ptr(v) as *const () as usize,
                    bytes: bytes(v.len(), 4)?,
                }),
            }
        }
        let physical = sum(peak, allocation_bytes(&mut allocations, &mut scratch)?)?;
        if physical > maximum_cpu_bytes {
            return Err(cpu_budget(physical, maximum_cpu_bytes));
        }
        let mut packets = vector(count)?;
        let mut escrow = vector(count)?;
        for (packet, &placement) in source
            .0
            .snapshot
            .packets
            .iter()
            .zip(source.0.model.placements.iter())
        {
            let super::snapshot::CommittedMaterialPacket3D::PbrMaterial { geometry, .. } = packet
            else {
                unreachable!("validated packet closure")
            };
            packets.push(CpuPacket {
                vertices: geometry.data.source.vertices.clone(),
                indices: geometry.data.source.indices.clone(),
                placement,
            });
            escrow.push(geometry.ownership.clone());
        }
        self.escrow_reveal_geometry(&mut charge, escrow);
        Ok((
            PbrObjectRevealExtentCpuJob3D {
                packets,
                ticket: ticket.clone(),
            },
            PbrObjectRevealExtentPending3D {
                context: self.clone(),
                source: source.clone(),
                ticket: Some(ticket),
                charge: Some(charge),
                _ui: PhantomData,
            },
        ))
    }

    pub fn finish_pbr_object_reveal_extent(
        &self,
        pending: PbrObjectRevealExtentPending3D,
        result: PbrObjectRevealExtentCpuResult3D,
        current: &PbrScene3D,
    ) -> Result<PbrObjectRevealExtent3D, PbrSourceError3D> {
        if !self.same_resource_family(&pending.context) {
            return Err(Context3DError::ContextMismatch.into());
        }
        if !pending.source.same_identity(current)
            || !pending
                .ticket
                .as_ref()
                .is_some_and(|t| Arc::ptr_eq(t, &result.ticket))
        {
            return Err(refusal("stale/mismatched reveal extent result"));
        }
        live_source(self, current)?;
        let retained = std::mem::size_of::<Extent>() + 2 * std::mem::size_of::<usize>();
        let charge = self.reserve_dynamic_storage(0, 0, 0, retained as u64)?;
        Ok(PbrObjectRevealExtent3D {
            inner: Arc::new(Extent {
                source: current.clone(),
                minimum: result.minimum,
                maximum: result.maximum,
                axis: result.axis,
                _charge: charge,
            }),
            _ui: PhantomData,
        })
    }
}

impl PbrObjectRevealExtentCpuJob3D {
    pub fn verify(
        self,
        cancel: Arc<AtomicBool>,
    ) -> Result<PbrObjectRevealExtentCpuResult3D, PbrObjectRevealExtentCpuError3D> {
        let mut minimum = [f64::INFINITY; 3];
        let mut maximum = [f64::NEG_INFINITY; 3];
        for packet in &self.packets {
            if cancel.load(Ordering::Acquire) {
                return Err(PbrObjectRevealExtentCpuError3D::Cancelled);
            }
            let q = super::direct_shadow::orientation64(packet.placement.orientation());
            let s = packet.placement.scale();
            let scale = glam::DVec3::new(f64::from(s.x()), f64::from(s.y()), f64::from(s.z()));
            let t = packet.placement.translation();
            let translation = glam::DVec3::new(t.x(), t.y(), t.z());
            let count = if packet.indices.is_indexed() {
                packet.indices.len()
            } else {
                packet.vertices.len()
            };
            for corner in 0..count {
                if cancel.load(Ordering::Acquire) {
                    return Err(PbrObjectRevealExtentCpuError3D::Cancelled);
                }
                let index = match &packet.indices {
                    GeometryIndices::Unindexed => corner,
                    GeometryIndices::U16(v) => usize::from(v[corner]),
                    GeometryIndices::U32(v) => v[corner] as usize,
                };
                let p = packet
                    .vertices
                    .get(index)
                    .ok_or_else(|| refusal("reveal index exceeds source vertices"))?
                    .position;
                let world = q
                    * (glam::DVec3::new(f64::from(p.x()), f64::from(p.y()), f64::from(p.z()))
                        * scale)
                    + translation;
                if !world.is_finite() {
                    return Err(refusal("reveal source-world arithmetic is nonfinite").into());
                }
                for axis in 0..3 {
                    minimum[axis] = minimum[axis].min(world[axis]);
                    maximum[axis] = maximum[axis].max(world[axis]);
                }
            }
        }
        let axis = [1usize, 0, 2]
            .into_iter()
            .find(|&i| maximum[i] > minimum[i])
            .ok_or_else(|| refusal("reveal source-world extent is degenerate"))?;
        if !(maximum[axis] - minimum[axis]).is_finite() {
            return Err(refusal("reveal source-world span is nonfinite").into());
        }
        if cancel.load(Ordering::Acquire) {
            return Err(PbrObjectRevealExtentCpuError3D::Cancelled);
        }
        drop(self.packets); // Last worker source references retire BEFORE its ticket.
        Ok(PbrObjectRevealExtentCpuResult3D {
            minimum,
            maximum,
            axis,
            ticket: self.ticket,
        })
    }
}

impl PbrObjectRevealExtent3D {
    #[cfg(test)]
    pub(crate) fn reference_for_test(&self) -> ([f64; 3], [f64; 3], usize) {
        (self.inner.minimum, self.inner.maximum, self.inner.axis)
    }
    pub(crate) fn matches(&self, source: &Arc<super::pbr::MaterialSceneSource3D>) -> bool {
        Arc::ptr_eq(&self.inner.source.0, source)
    }

    pub(crate) fn matches_source(&self, source: &super::pbr::MaterialSceneSource3D) -> bool {
        std::ptr::eq(self.inner.source.0.as_ref(), source)
    }

    pub(crate) fn parameters(
        &self,
        progress: f32,
        origin: WorldPoint3,
    ) -> Result<ObjectRevealTail3D, PbrError3D> {
        let i = self.inner.axis;
        let origin = [origin.x(), origin.y(), origin.z()][i];
        let k = self
            .inner
            .source
            .0
            .direct
            .as_ref()
            .map_or(1.0, |d| d.meters_per_scene_unit);
        let lower = ((self.inner.minimum[i] - origin) * k) as f32;
        let span = ((self.inner.maximum[i] - self.inner.minimum[i]) * k) as f32;
        if lower + span <= lower {
            return Err(PbrError3D::Association(
                "reveal extent collapses in render-relative f32 frame",
            ));
        }
        ObjectRevealTail3D::checked(progress, lower, span, i as u32)
    }
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Object128Reveal {
    original: ObjectBlockBytes,
    progress: f32,
    lower_axis: f32,
    span_axis: f32,
    axis: u32,
}

const _: [(); 128] = [(); std::mem::size_of::<Object128Reveal>()];
const _: [(); 16] = [(); std::mem::align_of::<Object128Reveal>()];
const _: [(); 112] = [(); std::mem::offset_of!(Object128Reveal, progress)];
const _: [(); 116] = [(); std::mem::offset_of!(Object128Reveal, lower_axis)];
const _: [(); 120] = [(); std::mem::offset_of!(Object128Reveal, span_axis)];
const _: [(); 124] = [(); std::mem::offset_of!(Object128Reveal, axis)];

impl Object128Reveal {
    #[cfg(test)]
    pub(crate) fn checked(
        original: ObjectBlockBytes,
        progress: f32,
        lower_axis: f32,
        span_axis: f32,
        axis: u32,
    ) -> Result<Self, PbrError3D> {
        ObjectRevealTail3D::checked(progress, lower_axis, span_axis, axis)?;
        Ok(Self {
            original,
            progress,
            lower_axis,
            span_axis,
            axis,
        })
    }
}

// Exact controlled-source anchors. A changed original is a typed refusal, never
// an unnoticed partial transform or an application-programmable shader seam.
const OBJECT112: &str =
    "struct Object112 { model_relative:mat4x4<f32>, normal_to_world:mat3x3<f32> }";
const OBJECT128: &str = "struct Object128Reveal { model_relative:mat4x4<f32>, normal_to_world:mat3x3<f32>, progress:f32, lower_axis:f32, span_axis:f32, axis:u32 }";
const OBJECT_BINDING: &str = "@group(1) @binding(0) var<uniform> object:Object112;";
const REVEAL_BINDING: &str = "@group(1) @binding(0) var<uniform> object:Object128Reveal;";
const VISIBLE: &str = r#"
fn object_reveal_visible(position:vec3<f32>)->bool {
 if object.progress==0.0 {return false;}
 if object.progress==1.0 {return true;}
 return position[object.axis]<=object.lower_axis+object.span_axis*object.progress;
}
"#;

pub(crate) fn controlled_material_source(
    profile: super::indirect::Presence3D,
    sampling: super::mip_sampling::SamplingFamily3D,
    reveal: bool,
) -> Result<String, PbrError3D> {
    if profile.0 >= 8 {
        return Err(PbrError3D::Association("unknown controlled D/E/G profile"));
    }
    let material = if sampling == super::mip_sampling::SamplingFamily3D::V2 {
        super::mip_sampling::shader_source()
    } else {
        String::from(super::pbr::PBR_WGSL)
    };
    let source = if profile.indirect() {
        super::indirect::shader_source_with_material(profile, &material)
    } else if profile.direct() {
        super::direct_shadow::shader_source_with_material(&material)
    } else {
        material
    };
    if reveal {
        shader_source(source)
    } else {
        Ok(source)
    }
}

fn replace_one(source: &mut String, original: &str, replacement: &str) -> Result<(), PbrError3D> {
    if source.matches(original).count() != 1 {
        return Err(PbrError3D::Association(
            "controlled reveal shader anchor changed",
        ));
    }
    *source = source.replacen(original, replacement, 1);
    Ok(())
}

pub(crate) fn shader_source(mut source: String) -> Result<String, PbrError3D> {
    replace_one(&mut source, OBJECT112, OBJECT128)?;
    replace_one(&mut source, OBJECT_BINDING, REVEAL_BINDING)?;
    let entries = [
        (
            "pbr_fragment",
            "@fragment fn pbr_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {\n let s=surface(i);\n if material.role_mode.y==1u && s.alpha<material.mr_ao_cutoff.w {discard;}",
        ),
        (
            "direct_fragment",
            "@fragment fn direct_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {\n let s=surface(i);if material.role_mode.y==1u&&s.alpha<material.mr_ao_cutoff.w{discard;}",
        ),
        (
            "indirect_fragment",
            "@fragment fn indirect_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32>{\nlet s=surface(i);if material.role_mode.y==1u&&s.alpha<material.mr_ao_cutoff.w{discard;}",
        ),
    ];
    let mut transformed = 0;
    for (name, anchor) in entries {
        if source.contains(&format!("@fragment fn {name}(")) {
            let replacement =
                format!("{anchor}\n if !object_reveal_visible(i.position) {{discard;}}");
            replace_one(&mut source, anchor, &replacement)?;
            transformed += 1;
        }
    }
    if transformed == 0 {
        return Err(PbrError3D::Association(
            "controlled reveal color entry absent",
        ));
    }
    source.push_str(VISIBLE);
    Ok(source)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::renderer::wgpu_renderer::WgpuRenderer;
    use crate::three_d::{Context3D, indirect::Presence3D, mip_sampling::SamplingFamily3D};

    #[test]
    fn hal372_real_device_object128_color_and_complete_shadow_pipelines() {
        let renderer = pollster::block_on(WgpuRenderer::new_headless())
            .expect("HAL372 actual backend pipeline proof requires a Device");
        let info = renderer.headless_adapter_for_test();
        eprintln!("HAL372 Object128 pipeline Device: {info:?}");
        if cfg!(target_os = "macos") {
            assert_eq!(info.backend, wgpu::Backend::Metal);
        }
        let context = Context3D::new(renderer.gpu_resources()).unwrap();
        let profile = Presence3D(7);
        let original = context.mip_program(profile).unwrap();
        let reveal = context
            .reveal_program(profile, SamplingFamily3D::V2)
            .unwrap();
        let again = context
            .reveal_program(profile, SamplingFamily3D::V2)
            .unwrap();
        assert!(Arc::ptr_eq(&reveal, &again));
        assert!(!Arc::ptr_eq(&original, &reveal));
        assert_eq!(original.object_binding_bytes, 112);
        assert_eq!(reveal.object_binding_bytes, 128);
        for samples in [1, 4] {
            for cull in [false, true] {
                let pipeline = reveal.pipeline(&context, samples, cull, false).unwrap();
                let cached = reveal.pipeline(&context, samples, cull, false).unwrap();
                assert!(Arc::ptr_eq(&pipeline, &cached));
                original.pipeline(&context, samples, cull, false).unwrap();
            }
        }
        for cull in [false, true] {
            for cube in [false, true] {
                reveal.shadow_pipeline(&context, cull, cube).unwrap();
                original.shadow_pipeline(&context, cull, cube).unwrap();
            }
        }
    }
}
