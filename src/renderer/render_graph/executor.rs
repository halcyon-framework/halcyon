//! SPEC-HAL-084: Pass Execution Context and Compiled Graph
//!
//! PassContext provides constrained access to pass executors.
//! CompiledGraph is the Compile phase output, ready for Execute.

use super::resources::{AttachmentHandle, BufferHandle};
use super::topology::RenderGraphTopology;
use crate::core::math::{RectF, Size};
use crate::core::scene::{Color, TextQuad};
use crate::renderer::batcher::FrameBatches;
use crate::renderer::compiled_draw_plan::SurfaceDrawCommand;
use crate::renderer::path::PathDraw;
use crate::renderer::pipelines::text_pipeline::TextPipeline;
use crate::renderer::pixel_coverage::{
    PhysicalExtent, PhysicalTargetMetrics, physical_composition_coverage,
};
use std::collections::HashMap;

/// Per-frame constants available to all pass executors.
#[derive(Debug, Clone, Copy)]
pub struct FrameConstants {
    /// Viewport size in physical pixels.
    pub viewport_size: [f32; 2],
    /// Physical width in pixels.
    pub physical_width: u32,
    /// Physical height in pixels.
    pub physical_height: u32,
    /// Logical width in pixels.
    pub logical_width: f32,
    /// Logical height in pixels.
    pub logical_height: f32,
    /// Scale factor (physical / logical).
    pub scale_factor: f32,
    /// Clear color for render passes.
    pub clear_color: Color,
}

impl FrameConstants {
    /// Convert the frame clear color to wgpu color.
    pub fn clear_color_wgpu(&self) -> wgpu::Color {
        wgpu::Color {
            r: self.clear_color.r as f64,
            g: self.clear_color.g as f64,
            b: self.clear_color.b as f64,
            a: self.clear_color.a as f64,
        }
    }

    /// Convert a logical clip rect to GPU scissor coordinates.
    ///
    /// # Arguments
    /// * `clip` - Optional clip rect in logical coordinates. None = full viewport.
    ///
    /// # Returns
    /// (x, y, width, height) in physical pixels, clamped to render target bounds.
    /// Empty coverage is returned as `(0, 0, 0, 0)` and must not be submitted to wgpu.
    pub fn clip_rect_to_scissor(&self, clip: Option<&RectF>) -> (u32, u32, u32, u32) {
        let logical_extent = Size::new(
            self.physical_width as f32 / self.scale_factor,
            self.physical_height as f32 / self.scale_factor,
        );
        let target = PhysicalTargetMetrics::new(
            logical_extent,
            PhysicalExtent::new(self.physical_width, self.physical_height),
            self.scale_factor,
        )
        .unwrap_or_else(|error| panic!("SPEC-HAL-266A: invalid scissor target: {error}"));
        let logical_rect = clip.copied().unwrap_or(RectF::new(
            0.0,
            0.0,
            logical_extent.width,
            logical_extent.height,
        ));
        physical_composition_coverage(logical_rect, target)
            .unwrap_or_else(|error| panic!("SPEC-HAL-266A: invalid scissor rect: {error}"))
            .map_or((0, 0, 0, 0), |rect| {
                (rect.x, rect.y, rect.width, rect.height)
            })
    }
}

/// Draw range owned by an executor pass within one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PassDrawScope {
    /// Main pass draws every command below the overlay layer in admitted Z order.
    MainUnderlay = 2,
    /// Overlay pass draws every command at or above the overlay layer in admitted Z order.
    Overlay = 3,
}

impl PassDrawScope {
    /// Whether a command's Z index belongs to this pass scope.
    pub const fn contains_z(self, z_index: u32) -> bool {
        match self {
            Self::MainUnderlay => z_index < crate::core::paint::LAYER_OVERLAY,
            Self::Overlay => z_index >= crate::core::paint::LAYER_OVERLAY,
        }
    }
}

/// Per-target execution data consumed by RenderGraph passes.
///
/// This uses raw renderer-local scene keys instead of compositor surface handles so
/// RenderGraph stays a command encoder and never owns presentation routing.
#[derive(Debug, Clone, Copy)]
pub struct TargetDrawContext {
    /// Raw SceneKey for the compositor target currently being encoded.
    pub scene_key: u64,
    /// Dynamic offset for target-scoped globals bind groups.
    pub globals_offset: u32,
    /// Dynamic offset for target-scoped path clip bind groups.
    pub path_clips_offset: u32,
}

impl TargetDrawContext {
    /// Create target-scoped execution data.
    pub fn new(scene_key: u64, globals_offset: u32, path_clips_offset: u32) -> Self {
        debug_assert!(scene_key != 0, "target scene_key must be non-zero");
        Self {
            scene_key,
            globals_offset,
            path_clips_offset,
        }
    }

    /// Whether a path draw belongs to this target.
    pub fn path_matches(self, path: &PathDraw) -> bool {
        path.scene_key.get() == self.scene_key
    }
}

/// Per-frame draw data from the Batcher.
///
/// This provides read-only access to batches and related draw information.
/// Executors use this to know what to draw, but cannot modify it.
#[derive(Clone, Copy)]
pub struct FrameDrawData<'a> {
    /// Target currently being encoded.
    pub target: TargetDrawContext,
    /// Reference to the frame batches from Batcher.
    pub frame_batches: &'a FrameBatches,
    /// Mapping from clip state hash to dynamic offset in clip buffer.
    pub rounded_clip_offsets: &'a HashMap<u64, u32>,
    /// Shadow params dynamic offsets keyed by batch param hash.
    pub shadow_params_offsets: &'a HashMap<u64, u32>,
    /// Gradient params dynamic offsets keyed by batch param hash.
    pub gradient_params_offsets: &'a HashMap<u64, u32>,
    pub(crate) analytic_surface_params_offsets: &'a HashMap<u64, u32>,
    /// ColorPicker SV params dynamic offsets keyed by hue bits.
    pub color_picker_sv_params_offsets: &'a HashMap<u64, u32>,
    /// RadialGauge params dynamic offsets keyed by param hash.
    pub radial_gauge_params_offsets: &'a HashMap<u64, u32>,
    /// Text quads for this target.
    pub text_quads: &'a [TextQuad],
    /// Offset into the frame-global text index buffer, measured in quads.
    pub text_quad_offset: u32,
    /// Offset into the frame-global path draw command list.
    pub path_draw_offset: usize,
    /// SPEC-HAL-359: Cross-pipeline commands for this target in admitted Z order.
    pub(crate) draw_commands: &'a [SurfaceDrawCommand],
}

/// Renderer-owned resources passed to executors.
///
/// These are long-lived resources owned by WgpuRenderer, not per-frame.
/// Executors receive read-only access; they cannot create or destroy.
///
/// # Invariant (INV-RG-015)
///
/// This does NOT include Device, Queue, Surface, or resource pools.
/// Only pre-created GPU objects that executors need for encoding.
#[derive(Clone, Copy)]
pub struct RenderResources<'a> {
    /// Rect rendering pipeline.
    pub rect_pipeline: &'a wgpu::RenderPipeline,
    /// Target-scoped globals bind group shared by non-path pipelines.
    pub rect_globals_bind_group: &'a wgpu::BindGroup,
    /// Rounded clip bind group.
    pub rounded_clip_bind_group: &'a wgpu::BindGroup,
    /// Shadow rendering pipeline.
    pub shadow_pipeline: &'a wgpu::RenderPipeline,
    /// Shadow params bind group.
    pub shadow_params_bind_group: &'a wgpu::BindGroup,
    /// Gradient rendering pipeline.
    pub gradient_pipeline: &'a wgpu::RenderPipeline,
    /// Gradient params bind group.
    pub gradient_params_bind_group: &'a wgpu::BindGroup,
    pub(crate) analytic_surface_pipeline: Option<&'a wgpu::RenderPipeline>,
    pub(crate) analytic_surface_params_bind_group: Option<&'a wgpu::BindGroup>,
    /// ColorPicker SV rendering pipeline.
    pub color_picker_sv_pipeline: &'a wgpu::RenderPipeline,
    /// ColorPicker SV params bind group.
    pub color_picker_sv_params_bind_group: &'a wgpu::BindGroup,
    /// RadialGauge rendering pipeline.
    pub radial_gauge_pipeline: &'a wgpu::RenderPipeline,
    /// RadialGauge params bind group.
    pub radial_gauge_params_bind_group: &'a wgpu::BindGroup,
    /// Optional SDF rect rendering pipeline.
    pub sdf_rect_pipeline: Option<&'a wgpu::RenderPipeline>,
    /// Optional analytic shape rendering pipeline.
    pub shape_pipeline: Option<&'a wgpu::RenderPipeline>,
    /// Image rendering pipeline.
    pub image_pipeline: &'a wgpu::RenderPipeline,
    pub(crate) presence_terminal_pipelines: &'a HashMap<u64, &'a wgpu::RenderPipeline>,
    /// Image texture bind groups keyed by texture id.
    pub image_bind_groups: &'a HashMap<u64, &'a wgpu::BindGroup>,
    /// Image index buffer capacity for execute-phase invariant checks.
    pub image_index_capacity: u32,
    /// Path globals bind group.
    pub path_globals_bind_group: &'a wgpu::BindGroup,
    /// Path clips bind group.
    pub path_clips_bind_group: &'a wgpu::BindGroup,
    /// Path renderer for encoding path draws.
    pub path_renderer: &'a crate::renderer::PathRenderer,
    /// Text pipeline (contains the render pipeline).
    pub text_pipeline: &'a TextPipeline,
    /// Text bind group (atlas + background texture).
    pub text_bind_group: &'a wgpu::BindGroup,
}

/// Renderer-owned GPU buffers passed to executors.
#[derive(Clone, Copy)]
pub struct PipelineBuffers<'a> {
    /// Rect vertex buffer.
    pub rect_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// Rect index buffer.
    pub rect_index_buffer: Option<&'a wgpu::Buffer>,
    /// Shadow vertex buffer.
    pub shadow_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// Shadow index buffer.
    pub shadow_index_buffer: Option<&'a wgpu::Buffer>,
    /// Gradient vertex buffer.
    pub gradient_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// Gradient index buffer.
    pub gradient_index_buffer: Option<&'a wgpu::Buffer>,
    pub(crate) analytic_surface_vertex_buffer: Option<&'a wgpu::Buffer>,
    pub(crate) analytic_surface_index_buffer: Option<&'a wgpu::Buffer>,
    /// ColorPicker SV vertex buffer.
    pub color_picker_sv_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// ColorPicker SV index buffer.
    pub color_picker_sv_index_buffer: Option<&'a wgpu::Buffer>,
    /// RadialGauge vertex buffer.
    pub radial_gauge_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// RadialGauge index buffer.
    pub radial_gauge_index_buffer: Option<&'a wgpu::Buffer>,
    /// SDF rect vertex buffer.
    pub sdf_rect_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// SDF rect index buffer.
    pub sdf_rect_index_buffer: Option<&'a wgpu::Buffer>,
    /// Shape vertex buffer.
    pub shape_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// Shape index buffer.
    pub shape_index_buffer: Option<&'a wgpu::Buffer>,
    /// Image vertex buffer.
    pub image_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// Image index buffer.
    pub image_index_buffer: Option<&'a wgpu::Buffer>,
    /// Text vertex buffer.
    pub text_vertex_buffer: Option<&'a wgpu::Buffer>,
    /// Text index buffer.
    pub text_index_buffer: Option<&'a wgpu::Buffer>,
}

/// Read-only view of bound physical resources.
///
/// Pass executors can look up bound textures/buffers by handle.
/// They cannot create, modify, or retain these resources.
pub struct BoundResources {
    pub(crate) frosted: Option<crate::renderer::frosted_glass::backing::FrostedPassResources>,
    /// Bound texture views by attachment handle.
    pub(crate) attachments: HashMap<AttachmentHandle, wgpu::TextureView>,
    /// Bound buffers by buffer handle.
    pub(crate) buffers: HashMap<BufferHandle, wgpu::Buffer>,
}

impl BoundResources {
    /// Create empty bound resources.
    pub fn new() -> Self {
        Self {
            frosted:None,
            attachments: HashMap::new(),
            buffers: HashMap::new(),
        }
    }

    /// Get a bound texture view by handle.
    pub fn get_attachment(&self, handle: AttachmentHandle) -> Option<&wgpu::TextureView> {
        self.attachments.get(&handle)
    }

    /// Get a bound buffer by handle.
    pub fn get_buffer(&self, handle: BufferHandle) -> Option<&wgpu::Buffer> {
        self.buffers.get(&handle)
    }
}

impl Default for BoundResources {
    fn default() -> Self {
        Self::new()
    }
}

/// Constrained context provided to pass executors.
///
/// # Invariant (INV-RG-015)
///
/// This context intentionally does NOT provide:
/// - `&wgpu::Device` — resource creation belongs in Compile
/// - `&wgpu::Queue` — submission belongs in platform layer
/// - `&wgpu::Surface` — presentation belongs in platform layer
/// - Resource pools — allocation belongs in Compile
/// - SceneCache — consumed upstream by Batcher
/// - Graph structure — executors are sandboxed
pub struct PassContext<'a> {
    /// Graph pass name currently being executed.
    pub pass_name: &'static str,
    /// Command encoder for GPU command recording.
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// Read-only view of bound physical resources (textures, buffers by handle).
    pub resources: &'a BoundResources,
    /// Per-frame constants (viewport, scale, clear_color, etc.).
    pub constants: FrameConstants,
    /// Per-frame draw data from Batcher.
    pub draw_data: FrameDrawData<'a>,
    /// Renderer-owned resources (pipelines, bind groups).
    pub render_resources: RenderResources<'a>,
    /// Renderer-owned GPU buffers.
    pub buffers: PipelineBuffers<'a>,
}

/// The output of RenderGraph::compile().
///
/// Contains the topology plus bound physical resources.
/// Ready for Execute phase.
pub struct CompiledGraph<'a> {
    /// The graph topology (passes, order, descriptors).
    pub(crate) topology: &'a RenderGraphTopology,
    /// Bound physical resources from pools.
    pub(crate) bound_resources: BoundResources,
    /// Per-frame constants.
    pub(crate) constants: FrameConstants,
}

impl CompiledGraph<'_> {
    /// Get the topology.
    pub fn topology(&self) -> &RenderGraphTopology {
        self.topology
    }

    /// Get the bound resources.
    pub fn resources(&self) -> &BoundResources {
        &self.bound_resources
    }

    /// Get the frame constants.
    pub fn constants(&self) -> FrameConstants {
        self.constants
    }
}
