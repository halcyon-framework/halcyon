//! HAL395 material-local optical source, joined to the final HAL359 schedule.

use crate::compositor::SurfaceId;
use crate::core::{
    math::RectF,
    paint::{EffectivePaintOrder, GlassSurfaceOptics, RoundedCorners, SurfaceBloom},
    scene_identity::SceneNodeKey,
};
use crate::renderer::{
    RendererCompileError,
    batcher::{ClipState, FlatPrimitive, PipelineId, PrimitiveData},
    compiled_draw_plan::{SurfaceCompiledData, SurfaceDrawCommand},
    draw_plan::SurfaceSceneData,
    path::PathDraw,
    pipelines::analytic_surface::light_params,
    render_graph::GraphError,
    render_target::TargetDescriptor,
    wgpu_renderer::RoundedClipUniform,
};
use bytemuck::{Pod, Zeroable};

pub(super) mod pipelines;
#[cfg(test)]
pub(crate) mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BloomBuildRefusal {
    MissingCoating,
    UnsupportedBodyOpacity,
    InvalidGeometry,
    InvalidClip { depth: usize },
    UnsupportedClip { depth: usize },
    AmbiguousOrder,
    OccurrenceLimit { limit: usize, requested: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BloomCompileRefusal {
    TargetUnavailable,
    InvalidTarget,
    UnsupportedFormat,
    NonOpaqueClear,
    MissingCompiledSource,
    UnsupportedCompanionGraph,
    UnsupportedUsage,
    KernelLimit { required: u32, limit: u32 },
    ResourceBudget { required_bytes: u64, limit_bytes: u64 },
    Graph(GraphError),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BloomOccurrence {
    pub surface_id: SurfaceId,
    pub node_key: Option<SceneNodeKey>,
    pub ordinal: u64,
    pub pipeline: PipelineId,
    pub order: EffectivePaintOrder,
    pub rect: RectF,
    pub corners: RoundedCorners,
    pub optics: GlassSurfaceOptics,
    pub bloom: SurfaceBloom,
    pub ancestor: Option<(RectF, [f32; 4])>,
    pub clip: ClipState,
}

impl BloomOccurrence {
    pub fn from_primitive(surface_id: SurfaceId, primitive: &FlatPrimitive) -> Option<Self> {
        let (rect, corners, optics, ancestor, ordinal, node_key) = match &primitive.data {
            PrimitiveData::AnalyticSurface { rect, corners, material, ancestor, ordinal, node_key } =>
                (rect, corners, material.glass_optics()?, ancestor, ordinal, node_key),
            PrimitiveData::FrostedGlass { rect, corners, appearance, ancestor, ordinal, node_key } =>
                (rect, corners, appearance.glass_optics()?, ancestor, ordinal, node_key),
            _ => return None,
        };
        Some(Self {
            surface_id,
            node_key: *node_key,
            ordinal: *ordinal,
            pipeline: primitive.key.pipeline,
            order: primitive.key.effective_order(),
            rect: *rect,
            corners: *corners,
            optics: *optics,
            bloom: optics.bloom()?,
            ancestor: *ancestor,
            clip: primitive.clip_state,
        })
    }

    pub fn error(&self, reason: BloomCompileRefusal) -> RendererCompileError {
        RendererCompileError::Bloom {
            surface_id: self.surface_id,
            node_key: self.node_key,
            reason,
        }
    }

    fn command_index(&self, scene: &SurfaceSceneData, surface: &SurfaceCompiledData)
        -> Result<usize, BloomCompileRefusal>
    {
        use BloomCompileRefusal::MissingCompiledSource;
        let mut captures = scene.primitives.iter().filter_map(|p| Self::from_primitive(self.surface_id, p));
        if captures.next().as_ref() != Some(self) || captures.next().is_some() {
            return Err(MissingCompiledSource);
        }
        let mut commands = surface.draw_commands.iter().enumerate().filter(|(_, cmd)| {
            if cmd.order() != &self.order {
                return false;
            }
            match cmd {
                SurfaceDrawCommand::Batch { batch_index, .. } if self.pipeline == PipelineId::AnalyticSurface =>
                    surface.frame_batches.batches.get(*batch_index).is_some_and(|batch| {
                        batch.key.pipeline == self.pipeline
                            && batch.key.texture_id == Some(self.ordinal)
                            && batch.clip_state == self.clip
                            && batch.index_range.count != 0
                            && surface.analytic_surface_params_offsets.contains_key(&self.ordinal)
                    }),
                SurfaceDrawCommand::FrostedGlass { ordinal, .. } =>
                    self.pipeline == PipelineId::FrostedGlass && *ordinal == self.ordinal,
                _ => false,
            }
        });
        let (index, _) = commands.next().ok_or(MissingCompiledSource)?;
        if commands.next().is_some() {
            return Err(MissingCompiledSource);
        }
        Ok(index)
    }
}

pub(super) fn admit(
    surface: SurfaceId,
    scale: f32,
    primitives: &[FlatPrimitive],
    paths: &[PathDraw],
    path_clips: &[RectF],
) -> Result<Option<BloomOccurrence>, (Option<SceneNodeKey>, BloomBuildRefusal)> {
    let mut candidates = primitives.iter().enumerate()
        .filter_map(|(index, p)| BloomOccurrence::from_primitive(surface, p).map(|source| (index, source)));
    let Some((owner, source)) = candidates.next() else { return Ok(None) };
    if let Some((_, second)) = candidates.next() {
        return Err((second.node_key, BloomBuildRefusal::OccurrenceLimit {
            limit: 1, requested: 2 + candidates.count(),
        }));
    }
    if !scale.is_finite() || scale <= 0.0 {
        return Err((source.node_key, BloomBuildRefusal::InvalidGeometry));
    }
    let padding = 2.0 / scale;
    let radius = (3.0 * source.bloom.sigma_logical() * scale).ceil() / scale;
    // Use a conservative paint support bound, including device-pixel scissor rounding.
    let support = expand(source.rect, radius + 2.0 * padding);
    let source_clip = (!source.clip.is_none()).then(|| expand(source.clip.rect, padding));
    let overlaps = |other: Option<RectF>| {
        other.is_none_or(|bounds| {
            support.intersection(&bounds)
                .is_some_and(|overlap| source_clip.is_none_or(|clip| clip.intersection(&overlap).is_some()))
        })
    };
    let overlapping_primitive = primitives.iter().enumerate().any(|(index, p)| {
        index != owner && p.key.effective_order() == source.order
            && (p.clip_state.is_none() || overlaps(Some(expand(p.clip_state.rect, padding))))
            && overlaps(primitive_extent(&p.data, padding))
    });
    let overlapping_path = paths.iter().any(|path| {
        !path.mesh.indices.is_empty() && !path.mesh.vertices.is_empty()
            && path.effective_order() == source.order
            && overlaps((path.clip_id != 0).then(|| {
                path_clips.get(path.clip_id as usize).map(|r| expand(*r, padding))
            }).flatten())
    });
    if overlapping_primitive || overlapping_path {
        return Err((source.node_key, BloomBuildRefusal::AmbiguousOrder));
    }
    Ok(Some(source))
}

fn expand(rect: RectF, padding: f32) -> RectF {
    RectF::new(rect.x - padding, rect.y - padding, rect.width + 2.0 * padding, rect.height + 2.0 * padding)
}

// Text and other families without a qualified extent use their captured scissor.
fn primitive_extent(primitive: &PrimitiveData, padding: f32) -> Option<RectF> {
    let (rect, halo) = match primitive {
        PrimitiveData::AnalyticSurface { rect, material, .. } => (*rect, material.halo_extent()),
        PrimitiveData::FrostedGlass { rect, appearance, .. } => (*rect, appearance.halo_extent()),
        PrimitiveData::Rect { rect, .. } | PrimitiveData::BorderRect { rect, .. }
        | PrimitiveData::RoundedRect { rect, .. } | PrimitiveData::Image { rect, .. }
        | PrimitiveData::GradientRect { rect, .. } => (*rect, 0.0),
        PrimitiveData::SdfRect { quad_rect, .. } | PrimitiveData::Shadow { quad_rect, .. } => (*quad_rect, 0.0),
        PrimitiveData::LocalAmbientGlow { bounds, .. } => (*bounds, 0.0),
        _ => return None,
    };
    Some(expand(rect, halo + padding))
}

pub(super) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub(super) const KERNEL_LIMIT: u32 = 192;

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct BloomSourceUniform {
    pub rect: [f32; 4],
    pub corners: [f32; 4],
    pub ancestor: RoundedClipUniform,
    pub lights: [[[f32; 4]; 4]; 2],
    pub shape: [f32; 4],
    pub optics: [f32; 4],
    pub tint: [f32; 4],
    pub control: [u32; 4],
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct BloomFilterUniform {
    pub target: [u32; 4],
    pub input_roi: [u32; 4],
    pub output_roi: [u32; 4],
    pub kernel: [u32; 4],
    pub weights: [[f32; 4]; 49],
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct BloomCompositeUniform {
    pub output_roi: [u32; 4],
    pub target: [u32; 4],
    pub ancestor: RoundedClipUniform,
    pub response: [f32; 4],
    pub control: [u32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum BloomStage {
    Source,
    Horizontal,
    Vertical,
    Composite,
}

impl BloomStage {
    pub const ALL: [Self; 4] = [Self::Source, Self::Horizontal, Self::Vertical, Self::Composite];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Source => "bloom_source",
            Self::Horizontal => "bloom_horizontal",
            Self::Vertical => "bloom_vertical",
            Self::Composite => "bloom_composite",
        }
    }

    pub const fn uniform_size(self) -> u64 {
        match self {
            Self::Source => std::mem::size_of::<BloomSourceUniform>() as u64,
            Self::Horizontal | Self::Vertical => std::mem::size_of::<BloomFilterUniform>() as u64,
            Self::Composite => std::mem::size_of::<BloomCompositeUniform>() as u64,
        }
    }
}

pub(crate) struct BloomPlan {
    pub(super) command: usize,
    pub(super) node_key: Option<SceneNodeKey>,
    pub(super) source_stride: u64,
    pub(super) source: BloomSourceUniform,
    pub(super) horizontal: BloomFilterUniform,
    pub(super) vertical: BloomFilterUniform,
    pub(super) composite: BloomCompositeUniform,
    pub(super) scissor: [u32; 4],
}

impl BloomPlan {
    pub fn qualify(
        occurrence: &BloomOccurrence,
        scene: &SurfaceSceneData,
        surface: &SurfaceCompiledData,
        target: &TargetDescriptor,
        format: wgpu::TextureFormat,
        adapter: &wgpu::Adapter,
        limits: &wgpu::Limits,
        clip_aa: f32,
    ) -> Result<Self, BloomCompileRefusal> {
        use BloomCompileRefusal::*;
        if occurrence.surface_id != target.surface_id {
            return Err(MissingCompiledSource);
        }
        if occurrence.optics.thin_film().is_some() {
            return Err(UnsupportedCompanionGraph);
        }
        if !matches!(format, wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb) {
            return Err(UnsupportedFormat);
        }
        let clear = surface.clear_color;
        if clear.a != 1.0 || ![clear.r, clear.g, clear.b].into_iter().all(|v| (0.0..=1.0).contains(&v)) {
            return Err(NonOpaqueClear);
        }
        let size = target.physical_size;
        let scale = f64::from(target.scale_factor);
        if size.0 == 0 || size.1 == 0 || !scale.is_finite() || scale <= 0.0
            || size.0 > limits.max_texture_dimension_2d || size.1 > limits.max_texture_dimension_2d
            || limits.max_uniform_buffer_binding_size < BloomStage::Horizontal.uniform_size() as u64
            || limits.max_dynamic_uniform_buffers_per_pipeline_layout < 2
            || !clip_aa.is_finite() || clip_aa < 0.0
        {
            return Err(InvalidTarget);
        }
        let features = adapter.get_texture_format_features(format);
        let hdr = adapter.get_texture_format_features(HDR_FORMAT);
        let attachments = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;
        if !features.allowed_usages.contains(attachments | wgpu::TextureUsages::COPY_SRC)
            || !features.flags.contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
            || !hdr.allowed_usages.contains(attachments)
        {
            return Err(UnsupportedUsage);
        }
        let sigma = f64::from(occurrence.bloom.sigma_logical()) * scale;
        let radius = (3.0 * sigma).ceil() as u32;
        if radius > KERNEL_LIMIT {
            return Err(KernelLimit { required: radius, limit: KERNEL_LIMIT });
        }
        let target_rect = [0, 0, size.0, size.1];
        let edges = |rect: RectF, margin: f64| {
            [
                (f64::from(rect.x) * scale - margin).floor().clamp(0.0, f64::from(size.0)) as u32,
                (f64::from(rect.y) * scale - margin).floor().clamp(0.0, f64::from(size.1)) as u32,
                ((f64::from(rect.x) + f64::from(rect.width)) * scale + margin).ceil().clamp(0.0, f64::from(size.0)) as u32,
                ((f64::from(rect.y) + f64::from(rect.height)) * scale + margin).ceil().clamp(0.0, f64::from(size.1)) as u32,
            ]
        };
        let original_scissor = if occurrence.clip.is_none() {
            target_rect
        } else {
            let [x, y, right, bottom] = edges(occurrence.clip.rect, 0.0);
            [x, y, right.saturating_sub(x), bottom.saturating_sub(y)]
        };
        let [x, y, right, bottom] = edges(occurrence.rect, 2.0);
        let visible = intersect([x, y, right.saturating_sub(x), bottom.saturating_sub(y)], original_scissor)
            .ok_or(InvalidTarget)?;
        let [x, y, w, h] = visible;
        let x0 = x.saturating_sub(radius);
        let y0 = y.saturating_sub(radius);
        let width = (x + w).saturating_add(radius).min(size.0) - x0;
        let height = (y + h).saturating_add(radius).min(size.1) - y0;
        let horizontal = [x0, y, width, h];
        let output = [x0, y0, width, height];
        let scissor = intersect(output, original_scissor).ok_or(InvalidTarget)?;
        let mut weights = [[0.0; 4]; 49];
        for k in 0..=radius as usize {
            weights[k / 4][k % 4] = (-0.5 * (k as f64 / sigma).powi(2)).exp() as f32;
        }
        let clip_flag = u32::from(occurrence.ancestor.is_some());
        let ancestor = occurrence.ancestor.map_or_else(RoundedClipUniform::zeroed, |(r, radii)| {
            RoundedClipUniform::new([r.x, r.y, r.width, r.height], radii, clip_aa)
        });
        let rect = occurrence.rect;
        let corners = occurrence.corners;
        let p = occurrence.optics.parameters();
        let tint = occurrence.optics.reflection_tint();
        let alignment = u64::from(limits.min_uniform_buffer_offset_alignment).max(256);
        let source_stride = BloomStage::Source.uniform_size().div_ceil(alignment) * alignment;
        if source_stride > limits.max_buffer_size {
            return Err(InvalidTarget);
        }
        Ok(Self {
            command: occurrence.command_index(scene, surface)?,
            node_key: occurrence.node_key,
            source_stride,
            source: BloomSourceUniform {
                rect: [rect.x, rect.y, rect.width, rect.height],
                corners: [corners.tl, corners.tr, corners.bl, corners.br],
                ancestor,
                lights: occurrence.optics.lights().map(light_params),
                shape: [p.bevel_width, p.bevel_slope, p.roughness, p.normal_reflectance],
                optics: [occurrence.bloom.gain(), occurrence.bloom.sigma_logical(), occurrence.bloom.mix(),
                    p.reflection_strength],
                tint: [tint.r, tint.g, tint.b, tint.a],
                control: [clip_flag, x, y, 0],
            },
            horizontal: BloomFilterUniform {
                target: target_rect, input_roi: visible, output_roi: horizontal, kernel: [0, radius, 0, 0], weights,
            },
            vertical: BloomFilterUniform {
                target: target_rect, input_roi: horizontal, output_roi: output, kernel: [1, radius, 0, 0], weights,
            },
            composite: BloomCompositeUniform {
                output_roi: output, target: target_rect, ancestor,
                response: [occurrence.bloom.mix(), 0.0, 0.0, 0.0], control: [clip_flag, 0, 0, 0],
            },
            scissor,
        })
    }

    pub fn regions(&self) -> [[u32; 4]; 3] {
        [self.horizontal.input_roi, self.horizontal.output_roi, self.vertical.output_roi]
    }

    pub fn bytes(&self) -> Option<u64> {
        let uniforms = self.source_stride + 2 * BloomStage::Horizontal.uniform_size() + BloomStage::Composite.uniform_size();
        self.regions().iter().try_fold(uniforms, |sum, &[_x, _y, w, h]| {
            sum.checked_add(u64::from(w).checked_mul(u64::from(h))?.checked_mul(8)?)
        })
    }
}

fn intersect(a: [u32; 4], b: [u32; 4]) -> Option<[u32; 4]> {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    let right = (a[0] + a[2]).min(b[0] + b[2]);
    let bottom = (a[1] + a[3]).min(b[1] + b[3]);
    if x < right && y < bottom { Some([x, y, right - x, bottom - y]) } else { None }
}
