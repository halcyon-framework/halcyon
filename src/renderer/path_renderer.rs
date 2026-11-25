//! SPEC-R-004: PathRenderer facade
//!
//! Stage 8: Owns PathPipeline + PathBatchOwned and provides a clean API
//! for the renderer to accumulate path geometry, upload to GPU, and encode draws.

use crate::renderer::pipelines::path_pipeline::{PathBatch, PathBatchOwned, PathPipeline};

/// Facade for GPU path rendering.
///
/// Owns the pipeline and an owned batch, exposing a simple API for:
/// - Accumulating path geometry into the batch
/// - Uploading to GPU via prepare_frame
/// - Encoding draws into a render pass
pub struct PathRenderer {
    pipeline: PathPipeline,
    batch: PathBatchOwned,
}

impl PathRenderer {
    /// Create a new PathRenderer with a fresh pipeline and empty batch.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            pipeline: PathPipeline::create_with_format(device, queue, surface_format),
            batch: PathBatchOwned::default(),
        }
    }

    /// Get a reference to the owned batch.
    pub fn batch(&self) -> &PathBatchOwned {
        &self.batch
    }

    /// Get a mutable reference to the owned batch.
    pub fn batch_mut(&mut self) -> &mut PathBatchOwned {
        &mut self.batch
    }

    /// Clear the batch for start-of-frame or reuse.
    pub fn clear_batch(&mut self) {
        self.batch.clear();
    }

    /// Upload batch data to GPU buffers.
    pub fn prepare_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let batch_view: PathBatch<'_> = self.batch.as_batch();
        self.pipeline.prepare_frame(device, queue, &batch_view);
    }

    /// Prepare path buffers while delegating transfer ownership to the renderer.
    pub(crate) fn prepare_frame_with_upload<F>(&mut self, device: &wgpu::Device, upload: F)
    where
        F: FnMut(&wgpu::Buffer, &[u8]),
    {
        let batch_view: PathBatch<'_> = self.batch.as_batch();
        self.pipeline
            .prepare_frame_with_upload(device, &batch_view, upload);
    }

    /// Encode draw commands into a render pass.
    ///
    /// Uses per-path draw calls to ensure correct rendering. Each path gets its own
    /// draw call with its specific index range and instance data. This fixes the
    /// hover bleed bug where all paths would incorrectly share all instance colors.
    ///
    /// # Arguments
    /// * `pass` - The render pass to encode into
    /// * `globals_bg` - Bind group for globals (group 0)
    /// * `clips_bg` - Bind group for clip rectangles (group 1)
    pub fn encode_draws<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        globals_bg: &'a wgpu::BindGroup,
        clips_bg: &'a wgpu::BindGroup,
    ) {
        self.encode_draws_filtered(pass, globals_bg, clips_bg, 0, 0, |_| true);
    }

    /// Encode selected draw commands into a render pass with explicit dynamic offsets.
    pub fn encode_draws_filtered<'a, F>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        globals_bg: &'a wgpu::BindGroup,
        clips_bg: &'a wgpu::BindGroup,
        globals_offset: u32,
        clips_offset: u32,
        mut include: F,
    ) where
        F: FnMut(usize) -> bool,
    {
        // Skip if nothing to draw
        if self.batch.draw_commands.is_empty() {
            return;
        }

        // Get buffers from pipeline
        let vertex_buffer = match self.pipeline.vertex_buffer() {
            Some(b) => b,
            None => return,
        };
        let index_buffer = match self.pipeline.index_buffer() {
            Some(b) => b,
            None => return,
        };
        let instance_buffer = match self.pipeline.instance_buffer() {
            Some(b) => b,
            None => return,
        };

        // Set up pipeline state once
        pass.set_pipeline(self.pipeline.render_pipeline());
        pass.set_bind_group(0, globals_bg, &[globals_offset]);
        pass.set_bind_group(1, clips_bg, &[clips_offset]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        // Emit one draw call per path (fixes hover bleed bug)
        for (path_idx, cmd) in self.batch.draw_commands.iter().enumerate() {
            if !include(path_idx) {
                continue;
            }

            pass.draw_indexed(
                cmd.start_index..(cmd.start_index + cmd.index_count),
                0,
                cmd.instance_id..(cmd.instance_id + 1),
            );
        }
    }

    /// SPEC-HAL-359: Encode one contiguous range from the frame-global path
    /// command list. Compile owns the exact range; Execute performs no search.
    pub(crate) fn encode_draw_range<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        globals_bg: &'a wgpu::BindGroup,
        clips_bg: &'a wgpu::BindGroup,
        globals_offset: u32,
        clips_offset: u32,
        range: std::ops::Range<usize>,
    ) {
        if range.is_empty() {
            return;
        }
        let Some(commands) = self.batch.draw_commands.get(range) else {
            debug_assert!(false, "SPEC-HAL-359: scheduled path range is out of bounds");
            return;
        };
        let (Some(vertex_buffer), Some(index_buffer), Some(instance_buffer)) = (
            self.pipeline.vertex_buffer(),
            self.pipeline.index_buffer(),
            self.pipeline.instance_buffer(),
        ) else {
            return;
        };

        pass.set_pipeline(self.pipeline.render_pipeline());
        pass.set_bind_group(0, globals_bg, &[globals_offset]);
        pass.set_bind_group(1, clips_bg, &[clips_offset]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for command in commands {
            pass.draw_indexed(
                command.start_index..(command.start_index + command.index_count),
                0,
                command.instance_id..(command.instance_id + 1),
            );
        }
    }

    /// Get readonly access to the pipeline (for bind group creation).
    pub fn pipeline(&self) -> &crate::renderer::pipelines::path_pipeline::PathPipeline {
        &self.pipeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::pipelines::path_pipeline::{PathInstance, PathVertex};

    fn create_test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))
        .ok()?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Test Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: Default::default(),
            trace: Default::default(),
        }))
        .ok()?;

        Some((device, queue))
    }

    fn create_test_config() -> wgpu::SurfaceConfiguration {
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 64,
            height: 64,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        }
    }

    #[test]
    fn test_path_renderer_batch_clear() {
        let Some((device, queue)) = create_test_device() else {
            return;
        };
        let config = create_test_config();

        let mut renderer = PathRenderer::new(&device, &queue, config.format);

        // Fill the batch
        {
            let batch = renderer.batch_mut();
            batch.vertices.push(PathVertex {
                position: [0.0, 0.0],
            });
            batch.vertices.push(PathVertex {
                position: [1.0, 0.0],
            });
            batch.indices.push(0);
            batch.indices.push(1);
            batch.instances.push(PathInstance {
                color: [1.0, 0.0, 0.0, 1.0],
                transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                clip_id: 0,
                stroke_flag: 0,
            });
        }

        // Before clear
        assert!(!renderer.batch().is_empty());
        assert_eq!(renderer.batch().len(), (2, 2, 1));

        // Clear
        renderer.clear_batch();

        // After clear
        assert!(renderer.batch().is_empty());
        assert_eq!(renderer.batch().len(), (0, 0, 0));
    }

    /// Helper to create dummy bind groups for testing.
    fn create_test_bind_groups(
        device: &wgpu::Device,
        renderer: &PathRenderer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        let layouts = renderer.pipeline().bind_group_layouts();

        // Globals buffer: 16 bytes (viewport_size: vec2<f32> + scale_factor: f32 + _padding: f32)
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test Globals Buffer"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Test Globals Bind Group"),
            layout: &layouts.globals,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        // Clips buffer: minimum 16 bytes for one ClipRect (min: vec2<f32> + max: vec2<f32>)
        let clips_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test Clips Buffer"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let clips_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Test Clips Bind Group"),
            layout: &layouts.clips,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: clips_buffer.as_entire_binding(),
            }],
        });

        (globals_bg, clips_bg)
    }

    #[test]
    fn test_path_renderer_prepare_and_encode_no_panic() {
        let Some((device, queue)) = create_test_device() else {
            return;
        };
        let config = create_test_config();

        let mut renderer = PathRenderer::new(&device, &queue, config.format);

        // Fill batch with a triangle for a real draw
        {
            let batch = renderer.batch_mut();
            batch.vertices.push(PathVertex {
                position: [0.0, 0.0],
            });
            batch.vertices.push(PathVertex {
                position: [1.0, 0.0],
            });
            batch.vertices.push(PathVertex {
                position: [0.5, 1.0],
            });
            batch.indices.push(0);
            batch.indices.push(1);
            batch.indices.push(2);
            batch.instances.push(PathInstance {
                color: [1.0, 0.0, 0.0, 1.0],
                transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                clip_id: 0,
                stroke_flag: 0,
            });
        }

        let (globals_bg, clips_bg) = create_test_bind_groups(&device, &renderer);

        renderer.prepare_frame(&device, &queue);

        // Create render target
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Texture"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Encode draws
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                multiview_mask: None,
                occlusion_query_set: None,
            });

            renderer.encode_draws(&mut pass, &globals_bg, &clips_bg);
        }

        queue.submit(Some(encoder.finish()));
    }
}
