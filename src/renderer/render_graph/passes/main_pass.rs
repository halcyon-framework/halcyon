//! SPEC-HAL-084: Main Pass Executor
//!
//! Renders all main-scope content to the swapchain in the immutable
//! cross-pipeline Z order compiled under SPEC-HAL-359.

use crate::renderer::render_graph::executor::{
    FrameConstants, FrameDrawData, PassContext, PassDrawScope, PipelineBuffers, RenderResources,
};
use crate::renderer::render_graph::pass::{PassExecutionError, PassExecutor};
use crate::renderer::render_graph::passes::pipeline_dispatch::encode_draw_commands_for_scope;
use crate::renderer::render_graph::resources::AttachmentHandle;

/// Executor for the main render pass.
///
/// Renders all content below `LAYER_OVERLAY` to the swapchain surface.
///
/// # INV-RG-015 Compliance
///
/// This executor receives only:
/// - Encoder via PassContext
/// - Bound resources (textures, buffers)
/// - Draw data (batches, text quads)
/// - Renderer resources (pipelines, bind groups)
///
/// No access to Device, Queue, Surface, or resource pools.
pub struct MainPassExecutor {
    /// Handle to the swapchain texture view.
    pub output_attachment: AttachmentHandle,
}

impl MainPassExecutor {
    /// Create a new main pass executor.
    ///
    /// # Arguments
    /// * `output_attachment` - Handle to the swapchain output attachment.
    pub fn new(output_attachment: AttachmentHandle) -> Self {
        Self { output_attachment }
    }
    pub(crate) fn encode_with<'a>(
        &self,
        ctx: &'a mut PassContext<'_>,
        draw: impl FnOnce(
            &mut wgpu::RenderPass<'a>,
            FrameConstants,
            FrameDrawData<'a>,
            RenderResources<'a>,
            PipelineBuffers<'a>,
        ),
    ) -> Result<(), PassExecutionError> {
        let output_view = ctx
            .resources
            .get_attachment(self.output_attachment)
            .ok_or_else(|| {
                PassExecutionError::missing_attachment(ctx.pass_name, self.output_attachment)
            })?;

        let clear_color = ctx.constants.clear_color_wgpu();
        let constants = ctx.constants;
        let draw_data = ctx.draw_data;
        let render_resources = ctx.render_resources;
        let buffers = ctx.buffers;

        {
            let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                multiview_mask: None,
                occlusion_query_set: None,
            });

            render_pass.set_scissor_rect(0, 0, constants.physical_width, constants.physical_height);
            draw(
                &mut render_pass,
                constants,
                draw_data,
                render_resources,
                buffers,
            );
        }

        Ok(())
    }
}

impl PassExecutor for MainPassExecutor {
    fn execute(&self, ctx: &mut PassContext) -> Result<(), PassExecutionError> {
        self.encode_with(
            ctx,
            |pass, constants, draw_data, render_resources, buffers| {
                encode_draw_commands_for_scope(
                    pass,
                    constants,
                    draw_data,
                    render_resources,
                    buffers,
                    PassDrawScope::MainUnderlay,
                );
            },
        )
    }
}
