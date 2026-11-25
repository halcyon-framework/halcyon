//! Shared target-scoped pipeline dispatch for RenderGraph passes.

use std::ops::Range;

use crate::core::math::RectF;
use crate::renderer::batcher::PipelineId;
use crate::renderer::compiled_draw_plan::SurfaceDrawCommand;
use crate::renderer::render_graph::executor::{
    FrameConstants, FrameDrawData, PassDrawScope, PipelineBuffers, RenderResources,
};

#[derive(Default)]
struct PipelineDispatchState {
    current_pipeline: Option<PipelineId>,
}

impl PipelineDispatchState {
    fn invalidate(&mut self) {
        self.current_pipeline = None;
    }
}

fn encode_batch<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    constants: FrameConstants,
    draw_data: FrameDrawData<'a>,
    render_resources: RenderResources<'a>,
    buffers: PipelineBuffers<'a>,
    batch: &crate::renderer::batcher::DrawBatch,
    state: &mut PipelineDispatchState,
) {
    let target = draw_data.target;
    let clip_ref = if batch.clip_state.is_none() {
        None
    } else {
        Some(&batch.clip_state.rect)
    };
    let (sx, sy, sw, sh) = constants.clip_rect_to_scissor(clip_ref);
    if sw == 0 || sh == 0 {
        return;
    }

    match batch.key.pipeline {
        PipelineId::Rect => {
            if let (Some(vb), Some(ib)) = (buffers.rect_vertex_buffer, buffers.rect_index_buffer) {
                if state.current_pipeline != Some(PipelineId::Rect) {
                    pass.set_pipeline(render_resources.rect_pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(PipelineId::Rect);
                }

                let clip_offset = draw_data
                    .rounded_clip_offsets
                    .get(&batch.clip_state.hash)
                    .copied()
                    .unwrap_or(0);
                pass.set_bind_group(1, render_resources.rounded_clip_bind_group, &[clip_offset]);
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::Shadow => {
            if let (Some(vb), Some(ib)) =
                (buffers.shadow_vertex_buffer, buffers.shadow_index_buffer)
            {
                if state.current_pipeline != Some(PipelineId::Shadow) {
                    pass.set_pipeline(render_resources.shadow_pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(PipelineId::Shadow);
                }

                let params_hash = batch.key.texture_id.unwrap_or(0);
                let params_offset = draw_data
                    .shadow_params_offsets
                    .get(&params_hash)
                    .copied()
                    .unwrap_or(0);
                pass.set_bind_group(
                    1,
                    render_resources.shadow_params_bind_group,
                    &[params_offset],
                );
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::Gradient => {
            if let (Some(vb), Some(ib)) = (
                buffers.gradient_vertex_buffer,
                buffers.gradient_index_buffer,
            ) {
                if state.current_pipeline != Some(PipelineId::Gradient) {
                    pass.set_pipeline(render_resources.gradient_pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(PipelineId::Gradient);
                }

                let params_hash = batch.key.texture_id.unwrap_or(0);
                let params_offset = draw_data
                    .gradient_params_offsets
                    .get(&params_hash)
                    .copied()
                    .unwrap_or(0);
                pass.set_bind_group(
                    1,
                    render_resources.gradient_params_bind_group,
                    &[params_offset],
                );
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::AnalyticSurface => {
            let (Some(pipeline), Some(params), Some(vb), Some(ib)) = (
                render_resources.analytic_surface_pipeline,
                render_resources.analytic_surface_params_bind_group,
                buffers.analytic_surface_vertex_buffer,
                buffers.analytic_surface_index_buffer,
            ) else {
                panic!("HAL386: admitted analytic resources missing during Execute")
            };
            if state.current_pipeline != Some(PipelineId::AnalyticSurface) {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                pass.set_bind_group(
                    0,
                    render_resources.rect_globals_bind_group,
                    &[target.globals_offset],
                );
                state.current_pipeline = Some(PipelineId::AnalyticSurface);
            }
            let ordinal = batch.key.texture_id.expect("HAL386: analytic ordinal");
            let offset = draw_data
                .analytic_surface_params_offsets
                .get(&ordinal)
                .expect("HAL386: actual surface/ordinal must have a prepared slot");
            pass.set_bind_group(1, params, &[*offset]);
            pass.set_scissor_rect(sx, sy, sw, sh);
            pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
        }
        PipelineId::ColorPickerSv => {
            if let (Some(vb), Some(ib)) = (
                buffers.color_picker_sv_vertex_buffer,
                buffers.color_picker_sv_index_buffer,
            ) {
                if state.current_pipeline != Some(PipelineId::ColorPickerSv) {
                    pass.set_pipeline(render_resources.color_picker_sv_pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(PipelineId::ColorPickerSv);
                }

                let hue_bits = batch.key.texture_id.unwrap_or(0);
                let params_offset = draw_data
                    .color_picker_sv_params_offsets
                    .get(&hue_bits)
                    .copied()
                    .unwrap_or(0);
                pass.set_bind_group(
                    1,
                    render_resources.color_picker_sv_params_bind_group,
                    &[params_offset],
                );
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::SdfRect => {
            if let (Some(pipeline), Some(vb), Some(ib)) = (
                render_resources.sdf_rect_pipeline,
                buffers.sdf_rect_vertex_buffer,
                buffers.sdf_rect_index_buffer,
            ) {
                if state.current_pipeline != Some(PipelineId::SdfRect) {
                    pass.set_pipeline(pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(PipelineId::SdfRect);
                }

                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::Shape => {
            if let (Some(pipeline), Some(vb), Some(ib)) = (
                render_resources.shape_pipeline,
                buffers.shape_vertex_buffer,
                buffers.shape_index_buffer,
            ) {
                if state.current_pipeline != Some(PipelineId::Shape) {
                    pass.set_pipeline(pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(PipelineId::Shape);
                }

                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::RadialGauge => {
            if let (Some(vb), Some(ib)) = (
                buffers.radial_gauge_vertex_buffer,
                buffers.radial_gauge_index_buffer,
            ) {
                if state.current_pipeline != Some(PipelineId::RadialGauge) {
                    pass.set_pipeline(render_resources.radial_gauge_pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(PipelineId::RadialGauge);
                }

                let param_hash = batch.key.texture_id.unwrap_or(0);
                let params_offset = draw_data
                    .radial_gauge_params_offsets
                    .get(&param_hash)
                    .copied()
                    .unwrap_or(0);
                pass.set_bind_group(
                    1,
                    render_resources.radial_gauge_params_bind_group,
                    &[params_offset],
                );
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::Image | PipelineId::CanonicalOverNeutral => {
            if let (Some(vb), Some(ib)) = (buffers.image_vertex_buffer, buffers.image_index_buffer)
            {
                if state.current_pipeline != Some(batch.key.pipeline)
                    || batch.key.pipeline == PipelineId::CanonicalOverNeutral
                {
                    let pipeline =
                        if batch.key.pipeline == PipelineId::CanonicalOverNeutral {
                            let Some(pipeline) = batch.key.texture_id.and_then(|id| {
                                render_resources.presence_terminal_pipelines.get(&id)
                            }) else {
                                return;
                            };
                            *pipeline
                        } else {
                            render_resources.image_pipeline
                        };
                    pass.set_pipeline(pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.set_bind_group(
                        0,
                        render_resources.rect_globals_bind_group,
                        &[target.globals_offset],
                    );
                    state.current_pipeline = Some(batch.key.pipeline);
                }

                #[cfg(debug_assertions)]
                {
                    let end = batch.index_range.end();
                    debug_assert!(
                        end <= render_resources.image_index_capacity,
                        "INV-IMG-001: Image batch index range {}..{} exceeds compiled buffer capacity {}",
                        batch.index_range.start,
                        end,
                        render_resources.image_index_capacity
                    );
                }

                let Some(tex_id) = batch.key.texture_id else {
                    return;
                };
                let image_bind_group = match render_resources.image_bind_groups.get(&tex_id) {
                    Some(bind_group) => *bind_group,
                    None => {
                        #[cfg(debug_assertions)]
                        panic!(
                            "INV-IMG-001: Execute phase found no entry for texture_id={} — INV-IMG-005 should have rejected this at compile",
                            tex_id
                        );
                        #[cfg(not(debug_assertions))]
                        return;
                    }
                };

                let clip_offset = draw_data
                    .rounded_clip_offsets
                    .get(&batch.clip_state.hash)
                    .copied()
                    .unwrap_or(0);
                #[cfg(debug_assertions)]
                debug_assert!(
                    draw_data
                        .rounded_clip_offsets
                        .contains_key(&batch.clip_state.hash),
                    "INV-IMG-001: Image batch clip_state.hash={} not in rounded_clip_offsets",
                    batch.clip_state.hash
                );
                pass.set_bind_group(1, image_bind_group, &[clip_offset]);
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(batch.index_range.start..batch.index_range.end(), 0, 0..1);
            }
        }
        PipelineId::FrostedGlass => panic!("INV-385-ORDER: frosting is a graph barrier, not an ordinary batch"),
        _ => {}
    }
}

/// SPEC-HAL-359: Execute the immutable per-surface schedule for one visible pass.
pub(crate) fn encode_draw_commands_for_scope<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    constants: FrameConstants,
    draw_data: FrameDrawData<'a>,
    render_resources: RenderResources<'a>,
    buffers: PipelineBuffers<'a>,
    scope: PassDrawScope,
) {
    encode_draw_commands_range_for_scope(pass,constants,draw_data,render_resources,buffers,
        scope,0..draw_data.draw_commands.len());
}

/// HAL385: path/text coalescing is bounded by the immutable segment range.
pub(crate) fn encode_draw_commands_range_for_scope<'a>(
    pass:&mut wgpu::RenderPass<'a>, constants:FrameConstants,
    draw_data:FrameDrawData<'a>, render_resources:RenderResources<'a>,
    buffers:PipelineBuffers<'a>, scope:PassDrawScope, range:std::ops::Range<usize>,
) {
    let mut state = PipelineDispatchState::default();
    let mut command_index = range.start;
    let end=range.end.min(draw_data.draw_commands.len());
    while command_index < end {
        let command = &draw_data.draw_commands[command_index];
        if !scope.contains_z(command.z_index()) {
            command_index += 1;
            continue;
        }

        match command {
            SurfaceDrawCommand::FrostedGlass {..} => {
                panic!("INV-385-ORDER: a composition marker reached an ordinary segment")
            }
            SurfaceDrawCommand::Batch {
                z_index,
                batch_index,
                ..
            } => {
                let Some(batch) = draw_data.frame_batches.batches.get(*batch_index) else {
                    debug_assert!(false, "SPEC-HAL-359: scheduled batch index is out of range");
                    command_index += 1;
                    continue;
                };
                debug_assert_eq!(batch.key.z_layer, *z_index);
                debug_assert_ne!(batch.key.pipeline, PipelineId::Text);
                if batch.scene_key.get() == draw_data.target.scene_key {
                    encode_batch(
                        pass,
                        constants,
                        draw_data,
                        render_resources,
                        buffers,
                        batch,
                        &mut state,
                    );
                }
                command_index += 1;
            }
            SurfaceDrawCommand::Path { path_index, .. } => {
                let range_start = *path_index;
                let mut range_end = path_index + 1;
                command_index += 1;
                while let Some(SurfaceDrawCommand::Path {
                    z_index,
                    path_index,
                    ..
                }) = draw_data.draw_commands.get(command_index).filter(|_|command_index<end)
                {
                    if !scope.contains_z(*z_index) || *path_index != range_end {
                        break;
                    }
                    range_end += 1;
                    command_index += 1;
                }

                state.invalidate();
                pass.set_scissor_rect(0, 0, constants.physical_width, constants.physical_height);
                let global_start = draw_data.path_draw_offset + range_start;
                let global_end = draw_data.path_draw_offset + range_end;
                render_resources.path_renderer.encode_draw_range(
                    pass,
                    render_resources.path_globals_bind_group,
                    render_resources.path_clips_bind_group,
                    draw_data.target.globals_offset,
                    draw_data.target.path_clips_offset,
                    global_start..global_end,
                );
            }
            SurfaceDrawCommand::Text {
                quad_start,
                quad_count,
                ..
            } => {
                let range_start = *quad_start;
                let mut range_end = quad_start + quad_count;
                command_index += 1;
                while let Some(SurfaceDrawCommand::Text {
                    z_index,
                    quad_start,
                    quad_count,
                    ..
                }) = draw_data.draw_commands.get(command_index).filter(|_|command_index<end)
                {
                    if !scope.contains_z(*z_index) || *quad_start != range_end {
                        break;
                    }
                    range_end += quad_count;
                    command_index += 1;
                }

                state.invalidate();
                encode_text_range(
                    pass,
                    constants,
                    draw_data,
                    render_resources,
                    buffers,
                    range_start..range_end,
                );
            }
        }
    }
}

fn encode_text_range<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    constants: FrameConstants,
    draw_data: FrameDrawData<'a>,
    render_resources: RenderResources<'a>,
    buffers: PipelineBuffers<'a>,
    quad_range: Range<u32>,
) {
    if quad_range.is_empty() {
        return;
    }
    let Some(quads) = draw_data
        .text_quads
        .get(quad_range.start as usize..quad_range.end as usize)
    else {
        debug_assert!(false, "SPEC-HAL-359: scheduled text range is out of bounds");
        return;
    };
    let (Some(vertex_buffer), Some(index_buffer)) =
        (buffers.text_vertex_buffer, buffers.text_index_buffer)
    else {
        return;
    };

    pass.set_pipeline(&render_resources.text_pipeline.pipeline);
    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.set_bind_group(
        0,
        render_resources.rect_globals_bind_group,
        &[draw_data.target.globals_offset],
    );
    pass.set_bind_group(1, render_resources.text_bind_group, &[]);

    let text_index_start = (draw_data.text_quad_offset + quad_range.start) * 6;
    let mut current_clip: Option<Option<RectF>> = None;
    let mut batch_start = text_index_start;
    let mut batch_visible = true;

    for (quad_index, quad) in quads.iter().enumerate() {
        let quad_clip = quad.clip_rect;
        if current_clip.is_none() || current_clip != Some(quad_clip) {
            if current_clip.is_some() {
                let batch_end = text_index_start + (quad_index * 6) as u32;
                if batch_visible && batch_end > batch_start {
                    pass.draw_indexed(batch_start..batch_end, 0, 0..1);
                }
            }

            let (sx, sy, sw, sh) = match quad_clip {
                None => (0, 0, constants.physical_width, constants.physical_height),
                Some(clip) => constants.clip_rect_to_scissor(Some(&clip)),
            };
            batch_visible = sw > 0 && sh > 0;
            if batch_visible {
                pass.set_scissor_rect(sx, sy, sw, sh);
            }
            current_clip = Some(quad_clip);
            batch_start = text_index_start + (quad_index * 6) as u32;
        }
    }

    let final_end = text_index_start + (quads.len() * 6) as u32;
    if batch_visible && final_end > batch_start {
        pass.draw_indexed(batch_start..final_end, 0, 0..1);
    }
}
