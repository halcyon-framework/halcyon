//! HAL375's optional Compile candidate and the GPU gate for its ordinary draws.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use crate::{
    core::{
        math::{Point, RectF},
        paint::LAYER_OVERLAY,
        scene::{RoundedCorners, TextQuad},
    },
    renderer::{
        batcher::{FlatPrimitive, PipelineId, PrimitiveData},
        custom_render_pass::HdrFrontReadLease,
        path::{Path, StrokeCap, StrokeJoin, StrokeParams},
        pipelines::path_pipeline::{PathInstance, PathVertex},
        render_graph::executor::{FrameConstants, RenderResources},
        text_realization::SurfaceTextDemand,
    },
    three_d::{
        PbrSourceError3D, PbrSourceResult3D,
        spatial_hud::{
            FONT_SIZE, HudCompileCharge, HudDepth3D, HudProgram3D, HudVisibilityInput, MAX_GLYPHS,
            MAX_LABELS, MAX_PIXEL_CENTERS, PADDING_X, PADDING_Y, PreparedHud3D, STROKE_WIDTH,
            place, refused,
        },
    },
};

use super::WgpuRenderer;
use crate::compositor::{MultiSurfaceContract, SurfaceId};
use crate::core::paint::EffectivePaintOrder;
use crate::renderer::batcher::{DrawBatch, DrawBatchId};
use crate::renderer::compiled_draw_plan::{MultiTargetDrawPlan, SurfaceCompiledData};
use crate::renderer::custom_render_prepare::{FrozenPresentationBindings, RecordedCompanion};
use crate::renderer::draw_plan::{DrawPlan, SurfaceSceneData};
use crate::renderer::glyph_atlas::GlyphKey;
use crate::renderer::image_binding::ImageBindingIdentity;
use crate::renderer::render_graph::passes::pipeline_dispatch::encode_draw_commands_for_scope;
use crate::renderer::render_graph::{
    AttachmentDesc, AttachmentHandle, AttachmentSize, MainPassExecutor, OverlayPassExecutor,
    PassContext, PassDrawScope, PassExecutionError, PassExecutor, RenderGraphBuilder,
    RenderGraphTopology, RenderPassNode, ResourceRef,
};
use crate::renderer::render_target::TargetDescriptor;
use crate::renderer::sdf_frame_custody::{
    SdfCompiledFrame, SdfFrameCustodyError, SdfFramePlan, SdfRenderOutput, SdfSceneCustody,
};

struct HudTextCandidate {
    prepared: Arc<PreparedHud3D>,
    image: FlatPrimitive,
    inputs: Vec<FlatPrimitive>,
    demand: SurfaceTextDemand,
}

struct ImageSurvival {
    batch: DrawBatchId,
    vertex_count: u32,
    index_count: u32,
    identity: ImageBindingIdentity,
}

struct HudCandidate {
    surface: SurfaceId,
    survivor: ImageSurvival,
    text: HudTextCandidate,
    _charge: HudCompileCharge,
}

struct HudDraw {
    order: EffectivePaintOrder,
    leader: bool,
    work: Arc<SpatialHudWork>,
}

impl HudDraw {
    fn tie_rank(&self) -> u8 {
        if self.leader { 1 } else { 2 }
    }
}

enum MainDraw {
    Ordinary(Range<usize>),
    Hud(HudDraw),
}

struct HudMainPass {
    main: MainPassExecutor,
    draws: Vec<MainDraw>,
    _charges: Vec<HudCompileCharge>,
}

impl PassExecutor for HudMainPass {
    fn execute(&self, ctx: &mut PassContext<'_>) -> Result<(), PassExecutionError> {
        self.main
            .encode_with(ctx, |pass, constants, data, resources, buffers| {
                for draw in &self.draws {
                    match draw {
                        MainDraw::Ordinary(range) => encode_draw_commands_for_scope(
                            pass,
                            constants,
                            crate::renderer::render_graph::FrameDrawData {
                                draw_commands: &data.draw_commands[range.clone()],
                                ..data
                            },
                            resources,
                            buffers,
                            PassDrawScope::MainUnderlay,
                        ),
                        MainDraw::Hud(draw) => draw.work.encode_draw(
                            pass,
                            constants,
                            resources,
                            data.target.globals_offset,
                            data.target.path_clips_offset,
                            draw.leader,
                        ),
                    }
                }
            })
    }
}

struct HudVisibilityPass {
    work: Arc<Vec<Arc<SpatialHudWork>>>,
}

impl PassExecutor for HudVisibilityPass {
    fn execute(&self, ctx: &mut PassContext<'_>) -> Result<(), PassExecutionError> {
        for work in self.work.iter() {
            work.encode_visibility(ctx.encoder);
        }
        Ok(())
    }
}

struct CompiledHudTarget {
    surface: SurfaceId,
    topology: RenderGraphTopology,
    output: AttachmentHandle,
    work: Arc<Vec<Arc<SpatialHudWork>>>,
    #[cfg(test)]
    draw_order: Vec<(bool, EffectivePaintOrder)>,
}

#[derive(Default)]
pub(crate) struct CompiledHudFrame {
    targets: Vec<CompiledHudTarget>,
    #[cfg(test)]
    refusals: Vec<(SurfaceId, PbrSourceError3D)>,
}

impl CompiledHudFrame {
    fn refuse(&mut self, surface: SurfaceId, error: PbrSourceError3D) {
        tracing::debug!(?surface, ?error, "HAL375 optional presentation refused");
        #[cfg(test)]
        self.refusals.push((surface, error));
    }

    fn target(&self, surface: SurfaceId) -> Option<&CompiledHudTarget> {
        self.targets.iter().find(|target| target.surface == surface)
    }

    fn encode(
        &self,
        renderer: &WgpuRenderer,
        compiled: &MultiTargetDrawPlan,
        descriptor: &TargetDescriptor,
        view: &wgpu::TextureView,
    ) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands> {
        if let Some(target) = self.target(descriptor.surface_id) {
            renderer.encode_compiled_target_with_graph(
                compiled,
                descriptor,
                view,
                &target.topology,
                target.output,
            )
        } else {
            renderer.encode_compiled_target(compiled, descriptor, view)
        }
    }

    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn work(&self) -> impl Iterator<Item = (SurfaceId, &Arc<SpatialHudWork>)> {
        self.targets
            .iter()
            .flat_map(|target| target.work.iter().map(move |work| (target.surface, work)))
    }

    #[cfg(test)]
    pub(crate) fn refusals(&self) -> &[(SurfaceId, PbrSourceError3D)] {
        &self.refusals
    }

    #[cfg(test)]
    pub(crate) fn draw_order_for_test(&self, surface: SurfaceId) -> &[(bool, EffectivePaintOrder)] {
        self.target(surface)
            .map_or(&[], |target| target.draw_order.as_slice())
    }
}

impl WgpuRenderer {
    pub(super) fn compile_sdf_frame_with_companions(
        &mut self,
        frame: SdfFramePlan,
        surfaces: Option<&HashSet<SurfaceId>>,
        bindings: FrozenPresentationBindings<'_>,
    ) -> Result<crate::renderer::RendererCompileOutcome<SdfCompiledFrame>, crate::renderer::RendererCompileError> {
        for record in bindings.recorded(self.frame_counter()) {
            if !surfaces.is_none_or(|ids|ids.contains(&record.surface)) {continue;}
            if let Some(bloom) = frame.plan.surface_data.get(&record.surface).and_then(|scene| scene.bloom.as_ref()) {
                return Err(bloom.error(crate::renderer::BloomCompileRefusal::UnsupportedCompanionGraph));
            }
            let Some(size)=self.multi_targets.configured_size_px(record.surface) else {continue};
            if !self.multi_targets.has_active_extent(record.surface) {continue;}
            let Some(scale)=self.multi_targets.scale_factor(record.surface) else {continue};
            let target=crate::renderer::render_target::TargetDescriptor{surface_id:record.surface,physical_size:size,
                logical_size:(size.0 as f32/scale,size.1 as f32/scale),scale_factor:scale,globals_offset:0,path_clips_offset:0};
            if let Some(scene)=frame.plan.surface_data.get(&record.surface) {
                for p in &scene.primitives {
                    if crate::renderer::frosted_glass::parameters::occurrence(p,&target,self.rounded_clip_aa_radius)
                        .map_err(|reason|crate::renderer::frosted_glass::error(record.surface,None,reason))?.is_some() {
                        return Err(crate::renderer::frosted_glass::error(record.surface,None,
                            crate::renderer::FrostedGlassCompileRefusal::UnsupportedCompanionGraph));
                    }
                }
            }
        }
        let SdfFramePlan { plan, mut custody } = frame;
        let sdf_custody = (!custody.is_empty()).then_some(&custody);
        let mut companions = CompiledHudFrame::default();
        let Some((compiled, pending)) = self.compile_multi_admitted(
            &plan,
            surfaces,
            sdf_custody,
            |renderer, required_text, protected| {
                let mut pending = Vec::new();
                for record in bindings.recorded(renderer.frame_counter()) {
                    let surface = record.surface;
                    if !required_text
                        .get(&surface)
                        .is_some_and(|demand| demand.outcome().is_complete())
                    {
                        continue;
                    }
                    match renderer.admit_hud_occurrence(
                        record,
                        &plan.surface_data[&surface],
                        sdf_custody,
                        protected,
                    ) {
                        Ok(candidate) => pending.push(candidate),
                        Err(error) => companions.refuse(surface, error),
                    }
                }
                pending
            },
        )?
        else {
            return Ok(crate::renderer::RendererCompileOutcome::NoTargets);
        };
        self.compile_hud_targets(&plan, &compiled, pending, &mut companions);
        // No compiled graph or output holds a UI pin or a frame record.
        drop(bindings);
        custody.retain_compiled(&compiled);
        let keep_companions = !companions.targets.is_empty();
        #[cfg(test)]
        let keep_companions = keep_companions || !companions.refusals.is_empty();
        self.publish_sdf_compile(SdfCompiledFrame {
            plan: compiled,
            custody,
            companions: keep_companions.then_some(companions),
        })
    }

    fn admit_hud_occurrence(
        &mut self,
        record: RecordedCompanion<'_>,
        scene: &SurfaceSceneData,
        sdf_custody: Option<&SdfSceneCustody>,
        protected: &mut HashSet<GlyphKey>,
    ) -> PbrSourceResult3D<HudCandidate> {
        let RecordedCompanion {
            surface,
            binding,
            metrics,
            prepared,
            occurrence,
        } = record;
        let target = self
            .custom_render_target_snapshot(surface)
            .map_err(|_| refused("HUD target is not current"))?;
        let current = target
            .metrics_for(metrics.final_logical_rect, metrics.final_logical_viewport)
            .map_err(|_| refused("HUD target metrics are unavailable"))?;
        let current = crate::renderer::CustomRenderTargetMetrics {
            app_frame_attempt: metrics.app_frame_attempt,
            ..current
        };
        let original = crate::renderer::CustomRenderTargetMetrics {
            app_frame_attempt: metrics.app_frame_attempt,
            ..prepared.metrics
        };
        if metrics != current
            || metrics != original
            || !matches!(
                metrics.activity,
                crate::renderer::CustomRenderActivity::Active { .. }
            )
            || metrics.scale_factor != occurrence.scale_factor
            || binding.canonical_over_neutral()
            || binding.texture_id() != prepared.texture_id
        {
            return Err(refused("HUD source, target or occurrence metrics changed"));
        }
        if !occurrence.clip.is_none()
            && occurrence
                .rect
                .intersection(&occurrence.clip.rect)
                .is_none()
        {
            return Err(refused("HUD Image is outside its inherited clip"));
        }
        let (image, batch) = hud_image_batch(scene, occurrence)?;
        self.validate_image_batch(batch, surface, sdf_custody)
            .map_err(|_| refused("HUD Image batch is ineligible for ordinary output"))?;
        let identity = self.current_hud_front(prepared)?;
        let survivor = ImageSurvival {
            batch: batch.id,
            vertex_count: batch.vertex_range.count,
            index_count: batch.index_range.count,
            identity,
        };
        let charge = prepared.reserve_cpu((4 * std::mem::size_of::<HudCandidate>()) as u64)?;
        let text =
            self.admit_spatial_hud_text(prepared, image, occurrence.scale_factor, protected)?;
        Ok(HudCandidate {
            surface,
            survivor,
            text,
            _charge: charge,
        })
    }

    fn current_hud_front(
        &self,
        prepared: &PreparedHud3D,
    ) -> PbrSourceResult3D<ImageBindingIdentity> {
        let identity = prepared
            .front
            .registered()
            .ok()
            .flatten()
            .map(|binding| binding.identity())
            .ok_or_else(|| refused("HUD current front is not registered"))?;
        if identity.texture_id() != prepared.texture_id
            || !self
                .image_textures
                .get(&identity.texture_id())
                .is_some_and(|image| {
                    image.generation == identity.generation()
                        && &image.view == prepared.front.view()
                })
        {
            return Err(refused("HUD front registration or sampled view changed"));
        }
        Ok(identity)
    }

    fn compile_hud_targets(
        &self,
        draw_plan: &DrawPlan<MultiSurfaceContract>,
        compiled: &MultiTargetDrawPlan,
        pending: Vec<HudCandidate>,
        companions: &mut CompiledHudFrame,
    ) {
        let mut pending = pending.into_iter().peekable();
        while let Some(first) = pending.next() {
            let surface = first.surface;
            let group = std::iter::once(first).chain(std::iter::from_fn(|| {
                if pending
                    .peek()
                    .is_some_and(|candidate| candidate.surface == surface)
                {
                    pending.next()
                } else {
                    None
                }
            }));
            let Some(target) = compiled.get_target(surface) else {
                continue;
            };
            let Some(data) = compiled.surface_compiled_data(surface) else {
                continue;
            };
            if !data.text_realization().outcome().is_complete() {
                continue;
            }
            let mut draws = Vec::new();
            let mut work = Vec::new();
            let mut charges = Vec::new();
            for candidate in group {
                match self.compile_surviving_hud(candidate, target, data) {
                    Ok(Some((pair, charge))) => {
                        work.push(Arc::clone(&pair[0].work));
                        draws.extend(pair);
                        charges.push(charge);
                    }
                    Ok(None) => {}
                    Err(error) => companions.refuse(surface, error),
                }
            }
            if draws.is_empty() {
                continue;
            }
            match self.build_hud_target(draw_plan, surface, data, draws, work, charges) {
                Ok(target) => companions.targets.push(target),
                Err(error) => companions.refuse(surface, error),
            }
        }
    }

    fn compile_surviving_hud(
        &self,
        candidate: HudCandidate,
        target: &TargetDescriptor,
        data: &SurfaceCompiledData,
    ) -> PbrSourceResult3D<Option<([HudDraw; 2], HudCompileCharge)>> {
        let HudCandidate { survivor, text, .. } = candidate;
        let metrics = text.prepared.metrics;
        let extent = metrics.target_physical_extent;
        if target.physical_size != (extent.width, extent.height)
            || target.scale_factor != metrics.scale_factor
            || self.current_hud_front(&text.prepared)? != survivor.identity
            || !data.frame_batches.batches.iter().any(|batch| {
                batch.id == survivor.batch
                    && batch.key == text.image.key
                    && batch.clip_state == text.image.clip_state
                    && batch.vertex_range.count == survivor.vertex_count
                    && batch.index_range.count == survivor.index_count
            })
        {
            return Err(refused("HUD Image did not survive ordinary Compile"));
        }
        // Covers the pair, merged ranges, target graph and overlapping custody.
        let cpu_bytes = 4
            * (std::mem::size_of::<CompiledHudTarget>()
                + std::mem::size_of::<HudMainPass>()
                + std::mem::size_of::<HudVisibilityPass>()
                + 5 * std::mem::size_of::<MainDraw>()
                + 3 * std::mem::size_of::<RenderPassNode>());
        let charge = text.prepared.reserve_cpu(cpu_bytes as u64)?;
        Ok(self
            .compile_spatial_hud(text, target.physical_size, target.scale_factor)?
            .map(|pair| (pair, charge)))
    }

    fn build_hud_target(
        &self,
        draw_plan: &DrawPlan<MultiSurfaceContract>,
        surface: SurfaceId,
        data: &SurfaceCompiledData,
        mut draws: Vec<HudDraw>,
        work: Vec<Arc<SpatialHudWork>>,
        charges: Vec<HudCompileCharge>,
    ) -> PbrSourceResult3D<CompiledHudTarget> {
        draws.sort_by(|a, b| {
            a.order
                .cmp(&b.order)
                .then_with(|| a.tie_rank().cmp(&b.tie_rank()))
        });
        let mut schedule = Vec::with_capacity(2 * draws.len() + 1);
        let mut start = 0;
        for draw in draws {
            let end = start
                + data.draw_commands[start..].partition_point(|command| {
                    (command.order(), command.tie_rank()) <= (&draw.order, draw.tie_rank())
                });
            if start != end {
                schedule.push(MainDraw::Ordinary(start..end));
            }
            schedule.push(MainDraw::Hud(draw));
            start = end;
        }
        if start != data.draw_commands.len() {
            schedule.push(MainDraw::Ordinary(start..data.draw_commands.len()));
        }
        #[cfg(test)]
        let draw_order = schedule
            .iter()
            .filter_map(|entry| match entry {
                MainDraw::Hud(draw) => Some((draw.leader, draw.order.clone())),
                MainDraw::Ordinary(_) => None,
            })
            .collect();

        let work = Arc::new(work);
        let mut graph = RenderGraphBuilder::new();
        let output = graph.declare_attachment(AttachmentDesc {
            label: "RenderGraph Output Attachment",
            size: AttachmentSize::SWAPCHAIN_FULL,
            format: self.surface_format,
            samples: 1,
            clear_color: None,
            load_op: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store_op: wgpu::StoreOp::Store,
        });
        let visibility = graph.add_pass(RenderPassNode::new(
            "hal375_visibility",
            Box::new(HudVisibilityPass {
                work: Arc::clone(&work),
            }),
        ));
        let main = graph.add_pass(
            RenderPassNode::new(
                "main_pass",
                Box::new(HudMainPass {
                    main: MainPassExecutor::new(output),
                    draws: schedule,
                    _charges: charges,
                }),
            )
            .write(ResourceRef::Attachment(output))
            .depends_on(visibility),
        );
        graph.add_pass(
            RenderPassNode::new("overlay_pass", Box::new(OverlayPassExecutor::new(output)))
                .read(ResourceRef::Attachment(output))
                .write(ResourceRef::Attachment(output))
                .depends_on(main),
        );
        let topology = graph
            .build(draw_plan)
            .map_err(|_| refused("HUD graph validation failed"))?;
        Ok(CompiledHudTarget {
            surface,
            topology,
            output,
            work,
            #[cfg(test)]
            draw_order,
        })
    }

    pub(super) fn render_sdf_frame_with_companions(
        &mut self,
        plan: MultiTargetDrawPlan,
        custody: SdfSceneCustody,
        companions: CompiledHudFrame,
        #[cfg(target_os = "macos")] watch_occlusion: Option<(SurfaceId, &mut bool)>,
    ) -> Result<Vec<SdfRenderOutput>, SdfFrameCustodyError> {
        self.render_sdf_frame_outputs(
            plan,
            custody,
            #[cfg(target_os = "macos")]
            watch_occlusion,
            |renderer, compiled, descriptor, view| {
                companions.encode(renderer, compiled, descriptor, view)
            },
            |surface| {
                companions
                    .target(surface)
                    .map(|target| PresentationCompanionUse(Arc::clone(&target.work)))
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn encode_sdf_target_for_test(
        &self,
        frame: &SdfCompiledFrame,
        surface: SurfaceId,
        view: &wgpu::TextureView,
    ) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands> {
        let descriptor = frame.plan.get_target(surface)?;
        match &frame.companions {
            Some(companions) => companions.encode(self, &frame.plan, descriptor, view),
            None => self.encode_compiled_target(&frame.plan, descriptor, view),
        }
    }

    /// Runs after all ordinary surface demands have been admitted and protected.
    fn admit_spatial_hud_text(
        &mut self,
        prepared: &Arc<PreparedHud3D>,
        image: &FlatPrimitive,
        scale: f32,
        protected: &mut std::collections::HashSet<crate::renderer::glyph_atlas::GlyphKey>,
    ) -> PbrSourceResult3D<HudTextCandidate> {
        if !matches!(image.data, PrimitiveData::Image { texture_id, rect, .. }
            if texture_id == prepared.texture_id && rect == prepared.metrics.final_logical_rect)
            || image.key.pipeline != PipelineId::Image
            || image.key.z_layer >= LAYER_OVERLAY - 2
            || scale != prepared.metrics.scale_factor
            || prepared.front.registered().ok().flatten().is_none()
        {
            return Err(refused(
                "HUD and registered current viewport Image do not match",
            ));
        }
        let mut key = image.key.clone();
        key.pipeline = PipelineId::Text;
        key.z_layer += 2;
        let inputs: Vec<_> = prepared
            .source
            .anchors()
            .iter()
            .enumerate()
            .map(|(slot, anchor)| FlatPrimitive {
                key: key.clone(),
                data: PrimitiveData::Text {
                    binding: Some(prepared.typography.binding.clone()),
                    position: Point::new(0.0, 0.0),
                    text: anchor.text.to_string(),
                    font_size: FONT_SIZE,
                    color: prepared.typography.color,
                    backgrounds: Vec::new(),
                },
                clip_state: image.clip_state,
                order_key: slot as u32,
            })
            .collect();
        let mut demand = SurfaceTextDemand::gather(&inputs, &mut self.text_engine, scale);
        if !demand.outcome().is_complete()
            || demand.runs.iter().any(|run| run.keys.len() > MAX_GLYPHS)
            || demand.runs.iter().map(|run| run.keys.len()).sum::<usize>() > MAX_LABELS * MAX_GLYPHS
        {
            return Err(refused("HUD shaping unavailable or exceeds glyph limit"));
        }
        let rasterizations = demand.admit(
            &mut self.text_engine,
            scale,
            &mut self.glyph_atlas,
            protected,
            &self.device,
            &self.queue,
            &mut self.gpu_arena,
            self.frame_epoch,
        );
        #[cfg(debug_assertions)]
        self.rasterization_count.set(
            self.rasterization_count
                .get()
                .saturating_add(rasterizations),
        );
        #[cfg(not(debug_assertions))]
        let _ = rasterizations;
        if !demand.outcome().is_complete() {
            if demand.failure.as_ref().is_some_and(|failure| {
                failure.reason == crate::renderer::TextRealizationFailureReason::AtlasCapacity
            }) {
                return Err(refused("optional HUD glyph working set exceeds atlas capacity"));
            }
            return Err(refused(
                "complete optional HUD glyph working set unavailable",
            ));
        }
        Ok(HudTextCandidate {
            prepared: prepared.clone(),
            image: image.clone(),
            inputs,
            demand,
        })
    }

    /// Lower actual shaped ink once the shared atlas is final. Only a complete
    /// candidate gets a resource lease and commands in the visible schedule.
    fn compile_spatial_hud(
        &self,
        candidate: HudTextCandidate,
        physical_size: (u32, u32),
        scale: f32,
    ) -> PbrSourceResult3D<Option<[HudDraw; 2]>> {
        let HudTextCandidate {
            prepared,
            image,
            inputs,
            demand,
        } = candidate;
        let labels = inputs.len();
        let quads = demand.runs.iter().map(|run| run.keys.len()).sum::<usize>() + labels;
        let quad_bytes = 2 * std::mem::size_of::<TextQuad>()
            + 4 * std::mem::size_of::<crate::renderer::text_vertex::TextVertex>()
            + 6 * std::mem::size_of::<u32>();
        let line_bytes = 2
            * (4 * std::mem::size_of::<PathVertex>() + 6 * std::mem::size_of::<u32>())
            + std::mem::size_of::<HudVisibilityInput>()
            + std::mem::size_of::<PathInstance>();
        let _scratch = prepared.reserve_cpu((quads * quad_bytes + labels * line_bytes) as u64)?;
        let (glyphs, _) = self
            .build_text_quads_from_surface_primitives(&inputs, &demand, scale)
            .map_err(|_| refused("HUD atlas lowering unavailable"))?;
        if (
            prepared.metrics.target_physical_extent.width,
            prepared.metrics.target_physical_extent.height,
        ) != physical_size
        {
            return Err(refused("compiled HUD target extent changed"));
        }
        let rect = prepared.metrics.final_logical_rect;
        let Some(mut crop) = rect
            .intersection(&prepared.metrics.final_logical_viewport)
            .and_then(|r| {
                r.intersection(&RectF::new(
                    0.0,
                    0.0,
                    prepared.metrics.target_logical_extent.width,
                    prepared.metrics.target_logical_extent.height,
                ))
            })
        else {
            return Ok(None);
        };
        let clip = image.clip_state;
        if !clip.is_none() {
            let Some(clipped) = crop.intersection(&clip.rect) else {
                return Ok(None);
            };
            crop = clipped;
        }
        let safe_crop = crop.shrink(6.0);
        if safe_crop.width <= 0.0 || safe_crop.height <= 0.0 {
            return Ok(None);
        }
        let mut jobs = [HudVisibilityInput::zeroed(); MAX_LABELS];
        let mut boxes = Vec::with_capacity(MAX_LABELS);
        let mut text = Vec::with_capacity(quads);
        let mut leader_vertices = Vec::with_capacity(4 * labels);
        let mut leader_indices = Vec::with_capacity(6 * labels);
        let mut leader_instances = [PathInstance::zeroed(); MAX_LABELS];
        let mut glyph_start = 0;
        let mut work = 0u32;
        let mut placed = false;
        for run in &demand.runs {
            let slot = run.primitive_index;
            let count = run
                .keys
                .iter()
                .filter(|k| !demand.no_ink.contains(k))
                .count();
            let ink = &glyphs[glyph_start..glyph_start + count];
            glyph_start += count;
            let ink_rect = ink_bounds(ink)?;
            let line_height = prepared.typography.line_height.max(ink_rect.height);
            if ink_rect.width > 240.0 || line_height > 28.0 {
                return Err(refused(
                    "measured label ink does not fit the one-line envelope",
                ));
            }
            let Some(anchor) = prepared.points[slot].filter(|p| {
                crop.contains(p.logical) && inside_clip(p.logical, clip.rect, clip.rounded)
            }) else {
                continue;
            };
            let Some((label, end)) = place(
                anchor.logical,
                ink_rect.width + 2.0 * PADDING_X,
                line_height + 2.0 * PADDING_Y,
                safe_crop,
                &boxes,
                |candidate| {
                    corners(candidate)
                        .into_iter()
                        .all(|p| inside_clip(p, clip.rect, clip.rounded))
                },
            ) else {
                continue;
            };

            let mut path = Path::new();
            path.move_to(anchor.logical.x, anchor.logical.y);
            path.line_to(end.x, end.y);
            let stroke = path
                .build_stroke_mesh(StrokeParams {
                    width: STROKE_WIDTH,
                    join: StrokeJoin::Miter,
                    cap: StrokeCap::Butt,
                    miter_limit: 4.0,
                })
                .ok_or_else(|| refused("leader stroke cannot be lowered"))?;
            if stroke.vertices.len() != 4 || stroke.indices.len() != 6 {
                return Err(refused("ordinary straight butt-stroke topology changed"));
            }
            // The existing path/glyph pipelines have rectangular scissoring.
            // Keep the entire actual footprint inside any inherited rounded arc.
            if !stroke
                .vertices
                .iter()
                .all(|p| inside_clip(Point::new(p[0], p[1]), clip.rect, clip.rounded))
            {
                continue;
            }
            let Some(leader_crop) = stroke.bounds.intersection(&crop) else {
                continue;
            };
            let label_bounds = centers(label, physical_size, scale);
            let leader_bounds = centers(leader_crop, physical_size, scale);
            work = work
                .checked_add(1)
                .and_then(|n| n.checked_add(area(label_bounds)?))
                .and_then(|n| n.checked_add(area(leader_bounds)?))
                .filter(|n| *n <= MAX_PIXEL_CENTERS)
                .ok_or_else(|| refused("physical pixel center work limit"))?;

            let vertex_start = leader_vertices.len() as u32;
            let leader_start = leader_indices.len() as u32;
            leader_vertices.extend(stroke.vertices.iter().map(|p| PathVertex { position: *p }));
            leader_indices.extend(stroke.indices.iter().map(|i| i + vertex_start));
            let color = prepared.typography.color;
            leader_instances[slot] = PathInstance {
                color: [color.r, color.g, color.b, color.a],
                transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                clip_id: 0,
                stroke_flag: 0,
            };
            let text_start = text.len() as u32 * 6;
            text.push(super::text_background_quad(
                label,
                prepared.typography.background,
                Some(crop),
            ));
            let dx = label.x + PADDING_X - ink_rect.x;
            let dy = label.y + PADDING_Y + (line_height - ink_rect.height) / 2.0 - ink_rect.y;
            let dy = (dy * scale).round() / scale;
            text.extend(ink.iter().map(|q| TextQuad {
                pos: RectF::new(q.pos.x + dx, q.pos.y + dy, q.pos.width, q.pos.height),
                clip_rect: Some(crop),
                ..*q
            }));
            jobs[slot] = HudVisibilityInput {
                anchor: [anchor.pixel[0], anchor.pixel[1], 1, 0],
                label_bounds,
                leader_bounds,
                segment: [anchor.logical.x, anchor.logical.y, end.x, end.y],
                viewport_rect: [rect.x, rect.y, rect.width, rect.height],
                depth_and_width: [
                    anchor.depth,
                    (STROKE_WIDTH / 2.0).powi(2),
                    scale.recip(),
                    0.0,
                ],
                draws: [
                    leader_start,
                    stroke.indices.len() as u32,
                    text_start,
                    (ink.len() as u32 + 1) * 6,
                ],
            };
            boxes.push(label);
            placed = true;
        }
        if !placed {
            return Ok(None);
        }
        let (text_vertices, text_indices) =
            Self::build_text_buffers(&text, scale, physical_size.0, physical_size.1);
        let data = [
            (
                bytemuck::cast_slice(&leader_vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            (
                bytemuck::cast_slice(&leader_indices),
                wgpu::BufferUsages::INDEX,
            ),
            (
                bytemuck::cast_slice(&leader_instances),
                wgpu::BufferUsages::VERTEX,
            ),
            (
                bytemuck::cast_slice(&text_vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            (
                bytemuck::cast_slice(&text_indices),
                wgpu::BufferUsages::INDEX,
            ),
            (
                bytemuck::cast_slice(&jobs[..inputs.len()]),
                wgpu::BufferUsages::STORAGE,
            ),
        ];
        let work = Arc::new(SpatialHudWork::new(
            &self.gpu_resources(),
            &prepared,
            &data,
            crop,
            inputs.len() as u32,
        )?);
        let mut key = image.key;
        key.z_layer += 1;
        let leader = HudDraw {
            order: key.effective_order(),
            leader: true,
            work: work.clone(),
        };
        key.z_layer += 1;
        Ok(Some([
            leader,
            HudDraw {
                order: key.effective_order(),
                leader: false,
                work,
            },
        ]))
    }
}

fn hud_image_batch<'a>(
    scene: &'a SurfaceSceneData,
    receipt: &crate::renderer::custom_render_prepare::ImageOccurrenceReceipt,
) -> PbrSourceResult3D<(&'a FlatPrimitive, &'a DrawBatch)> {
    let mut image_index = 0u32;
    let mut found = None;
    for primitive in &scene.primitives {
        if primitive.order_key == receipt.order_key {
            if found.is_some()
                || primitive.key != receipt.batch_key
                || primitive.key.pipeline != PipelineId::Image
                || primitive.clip_state != receipt.clip
                || !matches!(primitive.data, PrimitiveData::Image { rect, .. } if rect == receipt.rect)
            {
                return Err(refused("HUD Image occurrence is ambiguous or changed"));
            }
            found = Some((primitive, image_index));
        }
        if matches!(primitive.data, PrimitiveData::Image { .. }) {
            image_index = image_index
                .checked_add(1)
                .ok_or_else(|| refused("HUD Image index overflow"))?;
        }
    }
    let (image, index) = found.ok_or_else(|| refused("HUD Image occurrence is missing"))?;
    let contained = |range: crate::renderer::batcher::BufferRange, width: u32| {
        index
            .checked_mul(width)
            .zip(index.checked_add(1).and_then(|i| i.checked_mul(width)))
            .is_some_and(|(start, end)| {
                start >= range.start
                    && range
                        .start
                        .checked_add(range.count)
                        .is_some_and(|limit| end <= limit)
            })
    };
    let batch = scene
        .batches
        .iter()
        .find(|batch| {
            batch.key == image.key
                && batch.clip_state == receipt.clip
                && contained(batch.vertex_range, 4)
                && contained(batch.index_range, 6)
        })
        .ok_or_else(|| refused("HUD Image does not have an executable batch range"))?;
    Ok((image, batch))
}

fn ink_bounds(quads: &[TextQuad]) -> PbrSourceResult3D<RectF> {
    let Some(first) = quads.first() else {
        return Ok(RectF::zero());
    };
    let mut bounds = first.pos;
    for quad in quads {
        let r = quad.pos;
        if ![r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite()) {
            return Err(refused("nonfinite glyph ink"));
        }
        let left = bounds.x.min(r.x);
        let top = bounds.y.min(r.y);
        let right = (bounds.x + bounds.width).max(r.x + r.width);
        let bottom = (bounds.y + bounds.height).max(r.y + r.height);
        bounds = RectF::new(left, top, right - left, bottom - top);
    }
    Ok(bounds)
}

fn corners(rect: RectF) -> [Point; 4] {
    [
        Point::new(rect.x, rect.y),
        Point::new(rect.x + rect.width, rect.y),
        Point::new(rect.x, rect.y + rect.height),
        Point::new(rect.x + rect.width, rect.y + rect.height),
    ]
}

fn inside_clip(p: Point, rect: RectF, rounded: Option<RoundedCorners>) -> bool {
    let Some(r) = rounded else {
        return true;
    };
    if !rect.contains(p) {
        return false;
    }
    let x = p.x - rect.x;
    let y = p.y - rect.y;
    let left = x <= rect.width / 2.0;
    let top = y <= rect.height / 2.0;
    let radius = match (left, top) {
        (true, true) => r.tl,
        (false, true) => r.tr,
        (true, false) => r.bl,
        (false, false) => r.br,
    }
    .clamp(0.0, rect.width.min(rect.height) / 2.0);
    let x = if left { x } else { rect.width - x };
    let y = if top { y } else { rect.height - y };
    (radius - x).max(0.0).hypot((radius - y).max(0.0)) <= radius
}

fn centers(rect: RectF, extent: (u32, u32), scale: f32) -> [u32; 4] {
    let edge = |p: f32, size: u32| {
        (f64::from(p) * f64::from(scale) - 0.5)
            .ceil()
            .clamp(0.0, f64::from(size)) as u32
    };
    [
        edge(rect.x, extent.0),
        edge(rect.y, extent.1),
        edge(rect.x + rect.width, extent.0),
        edge(rect.y + rect.height, extent.1),
    ]
}

fn area(rect: [u32; 4]) -> Option<u32> {
    rect[2]
        .checked_sub(rect[0])?
        .checked_mul(rect[3].checked_sub(rect[1])?)
}

// Storage and indexed indirect records share one nonaliasing result buffer.
const RESULT_STRIDE: u64 = 44;
const LEADER_INDIRECT: u64 = 4;
const TEXT_INDIRECT: u64 = 24;

struct HudBuffers {
    buffers: [wgpu::Buffer; 6],
    visibility: wgpu::Buffer,
    binding: wgpu::BindGroup,
    program: Arc<HudProgram3D>,
    _depth: Arc<HudDepth3D>,
    _front: Arc<HdrFrontReadLease>,
    _charge: HudCompileCharge,
}

pub(crate) struct SpatialHudWork {
    storage: HudBuffers,
    crop: RectF,
    count: u32,
}

impl std::fmt::Debug for SpatialHudWork {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpatialHudWork")
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

impl PartialEq for SpatialHudWork {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl Eq for SpatialHudWork {}

impl SpatialHudWork {
    fn new(
        gpu: &super::GpuResources,
        prepared: &PreparedHud3D,
        inputs: &[(&[u8], wgpu::BufferUsages); 6],
        crop: RectF,
        count: u32,
    ) -> PbrSourceResult3D<Self> {
        if !prepared.context.matches_gpu_resources(gpu) {
            return Err(refused("HUD depth belongs to another Device or Queue"));
        }
        let upload = inputs
            .iter()
            .map(|(bytes, _)| bytes.len() as u64)
            .sum::<u64>();
        let result_bytes = RESULT_STRIDE * u64::from(count);
        let charge = prepared.reserve_compile(
            upload + result_bytes,
            upload,
            (std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()) as u64,
        )?;
        let allocation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let buffers = std::array::from_fn(|i| {
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("HAL375 bounded prepared draw/input"),
                    contents: inputs[i].0,
                    usage: inputs[i].1,
                })
        });
        let visibility = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HAL375 flags and gated draw ranges"),
            size: result_bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | if cfg!(any(test, feature = "test-harness")) {
                    wgpu::BufferUsages::COPY_SRC
                } else {
                    wgpu::BufferUsages::empty()
                },
            mapped_at_creation: false,
        });
        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HAL375 exact depth generation and visibility output"),
            layout: &prepared.program.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(prepared.depth.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers[5].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: visibility.as_entire_binding(),
                },
            ],
        });
        let validation = pollster::block_on(validation_scope.pop());
        let memory = pollster::block_on(allocation_scope.pop());
        if let Some(error) = validation.or(memory) {
            return Err(PbrSourceError3D::Gpu(error.to_string()));
        }
        Ok(Self {
            storage: HudBuffers {
                buffers,
                visibility,
                binding,
                program: prepared.program.clone(),
                _depth: prepared.depth.clone(),
                _front: prepared.front.read_lease(),
                _charge: charge,
            },
            crop,
            count,
        })
    }

    pub(crate) fn encode_visibility(&self, encoder: &mut wgpu::CommandEncoder) {
        let work = &self.storage;
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("HAL375 current viewport depth tests"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&work.program.pipeline);
        pass.set_bind_group(0, &work.binding, &[]);
        pass.dispatch_workgroups(self.count, 1, 1);
    }

    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn read_visibility_for_test(&self, gpu: &super::GpuResources) -> Vec<u32> {
        let bytes = u64::from(self.count) * RESULT_STRIDE;
        let copy = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HAL375 test-only visibility readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.storage.visibility, 0, &copy, 0, bytes);
        gpu.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        copy.slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap()
            });
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let data = copy.slice(..).get_mapped_range().expect("mapped readback range");
        let flags = data
            .chunks_exact(RESULT_STRIDE as usize)
            .map(|record| u32::from_ne_bytes(record[..4].try_into().unwrap()))
            .collect();
        drop(data);
        copy.unmap();
        flags
    }

    fn encode_draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        constants: FrameConstants,
        resources: RenderResources<'_>,
        globals_offset: u32,
        path_clips_offset: u32,
        leader: bool,
    ) {
        let work = &self.storage;
        let (x, y, w, h) = constants.clip_rect_to_scissor(Some(&self.crop));
        if w == 0 || h == 0 {
            return;
        }
        pass.set_scissor_rect(x, y, w, h);
        if leader {
            pass.set_pipeline(resources.path_renderer.pipeline().render_pipeline());
            pass.set_bind_group(0, resources.path_globals_bind_group, &[globals_offset]);
            pass.set_bind_group(1, resources.path_clips_bind_group, &[path_clips_offset]);
            pass.set_vertex_buffer(0, work.buffers[0].slice(..));
            pass.set_index_buffer(work.buffers[1].slice(..), wgpu::IndexFormat::Uint32);
            for i in 0..u64::from(self.count) {
                let start = i * std::mem::size_of::<PathInstance>() as u64;
                pass.set_vertex_buffer(1, work.buffers[2].slice(start..));
                pass.draw_indexed_indirect(&work.visibility, i * RESULT_STRIDE + LEADER_INDIRECT);
            }
        } else {
            pass.set_pipeline(&resources.text_pipeline.pipeline);
            pass.set_bind_group(0, resources.rect_globals_bind_group, &[globals_offset]);
            pass.set_bind_group(1, resources.text_bind_group, &[]);
            pass.set_vertex_buffer(0, work.buffers[3].slice(..));
            pass.set_index_buffer(work.buffers[4].slice(..), wgpu::IndexFormat::Uint32);
            for i in 0..u64::from(self.count) {
                pass.draw_indexed_indirect(&work.visibility, i * RESULT_STRIDE + TEXT_INDIRECT);
            }
        }
    }
}

pub(crate) struct PresentationCompanionUse(Arc<Vec<Arc<SpatialHudWork>>>);

impl PresentationCompanionUse {
    pub(super) fn retire_submitted(self, queue: &wgpu::Queue) {
        let Self(work) = self;
        crate::renderer::on_submitted_work_done(queue, move || drop(work));
    }
}
