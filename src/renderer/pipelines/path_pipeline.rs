//! SPEC-R-004: GPU Path Rendering Pipeline — Stage 6
//!
//! Stage 6 adds: Tessellated path upload and draw encoding.
//! prepare_frame() now accepts PathBatch and uploads to GPU.
//! encode_draws() now emits real wgpu draw calls.

use bytemuck::{Pod, Zeroable};

// ============================================================================
// Constants
// ============================================================================

/// Instance data flag: fill mode
pub const STROKE_FILL_FLAG_FILL: u32 = 0;

/// Instance data flag: stroke mode
pub const STROKE_FILL_FLAG_STROKE: u32 = 1;

/// Minimum vertex buffer capacity
const MIN_VERTEX_CAPACITY: u32 = 64;

/// Minimum index buffer capacity
const MIN_INDEX_CAPACITY: u32 = 128;

/// Minimum instance buffer capacity
const MIN_INSTANCE_CAPACITY: u32 = 16;

// ============================================================================
// Path Batch (CPU-side data container)
// ============================================================================

/// Temporary container for CPU-side tessellated path data.
///
/// This is a simple data holder with no logic. Will be replaced by
/// Scene Batcher integration in R-010.
pub struct PathBatch<'a> {
    pub vertices: &'a [PathVertex],
    pub indices: &'a [u32],
    pub instances: &'a [PathInstance],
}

/// Per-path draw command for correct instanced rendering.
///
/// Each path has its own geometry (vertices/indices) and instance data.
/// We need separate draw calls per path to avoid hover bleed bug where
/// all paths would be drawn with all instance colors.
#[derive(Debug, Clone, Copy)]
pub struct PathDrawCommand {
    /// Start index in the index buffer
    pub start_index: u32,
    /// Number of indices for this path
    pub index_count: u32,
    /// Instance ID (index into instance buffer)
    pub instance_id: u32,
}

/// Owned container for tessellated path data.
///
/// This is the owned variant of `PathBatch<'a>` that stores data in vectors.
/// Use `as_batch()` to get a borrowing `PathBatch<'_>` for `prepare_frame`.
#[derive(Debug, Default)]
pub struct PathBatchOwned {
    pub vertices: Vec<PathVertex>,
    pub indices: Vec<u32>,
    pub instances: Vec<PathInstance>,
    /// Per-path draw commands for correct rendering (one draw call per path)
    pub draw_commands: Vec<PathDrawCommand>,
}

impl PathBatchOwned {
    /// Returns a borrowing view into the owned buffers.
    pub fn as_batch(&self) -> PathBatch<'_> {
        PathBatch {
            vertices: &self.vertices,
            indices: &self.indices,
            instances: &self.instances,
        }
    }

    /// Clears all vectors without shrinking capacity.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.instances.clear();
        self.draw_commands.clear();
    }

    /// Returns `true` only if all buffers are empty.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.indices.is_empty() && self.instances.is_empty()
    }

    /// Returns `(vertices_len, indices_len, instances_len)`.
    pub fn len(&self) -> (usize, usize, usize) {
        (
            self.vertices.len(),
            self.indices.len(),
            self.instances.len(),
        )
    }
}

// ============================================================================
// Shader Source
// ============================================================================

/// WGSL shader source for path rendering.
///
/// Stage 4: Basic pass-through shader. Assumes positions are already in NDC.
/// Stage 13: Added clip rectangle testing in fragment shader.
/// Stage 14: Transform pipeline - applies affine transform and viewport conversion.
const PATH_SHADER_SOURCE: &str = include_str!("../shaders/path.wgsl");

// ============================================================================
// GPU Vertex Format
// ============================================================================

/// GPU vertex format for path rendering.
///
/// Each vertex contains only a 2D position. All other attributes
/// (color, transform, clip) are per-instance.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PathVertex {
    pub position: [f32; 2],
}

impl PathVertex {
    /// Vertex buffer layout for PathVertex (buffer slot 0).
    ///
    /// - array_stride: sizeof(PathVertex) = 8 bytes
    /// - step_mode: Vertex
    /// - attribute 0: float32x2 (position)
    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PathVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

// ============================================================================
// GPU Instance Format
// ============================================================================

/// GPU instance format for path rendering.
///
/// Each instance carries:
/// - color: RGBA color
/// - transform: 3x2 affine matrix (m00 m01 m02 m10 m11 m12)
/// - clip_id: Index into clip buffer (0 = no clip)
/// - stroke_flag: 0 = fill, 1 = stroke
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PathInstance {
    /// RGBA color
    pub color: [f32; 4],
    /// 3x2 affine matrix stored as [m00, m01, m02, m10, m11, m12]
    pub transform: [f32; 6],
    /// Clip buffer index (0 = no clip)
    pub clip_id: u32,
    /// Fill/stroke flag: 0 = fill, 1 = stroke
    pub stroke_flag: u32,
}

impl PathInstance {
    /// Instance buffer layout for PathInstance (buffer slot 1).
    ///
    /// - array_stride: sizeof(PathInstance) = 48 bytes
    /// - step_mode: Instance
    /// - attributes:
    ///   - location 1: float32x4 (color)
    ///   - location 2: float32x4 (transform m00 m01 m02 m10)
    ///   - location 3: float32x2 (transform m11 m12)
    ///   - location 4: uint32 (clip_id)
    ///   - location 5: uint32 (stroke_flag)
    pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PathInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // color: float32x4 at offset 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // transform[0..4]: float32x4 at offset 16
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // transform[4..6]: float32x2 at offset 32
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // clip_id: uint32 at offset 40
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
                // stroke_flag: uint32 at offset 44
                wgpu::VertexAttribute {
                    offset: 44,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

// ============================================================================
// Bind Group Layouts
// ============================================================================

/// Bind group layouts for path pipeline.
///
/// Stage 3: Only layout definitions - no actual bind groups created.
pub struct PathBindGroupLayouts {
    /// Layout for global uniforms (viewport, time, etc.)
    pub globals: wgpu::BindGroupLayout,
    /// Layout for clip buffer
    pub clips: wgpu::BindGroupLayout,
}

// ============================================================================
// Path Pipeline
// ============================================================================

/// GPU path rendering pipeline.
///
/// Stage 6: Real GPU pipeline with buffer management and draw encoding.
/// prepare_frame() uploads tessellated data, encode_draws() emits draw calls.
pub struct PathPipeline {
    /// Pipeline layout
    pub layout: wgpu::PipelineLayout,
    /// Vertex buffer layouts (position + instance)
    pub vertex_layouts: Vec<wgpu::VertexBufferLayout<'static>>,
    /// Bind group layouts
    pub bind_group_layouts: PathBindGroupLayouts,
    /// Render pipeline for path drawing
    pub render_pipeline: wgpu::RenderPipeline,

    /// GPU vertex buffer (None until first ensure_vertex_capacity call)
    vertex_buffer: Option<wgpu::Buffer>,
    /// GPU index buffer (None until first ensure_index_capacity call)
    index_buffer: Option<wgpu::Buffer>,
    /// GPU instance buffer (None until first ensure_instance_capacity call)
    instance_buffer: Option<wgpu::Buffer>,

    /// Current vertex buffer capacity in vertices
    vertex_capacity: u32,
    /// Current index buffer capacity in indices
    index_capacity: u32,
    /// Current instance buffer capacity in instances
    instance_capacity: u32,

    /// Actual uploaded vertex count (for draw calls)
    pub vertex_count: u32,
    /// Actual uploaded index count (for draw calls)
    pub index_count: u32,
    /// Actual uploaded instance count (for draw calls)
    pub instance_count: u32,
}

impl PathPipeline {
    /// Create a new path pipeline.
    ///
    /// Stage 4: Creates layouts and a real render pipeline with shaders.
    /// The pipeline is compiled and ready, but frame/draw APIs remain no-ops.
    ///
    /// # Arguments
    /// * `device` - The wgpu device for resource creation
    /// * `_queue` - The wgpu queue (unused in Stage 4)
    /// * `config` - Surface configuration for format
    /// Create a path pipeline for the given surface format.
    pub fn create_with_format(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        // Create bind group layout for globals (viewport uniforms)
        // SPEC-HAL-081B: Enable dynamic offset for per-target globals binding
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Path Pipeline Globals Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create bind group layout for clips (clip rectangles buffer)
        // SPEC-HAL-081B: has_dynamic_offset=true for per-target clip slabs
        let clips_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Path Pipeline Clips Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group_layouts = PathBindGroupLayouts {
            globals: globals_layout,
            clips: clips_layout,
        };

        // Create pipeline layout
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Path Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layouts.globals), Some(&bind_group_layouts.clips)],
            immediate_size: 0,
        });

        // Build vertex buffer layouts
        let vertex_layouts = vec![
            PathVertex::vertex_buffer_layout(),
            PathInstance::instance_buffer_layout(),
        ];

        // Create shader module from WGSL source
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Path Shader"),
            source: wgpu::ShaderSource::Wgsl(PATH_SHADER_SOURCE.into()),
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Path Render Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Some(PathVertex::vertex_buffer_layout()),
                    Some(PathInstance::instance_buffer_layout()),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                // Alpha model: PREMULTIPLIED - shader outputs (r*a, g*a, b*a, a)
                // BlendComponent::OVER = One / OneMinusSrcAlpha (premultiplied blending)
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            layout,
            vertex_layouts,
            bind_group_layouts,
            render_pipeline,
            vertex_buffer: None,
            index_buffer: None,
            instance_buffer: None,
            vertex_capacity: 0,
            index_capacity: 0,
            instance_capacity: 0,
            vertex_count: 0,
            index_count: 0,
            instance_count: 0,
        }
    }

    // ========================================================================
    // Buffer Capacity Management
    // ========================================================================

    /// Ensure vertex buffer has at least `min_vertices` capacity.
    ///
    /// Uses grow-double strategy: new_capacity = max(min_required, old_capacity * 2, MIN_SIZE).
    /// Allocates a new buffer if current capacity is insufficient.
    pub fn ensure_vertex_capacity(&mut self, device: &wgpu::Device, min_vertices: u32) {
        if self.vertex_capacity >= min_vertices {
            return;
        }

        let new_capacity = min_vertices
            .max(self.vertex_capacity.saturating_mul(2))
            .max(MIN_VERTEX_CAPACITY);

        let size = (new_capacity as usize) * std::mem::size_of::<PathVertex>();

        self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Vertex Buffer"),
            size: size as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        }));

        self.vertex_capacity = new_capacity;
    }

    /// Ensure index buffer has at least `min_indices` capacity.
    ///
    /// Uses grow-double strategy: new_capacity = max(min_required, old_capacity * 2, MIN_SIZE).
    /// Allocates a new buffer if current capacity is insufficient.
    pub fn ensure_index_capacity(&mut self, device: &wgpu::Device, min_indices: u32) {
        if self.index_capacity >= min_indices {
            return;
        }

        let new_capacity = min_indices
            .max(self.index_capacity.saturating_mul(2))
            .max(MIN_INDEX_CAPACITY);

        let size = (new_capacity as usize) * std::mem::size_of::<u32>();

        self.index_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Index Buffer"),
            size: size as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDEX,
            mapped_at_creation: false,
        }));

        self.index_capacity = new_capacity;
    }

    /// Ensure instance buffer has at least `min_instances` capacity.
    ///
    /// Uses grow-double strategy: new_capacity = max(min_required, old_capacity * 2, MIN_SIZE).
    /// Allocates a new buffer if current capacity is insufficient.
    pub fn ensure_instance_capacity(&mut self, device: &wgpu::Device, min_instances: u32) {
        if self.instance_capacity >= min_instances {
            return;
        }

        let new_capacity = min_instances
            .max(self.instance_capacity.saturating_mul(2))
            .max(MIN_INSTANCE_CAPACITY);

        let size = (new_capacity as usize) * std::mem::size_of::<PathInstance>();

        self.instance_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Instance Buffer"),
            size: size as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        }));

        self.instance_capacity = new_capacity;
    }

    // ========================================================================
    // Buffer Upload
    // ========================================================================

    /// Upload vertex data to GPU.
    ///
    /// # Panics
    /// Panics if `ensure_vertex_capacity` was not called with sufficient capacity.
    pub fn upload_vertices(&mut self, queue: &wgpu::Queue, data: &[PathVertex]) {
        let buffer = self
            .vertex_buffer
            .as_ref()
            .expect("upload_vertices called without ensure_vertex_capacity");

        assert!(
            data.len() <= self.vertex_capacity as usize,
            "upload_vertices: data length {} exceeds capacity {}",
            data.len(),
            self.vertex_capacity
        );

        queue.write_buffer(buffer, 0, bytemuck::cast_slice(data));
    }

    /// Upload index data to GPU.
    ///
    /// # Panics
    /// Panics if `ensure_index_capacity` was not called with sufficient capacity.
    pub fn upload_indices(&mut self, queue: &wgpu::Queue, data: &[u32]) {
        let buffer = self
            .index_buffer
            .as_ref()
            .expect("upload_indices called without ensure_index_capacity");

        assert!(
            data.len() <= self.index_capacity as usize,
            "upload_indices: data length {} exceeds capacity {}",
            data.len(),
            self.index_capacity
        );

        queue.write_buffer(buffer, 0, bytemuck::cast_slice(data));
    }

    /// Upload instance data to GPU.
    ///
    /// # Panics
    /// Panics if `ensure_instance_capacity` was not called with sufficient capacity.
    pub fn upload_instances(&mut self, queue: &wgpu::Queue, data: &[PathInstance]) {
        let buffer = self
            .instance_buffer
            .as_ref()
            .expect("upload_instances called without ensure_instance_capacity");

        assert!(
            data.len() <= self.instance_capacity as usize,
            "upload_instances: data length {} exceeds capacity {}",
            data.len(),
            self.instance_capacity
        );

        queue.write_buffer(buffer, 0, bytemuck::cast_slice(data));
    }

    // ========================================================================
    // Accessors (for testing)
    // ========================================================================

    /// Get current vertex buffer capacity.
    #[cfg(test)]
    pub fn vertex_capacity(&self) -> u32 {
        self.vertex_capacity
    }

    /// Get current index buffer capacity.
    #[cfg(test)]
    pub fn index_capacity(&self) -> u32 {
        self.index_capacity
    }

    /// Get current instance buffer capacity.
    #[cfg(test)]
    pub fn instance_capacity(&self) -> u32 {
        self.instance_capacity
    }

    /// Check if vertex buffer exists.
    #[cfg(test)]
    pub fn has_vertex_buffer(&self) -> bool {
        self.vertex_buffer.is_some()
    }

    /// Check if index buffer exists.
    #[cfg(test)]
    pub fn has_index_buffer(&self) -> bool {
        self.index_buffer.is_some()
    }

    /// Check if instance buffer exists.
    #[cfg(test)]
    pub fn has_instance_buffer(&self) -> bool {
        self.instance_buffer.is_some()
    }

    /// Get readonly access to bind group layouts.
    pub fn bind_group_layouts(&self) -> &PathBindGroupLayouts {
        &self.bind_group_layouts
    }

    /// Get readonly access to vertex buffer (for PathRenderer draw encoding).
    pub fn vertex_buffer(&self) -> Option<&wgpu::Buffer> {
        self.vertex_buffer.as_ref()
    }

    /// Get readonly access to index buffer (for PathRenderer draw encoding).
    pub fn index_buffer(&self) -> Option<&wgpu::Buffer> {
        self.index_buffer.as_ref()
    }

    /// Get readonly access to instance buffer (for PathRenderer draw encoding).
    pub fn instance_buffer(&self) -> Option<&wgpu::Buffer> {
        self.instance_buffer.as_ref()
    }

    /// Get readonly access to render pipeline (for PathRenderer draw encoding).
    pub fn render_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.render_pipeline
    }

    // ========================================================================
    // Frame API (Stage 6: Real Upload + Draw)
    // ========================================================================

    /// Prepare frame data for rendering.
    ///
    /// Resizes GPU buffers if needed and uploads tessellated path data.
    pub fn prepare_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, batch: &PathBatch) {
        self.prepare_frame_with_upload(device, batch, |destination, data| {
            queue.write_buffer(destination, 0, data);
        });
    }

    /// Prepare frame data and delegate each GPU upload to the renderer owner.
    ///
    /// Production rendering uses this path so transient transfer resources remain
    /// inside the renderer's SPEC-HAL-040 staging lifecycle.
    pub(crate) fn prepare_frame_with_upload<F>(
        &mut self,
        device: &wgpu::Device,
        batch: &PathBatch,
        mut upload: F,
    ) where
        F: FnMut(&wgpu::Buffer, &[u8]),
    {
        // Store actual counts for draw calls
        self.vertex_count = batch.vertices.len() as u32;
        self.index_count = batch.indices.len() as u32;
        self.instance_count = batch.instances.len() as u32;

        // Skip if nothing to upload
        if batch.vertices.is_empty() || batch.indices.is_empty() || batch.instances.is_empty() {
            return;
        }

        // Ensure capacity
        self.ensure_vertex_capacity(device, batch.vertices.len() as u32);
        self.ensure_index_capacity(device, batch.indices.len() as u32);
        self.ensure_instance_capacity(device, batch.instances.len() as u32);

        upload(
            self.vertex_buffer
                .as_ref()
                .expect("path vertex capacity must provide a buffer"),
            bytemuck::cast_slice(batch.vertices),
        );
        upload(
            self.index_buffer
                .as_ref()
                .expect("path index capacity must provide a buffer"),
            bytemuck::cast_slice(batch.indices),
        );
        upload(
            self.instance_buffer
                .as_ref()
                .expect("path instance capacity must provide a buffer"),
            bytemuck::cast_slice(batch.instances),
        );
    }

    /// Encode draw commands into a render pass.
    ///
    /// Binds pipeline, bind groups, buffers, and emits indexed draw call.
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
        // Skip if nothing to draw
        if self.index_count == 0 || self.instance_count == 0 {
            return;
        }

        let vertex_buffer = match &self.vertex_buffer {
            Some(b) => b,
            None => return,
        };
        let index_buffer = match &self.index_buffer {
            Some(b) => b,
            None => return,
        };
        let instance_buffer = match &self.instance_buffer {
            Some(b) => b,
            None => return,
        };

        pass.set_pipeline(&self.render_pipeline);
        pass.set_bind_group(0, globals_bg, &[0]);
        pass.set_bind_group(1, clips_bg, &[0]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..self.instance_count);
    }
}

#[cfg(test)]
impl PathPipeline {
    pub fn create_dummy() -> Self {
        use wgpu::{DeviceDescriptor, Instance};
        let instance = Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("Adapter unavailable");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default()))
                .expect("Device unavailable");

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };

        Self::create_with_format(&device, &queue, config.format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_vertex_size() {
        assert_eq!(std::mem::size_of::<PathVertex>(), 8);
    }

    #[test]
    fn test_path_instance_size() {
        // color: 16 bytes + transform: 24 bytes + clip_id: 4 bytes + stroke_flag: 4 bytes = 48 bytes
        assert_eq!(std::mem::size_of::<PathInstance>(), 48);
    }

    #[test]
    fn test_stroke_fill_flags() {
        assert_eq!(STROKE_FILL_FLAG_FILL, 0);
        assert_eq!(STROKE_FILL_FLAG_STROKE, 1);
    }

    // ========================================================================
    // Stage 5 Buffer Tests (require GPU)
    // ========================================================================

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

    #[test]
    fn test_path_pipeline_buffer_growth() {
        let Some((device, queue)) = create_test_device() else {
            return;
        };

        let dummy_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };

        let mut pipeline = PathPipeline::create_with_format(&device, &queue, dummy_config.format);

        // vertex
        pipeline.ensure_vertex_capacity(&device, 10);
        assert!(pipeline.vertex_capacity() >= 10);

        // index
        pipeline.ensure_index_capacity(&device, 10);
        assert!(pipeline.index_capacity() >= 10);

        // instance
        pipeline.ensure_instance_capacity(&device, 5);
        assert!(pipeline.instance_capacity() >= 5);
    }

    #[test]
    #[should_panic]
    fn test_upload_vertices_panics() {
        let Some((_device, queue)) = create_test_device() else {
            panic!("upload_vertices must panic without capacity");
        };
        let mut p = PathPipeline::create_dummy();
        p.upload_vertices(
            &queue,
            &[PathVertex {
                position: [0.0, 0.0],
            }],
        );
    }

    #[test]
    #[should_panic]
    fn test_upload_indices_panics() {
        let Some((_device, queue)) = create_test_device() else {
            panic!("upload_indices must panic without capacity");
        };
        let mut p = PathPipeline::create_dummy();
        p.upload_indices(&queue, &[0u32, 1, 2]);
    }

    #[test]
    #[should_panic]
    fn test_upload_instances_panics() {
        let Some((_device, queue)) = create_test_device() else {
            panic!("upload_instances must panic without capacity");
        };
        let mut p = PathPipeline::create_dummy();
        p.upload_instances(
            &queue,
            &[PathInstance {
                color: [1.0, 0.0, 0.0, 1.0],
                transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                clip_id: 0,
                stroke_flag: 0,
            }],
        );
    }

    // ========================================================================
    // Stage 6 Tests
    // ========================================================================

    #[test]
    fn test_prepare_frame_counts() {
        let Some((device, queue)) = create_test_device() else {
            return;
        };

        let dummy_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };

        let mut pipeline = PathPipeline::create_with_format(&device, &queue, dummy_config.format);

        let vertices = [
            PathVertex {
                position: [0.0, 0.0],
            },
            PathVertex {
                position: [1.0, 0.0],
            },
            PathVertex {
                position: [0.5, 1.0],
            },
        ];
        let indices = [0u32, 1, 2];
        let instances = [PathInstance {
            color: [1.0, 0.0, 0.0, 1.0],
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            clip_id: 0,
            stroke_flag: 0,
        }];

        let batch = PathBatch {
            vertices: &vertices,
            indices: &indices,
            instances: &instances,
        };

        pipeline.prepare_frame(&device, &queue, &batch);

        assert_eq!(pipeline.vertex_count, 3);
        assert_eq!(pipeline.index_count, 3);
        assert_eq!(pipeline.instance_count, 1);
    }

    /// Helper to create dummy bind groups for testing.
    fn create_test_bind_groups(
        device: &wgpu::Device,
        layouts: &PathBindGroupLayouts,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        // Globals buffer: 80 bytes (mat4x4<f32>=64 + vec2<f32>=8 + f32=4 + u32=4)
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test Globals Buffer"),
            size: 80,
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
    fn test_encode_draws_empty_batch() {
        let Some((device, queue)) = create_test_device() else {
            return;
        };

        let dummy_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 64,
            height: 64,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };

        let mut pipeline = PathPipeline::create_with_format(&device, &queue, dummy_config.format);
        let (globals_bg, clips_bg) =
            create_test_bind_groups(&device, pipeline.bind_group_layouts());

        // Prepare with empty batch - encode_draws should early-return
        let batch = PathBatch {
            vertices: &[],
            indices: &[],
            instances: &[],
        };
        pipeline.prepare_frame(&device, &queue, &batch);

        // Create a texture to render into
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

            // With empty batch, early-returns without drawing
            pipeline.encode_draws(&mut pass, &globals_bg, &clips_bg);
        }

        queue.submit(Some(encoder.finish()));
    }

    #[test]
    fn test_encode_draws_non_empty_batch() {
        let Some((device, queue)) = create_test_device() else {
            return;
        };

        let dummy_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 64,
            height: 64,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };

        let mut pipeline = PathPipeline::create_with_format(&device, &queue, dummy_config.format);
        let (globals_bg, clips_bg) =
            create_test_bind_groups(&device, pipeline.bind_group_layouts());

        // Prepare with non-empty batch (single triangle)
        let vertices = [
            PathVertex {
                position: [0.0, 0.0],
            },
            PathVertex {
                position: [1.0, 0.0],
            },
            PathVertex {
                position: [0.5, 1.0],
            },
        ];
        let indices = [0u32, 1, 2];
        let instances = [PathInstance {
            color: [1.0, 0.0, 0.0, 1.0],
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            clip_id: 0,
            stroke_flag: 0,
        }];

        let batch = PathBatch {
            vertices: &vertices,
            indices: &indices,
            instances: &instances,
        };
        pipeline.prepare_frame(&device, &queue, &batch);

        // Create a texture to render into
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

            // With non-empty batch and proper bind groups, should draw without panic
            pipeline.encode_draws(&mut pass, &globals_bg, &clips_bg);
        }

        queue.submit(Some(encoder.finish()));
    }

    // ========================================================================
    // Stage 7 PathBatchOwned Tests
    // ========================================================================

    #[test]
    fn test_path_batch_owned_defaults() {
        let batch = PathBatchOwned::default();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), (0, 0, 0));
        assert!(batch.as_batch().vertices.is_empty());
        assert!(batch.as_batch().indices.is_empty());
        assert!(batch.as_batch().instances.is_empty());
    }

    #[test]
    fn test_path_batch_owned_clear_and_len() {
        let mut batch = PathBatchOwned::default();

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

        assert!(!batch.is_empty());
        assert_eq!(batch.len(), (3, 3, 1));

        batch.clear();

        assert!(batch.is_empty());
        assert_eq!(batch.len(), (0, 0, 0));
    }

    #[test]
    fn test_path_batch_owned_as_batch_view() {
        let mut batch = PathBatchOwned::default();

        batch.vertices.push(PathVertex {
            position: [42.0, 99.0],
        });
        batch.indices.push(123);
        batch.instances.push(PathInstance {
            color: [0.25, 0.5, 0.75, 1.0],
            transform: [2.0, 0.0, 10.0, 0.0, 2.0, 20.0],
            clip_id: 7,
            stroke_flag: 1,
        });

        let view = batch.as_batch();

        // Verify lengths match
        assert_eq!(view.vertices.len(), batch.vertices.len());
        assert_eq!(view.indices.len(), batch.indices.len());
        assert_eq!(view.instances.len(), batch.instances.len());

        // Verify data matches (same underlying data, not a copy)
        assert_eq!(view.vertices[0].position, batch.vertices[0].position);
        assert_eq!(view.indices[0], batch.indices[0]);
        assert_eq!(view.instances[0].color, batch.instances[0].color);
        assert_eq!(view.instances[0].transform, batch.instances[0].transform);
        assert_eq!(view.instances[0].clip_id, batch.instances[0].clip_id);
        assert_eq!(
            view.instances[0].stroke_flag,
            batch.instances[0].stroke_flag
        );
    }
}
