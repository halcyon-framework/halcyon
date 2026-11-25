//! SPEC-HAL-084: Overlay Pass Executor
//!
//! Renders overlay-scope commands in immutable cross-pipeline Z order.

use crate::renderer::render_graph::executor::{PassContext, PassDrawScope};
use crate::renderer::render_graph::pass::{PassExecutionError, PassExecutor};
use crate::renderer::render_graph::passes::pipeline_dispatch::encode_draw_commands_for_scope;
use crate::renderer::render_graph::resources::AttachmentHandle;

/// Executor for overlay batches at or above the overlay layer.
pub struct OverlayPassExecutor {
    /// Handle to the swapchain texture view.
    pub output_attachment: AttachmentHandle,
}

impl OverlayPassExecutor {
    /// Create a new overlay pass executor.
    pub fn new(output_attachment: AttachmentHandle) -> Self {
        Self { output_attachment }
    }
}

impl PassExecutor for OverlayPassExecutor {
    fn execute(&self, ctx: &mut PassContext) -> Result<(), PassExecutionError> {
        if !ctx
            .draw_data
            .draw_commands
            .iter()
            .any(|command| PassDrawScope::Overlay.contains_z(command.z_index()))
        {
            return Ok(());
        }

        let output_view = ctx
            .resources
            .get_attachment(self.output_attachment)
            .ok_or_else(|| {
                PassExecutionError::missing_attachment(ctx.pass_name, self.output_attachment)
            })?;

        let constants = ctx.constants;
        let draw_data = ctx.draw_data;
        let render_resources = ctx.render_resources;
        let buffers = ctx.buffers;

        {
            let mut overlay_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                multiview_mask: None,
                occlusion_query_set: None,
            });

            overlay_pass.set_scissor_rect(
                0,
                0,
                constants.physical_width,
                constants.physical_height,
            );
            encode_draw_commands_for_scope(
                &mut overlay_pass,
                constants,
                draw_data,
                render_resources,
                buffers,
                PassDrawScope::Overlay,
            );
        }

        Ok(())
    }
}
