//! HAL375 passive, source-associated viewport annotations.

use std::{num::NonZeroU64, sync::Arc};

use crate::core::{
    math::{Point, RectF},
    paragraph::TextStyle,
    scene::Color,
    text_engine::{TextBinding, TextEngine},
    theme::Theme,
};
use crate::renderer::custom_render_prepare::CustomRenderTargetMetrics;

use super::{
    Context3D, Context3DError, PbrScene3D, PbrSourceError3D, PbrSourceResult3D, ViewDescriptor3,
    WorldPoint3,
    context::{ContextId, DynamicStorageLease3D},
    snapshot::CommittedMaterialPacket3D,
};

pub(crate) const MAX_LABELS: usize = 8;
pub(crate) const MAX_GLYPHS: usize = 64;
pub(crate) const FONT_SIZE: f32 = 13.0;
pub(crate) const STROKE_WIDTH: f32 = 1.5;
pub(crate) const MAX_PIXEL_CENTERS: u32 = 1_048_576;
pub(crate) const PADDING_X: f32 = 8.0;
pub(crate) const PADDING_Y: f32 = 4.0;

/// Consumer-owned text and world point for one occurrence of an admitted PBR scene.
#[derive(Debug, Clone, PartialEq)]
pub struct PbrSpatialHudAnchor3D {
    pub label_id: NonZeroU64,
    pub packet_slot: u32,
    pub world: WorldPoint3,
    pub text: Arc<str>,
}

struct HudSource {
    context: ContextId,
    scene: PbrScene3D,
    anchors: Box<[PbrSpatialHudAnchor3D]>,
    _charge: DynamicStorageLease3D,
}

/// Immutable HAL-admitted association of an ordered passive HUD with one PBR scene.
#[derive(Clone)]
pub struct PbrSpatialHud3D(Arc<HudSource>);

impl std::fmt::Debug for PbrSpatialHud3D {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PbrSpatialHud3D")
            .field("anchors", &self.0.anchors.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn refused(cause: &'static str) -> PbrSourceError3D {
    PbrSourceError3D::Source {
        field: "HAL375 spatial HUD",
        cause,
    }
}

impl PbrSpatialHud3D {
    pub(super) fn validate_context(&self, context: &Context3D) -> PbrSourceResult3D<()> {
        if !self.0.context.same_as(&context.context_identity()) {
            return Err(Context3DError::ContextMismatch.into());
        }
        if context.device_failure().is_some() || !context.gpu_resources()?.private_hdr_owner_live()
        {
            return Err(refused("renderer or Device retired"));
        }
        self.0.scene.0.snapshot.validate_live_resources(context)?;
        Ok(())
    }

    pub(super) fn matches_source(&self, scene: &Arc<super::pbr::MaterialSceneSource3D>) -> bool {
        Arc::ptr_eq(&self.0.scene.0, scene)
    }

    pub(super) fn same_request(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.scene.same_identity(&other.0.scene) && self.0.anchors == other.0.anchors)
    }

    pub(crate) fn anchors(&self) -> &[PbrSpatialHudAnchor3D] {
        &self.0.anchors
    }
}

impl Context3D {
    /// Associate immutable passive labels with exact PBR occurrence slots.
    /// Slot admission does not qualify the visibility of any supplied point.
    pub fn admit_pbr_spatial_hud(
        &self,
        scene: &PbrScene3D,
        anchors: &[PbrSpatialHudAnchor3D],
    ) -> PbrSourceResult3D<PbrSpatialHud3D> {
        let gpu = self.gpu_resources()?;
        if self.device_failure().is_some() || !gpu.private_hdr_owner_live() {
            return Err(refused("renderer or Device retired"));
        }
        scene.0.validate(self)?;
        qualify_depth_device(&gpu, scene.0.samples)?;
        scene.0.snapshot.validate_live_resources(self)?;
        if anchors.len() > MAX_LABELS {
            return Err(refused("label count exceeds eight"));
        }
        let mut retained = std::mem::size_of::<HudSource>()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of_val(anchors);
        for (i, anchor) in anchors.iter().enumerate() {
            if anchors[..i].iter().any(|a| a.label_id == anchor.label_id) {
                return Err(refused("duplicate label ID"));
            }
            if !matches!(
                scene.0.snapshot.packets.get(anchor.packet_slot as usize),
                Some(CommittedMaterialPacket3D::PbrMaterial { .. })
            ) {
                return Err(refused(
                    "anchor slot is not an occurrence of this PBR scene",
                ));
            }
            if ![anchor.world.x(), anchor.world.y(), anchor.world.z()]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err(refused("nonfinite anchor"));
            }
            if anchor.text.len() > 96
                || anchor.text.chars().any(|c| {
                    matches!(
                        c,
                        '\n' | '\r'
                            | '\u{000b}'
                            | '\u{000c}'
                            | '\u{0085}'
                            | '\u{2028}'
                            | '\u{2029}'
                    )
                })
            {
                return Err(refused("label must be one line of at most 96 UTF-8 bytes"));
            }
            if !anchors[..i]
                .iter()
                .any(|a| Arc::ptr_eq(&a.text, &anchor.text))
            {
                retained += 2 * std::mem::size_of::<usize>() + anchor.text.len();
            }
        }
        let charge = self.reserve_dynamic_storage(0, 0, 0, retained as u64)?;
        Ok(PbrSpatialHud3D(Arc::new(HudSource {
            context: self.context_identity(),
            scene: scene.clone(),
            anchors: anchors.into(),
            _charge: charge,
        })))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HudTypography {
    pub(crate) binding: TextBinding,
    pub(crate) line_height: f32,
    pub(crate) color: Color,
    pub(crate) background: Color,
}

impl HudTypography {
    pub(super) fn measure(engine: &mut dyn TextEngine, theme: &Theme) -> PbrSourceResult3D<Self> {
        let style = TextStyle {
            font_size: FONT_SIZE,
            ..TextStyle::default()
        };
        let resolved = engine
            .resolve_text_style(&style)
            .map_err(|_| refused("ordinary typography unavailable"))?;
        let line_height = engine
            .metrics_styled(Some(&style), FONT_SIZE)
            .map_err(|_| refused("ordinary line metrics unavailable"))?
            .line_height;
        if !line_height.is_finite() || line_height <= 0.0 || line_height > 28.0 {
            return Err(refused("label line height outside the HAL375 envelope"));
        }
        Ok(Self {
            binding: resolved.binding,
            line_height,
            color: theme.colors.typography.primary.into(),
            background: theme.colors.surfaces.level3.into(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct HudPoint {
    pub(crate) logical: Point,
    pub(crate) pixel: [u32; 2],
    pub(crate) depth: f32,
}

pub(super) fn project(
    source: &PbrSpatialHud3D,
    view: ViewDescriptor3,
    origin: WorldPoint3,
    extent: (u32, u32),
    metrics: &CustomRenderTargetMetrics,
) -> PbrSourceResult3D<[Option<HudPoint>; MAX_LABELS]> {
    let scene = &source.0.scene.0;
    let packed = super::raster::pack_material_view(scene, view, origin, extent.0, extent.1)?;
    let transform = glam::Mat4::from_cols_array_2d(&packed.view_projection());
    let rect = metrics.final_logical_rect;
    let Some(crop) = rect
        .intersection(&metrics.final_logical_viewport)
        .and_then(|r| {
            r.intersection(&RectF::new(
                0.0,
                0.0,
                metrics.target_logical_extent.width,
                metrics.target_logical_extent.height,
            ))
        })
    else {
        return Ok([None; MAX_LABELS]);
    };
    let mut result = [None; MAX_LABELS];
    for (slot, anchor) in source.anchors().iter().enumerate() {
        let p = if let Some(direct) = &scene.direct {
            let k = direct.meters_per_scene_unit;
            let metric = [
                anchor.world.x() - origin.x(),
                anchor.world.y() - origin.y(),
                anchor.world.z() - origin.z(),
            ]
            .map(|v| v * k);
            let lowered = metric.map(|v| v as f32);
            if metric
                .into_iter()
                .zip(lowered)
                .any(|(v, f)| !v.is_finite() || !f.is_finite() || (v != 0.0 && f == 0.0))
            {
                continue;
            }
            lowered
        } else {
            let Ok(p) = anchor.world.render_relative_to(origin) else {
                continue;
            };
            [p.x(), p.y(), p.z()]
        };
        let q = transform * glam::Vec4::new(p[0], p[1], p[2], 1.0);
        if !q.is_finite()
            || q.w <= 0.0
            || q.z < 0.0
            || q.z > q.w
            || q.x.abs() > q.w
            || q.y.abs() > q.w
        {
            continue;
        }
        let u = 0.5 + 0.5 * (q.x / q.w);
        let v = 0.5 - 0.5 * (q.y / q.w);
        let physical = [u * extent.0 as f32, v * extent.1 as f32];
        let logical = Point::new(rect.x + u * rect.width, rect.y + v * rect.height);
        if physical[0] >= extent.0 as f32
            || physical[1] >= extent.1 as f32
            || !crop.contains(logical)
        {
            continue;
        }
        result[slot] = Some(HudPoint {
            logical,
            pixel: [physical[0] as u32, physical[1] as u32],
            depth: q.z / q.w,
        });
    }
    Ok(result)
}

/// Resolve one actual measured label in source order against earlier reservations.
pub(crate) fn place(
    point: Point,
    width: f32,
    height: f32,
    crop: RectF,
    reserved: &[RectF],
    inside_clip: impl Fn(RectF) -> bool,
) -> Option<(RectF, Point)> {
    let candidates = [
        Point::new(point.x - width / 2.0, point.y - 6.0 - height),
        Point::new(point.x + 6.0, point.y - height / 2.0),
        Point::new(point.x - width / 2.0, point.y + 6.0),
        Point::new(point.x - 6.0 - width, point.y - height / 2.0),
    ];
    candidates.into_iter().find_map(|position| {
        let rect = RectF::new(position.x, position.y, width, height);
        if rect.intersection(&crop) != Some(rect)
            || reserved
                .iter()
                .any(|other| rect.intersection(other).is_some())
            || !inside_clip(rect)
        {
            return None;
        }
        let end = Point::new(
            point.x.clamp(rect.x, rect.x + rect.width),
            point.y.clamp(rect.y, rect.y + rect.height),
        );
        ((point.x - end.x).hypot(point.y - end.y) <= 160.0).then_some((rect, end))
    })
}

/// The actual canonical depth attachment for one HUD-enabled writer generation.
/// The HDR writer and compiled readers retain it through their Queue boundaries.
pub(crate) struct HudDepth3D {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    _charge: super::context::TargetBudgetLease3D,
    pub(crate) extent: (u32, u32),
    pub(crate) samples: u32,
}

fn qualify_depth_device(
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
    samples: u32,
) -> PbrSourceResult3D<()> {
    let features =
        wgpu::TextureFormat::Depth32Float.guaranteed_format_features(gpu.device.features());
    let usages = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    if !matches!(samples, 1 | 4)
        || !features.allowed_usages.contains(usages)
        || !features.flags.sample_count_supported(samples)
    {
        return Err(refused("Device lacks canonical depth sampling"));
    }
    Ok(())
}

impl HudDepth3D {
    pub(super) fn new(
        context: &Context3D,
        extent: (u32, u32),
        samples: u32,
        direct: bool,
    ) -> PbrSourceResult3D<Arc<Self>> {
        let gpu = context.gpu_resources()?;
        qualify_depth_device(&gpu, samples)?;
        let format = wgpu::TextureFormat::Depth32Float;
        let usages = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let p = u64::from(extent.0) * u64::from(extent.1);
        let depth_bytes = p
            .checked_mul(4 * u64::from(samples))
            .ok_or_else(|| refused("depth byte overflow"))?;
        // During usage replacement the prior ordinary attachment remains
        // charged. Only the installed canonical attachment is subsequently used.
        let combined = p
            .checked_mul(20 * u64::from(samples) + 16)
            .and_then(|n| n.checked_add(depth_bytes))
            .and_then(|n| {
                n.checked_add(if direct {
                    super::direct_shadow::POOL_BYTES
                } else {
                    0
                })
            })
            .ok_or_else(|| refused("combined target byte overflow"))?;
        let limit = context.limits().policy.max_single_target_attachment_bytes;
        if combined > limit {
            return Err(Context3DError::PolicyBudgetExceeded {
                budget: super::PolicyBudget3D::SingleTargetAttachmentBytes,
                requested: combined,
                maximum: limit,
            }
            .into());
        }
        let charge = context.reserve_target_budget(0, depth_bytes)?;
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HAL375 canonical sampled depth generation"),
            size: wgpu::Extent3d {
                width: extent.0,
                height: extent.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: usages,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let validation = pollster::block_on(validation_scope.pop());
        let memory = pollster::block_on(allocation_scope.pop());
        if let Some(error) = validation.or(memory) {
            return Err(PbrSourceError3D::Gpu(error.to_string()));
        }
        Ok(Arc::new(Self {
            _texture: texture,
            view,
            _charge: charge,
            extent,
            samples,
        }))
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

pub(crate) struct PreparedHud3D {
    pub(crate) context: Arc<Context3D>,
    pub(crate) source: PbrSpatialHud3D,
    pub(crate) metrics: CustomRenderTargetMetrics,
    pub(crate) points: [Option<HudPoint>; MAX_LABELS],
    pub(crate) typography: HudTypography,
    pub(crate) depth: Arc<HudDepth3D>,
    pub(crate) program: Arc<HudProgram3D>,
    pub(crate) front: crate::renderer::custom_render_pass::HdrFrontPin,
    pub(crate) texture_id: u64,
    pub(crate) epoch: u64,
    pub(crate) mount: u64,
    pub(crate) request: super::Viewport3DRequestSerial,
    pub(crate) storage: u64,
    pub(crate) upload: u64,
    pub(super) _cpu: DynamicStorageLease3D,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct HudVisibilityInput {
    pub(crate) anchor: [u32; 4], // pixel x/y, enabled, reserved
    pub(crate) label_bounds: [u32; 4],
    pub(crate) leader_bounds: [u32; 4],
    pub(crate) segment: [f32; 4],
    pub(crate) viewport_rect: [f32; 4],
    pub(crate) depth_and_width: [f32; 4],
    pub(crate) draws: [u32; 4], // leader first/count, text first/count
}

const VISIBILITY_SHADER: &str = r#"
struct Input {
    anchor: vec4<u32>,
    label_bounds: vec4<u32>,
    leader_bounds: vec4<u32>,
    segment: vec4<f32>,
    viewport_rect: vec4<f32>,
    depth_and_width: vec4<f32>,
    draws: vec4<u32>,
}
struct IndexedDraw {
    count: u32, instances: u32, first: u32, base: i32, first_instance: u32,
}
struct Visibility {
    visible: u32, leader: IndexedDraw, label: IndexedDraw,
}
@group(0) @binding(0) var depth: DEPTH_TYPE;
@group(0) @binding(1) var<storage, read> inputs: array<Input>;
@group(0) @binding(2) var<storage, read_write> results: array<Visibility>;
var<workgroup> visible: atomic<u32>;

fn clear_at(pixel: vec2<u32>, reference: f32) -> bool {
    var clear = true;
    for (var sample = 0; sample < SAMPLE_COUNT; sample += 1) {
        let z = textureLoad(depth, vec2<i32>(pixel), LOAD_INDEX);
        clear = clear && (z == 0.0 || reference > z);
    }
    return clear;
}

fn span(bounds: vec4<u32>) -> vec2<u32> {
    return bounds.zw - bounds.xy;
}

fn pixel_at(bounds: vec4<u32>, index: u32) -> vec2<u32> {
    let width = bounds.z - bounds.x;
    return bounds.xy + vec2(index % width, index / width);
}

fn logical_at(job: Input, pixel: vec2<u32>) -> vec2<f32> {
    return (vec2<f32>(pixel) + vec2(0.5)) * job.depth_and_width.z;
}

fn clear_surface_pixel(job: Input, pixel: vec2<u32>) -> bool {
    let uv = (logical_at(job, pixel) - job.viewport_rect.xy) / job.viewport_rect.zw;
    let size = textureDimensions(depth);
    if any(uv < vec2(0.0)) || any(uv >= vec2(1.0)) { return false; }
    let target_pixel = min(vec2<u32>(uv * vec2<f32>(size)), size - vec2(1u));
    return clear_at(target_pixel, job.depth_and_width.x);
}

fn on_leader(job: Input, pixel: vec2<u32>) -> bool {
    let p = logical_at(job, pixel);
    let direction = job.segment.zw - job.segment.xy;
    let relative = p - job.segment.xy;
    let along = dot(relative, direction);
    let square_length = dot(direction, direction);
    let across = relative.x * direction.y - relative.y * direction.x;
    return along >= 0.0 && along <= square_length
        && across * across <= job.depth_and_width.y * square_length;
}

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let job = inputs[group.x];
    if lane == 0u {
        atomicStore(&visible, job.anchor.z);
    }
    workgroupBarrier();
    if job.anchor.z != 0u {
        if lane == 0u && !clear_at(job.anchor.xy, job.depth_and_width.x) {
            atomicStore(&visible, 0u);
        }
        let box_span = span(job.label_bounds);
        let leader_span = span(job.leader_bounds);
        for (var i = lane; i < box_span.x * box_span.y; i += 64u) {
            if !clear_surface_pixel(job, pixel_at(job.label_bounds, i)) {
                atomicStore(&visible, 0u);
            }
        }
        for (var i = lane; i < leader_span.x * leader_span.y; i += 64u) {
            let pixel = pixel_at(job.leader_bounds, i);
            if on_leader(job, pixel) && !clear_surface_pixel(job, pixel) {
                atomicStore(&visible, 0u);
            }
        }
    }
    workgroupBarrier();
    if lane == 0u {
        let flag = atomicLoad(&visible);
        results[group.x] = Visibility(
            flag,
            IndexedDraw(job.draws.y, flag, job.draws.x, 0, 0u),
            IndexedDraw(job.draws.w, flag, job.draws.z, 0, 0u),
        );
    }
}
"#;

pub(crate) struct HudProgram3D {
    pub(crate) samples: u32,
    pub(crate) pipeline: wgpu::ComputePipeline,
    pub(crate) layout: wgpu::BindGroupLayout,
    _module: super::context::ShaderModuleResidency,
    _pipeline: super::raster::ControlledPipelineResidency,
}

impl HudProgram3D {
    pub(super) fn new(context: &Context3D, samples: u32) -> PbrSourceResult3D<Self> {
        let gpu = context.gpu_resources()?;
        qualify_depth_device(&gpu, samples)?;
        if !gpu.adapter.get_downlevel_capabilities().flags.contains(
            wgpu::DownlevelFlags::COMPUTE_SHADERS | wgpu::DownlevelFlags::INDIRECT_EXECUTION,
        ) {
            return Err(refused("Device lacks gated indirect HUD draws"));
        }
        let raster = context.raster_gpu()?;
        let mut pipeline_charge =
            raster.reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
        let module_charge = context.reserve_controlled_shader_module()?;
        let source = VISIBILITY_SHADER
            .replace(
                "DEPTH_TYPE",
                if samples == 4 {
                    "texture_depth_multisampled_2d"
                } else {
                    "texture_depth_2d"
                },
            )
            .replace("SAMPLE_COUNT", if samples == 4 { "4" } else { "1" })
            .replace("LOAD_INDEX", if samples == 4 { "sample" } else { "0" });
        let ir = naga::front::wgsl::parse_str(&source)
            .map_err(|e| PbrSourceError3D::Gpu(e.emit_to_string(&source)))?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&ir)
        .map_err(|e| PbrSourceError3D::Gpu(e.emit_to_string(&source)))?;
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("HAL375 canonical-depth whole-pair visibility"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("HAL375 depth visibility"),
                layout: None,
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let validation = pollster::block_on(validation_scope.pop());
        let memory = pollster::block_on(allocation_scope.pop());
        if let Some(error) = validation.or(memory) {
            return Err(PbrSourceError3D::Gpu(error.to_string()));
        }
        let layout = pipeline.get_bind_group_layout(0);
        pipeline_charge.commit();
        Ok(Self {
            samples,
            pipeline,
            layout,
            _module: module_charge.commit_one(),
            _pipeline: pipeline_charge,
        })
    }
}

pub(crate) struct HudCompileCharge {
    _storage: DynamicStorageLease3D,
}

impl PreparedHud3D {
    pub(crate) fn reserve_cpu(&self, cpu_bytes: u64) -> PbrSourceResult3D<HudCompileCharge> {
        Ok(HudCompileCharge {
            _storage: self.context.reserve_dynamic_storage(0, 0, 0, cpu_bytes)?,
        })
    }

    pub(crate) fn reserve_compile(
        &self,
        gpu_bytes: u64,
        upload_bytes: u64,
        cpu_bytes: u64,
    ) -> PbrSourceResult3D<HudCompileCharge> {
        let policy = self.context.limits().policy;
        for (budget, requested, maximum) in [
            (
                super::PolicyBudget3D::DynamicStorageBytesPerTarget,
                self.storage.saturating_add(gpu_bytes),
                policy.max_dynamic_storage_bytes_per_target,
            ),
            (
                super::PolicyBudget3D::DynamicUploadBytesPerFrame,
                self.storage.min(self.upload).saturating_add(upload_bytes),
                policy.max_dynamic_upload_bytes_per_frame,
            ),
            (
                super::PolicyBudget3D::UploadBytesPerFrame,
                self.upload.saturating_add(upload_bytes),
                policy.max_upload_bytes_per_frame,
            ),
        ] {
            if requested > maximum {
                return Err(Context3DError::PolicyBudgetExceeded {
                    budget,
                    requested,
                    maximum,
                }
                .into());
            }
        }
        Ok(HudCompileCharge {
            _storage: self
                .context
                .reserve_dynamic_storage(gpu_bytes, 0, 1, cpu_bytes)?,
        })
    }
}

impl std::fmt::Debug for PreparedHud3D {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedHud3D")
            .field("epoch", &self.epoch)
            .field("request", &self.request)
            .field("texture_id", &self.texture_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hal375_collision_reservations_follow_the_four_candidates() {
        let crop = RectF::new(6.0, 6.0, 788.0, 588.0);
        let anchor = Point::new(400.0, 300.0);
        let mut reserved = Vec::new();
        for expected in [
            RectF::new(391.0, 284.0, 18.0, 10.0),
            RectF::new(406.0, 295.0, 18.0, 10.0),
            RectF::new(391.0, 306.0, 18.0, 10.0),
            RectF::new(376.0, 295.0, 18.0, 10.0),
        ] {
            let (bounds, end) = place(anchor, 18.0, 10.0, crop, &reserved, |_| true).unwrap();
            assert_eq!(bounds, expected);
            assert_eq!((end.x - anchor.x).hypot(end.y - anchor.y), 6.0);
            reserved.push(bounds);
        }
        assert!(place(anchor, 18.0, 10.0, crop, &reserved, |_| true).is_none());
    }
}
