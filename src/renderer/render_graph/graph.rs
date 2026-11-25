//! SPEC-HAL-084: RenderGraph Core
//!
//! The RenderGraph struct that orchestrates Build/Compile/Execute phases.
//! This is the main entry point for the render graph system.

use super::executor::{
    BoundResources, CompiledGraph, FrameConstants, FrameDrawData, PassContext, PipelineBuffers,
    RenderResources,
};
use super::pass::PassExecutionError;
use super::resources::AttachmentHandle;
use super::topology::RenderGraphTopology;
use std::collections::HashMap;

/// The RenderGraph orchestrates GPU pass execution.
///
/// # Phase Separation (INV-RG-003)
///
/// 1. **Build**: RenderGraphBuilder produces RenderGraphTopology
/// 2. **Compile**: RenderGraph::compile() binds physical resources
/// 3. **Execute**: RenderGraph::execute() runs pass executors
///
/// # Invariants
///
/// - INV-RG-001: Orchestration only — no geometry generation
/// - INV-RG-002: DrawPlan is sole input; SceneCache never reaches RenderGraph
/// - INV-RG-013: No submission/presentation — returns after encoding
/// - INV-RG-015: Pass executors have constrained access
pub struct RenderGraph;

impl RenderGraph {
    /// Compile phase: bind physical resources to logical handles.
    ///
    /// # Arguments
    ///
    /// * `topology` - The Build phase output.
    /// * `constants` - Per-frame constants (viewport, scale, etc.).
    /// * `attachments` - Physical texture views bound to attachment handles.
    ///
    /// # Returns
    ///
    /// CompiledGraph ready for Execute phase.
    pub fn compile<'a>(
        topology: &'a RenderGraphTopology,
        constants: FrameConstants,
        attachments: HashMap<AttachmentHandle, wgpu::TextureView>,
    ) -> CompiledGraph<'a> {
        let bound_resources = BoundResources {
            frosted:None,
            attachments,
            buffers: HashMap::new(),
        };

        CompiledGraph {
            topology,
            bound_resources,
            constants,
        }
    }

    /// Execute phase: run all pass executors in topological order.
    ///
    /// # Arguments
    ///
    /// * `compiled` - The Compile phase output.
    /// * `encoder` - The command encoder to record GPU commands into.
    /// * `draw_data` - Per-frame draw data from Batcher.
    /// * `render_resources` - Renderer-owned resources (pipelines, bind groups).
    /// * `buffers` - Renderer-owned GPU buffers.
    ///
    /// # INV-RG-013 Compliance
    ///
    /// This method does NOT call queue.submit() or surface.present().
    /// Those operations belong to the platform layer.
    #[allow(clippy::too_many_arguments)]
    pub fn execute<'a>(
        compiled: &'a CompiledGraph,
        encoder: &'a mut wgpu::CommandEncoder,
        draw_data: FrameDrawData<'a>,
        render_resources: RenderResources<'a>,
        buffers: PipelineBuffers<'a>,
    ) -> Result<(), PassExecutionError> {
        // Create PassContext for executors
        // Note: We need to handle the mutable borrow of encoder carefully
        // by executing passes sequentially.

        for pass in compiled.topology.passes_in_order() {
            // Create a fresh PassContext for each pass
            // This is a bit awkward because we need &mut encoder
            let mut ctx = PassContext {
                pass_name: pass.name,
                encoder,
                resources: &compiled.bound_resources,
                constants: compiled.constants,
                draw_data: FrameDrawData {
                    target: draw_data.target,
                    frame_batches: draw_data.frame_batches,
                    rounded_clip_offsets: draw_data.rounded_clip_offsets,
                    shadow_params_offsets: draw_data.shadow_params_offsets,
                    gradient_params_offsets: draw_data.gradient_params_offsets,
                    analytic_surface_params_offsets: draw_data.analytic_surface_params_offsets,
                    color_picker_sv_params_offsets: draw_data.color_picker_sv_params_offsets,
                    radial_gauge_params_offsets: draw_data.radial_gauge_params_offsets,
                    text_quads: draw_data.text_quads,
                    text_quad_offset: draw_data.text_quad_offset,
                    path_draw_offset: draw_data.path_draw_offset,
                    draw_commands: draw_data.draw_commands,
                },
                render_resources: RenderResources {
                    rect_pipeline: render_resources.rect_pipeline,
                    rect_globals_bind_group: render_resources.rect_globals_bind_group,
                    rounded_clip_bind_group: render_resources.rounded_clip_bind_group,
                    shadow_pipeline: render_resources.shadow_pipeline,
                    shadow_params_bind_group: render_resources.shadow_params_bind_group,
                    gradient_pipeline: render_resources.gradient_pipeline,
                    gradient_params_bind_group: render_resources.gradient_params_bind_group,
                    analytic_surface_pipeline: render_resources.analytic_surface_pipeline,
                    analytic_surface_params_bind_group: render_resources
                        .analytic_surface_params_bind_group,
                    color_picker_sv_pipeline: render_resources.color_picker_sv_pipeline,
                    color_picker_sv_params_bind_group: render_resources
                        .color_picker_sv_params_bind_group,
                    radial_gauge_pipeline: render_resources.radial_gauge_pipeline,
                    radial_gauge_params_bind_group: render_resources.radial_gauge_params_bind_group,
                    sdf_rect_pipeline: render_resources.sdf_rect_pipeline,
                    shape_pipeline: render_resources.shape_pipeline,
                    image_pipeline: render_resources.image_pipeline,
                    presence_terminal_pipelines: render_resources.presence_terminal_pipelines,
                    image_bind_groups: render_resources.image_bind_groups,
                    image_index_capacity: render_resources.image_index_capacity,
                    path_globals_bind_group: render_resources.path_globals_bind_group,
                    path_clips_bind_group: render_resources.path_clips_bind_group,
                    path_renderer: render_resources.path_renderer,
                    text_pipeline: render_resources.text_pipeline,
                    text_bind_group: render_resources.text_bind_group,
                },
                buffers,
            };

            pass.executor.execute(&mut ctx)?;
        }

        Ok(())
    }
}
