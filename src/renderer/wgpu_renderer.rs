// SPEC-HAL-081A: Renderer (frozen contract).

use crate::compositor::SurfaceId;
use crate::core::glyph_quad::{GlyphToQuadParams, build_quad_from_glyph};
use crate::core::paint::{LuminousContourRegion, visit_luminous_contour_regions};
use crate::core::scene::{Color, TextQuad};
use crate::platform::backend_text_engine::BackendTextEngine;
use wgpu::util::DeviceExt;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::platform::native_font_backend_coretext::CoreTextBackend;

#[cfg(target_os = "windows")]
use crate::platform::native_font_backend_directwrite::DirectWriteBackend;

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios")),
    not(target_arch = "wasm32")
))]
use crate::platform::native_font_backend_freetype::FreeTypeBackend;

use crate::core::math::{RectF, snap_to_physical};
#[cfg(target_arch = "wasm32")]
use crate::platform::native_font_backend_glyphon::GlyphonBackend;
use crate::renderer::FrameEpoch;
use crate::renderer::batcher::{FrameBatches, PipelineId, PrimitiveData};
use crate::renderer::compiled_draw_plan::{MultiTargetRenderOutput, SurfaceDrawCommand};
use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
use crate::renderer::draw_plan::{GradientVertex, RectVertex, ShadowVertex};
use crate::renderer::glyph_atlas::GlyphAtlas;
use crate::renderer::gpu_arena::{GpuResourceArena, GpuResourceId};
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::path_renderer::PathRenderer;
use crate::renderer::pipelines::path_pipeline::PathVertex;
use crate::renderer::pipelines::shape_pipeline::{ShapePipeline, ShapeType, ShapeVertex};
use crate::renderer::pipelines::text_pipeline::{TextPipeline, create_text_pipeline};
#[cfg(test)]
use crate::renderer::pixel_coverage::PhysicalRect;
use crate::renderer::pixel_coverage::{PhysicalExtent, PixelCoverageError};
use crate::renderer::sdf_frame_custody::{
    SdfCompiledFrame, SdfFrameCustodyError, SdfFramePlan, SdfRenderOutput, SdfSceneCustody,
};
use crate::renderer::sdf_image_generation_custody::{
    SdfImageCpuUse, SdfImageCustodyError, SdfImageSamplingCompletion,
};
use crate::renderer::staging_pool::StagingBufferPool;
use crate::renderer::text_realization::{
    SurfaceTextDemand, SurfaceTextRealization, TextRealizationFailure, TextRealizationFailureReason,
};
use crate::renderer::text_vertex::TextVertex;
use crate::renderer::vertex::{ImageVertex, SdfRectVertex};
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "loading_presence_output.rs"]
mod loading_presence_output;
pub(crate) mod spatial_hud;
pub(crate) use loading_presence_output::{
    PresenceFrameUse, PresenceOutput, PresenceTerminalPipeline,
};

/// Failures of the bounded diagnostic Image sample, never a production source
/// admission or permission to replace a missing image with a pixel.
#[cfg(any(test, feature = "test-harness"))]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TestHarnessImageProofError {
    #[error("image {0} is not registered")]
    MissingImage(u64),
    #[error("image {0} has a pending registration or removal; drain registrations first")]
    RegistrationPending(u64),
    #[error("image registration owner lock is poisoned")]
    RegistrationLock,
    #[error("diagnostic input contains a nonfinite component")]
    NonfiniteInput,
    #[error("unsupported diagnostic RGBA8 output format {0:?}")]
    UnsupportedFormat(wgpu::TextureFormat),
    #[error("the existing Device is unavailable")]
    DeviceUnavailable,
    #[error("diagnostic map did not complete within five seconds")]
    MapTimeout,
    #[error("diagnostic copy/map: {0}")]
    Readback(String),
}

/// Shared only by the two bounded pixel diagnostics. The largest allocation is
/// the existing4x4 Image probe's1024-byte padded readback; no arbitrary buffer
/// inspection or unbounded blocking Device poll is admitted here.
#[cfg(any(test, feature = "test-harness"))]
pub(crate) fn mapped_test_pixel_bytes(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
) -> Result<Vec<u8>, TestHarnessImageProofError> {
    if buffer.size() == 0 || buffer.size() > 1024 {
        return Err(TestHarnessImageProofError::Readback(
            "pixel readback buffer exceeds the fixed diagnostic bound".into(),
        ));
    }
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = send.send(result);
    });
    let started = web_time::Instant::now();
    let mapped = loop {
        if let Err(error) = device.poll(wgpu::PollType::Poll) {
            break Err(TestHarnessImageProofError::Readback(error.to_string()));
        }
        match receive.try_recv() {
            Ok(result) => {
                break result.map_err(|e| TestHarnessImageProofError::Readback(e.to_string()));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                break Err(TestHarnessImageProofError::Readback(
                    "map callback disconnected".into(),
                ));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if started.elapsed() >= std::time::Duration::from_secs(5) {
            break Err(TestHarnessImageProofError::MapTimeout);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    if let Err(error) = mapped {
        buffer.unmap();
        return Err(error);
    }
    let result = slice
        .get_mapped_range()
        .map(|bytes| bytes.to_vec())
        .map_err(|error| TestHarnessImageProofError::Readback(error.to_string()));
    buffer.unmap();
    result
}

#[allow(dead_code)] // Explicit resource capability; canonical App invocation is not wired.
mod sdf_image_reservation;
pub(super) use sdf_image_reservation::{SdfReservationError, SdfTextureReservation};
mod sdf_material_staging;
pub(crate) use sdf_material_staging::{SdfMaterialFrameInput, SdfMaterialStaging};

/// SPEC-HAL-359: Build one immutable execution schedule from the three
/// individually Z-ordered pipeline streams.
pub(crate) fn build_surface_draw_commands(
    frame_batches: &FrameBatches,
    text_commands: Vec<SurfaceDrawCommand>,
) -> Vec<SurfaceDrawCommand> {
    let batch_commands = frame_batches
        .batches
        .iter()
        .enumerate()
        .map(|(batch_index, batch)| {
            if batch.key.pipeline==PipelineId::FrostedGlass {
                SurfaceDrawCommand::FrostedGlass {order:batch.key.effective_order(),
                    z_index:batch.key.z_layer,ordinal:batch.key.texture_id.expect("HAL385 marker ordinal")}
            } else {SurfaceDrawCommand::Batch {
                order: batch.key.effective_order(),
                z_index: batch.key.z_layer,
                batch_index,
            }}
        });
    let path_commands = frame_batches
        .paths
        .iter()
        .filter(|path| !path.mesh.vertices.is_empty() && !path.mesh.indices.is_empty())
        .enumerate()
        .map(|(path_index, path)| SurfaceDrawCommand::Path {
            order: path.effective_order(),
            z_index: path.z_index,
            path_index,
        });

    merge_ordered_surface_draw_commands(batch_commands, path_commands, text_commands.into_iter())
}

fn text_background_quad(
    rect: RectF,
    color: crate::core::scene::Color,
    clip_rect: Option<RectF>,
) -> TextQuad {
    TextQuad {
        pos: rect,
        uv: RectF::new(0.0, 0.0, 1.0, 1.0),
        color,
        clip_rect,
        downsample_u: -1.0,
    }
}

/// Merge three nondecreasing effective-order command streams in O(k) total work.
///
/// The fixed tie rank keeps equal-Z commands deterministic while advancing
/// each input only from the front, preserving source order within each family.
pub(crate) fn merge_ordered_surface_draw_commands<B, P, T>(
    batch_commands: B,
    path_commands: P,
    text_commands: T,
) -> Vec<SurfaceDrawCommand>
where
    B: Iterator<Item = SurfaceDrawCommand>,
    P: Iterator<Item = SurfaceDrawCommand>,
    T: Iterator<Item = SurfaceDrawCommand>,
{
    let capacity = [
        batch_commands.size_hint(),
        path_commands.size_hint(),
        text_commands.size_hint(),
    ]
    .into_iter()
    .map(|(lower, upper)| upper.unwrap_or(lower))
    .fold(0usize, usize::saturating_add);
    let mut batch_commands = batch_commands.peekable();
    let mut path_commands = path_commands.peekable();
    let mut text_commands = text_commands.peekable();
    let mut merged = Vec::with_capacity(capacity);

    loop {
        let next_stream = [
            batch_commands
                .peek()
                .map(|command| (command.order(), command.tie_rank(), 0u8)),
            path_commands
                .peek()
                .map(|command| (command.order(), command.tie_rank(), 1u8)),
            text_commands
                .peek()
                .map(|command| (command.order(), command.tie_rank(), 2u8)),
        ]
        .into_iter()
        .flatten()
        .min_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(_, _, stream)| stream);

        let command = match next_stream {
            Some(0) => batch_commands.next(),
            Some(1) => path_commands.next(),
            Some(2) => text_commands.next(),
            Some(_) => unreachable!("surface draw command stream index is fixed"),
            None => break,
        };
        let command = command.expect("selected surface draw command stream must be non-empty");
        debug_assert!(
            merged
                .last()
                .is_none_or(|previous: &SurfaceDrawCommand| previous.order() <= command.order()),
            "SPEC-HAL-359: compiled pipeline streams must share nondecreasing effective order"
        );
        merged.push(command);
    }

    merged
}

// ============================================================================
// SPEC-R-020 Stage 14: Dynamic Uniform Buffer Constants for Per-Batch Clips
// ============================================================================

/// Maximum number of unique rounded clips per frame.
/// Each clip takes one aligned slot in the uniform buffer.
const MAX_ROUNDED_CLIPS_PER_FRAME: usize = 64;

/// Alignment for uniform buffer dynamic offsets.
/// WGPU's default `min_uniform_buffer_offset_alignment` is 256 bytes.
/// RoundedClipUniform is 64 bytes, so each slot is padded to 256 bytes.
const ROUNDED_CLIP_UNIFORM_ALIGNMENT: u64 = 256;

/// SPEC-HAL-081B: Alignment for globals uniform buffer dynamic offsets.
/// WGPU's default `min_uniform_buffer_offset_alignment` is 256 bytes.
/// Globals is 80 bytes, so each slot is padded to 256 bytes.
const GLOBALS_UNIFORM_ALIGNMENT: u64 = 256;

/// SPEC-HAL-081B: Alignment for path clips storage buffer dynamic offsets.
/// WGPU's default `min_storage_buffer_offset_alignment` is 256 bytes.
/// Each target's clips are padded to this alignment for dynamic offset binding.
const PATH_CLIPS_SLAB_ALIGNMENT: u64 = 256;

/// SPEC-HAL-030A: Canonical window-surface queue-depth hint.
///
/// Controls how many unpresented frames wgpu queues before blocking the next
/// `get_current_texture()` call. Lower values reduce input latency and prevent
/// frame-pacing spikes on high-refresh displays at the cost of reduced
/// CPU/GPU overlap.
///
/// Value of 2 keeps one frame queued so the CPU and GPU can overlap without
/// growing an unbounded presentation backlog.
pub const SWAPCHAIN_MAX_FRAME_LATENCY: u32 = 2;

/// WebGPU canvases expose Unorm storage; linear output needs its sRGB view.
#[cfg(any(target_arch = "wasm32", test))]
fn web_color_attachment_format(format: wgpu::TextureFormat) -> wgpu::TextureFormat {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => format,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn web_canvas_formats(
    attachment_format: wgpu::TextureFormat,
) -> (wgpu::TextureFormat, Vec<wgpu::TextureFormat>) {
    match attachment_format {
        wgpu::TextureFormat::Rgba8UnormSrgb => {
            (wgpu::TextureFormat::Rgba8Unorm, vec![attachment_format])
        }
        wgpu::TextureFormat::Bgra8UnormSrgb => {
            (wgpu::TextureFormat::Bgra8Unorm, vec![attachment_format])
        }
        _ => (attachment_format, vec![]),
    }
}

/// SPEC-HAL-030A: Build the complete renderer-owned window-surface
/// configuration used by both registration and resize.
fn window_surface_configuration(
    attachment_format: wgpu::TextureFormat,
    physical_size: (u32, u32),
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
) -> wgpu::SurfaceConfiguration {
    #[cfg(target_arch = "wasm32")]
    let (format, view_formats) = web_canvas_formats(attachment_format);
    #[cfg(not(target_arch = "wasm32"))]
    let (format, view_formats) = (attachment_format, vec![]);
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        color_space: wgpu::SurfaceColorSpace::Auto,
        width: physical_size.0,
        height: physical_size.1,
        present_mode,
        alpha_mode,
        view_formats,
        desired_maximum_frame_latency: SWAPCHAIN_MAX_FRAME_LATENCY,
    }
}

/// SPEC-R-013-v2: Shadow shader source.
const SHADOW_SHADER_SRC: &str = include_str!("shaders/shadow.wgsl");

/// SPEC-R-012-v2: Gradient shader source.
const GRADIENT_SHADER_SRC: &str = include_str!("shaders/gradient.wgsl");
/// SPEC-HAL-272: ColorPicker SV shader source.
const COLOR_PICKER_SV_SHADER_SRC: &str = include_str!("shaders/color_picker_sv.wgsl");
/// SPEC-HAL-278: RadialGauge shader source.
const RADIAL_GAUGE_SHADER_SRC: &str = include_str!("shaders/radial_gauge.wgsl");
/// SPEC-R-022: SDF RoundedRect shader source.
const SDF_RECT_SHADER_SRC: &str = include_str!("shaders/sdf_rect.wgsl");

/// SPEC-R-013-v2: Maximum number of unique shadow param sets per frame.
/// Each takes one aligned slot (256 bytes) in the shadow params buffer.
const MAX_SHADOW_PARAMS_PER_FRAME: usize = 64;

/// SPEC-HAL-081B: Per-target slot size for path clips slab buffer.
/// 1024 bytes = 64 ClipRectUniforms (63 actual clips + 1 fullscreen).
/// Must be a multiple of PATH_CLIPS_SLAB_ALIGNMENT.
const PATH_CLIPS_SLOT_SIZE: u64 = 1024;

/// SPEC-HAL-081B: Initial number of per-target slots in path clips slab.
const INITIAL_PATH_CLIPS_SLAB_CAPACITY: u32 = 4;

// RectVertex moved to crate::renderer::draw_plan (SPEC-HAL-084)

// ============================================================================
// SPEC-R-004: Path Pipeline Uniform Types
// ============================================================================

/// Uniform buffer for path pipeline globals.
/// Must match WGSL: struct Globals { viewport_size: vec2<f32>, scale_factor: f32, _padding: f32 }
/// SPEC-R-004 Stage 14: Added scale_factor for transform pipeline.
/// R-010C: Text globals uniform for per-channel LCD subpixel rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextGlobalsUniform {
    gamma_r: f32,
    gamma_g: f32,
    gamma_b: f32,
    _pad: f32,
}

// SPEC-HAL-085: PathGlobalsUniform is now replaced by shared Globals struct
// Path pipeline uses the same Globals as rect pipeline

/// Clip rectangle for path pipeline.
/// Must match WGSL: struct ClipRect { min: vec2<f32>, max: vec2<f32> }
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ClipRectUniform {
    min: [f32; 2],
    max: [f32; 2],
}

// ============================================================================
// SPEC-HAL-085: Global Uniform Types (Shared Across All Pipelines)
// ============================================================================

/// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms
/// Must match WGSL: struct Globals
///
/// This is shared by ALL pipelines at @group(0) @binding(0).
/// Total size: 80 bytes (mat4x4=64 + vec2=8 + f32=4 + u32=4)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Globals {
    /// Absolute Logical → NDC transform matrix
    view_proj: [[f32; 4]; 4],
    /// Physical surface size in pixels
    surface_size: [f32; 2],
    /// Frame time in seconds (SPEC-HAL-041)
    time_seconds: f32,
    /// Global debug flags:
    /// - bit 0: rounded_clip_enabled
    flags: u32,
}

impl Globals {
    /// Create globals with orthographic projection for 2D UI.
    ///
    /// The view_proj matrix converts from logical coordinates (origin top-left, y-down)
    /// to NDC (origin center, y-up, range -1..1).
    fn new(
        logical_width: f32,
        logical_height: f32,
        physical_width: f32,
        physical_height: f32,
        rounded_clip_enabled: bool,
    ) -> Self {
        // Orthographic projection: logical coords → NDC
        // NDC x = (logical_x / half_width) - 1.0
        // NDC y = 1.0 - (logical_y / half_height)
        let view_proj = [
            [2.0 / logical_width, 0.0, 0.0, 0.0],
            [0.0, -2.0 / logical_height, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ];

        let flags = if rounded_clip_enabled { 1u32 } else { 0u32 };

        Self {
            view_proj,
            surface_size: [physical_width, physical_height],
            time_seconds: 0.0,
            flags,
        }
    }
}

/// Rounded clip parameters for rect pipeline.
/// Must match WGSL: struct RoundedClip
/// SPEC-HAL-085: Pipeline-specific resources at @group(1) @binding(0)
/// SPEC-R-020 Stage 9: Added precomputed corner zone bounds for GPU optimization
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct RoundedClipUniform {
    /// Clip rectangle in logical coordinates: [x, y, width, height]
    rect_logical: [f32; 4],
    /// Corner radii in logical coordinates: [tl, tr, bl, br]
    radii_logical: [f32; 4],
    /// SPEC-R-020 Stage 9: Precomputed inner safe zone start (local coords)
    /// inner_min.x = max(tl, bl), inner_min.y = max(tl, tr)
    inner_min: [f32; 2],
    /// SPEC-R-020 Stage 9: Precomputed inner safe zone end (local coords)
    /// inner_max.x = width - max(tr, br), inner_max.y = height - max(bl, br)
    inner_max: [f32; 2],
    /// SPEC-R-020 Stage 9: Precomputed outer bounds start (local coords, AA-expanded)
    /// outer_min = [-aa, -aa] where aa = 1.0
    outer_min: [f32; 2],
    /// SPEC-R-020 Stage 9: Precomputed outer bounds end (local coords, AA-expanded)
    /// outer_max = [width + aa, height + aa] where aa = 1.0
    outer_max: [f32; 2],
    /// SPEC-HAL-085: AA radius moved from globals to pipeline-specific
    aa_radius_logical: f32,
    /// Alignment padding to reach 80 bytes (WGSL uniform 16-byte alignment)
    _padding: [f32; 3],
}

impl RoundedClipUniform {
    /// SPEC-R-020 Stage 9+12+13: Construct uniform with CPU-precomputed corner zone bounds
    ///
    /// # Arguments
    /// * `rect_logical` - [x, y, width, height] in logical coordinates
    /// * `radii_logical` - [tl, tr, bl, br] corner radii in logical coordinates
    /// * `aa` - AA band width in logical pixels (Stage 12: parameterized)
    ///
    /// Stage 13: Inner bounds are now AA-aware, ensuring the safe zone is fully inside
    /// the 100% coverage region for all corners (accounting for smoothstep AA band).
    pub(crate) fn new(rect_logical: [f32; 4], radii_logical: [f32; 4], aa: f32) -> Self {
        let [_x, _y, width, height] = rect_logical;
        let [tl, tr, bl, br] = radii_logical;
        let aa = aa.max(0.0); // Ensure non-negative

        // Stage 13: Per-corner AA-extended radii for inner safe zone computation
        let tl_ext = tl + aa;
        let tr_ext = tr + aa;
        let bl_ext = bl + aa;
        let br_ext = br + aa;

        // Inner safe zone (AA-aware): guaranteed full coverage, no corner testing needed
        // Points here are at least `aa` away from any curved corner arc
        let inner_min_x = tl_ext.max(bl_ext).min(width);
        let inner_min_y = tl_ext.max(tr_ext).min(height);
        let inner_max_x = (width - tr_ext.max(br_ext)).max(0.0);
        let inner_max_y = (height - bl_ext.max(br_ext)).max(0.0);
        let inner_min = [inner_min_x, inner_min_y];
        let inner_max = [inner_max_x, inner_max_y];

        // Outer bounds (AA-expanded rect for early reject)
        // Points outside this are guaranteed fully outside radius+AA
        let outer_min = [-aa, -aa];
        let outer_max = [width + aa, height + aa];

        Self {
            rect_logical,
            radii_logical,
            inner_min,
            inner_max,
            outer_min,
            outer_max,
            aa_radius_logical: aa,
            _padding: [0.0; 3],
        }
    }
}

/// SPEC-R-013-v2: Shadow pipeline per-batch uniform.
/// Must match WGSL: struct ShadowParams
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ShadowParamsUniform {
    /// Shape center in logical coordinates
    shape_center: [f32; 2],
    /// Shape half-size in logical coordinates
    shape_half_size: [f32; 2],
    /// Corner radius (logical pixels)
    shape_radius: f32,
    /// Gaussian sigma (logical pixels)
    sigma: f32,
    /// Shadow mode: 0=drop, 1=inner, 2=ring
    mode: u32,
    /// Spread (logical pixels)
    spread: f32,
}

/// SPEC-R-012-v2: Gradient stop for GPU uniform.
/// Must match WGSL: struct GradientStop
/// 32 bytes per stop (vec4 color + f32 offset + 3x padding)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GradientStopUniform {
    color: [f32; 4], // linear-space, premultiplied RGBA
    offset: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

/// SPEC-R-012-v2: Gradient pipeline per-batch uniform.
/// Must match WGSL: struct GradientParams
/// 384 bytes, including the existing clip projection, within a 512-byte aligned slot.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GradientParamsUniform {
    rect_origin: [f32; 2],
    rect_size: [f32; 2],
    p0: [f32; 2],
    p1: [f32; 2],
    corner_radius: f32,
    kind: u32,
    stop_count: u32,
    clip_enabled: u32,
    stops: [GradientStopUniform; 8],
    clip: RoundedClipUniform,
}

/// SPEC-HAL-272: Per-instance uniform for the color-picker SV pipeline.
/// Size: 32 bytes (8 × f32). Aligned slot: 256 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorPickerSvParamsUniform {
    rect_origin: [f32; 2],
    rect_size: [f32; 2],
    hue: f32,
    corner_radius: f32,
    _pad: [f32; 2],
}

/// SPEC-HAL-278: Per-instance uniform for the radial gauge pipeline.
/// Size: 240 bytes. Aligned slot: 256 bytes.
/// Zone ranges are vec4 (xy=range, zw=unused) to match WGSL vec4 alignment.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RadialGaugeParamsUniform {
    track_color: [f32; 4], // 16 bytes
    fill_color: [f32; 4],  // 16 bytes
    rect_origin: [f32; 2], // 8 bytes
    rect_size: [f32; 2],   // 8 bytes
    fill: f32,             // 4 bytes
    thickness: f32,        // 4 bytes
    start_angle: f32,      // 4 bytes
    sweep_angle: f32,      // 4 bytes
    origin_fill: f32,      // 4 bytes
    zone_count: u32,       // 4 bytes
    mode: u32,             // 4 bytes
    fill_style: u32,       // 0=solid, 1=zone-colored, 2=value-lerp
    zone0_range: [f32; 4], // xy = start, end; zw = 0 (16 bytes)
    zone0_color: [f32; 4], // 16 bytes
    zone1_range: [f32; 4],
    zone1_color: [f32; 4],
    zone2_range: [f32; 4],
    zone2_color: [f32; 4],
    zone3_range: [f32; 4],
    zone3_color: [f32; 4],
    // Tick marks (32 bytes)
    major_divisions: u32,          // 4 @ 208
    minor_per_major: u32,          // 4 @ 212
    tick_length_major: f32,        // 4 @ 216
    tick_length_minor: f32,        // 4 @ 220
    tick_color: [f32; 4],          // 16 @ 224
    fill_color_negative: [f32; 4], // 16 @ 240 — total: 256
}

/// SPEC-R-012-v2: Maximum number of unique gradient param sets per frame.
const MAX_GRADIENT_PARAMS_PER_FRAME: usize = 512;

/// SPEC-R-012-v2: Alignment for gradient params uniform buffer.
/// GradientParamsUniform is 288 bytes. Next aligned boundary at 512 bytes
/// (must be multiple of min_uniform_buffer_offset_alignment = 256).
const GRADIENT_PARAMS_UNIFORM_ALIGNMENT: u64 = 512;

/// SPEC-HAL-272: Maximum number of unique color-picker SV param sets per frame.
const MAX_COLOR_PICKER_SV_PARAMS_PER_FRAME: usize = 64;

/// SPEC-HAL-272: Aligned slot size for color-picker SV params buffer.
/// ColorPickerSvParamsUniform is 32 bytes. Aligned to 256 bytes.
const COLOR_PICKER_SV_PARAMS_UNIFORM_ALIGNMENT: u64 = 256;

/// SPEC-HAL-278: Maximum number of unique radial gauge param sets per frame.
const MAX_RADIAL_GAUGE_PARAMS_PER_FRAME: usize = 64;

/// SPEC-HAL-278: Aligned slot size for radial gauge params buffer.
/// RadialGaugeParamsUniform is 64 bytes (vec4s first for WGSL alignment). Aligned to 256 bytes.
const RADIAL_GAUGE_PARAMS_UNIFORM_ALIGNMENT: u64 = 256;

/// SPEC-HAL-272: Vertex type for the color-picker SV pipeline (position only).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorPickerSvVertex {
    position: [f32; 2],
}

impl ColorPickerSvVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ColorPickerSvVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

/// SPEC-HAL-278: Vertex type for the radial gauge pipeline (position only).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RadialGaugeVertex {
    position: [f32; 2],
}

impl RadialGaugeVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RadialGaugeVertex>() as wgpu::BufferAddress,
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
// SPEC-HAL-029: Text Vertex imported from pipelines::text_pipeline
// ============================================================================

/// SPEC-HAL-039: GPU buffer update strategy for a frame.
enum GpuUpdate {
    /// Allocate (or reallocate) buffers and upload entire contents.
    FullRebuild {
        vertices: Vec<RectVertex>,
        indices: Vec<u16>,
    },
    /// Reuse existing buffers and overwrite a contiguous range.
    ///
    /// v1 uses offset = 0 and covers the whole range, but the shape allows
    /// per-batch deltas later.
    OverwriteRange {
        vertex_offset: u32,
        vertices: Vec<RectVertex>,
        index_offset: u32,
        indices: Vec<u16>,
    },
}

/// WGPU-based renderer
///
/// # Multi-Surface Architecture (SPEC-HAL-081B)
///
/// The renderer operates in multi-surface mode only. All rendering goes through
/// registered render targets via `compile_multi()` / `render_multi()`.
///
/// To render to a window:
/// 1. Create surface: `renderer.create_surface_for_window(window)`
/// 2. Register target: `renderer.register_render_target(surface_id, surface, size, scale)`
/// 3. Build draw plan: `draw_plan_builder.build_multi(&scene, compositor.snapshot_multi())`
/// 4. Compile: `renderer.compile_multi(&draw_plan)`
/// 5. Render: `renderer.render_multi(compiled)`

/// SPEC-HAL-263: Per-texture GPU resource entry.
///
/// Owns all GPU handles for an image texture: Texture + TextureView + BindGroup.
/// Single `image_textures: HashMap<u64, ImageTextureEntry>` is the source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageSampling {
    Linear,
    Nearest,
}

enum ImageBatchRefusal {
    PresenceChanged,
    Unavailable,
}

struct ImageTextureEntry {
    /// Owned texture handle. Some when renderer owns the texture
    /// (register_image_texture / register_image_rgba8_srgb).
    /// None when texture is externally owned (register_image_from_view).
    #[allow(dead_code)]
    _texture: Option<wgpu::Texture>,
    view: wgpu::TextureView,
    sampling: ImageSampling,
    bind_group: wgpu::BindGroup,
    generation: std::num::NonZeroU64,
    admission: Option<Arc<crate::renderer::image_binding::ImageBindingState>>,
}

impl ImageTextureEntry {
    fn revoke(&self, reason: crate::renderer::image_binding::ImageBindingFailure) {
        if let Some(state) = &self.admission {
            state.revoke(reason);
        }
    }
}

impl Drop for ImageTextureEntry {
    fn drop(&mut self) {
        self.revoke(crate::renderer::image_binding::ImageBindingFailure::RendererDestroyed);
    }
}

/// SPEC-HAL-295: GPU resource handles for widgets that perform custom rendering.
///
/// Contains Arc-cloned handles to the shared Device and Queue, plus
/// the surface format. Multiple widgets can hold their own GpuResources
/// simultaneously — the Arc clones are cheap and non-exclusive.
///
/// Obtained via `WgpuRenderer::gpu_resources()`.
/// SPEC-HAL-295: Pending texture registration for deferred processing.
///
/// Widgets that create offscreen textures (e.g., Plot) push entries
/// here during layout. The frame loop drains them before compile,
/// calling register_image_from_view or register_image_from_view_nearest.
/// The view must yield premultiplied-linear RGBA when sampled. `nearest`
/// selects filtering only and does not change the color or alpha contract.
pub struct PendingImageRegistration {
    pub texture_id: u64,
    pub view: wgpu::TextureView,
    pub nearest: bool,
}

/// SPEC-HAL-295: Pending texture unregistration for deferred processing.
pub struct PendingImageUnregistration {
    pub texture_id: u64,
    /// Widget-owned textures kept alive until the renderer drains this removal.
    pub retained_textures: Vec<wgpu::Texture>,
}

// Private R-027 custody. The public pending entry stays source-compatible. All
// association/state changes below occur while pending_unregistrations is locked.
pub(crate) struct HdrFinalCustody {
    texture_id: u64,
    _charge: Box<dyn Send + Sync>,
}

struct HdrFinalRecord {
    custody: Arc<HdrFinalCustody>,
    pending: bool,
    textures: [wgpu::Texture; 2],
}

impl HdrFinalRecord {
    fn matches_textures(&self, textures: &[wgpu::Texture]) -> bool {
        textures.len() == 2
            && ((textures[0] == self.textures[0] && textures[1] == self.textures[1])
                || (textures[0] == self.textures[1] && textures[1] == self.textures[0]))
    }
}

#[derive(Default)]
struct HdrFinalCustodies {
    entries: std::collections::HashMap<u64, HdrFinalRecord>,
    closed: bool,
}

struct PendingImageRgba8Registration {
    admission: crate::renderer::image_binding::PendingImageAdmission,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Default)]
struct PendingImageRgba8Registrations {
    pending: std::sync::Mutex<PendingImageRgba8Entries>,
    has_items: std::sync::atomic::AtomicBool,
    // This existing shared Image-ingress owner hosts the lazy private custody;
    // no HDR map/allocation exists for ordinary RGBA8/legacy callers.
    hdr_finals: std::sync::OnceLock<std::sync::Mutex<HdrFinalCustodies>>,
    presence: std::sync::OnceLock<std::sync::Mutex<loading_presence_output::PresenceIngress>>,
    presence_layout: std::sync::OnceLock<loading_presence_output::PresenceImageLayout>,
}

#[derive(Default)]
struct PendingImageRgba8Entries {
    items: Option<Vec<PendingImageRgba8Registration>>,
    closed: bool,
}

/// Synchronous decoded-pixel ingress failures under SPEC-HAL-263A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageRgba8RegistrationError {
    RendererDestroyed,
    ZeroDimensions { width: u32, height: u32 },
    ByteCountOverflow { width: u32, height: u32 },
    DataLengthMismatch { expected: usize, actual: usize },
    DimensionLimitExceeded { width: u32, height: u32, limit: u32 },
}

impl std::fmt::Display for ImageRgba8RegistrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Image RGBA8 registration rejected: {self:?}")
    }
}

impl std::error::Error for ImageRgba8RegistrationError {}

fn image_rgba8_byte_count(width: u32, height: u32) -> Option<usize> {
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)
}

#[derive(Clone)]
pub struct GpuResources {
    pub device: std::sync::Arc<wgpu::Device>,
    pub queue: std::sync::Arc<wgpu::Queue>,
    pub format: wgpu::TextureFormat,
    pub(crate) adapter: wgpu::Adapter,
    /// Deferred low-level texture registrations. Admitted producers queue these
    /// through their declared lifecycle, and the frame loop drains before
    /// compile without converting content. Every queued view must already yield
    /// premultiplied-linear RGBA when sampled.
    pub pending_registrations: std::sync::Arc<std::sync::Mutex<Vec<PendingImageRegistration>>>,
    /// Deferred texture unregistrations. Drained before registrations.
    pub pending_unregistrations: std::sync::Arc<std::sync::Mutex<Vec<PendingImageUnregistration>>>,
    pending_rgba8_registrations: Arc<PendingImageRgba8Registrations>,
    pub image_admission: crate::renderer::image_binding::ImageAdmissionMailbox,
    /// Legacy primary-window scale metadata retained for source compatibility.
    /// It is not target authority and must not size conforming custom resources;
    /// SPEC-HAL-266A preparation supplies renderer-owned per-surface metrics.
    pub scale_factor: f32,
}

impl GpuResources {
    pub(crate) fn private_hdr_owner_live(&self) -> bool {
        let _removals = self
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        !self
            .pending_rgba8_registrations
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed
    }

    pub(crate) fn reserve_hdr_final_custody(
        &self,
        texture_id: u64,
        charge: Box<dyn Send + Sync>,
        textures: [wgpu::Texture; 2],
    ) -> Result<Arc<HdrFinalCustody>, &'static str> {
        let _removals = self
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if self
            .pending_rgba8_registrations
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed
        {
            return Err("renderer retired");
        }
        let mut state = self
            .pending_rgba8_registrations
            .hdr_finals
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return Err("renderer retired");
        }
        if state.entries.contains_key(&texture_id) {
            return Err("duplicate HDR target identity");
        }
        state
            .entries
            .try_reserve(1)
            .map_err(|_| "HDR retirement custody capacity")?;
        let custody = Arc::new(HdrFinalCustody {
            texture_id,
            _charge: charge,
        });
        state.entries.insert(
            texture_id,
            HdrFinalRecord {
                custody: custody.clone(),
                pending: false,
                textures,
            },
        );
        Ok(custody)
    }

    pub(crate) fn hdr_final_is_live(&self, custody: &Arc<HdrFinalCustody>) -> bool {
        let _removals = self
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.pending_rgba8_registrations
            .hdr_finals
            .get()
            .is_some_and(|state| {
                let state = state.lock().unwrap_or_else(|e| e.into_inner());
                !state.closed
                    && state
                        .entries
                        .get(&custody.texture_id)
                        .is_some_and(|entry| !entry.pending && Arc::ptr_eq(&entry.custody, custody))
            })
    }

    // The caller holds the SAME removal-queue guard used by the renderer drain.
    pub(crate) fn queue_hdr_final_removal(
        &self,
        removals: &mut Vec<PendingImageUnregistration>,
        custody: &Arc<HdrFinalCustody>,
        textures: Vec<wgpu::Texture>,
    ) {
        let mut state = self
            .pending_rgba8_registrations
            .hdr_finals
            .get()
            .expect("HDR custody was reserved")
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.closed {
            // The renderer already removed its registry and retained its custody
            // through queue completion. A still-live pass retains its own charge.
            let custody = custody.clone();
            crate::renderer::on_submitted_work_done(&self.queue, move || drop((textures, custody)));
            return;
        }
        let entry = state
            .entries
            .get_mut(&custody.texture_id)
            .expect("qualified HDR target");
        assert!(
            Arc::ptr_eq(&entry.custody, custody) && !entry.pending,
            "one HDR retirement per target"
        );
        assert!(
            entry.matches_textures(&textures),
            "exact HDR final pair accompanies its retirement"
        );
        assert!(
            !removals
                .iter()
                .any(|entry| entry.texture_id == custody.texture_id),
            "unmatched HDR removal"
        );
        removals.push(PendingImageUnregistration {
            texture_id: custody.texture_id,
            retained_textures: textures,
        });
        entry.pending = true;
    }

    /// Queues owned decoded straight-sRGB RGBA8 pixels under SPEC-HAL-263A.
    /// Validation failures queue neither pixels nor admission work. The renderer
    /// drains removals, low-level views, then these pixels in FIFO order, using
    /// `register_image_rgba8_srgb` for the sole normalization/upload implementation.
    /// Each request names its own registration generation, even if a later
    /// same-ID entry replaces it during that drain. Renderer retirement releases
    /// queued pixels and rejects further calls before accepting work.
    pub fn queue_image_rgba8_srgb(
        &self,
        texture_id: u64,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<crate::renderer::image_binding::ImageBindingRequest, ImageRgba8RegistrationError>
    {
        let mut pending = self
            .pending_rgba8_registrations
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if pending.closed {
            return Err(ImageRgba8RegistrationError::RendererDestroyed);
        }
        if width == 0 || height == 0 {
            return Err(ImageRgba8RegistrationError::ZeroDimensions { width, height });
        }
        let expected = image_rgba8_byte_count(width, height)
            .ok_or(ImageRgba8RegistrationError::ByteCountOverflow { width, height })?;
        if rgba.len() != expected {
            return Err(ImageRgba8RegistrationError::DataLengthMismatch {
                expected,
                actual: rgba.len(),
            });
        }
        let limit = self.device.limits().max_texture_dimension_2d;
        if width > limit || height > limit {
            return Err(ImageRgba8RegistrationError::DimensionLimitExceeded {
                width,
                height,
                limit,
            });
        }

        let (request, admission) =
            crate::renderer::image_binding::ImageBindingRequest::pending(texture_id);
        let items = pending.items.get_or_insert_with(Vec::new);
        let newly_pending = items.is_empty();
        items.push(PendingImageRgba8Registration {
            admission,
            width,
            height,
            rgba,
        });
        self.pending_rgba8_registrations
            .has_items
            .store(true, std::sync::atomic::Ordering::Release);
        drop(pending);
        if newly_pending {
            self.image_admission.notify();
        }
        Ok(request)
    }

    #[cfg(test)]
    pub(crate) fn image_rgba8_ingress_state_for_test(&self) -> (bool, Option<usize>) {
        let pending = self
            .pending_rgba8_registrations
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (
            pending.closed,
            pending
                .items
                .as_ref()
                .map(|entries| entries.iter().map(|entry| entry.rgba.len()).sum()),
        )
    }

    /// Requests CPU evidence from the renderer after its existing queue drain.
    pub fn request_image_binding(
        &self,
        texture_id: u64,
    ) -> crate::renderer::image_binding::ImageBindingRequest {
        self.image_admission.request(texture_id)
    }

    pub(crate) fn observe_device_lifetime(&self) -> DeviceLifetimeObservation {
        DeviceLifetimeObservation::for_device(&self.device)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuDeviceFailureReason {
    Unknown,
    Destroyed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuDeviceFailure {
    reason: GpuDeviceFailureReason,
    message: Arc<str>,
}

impl GpuDeviceFailure {
    pub const fn reason(&self) -> GpuDeviceFailureReason {
        self.reason
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for GpuDeviceFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "GPU device became terminal ({:?}): {}",
            self.reason, self.message
        )
    }
}

impl std::error::Error for GpuDeviceFailure {}

#[derive(Default)]
struct DeviceLifetimeState {
    failure: std::sync::Mutex<Option<GpuDeviceFailure>>,
    image_bindings:
        std::sync::Mutex<Vec<std::sync::Weak<crate::renderer::image_binding::ImageBindingState>>>,
}

impl DeviceLifetimeState {
    fn failure(&self) -> Option<GpuDeviceFailure> {
        self.failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn record(&self, reason: wgpu::DeviceLostReason, message: String) {
        let mut failure = self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if failure.is_none() {
            *failure = Some(GpuDeviceFailure {
                reason: match reason {
                    wgpu::DeviceLostReason::Unknown => GpuDeviceFailureReason::Unknown,
                    wgpu::DeviceLostReason::Destroyed => GpuDeviceFailureReason::Destroyed,
                },
                message: Arc::from(message),
            });
        }
        drop(failure);
        let mut bindings = self
            .image_bindings
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        bindings.retain(|binding| {
            if let Some(binding) = binding.upgrade() {
                binding.revoke(crate::renderer::image_binding::ImageBindingFailure::DeviceLost);
                true
            } else {
                false
            }
        });
    }
}

#[derive(Clone)]
pub(crate) struct DeviceLifetimeObservation {
    state: Arc<DeviceLifetimeState>,
}

impl DeviceLifetimeObservation {
    pub(crate) fn observe_image_binding(
        &self,
        binding: &Arc<crate::renderer::image_binding::ImageBindingState>,
    ) {
        let mut bindings = self
            .state
            .image_bindings
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        bindings.retain(|binding| {
            binding
                .upgrade()
                .is_some_and(|binding| !binding.is_revoked())
        });
        bindings.push(Arc::downgrade(binding));
        if self.failure().is_some() {
            binding.revoke(crate::renderer::image_binding::ImageBindingFailure::DeviceLost);
        }
    }

    fn for_device(device: &Arc<wgpu::Device>) -> Self {
        static OBSERVATIONS: std::sync::OnceLock<
            std::sync::Mutex<
                std::collections::HashMap<
                    usize,
                    (
                        std::sync::Weak<wgpu::Device>,
                        std::sync::Weak<DeviceLifetimeState>,
                    ),
                >,
            >,
        > = std::sync::OnceLock::new();

        // GpuResources clones preserve the renderer-owned Arc identity. The
        // Weak device identity prevents a surviving CPU observation from matching
        // a later device allocated at a retired Arc address. Neither weak value
        // retains the GPU device.
        let key = Arc::as_ptr(device) as usize;
        let registry =
            OBSERVATIONS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
        let mut registry = registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.retain(|_, (device, state)| device.strong_count() > 0 && state.strong_count() > 0);
        if let Some(state) = registry.get(&key).and_then(|(_, state)| state.upgrade()) {
            return Self { state };
        }

        let state = Arc::new(DeviceLifetimeState::default());
        let callback_state = state.clone();
        device.set_device_lost_callback(move |reason, message| {
            callback_state.record(reason, message);
        });
        registry.insert(key, (Arc::downgrade(device), Arc::downgrade(&state)));

        #[cfg(test)]
        {
            crate::renderer::custom_render_pass::audit::record_device_lifetime_observation();
            crate::renderer::custom_render_pass::audit::record_device_lost_callback();
        }

        Self { state }
    }

    pub(crate) fn failure(&self) -> Option<GpuDeviceFailure> {
        self.state.failure()
    }
}

#[derive(Debug)]
struct PendingStagingSubmission {
    done: Arc<AtomicBool>,
    handles: Vec<crate::renderer::staging_pool::StagingBufferHandle>,
}

#[derive(Debug)]
struct PendingStagingRemap {
    done: Arc<AtomicBool>,
    succeeded: Arc<AtomicBool>,
    handle: crate::renderer::staging_pool::StagingBufferHandle,
}

fn stage_buffer_upload_parts(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gpu_arena: &mut GpuResourceArena,
    staging_pool: &mut StagingBufferPool,
    frame_epoch: FrameEpoch,
    staging_upload_encoder: &mut Option<wgpu::CommandEncoder>,
    staging_upload_handles: &mut Vec<crate::renderer::staging_pool::StagingBufferHandle>,
    destination: &wgpu::Buffer,
    destination_offset: u64,
    data: &[u8],
) {
    if data.is_empty() {
        return;
    }

    let size = data.len() as u64;
    let copy_alignment = wgpu::COPY_BUFFER_ALIGNMENT;

    if destination_offset % copy_alignment != 0 || size % copy_alignment != 0 {
        queue.write_buffer(destination, destination_offset, data);
        return;
    }

    debug_assert!(
        destination_offset + size <= destination.size(),
        "SPEC-HAL-040: staging upload would overrun destination buffer"
    );

    let map_size = WgpuRenderer::align_buffer_upload_size(size, wgpu::MAP_ALIGNMENT);
    let handle = staging_pool.alloc(device, gpu_arena, frame_epoch, map_size);

    {
        let mut mapped = handle
            .buffer
            .slice(0..map_size)
            .get_mapped_range_mut()
            .expect("staging pool allocated a mapped writable upload");
        let data_len = data.len();
        mapped.slice(..data_len).copy_from_slice(data);
        mapped.slice(data_len..map_size as usize).fill(0);
    }
    handle.buffer.unmap();

    if staging_upload_encoder.is_none() {
        *staging_upload_encoder = Some(device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Staging Buffer Upload Encoder"),
            },
        ));
    }

    staging_upload_encoder
        .as_mut()
        .expect("SPEC-HAL-040: staging upload encoder must exist")
        .copy_buffer_to_buffer(&handle.buffer, 0, destination, destination_offset, size);
    staging_upload_handles.push(handle);
}

enum SuspendedCompile {
    Multi(crate::renderer::MultiTargetDrawPlan),
    Sdf(SdfCompiledFrame),
}
impl SuspendedCompile {
    fn plan(&self) -> &crate::renderer::MultiTargetDrawPlan {
        match self { Self::Multi(plan) => plan, Self::Sdf(frame) => frame.plan() }
    }
}

pub struct WgpuRenderer {
    adapter: wgpu::Adapter,
    font_manager: crate::platform::font_manager::FontManager,
    // SPEC-HAL-081B: Persistent instance for creating additional surfaces (multi-window)
    instance: wgpu::Instance,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    // Surface format determined at construction (from adapter capabilities)
    surface_format: wgpu::TextureFormat,
    // Present mode and alpha mode defaults (can be overridden per-target)
    default_present_mode: wgpu::PresentMode,
    default_alpha_mode: wgpu::CompositeAlphaMode,
    rect_pipeline: wgpu::RenderPipeline,
    // SPEC-R-020 Stage 4: Rect pipeline uniform buffers and bind groups
    rect_globals_buffer: wgpu::Buffer,
    rect_globals_bind_group: wgpu::BindGroup,
    // DEBUG: Store layout for per-draw buffer test
    rect_globals_layout: wgpu::BindGroupLayout,
    // SPEC-HAL-081B: Current capacity of globals slab buffer (in aligned slots).
    // Grows dynamically to accommodate any number of render targets.
    globals_slab_capacity: usize,
    rounded_clip_buffer: wgpu::Buffer,
    rounded_clip_bind_group: wgpu::BindGroup,
    // SPEC-R-013-v2: Shadow pipeline
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_params_buffer: wgpu::Buffer,
    shadow_params_bind_group: wgpu::BindGroup,
    shadow_vertex_buffer: Option<wgpu::Buffer>,
    shadow_index_buffer: Option<wgpu::Buffer>,
    shadow_vertex_capacity: u32,
    shadow_index_capacity: u32,
    // SPEC-R-012-v2: Gradient pipeline
    gradient_pipeline: wgpu::RenderPipeline,
    analytic_surface: Option<crate::renderer::pipelines::analytic_surface::AnalyticSurfacePipeline>,
    frosted_glass:crate::renderer::frosted_glass::FrostPool,
    gradient_params_buffer: wgpu::Buffer,
    gradient_params_bind_group: wgpu::BindGroup,
    gradient_vertex_buffer: Option<wgpu::Buffer>,
    gradient_index_buffer: Option<wgpu::Buffer>,
    gradient_vertex_capacity: u32,
    gradient_index_capacity: u32,
    // SPEC-HAL-272: ColorPicker SV pipeline
    color_picker_sv_pipeline: wgpu::RenderPipeline,
    color_picker_sv_params_buffer: wgpu::Buffer,
    color_picker_sv_params_bind_group: wgpu::BindGroup,
    color_picker_sv_vertex_buffer: Option<wgpu::Buffer>,
    color_picker_sv_index_buffer: Option<wgpu::Buffer>,
    color_picker_sv_vertex_capacity: u32,
    color_picker_sv_index_capacity: u32,
    // SPEC-HAL-278: RadialGauge pipeline
    radial_gauge_pipeline: wgpu::RenderPipeline,
    radial_gauge_params_buffer: wgpu::Buffer,
    radial_gauge_params_bind_group: wgpu::BindGroup,
    radial_gauge_vertex_buffer: Option<wgpu::Buffer>,
    radial_gauge_index_buffer: Option<wgpu::Buffer>,
    radial_gauge_vertex_capacity: u32,
    radial_gauge_index_capacity: u32,
    // SPEC-R-022: SDF RoundedRect pipeline
    sdf_rect_pipeline: Option<wgpu::RenderPipeline>,
    sdf_rect_vertex_buffer: Option<wgpu::Buffer>,
    sdf_rect_index_buffer: Option<wgpu::Buffer>,
    sdf_rect_vertex_capacity: u32,
    sdf_rect_index_capacity: u32,
    // SPEC-R-003 Stage 2B: Analytic shape pipeline geometry buffers
    shape_pipeline: ShapePipeline,
    shape_vertex_buffer: Option<wgpu::Buffer>,
    shape_index_buffer: Option<wgpu::Buffer>,
    shape_vertex_capacity: u32,
    shape_index_capacity: u32,
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    text_engine: BackendTextEngine<CoreTextBackend>,
    #[cfg(target_os = "windows")]
    text_engine: BackendTextEngine<DirectWriteBackend>,
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios")),
        not(target_arch = "wasm32")
    ))]
    text_engine: BackendTextEngine<FreeTypeBackend>,
    #[cfg(target_arch = "wasm32")]
    text_engine: BackendTextEngine<GlyphonBackend>,

    // SPEC-HAL-039: Persistent batched geometry buffers (rect pipeline)
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,

    // Capacities in elements (not bytes)
    vertex_capacity: u32,
    index_capacity: u32,

    // SPEC-HAL-263: Dedicated image pipeline geometry buffers
    image_vertex_buffer: Option<wgpu::Buffer>,
    image_index_buffer: Option<wgpu::Buffer>,
    image_vertex_capacity: u32,
    image_index_capacity: u32,

    // SPEC-HAL-263: Image pipeline (texture-sampled quads)
    image_pipeline: wgpu::RenderPipeline,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
    nearest_sampler: wgpu::Sampler,
    /// SPEC-HAL-263: Per-texture GPU resources (single source of truth).
    /// Each entry owns Texture + TextureView + BindGroup.
    /// Invariant: image_textures.len() == number of registered textures.
    image_textures: std::collections::HashMap<u64, ImageTextureEntry>,
    #[cfg(any(test, feature = "test-harness"))]
    headless_adapter_info: Option<wgpu::AdapterInfo>,
    // Unauthenticated registration and removal paths must reject these IDs.
    protected_sdf_texture_ids: std::collections::HashSet<u64>,
    sdf_texture_reservations:
        std::collections::HashMap<u64, sdf_image_reservation::SdfReservationState>,
    next_sdf_texture_id: Option<u64>,
    /// Shared registration queue (same Arc as GpuResources clones)
    pending_registrations: std::sync::Arc<std::sync::Mutex<Vec<PendingImageRegistration>>>,
    /// Shared unregistration queue (same Arc as GpuResources clones)
    pending_unregistrations: std::sync::Arc<std::sync::Mutex<Vec<PendingImageUnregistration>>>,
    pending_rgba8_registrations: std::sync::OnceLock<Arc<PendingImageRgba8Registrations>>,
    image_admission: std::sync::OnceLock<crate::renderer::image_binding::ImageAdmissionMailbox>,

    // Last frame's batch layout, for SPEC-039 diffing
    last_batches: Option<FrameBatches>,

    // Window focus state - skip rendering when unfocused to avoid macOS 1s throttle
    window_focused: bool,

    // SPEC-R-020 Stage 10: Global toggle for GPU rounded clipping
    gpu_rounded_clips_enabled: bool,

    // SPEC-R-020 Stage 12: AA radius for rounded clips, in logical pixels
    rounded_clip_aa_radius: f32,

    // SPEC-HAL-029: Glyph atlas and text rendering pipeline
    glyph_atlas: GlyphAtlas,
    text_pipeline: TextPipeline,
    text_bind_group: wgpu::BindGroup,
    text_vertex_buffer: Option<wgpu::Buffer>,
    text_index_buffer: Option<wgpu::Buffer>,
    text_vertex_capacity: u32,
    text_index_capacity: u32,
    // R-010: Text globals buffer for gamma-aware rendering
    text_globals_buffer: wgpu::Buffer,

    // SPEC-R-004: Path rendering (owns PathPipeline + PathBatchOwned)
    path_renderer: PathRenderer,
    // Bind groups for path rendering
    path_globals_buffer: wgpu::Buffer,
    path_globals_bind_group: wgpu::BindGroup,
    path_clips_buffer: wgpu::Buffer,
    path_clips_bind_group: wgpu::BindGroup,
    /// SPEC-HAL-081B: Number of per-target clip slots in the slab buffer.
    /// Each slot is `path_clips_slot_stride` bytes and contains one target's clips.
    path_clips_slab_capacity: u32,
    /// SPEC-HAL-081B / INV-PATH-CLIPS: byte stride of each per-target clip slot.
    /// Grows on demand to fit the frame's largest clip count; never shrinks.
    /// Floor = PATH_CLIPS_SLOT_SIZE. Always a multiple of PATH_CLIPS_SLAB_ALIGNMENT.
    path_clips_slot_stride: u64,

    // SPEC-HAL-264: Vector mesh cache (content-addressed tessellation cache)
    mesh_cache: MeshCache,
    // SPEC-HAL-264: Frame counter for mesh cache LRU eviction
    frame_counter: u64,
    current_preparation: Option<Arc<()>>,
    preparation_gate: Option<crate::renderer::RendererPreparationIdentity>,
    suspended_compile: Option<SuspendedCompile>,

    // SPEC-HAL-029: Rasterization counter for debugging
    #[cfg(debug_assertions)]
    rasterization_count: std::cell::Cell<u32>,

    // SPEC-HAL-084: Phase ordering enforcement (debug only).
    // Tracks the epoch at which compile_multi() was last called, and whether render_multi() consumed it.
    // Invariants:
    // - compile_multi() must be called before render_multi() in a frame
    // - render_multi() must not be called twice for the same compile_multi() output
    #[cfg(debug_assertions)]
    last_compile_epoch: std::cell::Cell<Option<u64>>,
    #[cfg(debug_assertions)]
    compile_consumed: std::cell::Cell<bool>,

    // SPEC-HAL-040 Stage 1: Frame Epoch — monotonically increasing per successful GPU frame.
    // This is the canonical source of truth for resource lifetimes in later stages of SPEC-HAL-040.
    frame_epoch: FrameEpoch,

    // SPEC-HAL-040 Stage 2: Unified GPU resource arena for tracking resource lifetimes.
    // This is the central registry for all GPU resources - holds metadata only, not actual handles.
    gpu_arena: GpuResourceArena,

    // SPEC-HAL-040 Stage 4: Staging buffer pool for CPU→GPU uploads.
    // Reuses buffers across frames instead of allocating new ones each time.
    staging_pool: StagingBufferPool,
    staging_upload_encoder: Option<wgpu::CommandEncoder>,
    staging_upload_handles: Vec<crate::renderer::staging_pool::StagingBufferHandle>,
    pending_staging_submissions: Vec<PendingStagingSubmission>,
    pending_staging_remaps: Vec<PendingStagingRemap>,

    // SPEC-HAL-040 Stage 7: Pipeline & BindGroup revalidation tracking.
    // Incremented when any event occurs that may invalidate pipelines/bind groups.
    pipeline_generation: u64,
    // The generation at which pipelines were last validated/rebuilt.
    last_validated_pipeline_generation: u64,
    // Track atlas texture ID to detect when it changes (growth).
    last_atlas_texture_id: GpuResourceId,
    // Track surface format to detect when it changes (reconfigure).
    last_surface_format: wgpu::TextureFormat,

    // ========================================================================
    // SPEC-HAL-081B: Multi-Surface Rendering (Unified Path)
    // ========================================================================
    //
    // All rendering goes through registered render targets.
    // There is no implicit primary surface.
    //
    /// Registry for all render targets.
    multi_targets: crate::renderer::render_target::RenderTargetRegistry,
}

impl Drop for WgpuRenderer {
    fn drop(&mut self) {
        self.invalidate_compile_preparation();
        self.close_loading_presence();
        // Terminal registry disposal precedes release; a callback owns only the
        // textures/opaque charges, never a strong Device/Queue reference.
        if let Some(shared) = self.pending_rgba8_registrations.get() {
            // Serialize shutdown against first-ever HDR custody creation too;
            // the lazy sidecar need not have existed before renderer teardown.
            let mut removals = self
                .pending_unregistrations
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            shared
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .closed = true;
            if let Some(custodies) = shared.hdr_finals.get() {
                let mut state = custodies.lock().unwrap_or_else(|e| e.into_inner());
                state.closed = true;
                let held = std::mem::take(&mut state.entries);
                for id in held.keys() {
                    if let Some(entry) = self.image_textures.remove(id) {
                        entry.revoke(
                            crate::renderer::image_binding::ImageBindingFailure::Unregistered,
                        );
                    }
                }
                // Each record already retains the exact native texture pair.
                // Dispose queued duplicates of those handles only after removal.
                removals.retain(|removal| !held.contains_key(&removal.texture_id));
                drop(state);
                drop(removals);
                crate::renderer::on_submitted_work_done(&self.queue, move || drop(held));
            }
        }
        let cancelled_pixels = if let Some(queue) = self.pending_rgba8_registrations.get() {
            let mut pending = queue
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            pending.closed = true;
            let entries = pending.items.take().unwrap_or_default();
            queue
                .has_items
                .store(false, std::sync::atomic::Ordering::Release);
            let had_entries = !entries.is_empty();
            drop(pending);
            drop(entries);
            had_entries
        } else {
            false
        };
        if let Some(mailbox) = self.image_admission.get() {
            mailbox.close();
            if cancelled_pixels {
                mailbox.notify();
            }
        }
    }
}

/// SPEC-R-020 Stage 11: Detect if CPU fallback for rounded clips is needed.
///
/// Returns true when the GPU adapter is known to have issues with discard-heavy
/// fragment shaders or other features required for GPU rounded clipping.
///
/// Triggers on:
/// - Old Intel iGPUs (HD, UHD, Iris series with known issues)
/// - Software renderers (SwiftShader, llvmpipe, etc.)
/// - OpenGL backend (less reliable discard semantics)
fn should_use_cpu_fallback(adapter: &wgpu::Adapter) -> bool {
    let info = adapter.get_info();
    let name = info.name.to_lowercase();

    // Check for software renderers
    if name.contains("swiftshader") || name.contains("software") || name.contains("llvmpipe") {
        return true;
    }

    // Check for OpenGL backend (less reliable discard semantics)
    if info.backend == wgpu::Backend::Gl {
        return true;
    }

    // Check for old Intel integrated GPUs (vendor 0x8086)
    // These often have issues with complex fragment shaders
    if info.vendor == 0x8086 {
        // Intel HD/UHD/Iris naming patterns for older problematic GPUs
        if name.contains("intel(r) hd")
            || name.contains("intel(r) uhd")
            || name.contains("intel(r) iris")
        {
            // Allow newer Iris Xe and Arc GPUs
            if name.contains("iris xe") || name.contains("arc") {
                return false;
            }
            return true;
        }
    }

    false
}

impl WgpuRenderer {
    pub async fn new(
        window: Arc<winit::window::Window>,
        _physical_width: u32,
        _physical_height: u32,
    ) -> Self {
        let scale_factor = window.scale_factor() as f32;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        #[cfg(target_os = "ios")]
        tracing::info!(adapter = ?adapter.get_info(), "iOS GPU adapter selected");

        // SPEC-R-020 Stage 11: Detect if CPU fallback is needed for this adapter
        let use_cpu_fallback = should_use_cpu_fallback(&adapter);
        if use_cpu_fallback {
            tracing::warn!("Halcyon: GPU rounded clipping disabled — CPU fallback active");
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                label: None,
                trace: Default::default(),
            })
            .await
            .unwrap();

        #[cfg(target_arch = "wasm32")]
        device.on_uncaptured_error(Arc::new(|error| {
            panic!("Halcyon WebGPU operation failed: {error}");
        }));

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // SPEC-HAL-081B: Query surface capabilities for format selection.
        // The temporary surface is used only for adapter/format discovery.
        // Actual render targets are registered via register_render_target().
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .format_capabilities
            .iter()
            .map(|capability| capability.format)
            .filter(|format| surface_caps.formats.contains(format))
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        #[cfg(target_arch = "wasm32")]
        let surface_format = web_color_attachment_format(surface_format);

        // Store defaults for render target configuration
        let default_present_mode = wgpu::PresentMode::AutoVsync;
        let default_alpha_mode = surface_caps.alpha_modes[0];

        // SPEC-HAL-081B: Drop the temporary surface - all rendering goes through
        // registered render targets. The surface was only used for adapter selection
        // and capability querying.
        drop(surface);

        Self::init_core(
            instance,
            &adapter,
            device,
            queue,
            surface_format,
            default_present_mode,
            default_alpha_mode,
            use_cpu_fallback,
            scale_factor,
        )
    }

    /// Create a headless WgpuRenderer without a window or surface.
    ///
    /// Returns `None` if no GPU adapter is available (e.g. headless CI).
    /// The renderer can compile draw plans but cannot present to a surface.
    /// Use `register_headless_render_target()` to register compile-phase targets.
    pub async fn new_headless() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                apply_limit_buckets: false,
                force_fallback_adapter: false,
            })
            .await
            .ok()?;

        let use_cpu_fallback = should_use_cpu_fallback(&adapter);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                label: None,
                trace: Default::default(),
            })
            .await
            .ok()?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Headless: use standard sRGB format (no surface caps to query)
        let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let default_present_mode = wgpu::PresentMode::AutoVsync;
        let default_alpha_mode = wgpu::CompositeAlphaMode::Auto;

        let renderer = Self::init_core(
            instance,
            &adapter,
            device,
            queue,
            surface_format,
            default_present_mode,
            default_alpha_mode,
            use_cpu_fallback,
            1.0,
        );
        #[cfg(any(test, feature = "test-harness"))]
        let renderer = {
            let mut renderer = renderer;
            renderer.headless_adapter_info = Some(adapter.get_info());
            renderer
        };
        Some(renderer)
    }

    /// Instrumentation-only Device; ordinary/native/headless constructors stay feature-free.
    #[cfg(test)]
    pub(crate) async fn new_headless_timestamp_for_test()
    -> Result<(Self, wgpu::Features, wgpu::TextureFormatFeatures), String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                apply_limit_buckets: false,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        let features = adapter.features();
        if info.name != "Apple M4 Max"
            || info.backend != wgpu::Backend::Metal
            || !features.contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return Err(format!(
                "R028 timestamp Device BLOCK: {info:?}, {features:?}"
            ));
        }
        let format = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float);
        let use_cpu_fallback = should_use_cpu_fallback(&adapter);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::TIMESTAMP_QUERY,
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                label: Some("R028 test-only hardware timestamp Device"),
                trace: Default::default(),
            })
            .await
            .map_err(|e| e.to_string())?;
        assert_eq!(device.features(), wgpu::Features::TIMESTAMP_QUERY);
        let mut renderer = Self::init_core(
            instance,
            &adapter,
            Arc::new(device),
            Arc::new(queue),
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::PresentMode::AutoVsync,
            wgpu::CompositeAlphaMode::Auto,
            use_cpu_fallback,
            1.0,
        );
        renderer.headless_adapter_info = Some(info);
        Ok((renderer, features, format))
    }

    /// Shared initialization core: creates all pipelines, buffers, bind groups, and subsystems.
    ///
    /// Both `new()` and `new_headless()` delegate here after adapter/device creation.
    /// No surface or window state is touched — only GPU pipeline and subsystem setup.
    fn init_core(
        instance: wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        default_present_mode: wgpu::PresentMode,
        default_alpha_mode: wgpu::CompositeAlphaMode,
        use_cpu_fallback: bool,
        scale_factor: f32,
    ) -> Self {
        let (rect_pipeline, rect_globals_layout, rect_rounded_clip_layout) =
            Self::create_rect_pipeline(&device, surface_format);

        // SPEC-HAL-085: Create shared globals buffer (used by all pipelines)
        // SPEC-HAL-081B: Initial capacity of 1 slot (one window), grows dynamically
        // Each target gets its own aligned slot for dynamic offset binding
        const INITIAL_GLOBALS_SLAB_CAPACITY: usize = 1;
        let rect_globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Globals Buffer (Multi-Target Slab)"),
            size: INITIAL_GLOBALS_SLAB_CAPACITY as u64 * GLOBALS_UNIFORM_ALIGNMENT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // SPEC-HAL-081B: Bind only one uniform-sized window for dynamic offset
        let rect_globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rect Globals Bind Group"),
            layout: &rect_globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &rect_globals_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-R-020 Stage 14: Create buffer sized for multiple clips with alignment
        // Each clip uses ROUNDED_CLIP_UNIFORM_ALIGNMENT bytes (256), not sizeof(RoundedClipUniform)
        let rounded_clip_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rounded Clip Buffer"),
            size: MAX_ROUNDED_CLIPS_PER_FRAME as u64 * ROUNDED_CLIP_UNIFORM_ALIGNMENT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // SPEC-R-020 Stage 14: Bind only one uniform-sized window for dynamic offset
        let rounded_clip_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rounded Clip Bind Group"),
            layout: &rect_rounded_clip_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &rounded_clip_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<RoundedClipUniform>() as u64)
                            .unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-HAL-040: Initialize arena and epoch early so GlyphAtlas can register its texture
        let mut gpu_arena = GpuResourceArena::new();
        let frame_epoch = FrameEpoch::zero();

        // SPEC-HAL-029: Create glyph atlas and text pipeline
        // SPEC-HAL-040 Stage 5: Pass arena/epoch for texture registration
        let glyph_atlas = GlyphAtlas::new(&device, 1024, 1024, &mut gpu_arena, frame_epoch);
        // SPEC-HAL-040 Stage 7: Capture texture ID before moving glyph_atlas
        let initial_atlas_texture_id = glyph_atlas.texture_id;
        // SPEC-HAL-085: Text pipeline uses shared globals layout
        let text_pipeline = create_text_pipeline(&device, surface_format, &rect_globals_layout);

        // R-010C: Create text globals buffer for per-channel LCD subpixel rendering
        let text_globals = TextGlobalsUniform {
            gamma_r: 1.55,
            gamma_g: 1.75,
            gamma_b: 1.65,
            _pad: 0.0,
        };
        let text_globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Globals Buffer"),
            contents: bytemuck::bytes_of(&text_globals),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let text_bind_group = Self::create_text_bind_group(
            &device,
            &text_pipeline.resources_bind_group_layout,
            &glyph_atlas,
            &text_globals_buffer,
        );

        // SPEC-HAL-103 Stage 3: Bootstrap text engine with SystemUI (falls back to Inter)
        // Renderer consumes the engine's identity verbatim - no recomputation
        let bootstrap_engine =
            crate::platform::font_manager_text_engine::FontManagerTextEngine::new_with_system_ui();
        let font_data = bootstrap_engine.font_data().clone();
        let font_id = bootstrap_engine.font_id();
        let font_manager = bootstrap_engine
            .bootstrap_font_manager()
            .expect("complete bootstrap font catalog must be admitted");

        // Create TextShaper from the same font data (rustybuzz)
        let shaper = crate::core::text_shaper::TextShaper::from_data(font_data.clone())
            .expect("Failed to create shaper from SystemUI/legacy font");
        let shaper = Arc::new(shaper);

        // Create backend from the SAME font data (ensures glyph ID match)
        // CRITICAL: Glyph IDs are font-specific. Shaper and rasterizer MUST use identical bytes.
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let backend = CoreTextBackend::new_from_data(font_data.clone(), font_id)
            .expect("SystemUI bytes must initialize the CoreText rasterizer");
        #[cfg(target_os = "windows")]
        let backend = DirectWriteBackend::new_from_data(font_data.clone(), font_id)
            .expect("SystemUI bytes must initialize the DirectWrite rasterizer");
        #[cfg(all(
            unix,
            not(any(target_os = "macos", target_os = "ios")),
            not(target_arch = "wasm32")
        ))]
        let backend =
            FreeTypeBackend::new(font_data.clone(), font_id).expect("FreeType: failed to init");
        #[cfg(target_arch = "wasm32")]
        let backend = <GlyphonBackend as crate::platform::native_font_backend::NativeFontBackend>::from_font_data(font_data.clone(), font_id)
            .expect("SystemUI bytes must initialize the Glyphon rasterizer");

        let mut text_engine = BackendTextEngine::new(backend);
        text_engine.set_scale(scale_factor);
        text_engine.set_shaper(shaper, font_id);
        text_engine
            .admit_face_catalog(font_manager.face_catalog())
            .expect("bootstrap catalog must rasterize");

        // SPEC-R-004: Create path renderer and its bind groups
        let path_renderer = PathRenderer::new(&device, &queue, surface_format);

        // Create globals buffer for path rendering
        // SPEC-HAL-085: Path pipeline uses same Globals struct as rect pipeline
        // SPEC-HAL-081B: Multi-target slab buffer (same as rect globals)
        let path_globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Globals Buffer (Multi-Target Slab)"),
            size: INITIAL_GLOBALS_SLAB_CAPACITY as u64 * GLOBALS_UNIFORM_ALIGNMENT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // SPEC-HAL-081B: Bind only one uniform-sized window for dynamic offset
        let path_globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Path Globals Bind Group"),
            layout: &path_renderer.pipeline().bind_group_layouts().globals,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &path_globals_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-R-004 Stage 13: Create clips buffer for path rendering with initial capacity
        // SPEC-HAL-081B: Use slab-style buffer for per-target clips with dynamic offsets
        let path_clips_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Clips Buffer (Multi-Target Slab)"),
            size: INITIAL_PATH_CLIPS_SLAB_CAPACITY as u64 * PATH_CLIPS_SLOT_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // SPEC-HAL-081B: Bind entire buffer for dynamic offset binding
        let path_clips_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Path Clips Bind Group"),
            layout: &path_renderer.pipeline().bind_group_layouts().clips,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: path_clips_buffer.as_entire_binding(),
            }],
        });

        // SPEC-HAL-263: Create image pipeline and sampler (no dummy texture — SPEC requires
        // debug panic / release skip for missing texture_id, not placeholder rendering)
        let (image_pipeline, image_bind_group_layout) =
            Self::create_image_pipeline(&device, surface_format, &rect_globals_layout);
        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Image Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // SPEC-R-013-v2: Shadow pipeline
        let (shadow_pipeline, shadow_params_layout) =
            Self::create_shadow_pipeline(&device, surface_format, &rect_globals_layout);

        let shadow_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Params Buffer"),
            size: MAX_SHADOW_PARAMS_PER_FRAME as u64 * ROUNDED_CLIP_UNIFORM_ALIGNMENT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Params Bind Group"),
            layout: &shadow_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &shadow_params_buffer,
                    offset: 0,
                    size:
                        Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<ShadowParamsUniform>() as u64
                            )
                            .unwrap(),
                        ),
                }),
            }],
        });

        // SPEC-R-012-v2: Gradient pipeline
        let (gradient_pipeline, gradient_params_layout) =
            Self::create_gradient_pipeline(&device, surface_format, &rect_globals_layout);

        let gradient_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Gradient Params Buffer"),
            size: MAX_GRADIENT_PARAMS_PER_FRAME as u64 * GRADIENT_PARAMS_UNIFORM_ALIGNMENT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let gradient_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Gradient Params Bind Group"),
            layout: &gradient_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &gradient_params_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(
                            std::mem::size_of::<GradientParamsUniform>() as u64
                        )
                        .unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-HAL-272: ColorPicker SV pipeline
        let (color_picker_sv_pipeline, color_picker_sv_params_layout) =
            Self::create_color_picker_sv_pipeline(&device, surface_format, &rect_globals_layout);

        let color_picker_sv_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ColorPicker SV Params Buffer"),
            size: MAX_COLOR_PICKER_SV_PARAMS_PER_FRAME as u64
                * COLOR_PICKER_SV_PARAMS_UNIFORM_ALIGNMENT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let color_picker_sv_params_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ColorPicker SV Params Bind Group"),
                layout: &color_picker_sv_params_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &color_picker_sv_params_buffer,
                        offset: 0,
                        size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<ColorPickerSvParamsUniform>() as u64,
                            )
                            .unwrap(),
                        ),
                    }),
                }],
            });

        // SPEC-HAL-278: RadialGauge pipeline
        let (radial_gauge_pipeline, radial_gauge_params_layout) =
            Self::create_radial_gauge_pipeline(&device, surface_format, &rect_globals_layout);

        let radial_gauge_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RadialGauge Params Buffer"),
            size: MAX_RADIAL_GAUGE_PARAMS_PER_FRAME as u64 * RADIAL_GAUGE_PARAMS_UNIFORM_ALIGNMENT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let radial_gauge_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RadialGauge Params Bind Group"),
            layout: &radial_gauge_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &radial_gauge_params_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(
                            std::mem::size_of::<RadialGaugeParamsUniform>() as u64
                        )
                        .unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-R-022: SDF RoundedRect pipeline
        let sdf_rect_pipeline =
            Self::create_sdf_rect_pipeline(&device, surface_format, &rect_globals_layout);
        // SPEC-R-003 Stage 2B: Shape pipeline shares renderer Globals at group(0).
        let shape_pipeline = ShapePipeline::new(&device, surface_format, &rect_globals_layout);

        Self {
            instance,
            font_manager,
            device,
            queue,
            surface_format,
            default_present_mode,
            default_alpha_mode,
            rect_pipeline,
            rect_globals_buffer,
            rect_globals_bind_group,
            rect_globals_layout,
            globals_slab_capacity: INITIAL_GLOBALS_SLAB_CAPACITY,
            rounded_clip_buffer,
            rounded_clip_bind_group,
            shadow_pipeline,
            shadow_params_buffer,
            shadow_params_bind_group,
            shadow_vertex_buffer: None,
            shadow_index_buffer: None,
            shadow_vertex_capacity: 0,
            shadow_index_capacity: 0,
            gradient_pipeline,
            analytic_surface: None,
            frosted_glass:Default::default(),
            gradient_params_buffer,
            gradient_params_bind_group,
            gradient_vertex_buffer: None,
            gradient_index_buffer: None,
            gradient_vertex_capacity: 0,
            gradient_index_capacity: 0,
            color_picker_sv_pipeline,
            color_picker_sv_params_buffer,
            color_picker_sv_params_bind_group,
            color_picker_sv_vertex_buffer: None,
            color_picker_sv_index_buffer: None,
            color_picker_sv_vertex_capacity: 0,
            color_picker_sv_index_capacity: 0,
            radial_gauge_pipeline,
            radial_gauge_params_buffer,
            radial_gauge_params_bind_group,
            radial_gauge_vertex_buffer: None,
            radial_gauge_index_buffer: None,
            radial_gauge_vertex_capacity: 0,
            radial_gauge_index_capacity: 0,
            sdf_rect_pipeline: Some(sdf_rect_pipeline),
            sdf_rect_vertex_buffer: None,
            sdf_rect_index_buffer: None,
            sdf_rect_vertex_capacity: 0,
            sdf_rect_index_capacity: 0,
            shape_pipeline,
            shape_vertex_buffer: None,
            shape_index_buffer: None,
            shape_vertex_capacity: 0,
            shape_index_capacity: 0,
            text_engine,
            vertex_buffer: None,
            index_buffer: None,
            vertex_capacity: 0,
            index_capacity: 0,
            image_vertex_buffer: None,
            image_index_buffer: None,
            image_vertex_capacity: 0,
            image_index_capacity: 0,
            image_pipeline,
            image_bind_group_layout,
            image_sampler,
            nearest_sampler,
            image_textures: std::collections::HashMap::new(),
            #[cfg(any(test, feature = "test-harness"))]
            headless_adapter_info: None,
            protected_sdf_texture_ids: std::collections::HashSet::new(),
            sdf_texture_reservations: std::collections::HashMap::new(),
            next_sdf_texture_id: Some(1),
            pending_registrations: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            pending_unregistrations: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            pending_rgba8_registrations: std::sync::OnceLock::new(),
            image_admission: std::sync::OnceLock::new(),
            last_batches: None,
            window_focused: true,
            adapter: adapter.clone(),
            gpu_rounded_clips_enabled: !use_cpu_fallback, // SPEC-R-020 Stage 10+11: Disabled on fallback
            rounded_clip_aa_radius: 1.0,                  // SPEC-R-020 Stage 12: Default AA radius
            glyph_atlas,
            text_pipeline,
            text_bind_group,
            text_vertex_buffer: None,
            text_index_buffer: None,
            text_vertex_capacity: 0,
            text_index_capacity: 0,
            // R-010: Text globals buffer
            text_globals_buffer,
            path_renderer,
            path_globals_buffer,
            path_globals_bind_group,
            path_clips_buffer,
            path_clips_bind_group,
            path_clips_slab_capacity: INITIAL_PATH_CLIPS_SLAB_CAPACITY,
            path_clips_slot_stride: PATH_CLIPS_SLOT_SIZE,
            mesh_cache: MeshCache::new(),
            frame_counter: 0,
            current_preparation: None,
            preparation_gate: None,
            suspended_compile: None,
            #[cfg(debug_assertions)]
            rasterization_count: std::cell::Cell::new(0),
            #[cfg(debug_assertions)]
            last_compile_epoch: std::cell::Cell::new(None),
            #[cfg(debug_assertions)]
            compile_consumed: std::cell::Cell::new(true),
            // SPEC-HAL-040: Use arena/epoch created earlier (glyph atlas texture already registered)
            frame_epoch,
            gpu_arena,
            // SPEC-HAL-040 Stage 4: Initialize empty staging buffer pool
            staging_pool: StagingBufferPool::new(),
            staging_upload_encoder: None,
            staging_upload_handles: Vec::new(),
            pending_staging_submissions: Vec::new(),
            pending_staging_remaps: Vec::new(),
            // SPEC-HAL-040 Stage 7: Pipeline revalidation tracking
            pipeline_generation: 0,
            last_validated_pipeline_generation: 0,
            last_atlas_texture_id: initial_atlas_texture_id,
            last_surface_format: surface_format,
            // SPEC-HAL-081B: Multi-surface is the only mode
            multi_targets: crate::renderer::render_target::RenderTargetRegistry::new(),
        }
    }

    /// Admit immutable application fonts before the first compile or Screen measurement.
    pub fn admit_font_assets(
        &mut self,
        assets: &[crate::platform::font_manager::FontAsset],
    ) -> Result<(), crate::core::text_engine::TextInputError> {
        if assets.is_empty() {
            return Ok(());
        }
        if self.frame_counter != 0 {
            return Err(crate::core::text_engine::TextInputError::UnsupportedTypography);
        }
        let mut admitted = self.font_manager.clone();
        for asset in assets {
            admitted
                .register_font_asset(asset.clone())
                .map_err(|_| crate::core::text_engine::TextInputError::FontUnavailable)?;
        }
        self.text_engine
            .admit_face_catalog(admitted.face_catalog())?;
        self.font_manager = admitted;
        Ok(())
    }

    pub fn font_face_catalog(&self) -> crate::platform::font_manager::FontFaceCatalog {
        self.font_manager.face_catalog()
    }

    /// Register a headless render target (no wgpu::Surface).
    ///
    /// Provides compile-phase metadata only. Cannot be used for rendering.
    /// Useful for testing compile_multi without a real window.
    pub fn register_headless_render_target(
        &mut self,
        surface_id: SurfaceId,
        physical_size: (u32, u32),
        scale_factor: f32,
    ) -> Result<(), crate::renderer::render_target::RenderTargetError> {
        self.multi_targets
            .register_headless(surface_id, physical_size, scale_factor)
    }

    fn create_rect_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
    ) {
        // SPEC-R-020 Stage 4: Create bind group layouts for rect pipeline uniforms
        // SPEC-HAL-081B: Enable dynamic offset for per-target globals in multi-surface mode
        let globals_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Rect Globals Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // SPEC-R-020 Stage 14: Enable dynamic offsets for per-batch clip binding
        let rounded_clip_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Rounded Clip Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rect Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rect.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rect Pipeline Layout"),
            bind_group_layouts: &[Some(&globals_bind_group_layout), Some(&rounded_clip_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rect Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(RectVertex::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                // Alpha model: PREMULTIPLIED - shader outputs (r*a, g*a, b*a, a)
                // BlendComponent::OVER = One / OneMinusSrcAlpha (premultiplied blending)
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (
            pipeline,
            globals_bind_group_layout,
            rounded_clip_bind_group_layout,
        )
    }

    /// SPEC-HAL-263: Create image pipeline with texture+sampler bind group.
    ///
    /// Bind group layout:
    /// - group(0) = Globals (shared with rect, passed as `globals_layout`)
    /// - group(1) = image_tex (texture_2d) + image_sampler (sampler)
    ///
    /// Vertex layout: ImageVertex (position + uv + color)
    fn create_image_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        // group(1): texture + sampler + rounded_clip_ubo (SPEC-HAL-263 Section 6.1)
        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Image Bind Group Layout"),
                entries: &[
                    // @binding(0) image_tex: texture_2d<f32>
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // @binding(1) image_sampler: sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // @binding(2) rounded_clip: uniform with dynamic offset
                    // Same model as Rect pipeline (SPEC-HAL-263 Section 6.1)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/image.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Image Pipeline Layout"),
            bind_group_layouts: &[Some(globals_layout), Some(&image_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ImageVertex::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (pipeline, image_bind_group_layout)
    }

    /// SPEC-R-013-v2: Create shadow pipeline.
    ///
    /// Bind group layout:
    ///   group(0) = Globals (shared, reused from rect pipeline)
    ///   group(1) = ShadowParams uniform with dynamic offset (per-batch)
    ///
    /// Vertex layout: ShadowVertex (position + color + shadow_offset)
    fn create_shadow_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let shadow_params_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Shadow Params Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[Some(globals_layout), Some(&shadow_params_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ShadowVertex::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (pipeline, shadow_params_layout)
    }

    /// SPEC-R-012-v2: Create gradient pipeline.
    ///
    /// Bind group layout:
    ///   group(0) = Globals (shared)
    ///   group(1) = GradientParams uniform with dynamic offset (per-batch)
    ///
    /// Vertex layout: GradientVertex (position only)
    fn create_gradient_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let gradient_params_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Gradient Params Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Gradient Shader"),
            source: wgpu::ShaderSource::Wgsl(GRADIENT_SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Gradient Pipeline Layout"),
            bind_group_layouts: &[Some(globals_layout), Some(&gradient_params_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Gradient Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(GradientVertex::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        (pipeline, gradient_params_layout)
    }

    fn create_color_picker_sv_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        rect_globals_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ColorPicker SV Shader"),
            source: wgpu::ShaderSource::Wgsl(COLOR_PICKER_SV_SHADER_SRC.into()),
        });

        let params_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ColorPicker SV Params Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<ColorPickerSvParamsUniform>() as u64,
                            )
                            .unwrap(),
                        ),
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ColorPicker SV Pipeline Layout"),
            bind_group_layouts: &[Some(rect_globals_layout), Some(&params_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ColorPicker SV Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ColorPickerSvVertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        (pipeline, params_bind_group_layout)
    }

    /// SPEC-HAL-278: Create the radial gauge render pipeline.
    fn create_radial_gauge_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        rect_globals_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RadialGauge Shader"),
            source: wgpu::ShaderSource::Wgsl(RADIAL_GAUGE_SHADER_SRC.into()),
        });

        let params_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("RadialGauge Params Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<RadialGaugeParamsUniform>() as u64,
                            )
                            .unwrap(),
                        ),
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RadialGauge Pipeline Layout"),
            bind_group_layouts: &[Some(rect_globals_layout), Some(&params_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("RadialGauge Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(RadialGaugeVertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        (pipeline, params_bind_group_layout)
    }

    /// SPEC-HAL-263: Create a bind group for a specific image texture.
    ///
    /// Each per-texture bind group contains:
    /// - binding(0): texture_view for this texture_id
    /// - binding(1): shared image sampler
    /// - binding(2): shared rounded_clip_buffer (dynamic offset per batch)
    fn create_image_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        rounded_clip_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Image Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: rounded_clip_buffer,
                        offset: 0,
                        size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<RoundedClipUniform>() as u64
                            )
                            .unwrap(),
                        ),
                    }),
                },
            ],
        })
    }

    /// Create the renderer-global text resource bind group.
    ///
    /// @binding(0) atlas_tex - glyph atlas texture
    /// @binding(1) atlas_sampler - atlas sampler
    /// @binding(2) text_params - gamma parameters
    fn create_text_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        atlas: &GlyphAtlas,
        text_globals_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout,
            entries: &[
                // @binding(0) atlas_tex
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                // @binding(1) atlas_sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                },
                // @binding(2) text_globals (R-010)
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: text_globals_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// SPEC-R-020 Stage 10: Enable or disable GPU rounded clipping globally.
    ///
    /// When disabled, the shader takes a trivial pass-through path and clipping
    /// degenerates to scissor-only (using existing scissor logic).
    ///
    /// Default is `true` (enabled).
    pub fn set_gpu_rounded_clips_enabled(&mut self, enabled: bool) {
        self.gpu_rounded_clips_enabled = enabled;
    }

    /// SPEC-R-020 Stage 12: Set AA radius for GPU rounded clips (in logical pixels).
    ///
    /// Value is clamped to [0.25, 4.0] logical pixels to avoid artifacts.
    /// Default is 1.0.
    pub fn set_rounded_clip_aa_radius(&mut self, radius: f32) {
        self.rounded_clip_aa_radius = radius.clamp(0.25, 4.0);
    }

    /// R-010C: Set text gamma for per-channel LCD subpixel rendering.
    ///
    /// Higher values (>1.0) make text appear thicker/bolder.
    /// Lower values (<1.0) make text appear thinner/lighter.
    /// Default is (1.55, 1.75, 1.65) matching Apple's internal LCD gamma weighting.
    pub fn set_text_gamma(&mut self, gamma: f32) {
        // For API compatibility, apply uniform gamma to all channels
        let globals = TextGlobalsUniform {
            gamma_r: gamma,
            gamma_g: gamma,
            gamma_b: gamma,
            _pad: 0.0,
        };
        self.stage_buffer_upload(
            self.text_globals_buffer.clone(),
            0,
            bytemuck::bytes_of(&globals),
        );
        self.flush_staging_uploads();
    }

    /// SPEC-HAL-040 Stage 7: Central pipeline/bind group validation.
    ///
    /// Checks if any GPU resource has changed in a way that requires pipeline
    /// or bind group rebuilding. If `pipeline_generation` has advanced since
    /// last validation, rebuilds all affected resources.
    ///
    /// This method is:
    /// - **Always cheap**: returns immediately if nothing changed
    /// - **Always correct**: rebuilds everything needed
    /// - **Idempotent**: safe to call multiple times
    ///
    /// Called once per frame at the start of compile_multi(), before encoding passes.
    fn validate_pipelines(&mut self) {
        let validate_start = web_time::Instant::now();
        let pipeline_generation_changed =
            self.last_validated_pipeline_generation != self.pipeline_generation;

        // Pipeline/bind group recreation is format-dependent, not size-dependent.
        // Only rebuild when format actually changes.
        if !pipeline_generation_changed {
            let total_us = validate_start.elapsed().as_micros();
            if total_us > 50 {
                tracing::debug!(
                    total_us,
                    gen = self.pipeline_generation,
                    "validate_pipelines: early exit (no gen change)"
                );
            }
            return; // Pipelines up to date
        }

        tracing::debug!(
            old_gen = self.last_validated_pipeline_generation,
            new_gen = self.pipeline_generation,
            "validate_pipelines: PIPELINE REBUILD triggered"
        );

        // Rebuild rect pipeline with current surface format
        let (rect_pipeline, rect_globals_layout, rect_rounded_clip_layout) =
            Self::create_rect_pipeline(&self.device, self.surface_format);
        self.rect_pipeline = rect_pipeline;
        self.rect_globals_layout = rect_globals_layout;
        self.analytic_surface = None;

        // SPEC-R-020 Stage 4: Recreate rect bind groups with new layouts
        // SPEC-HAL-081B: Bind only one uniform-sized window for dynamic offset
        self.rect_globals_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rect Globals Bind Group"),
            layout: &self.rect_globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.rect_globals_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-R-020 Stage 14: Bind only one uniform-sized window for dynamic offset
        self.rounded_clip_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rounded Clip Bind Group"),
            layout: &rect_rounded_clip_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.rounded_clip_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<RoundedClipUniform>() as u64)
                            .unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-HAL-085: Rebuild text pipeline with shared globals layout
        self.text_pipeline =
            create_text_pipeline(&self.device, self.surface_format, &self.rect_globals_layout);
        self.text_bind_group = Self::create_text_bind_group(
            &self.device,
            &self.text_pipeline.resources_bind_group_layout,
            &self.glyph_atlas,
            &self.text_globals_buffer,
        );

        // SPEC-HAL-263: Rebuild image pipeline with shared globals layout
        let (image_pipeline, image_bind_group_layout) = Self::create_image_pipeline(
            &self.device,
            self.surface_format,
            &self.rect_globals_layout,
        );
        self.image_pipeline = image_pipeline;
        self.image_bind_group_layout = image_bind_group_layout;
        // SPEC-HAL-263B: Registration identity and sampling policy survive
        // same-device pipeline revalidation. Only layout-derived bind groups
        // are replaced here.
        for entry in self.image_textures.values_mut() {
            let sampler = match entry.sampling {
                ImageSampling::Linear => &self.image_sampler,
                ImageSampling::Nearest => &self.nearest_sampler,
            };
            entry.bind_group = Self::create_image_bind_group(
                &self.device,
                &self.image_bind_group_layout,
                &entry.view,
                sampler,
                &self.rounded_clip_buffer,
            );
        }
        // INV-VMP-008: Device loss invalidates all cached meshes.
        self.mesh_cache.clear();

        // Update tracking
        self.last_surface_format = self.surface_format;
        self.last_validated_pipeline_generation = self.pipeline_generation;

        let total_us = validate_start.elapsed().as_micros();
        tracing::debug!(
            total_us,
            gen = self.pipeline_generation,
            "validate_pipelines: FULL REBUILD complete"
        );
    }

    /// Set window focus state.
    ///
    /// SPEC-HAL-082A: The renderer does NOT use this to gate rendering.
    /// This is provided for the App layer to query via `is_focused()` when
    /// deciding whether to call compile/render (e.g., to avoid macOS 1-second
    /// throttle on `get_current_texture()` when unfocused).
    pub fn set_focused(&mut self, focused: bool) {
        self.window_focused = focused;
    }

    /// Returns whether the window is currently focused.
    ///
    /// SPEC-HAL-082A: The renderer is execution-only and does not use this
    /// internally. The App layer should check this before calling compile/render.
    pub fn is_focused(&self) -> bool {
        self.window_focused
    }

    /// SPEC-HAL-081B: Ensure globals slab buffer can hold at least `required_slots` targets.
    ///
    /// If the current capacity is insufficient, the buffer is reallocated to the
    /// next power of 2 >= required_slots. The bind group is recreated to reference
    /// the new buffer.
    ///
    /// # Growth Strategy
    ///
    /// Uses `next_power_of_two()` to minimize reallocations while avoiding
    /// arbitrary caps. Any number of targets is supported until allocation fails.
    fn ensure_globals_slab_capacity(&mut self, required_slots: usize) {
        if required_slots <= self.globals_slab_capacity {
            return;
        }

        // Grow to next power of 2 (minimum 1)
        let new_capacity = required_slots.max(1).next_power_of_two();

        tracing::debug!(
            "SPEC-HAL-081B: Growing globals slab {} -> {} slots",
            self.globals_slab_capacity,
            new_capacity
        );

        let new_size = new_capacity as u64 * GLOBALS_UNIFORM_ALIGNMENT;

        // Reallocate rect globals buffer
        self.rect_globals_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Globals Buffer (Multi-Target Slab)"),
            size: new_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Recreate rect globals bind group to reference new buffer
        self.rect_globals_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rect Globals Bind Group"),
            layout: &self.rect_globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.rect_globals_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap(),
                    ),
                }),
            }],
        });

        // SPEC-HAL-081B: Also grow path globals buffer (same multi-target slab)
        self.path_globals_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Globals Buffer (Multi-Target Slab)"),
            size: new_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Recreate path globals bind group to reference new buffer
        self.path_globals_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Path Globals Bind Group"),
            layout: &self.path_renderer.pipeline().bind_group_layouts().globals,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.path_globals_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap(),
                    ),
                }),
            }],
        });

        self.globals_slab_capacity = new_capacity;
    }

    /// SPEC-HAL-081B: Ensure the path clips slab can hold `required_slots` targets,
    /// each with a per-target slot of at least `required_stride` bytes.
    ///
    /// INV-PATH-CLIPS: the slot stride must be >= the largest target's clip bytes so
    /// no target's clips overrun into the next slot. Both the slot count and the slot
    /// stride grow on demand and never shrink (mirrors the globals slab — no fixed cap).
    fn ensure_path_clips_slab_capacity(&mut self, required_slots: usize, required_stride: u64) {
        let new_stride = self.path_clips_slot_stride.max(required_stride);
        let grow_slots = required_slots > self.path_clips_slab_capacity as usize;
        let grow_stride = new_stride > self.path_clips_slot_stride;
        if !grow_slots && !grow_stride {
            return;
        }

        // Grow slot count to next power of 2 (never shrink).
        let new_capacity = required_slots
            .max(self.path_clips_slab_capacity as usize)
            .max(1)
            .next_power_of_two() as u32;

        tracing::debug!(
            "SPEC-HAL-081B: Growing path clips slab {}x{}B -> {}x{}B",
            self.path_clips_slab_capacity,
            self.path_clips_slot_stride,
            new_capacity,
            new_stride
        );

        let new_size = new_capacity as u64 * new_stride;

        // Reallocate path clips buffer
        self.path_clips_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Clips Buffer (Multi-Target Slab)"),
            size: new_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Recreate path clips bind group to reference new buffer
        self.path_clips_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Path Clips Bind Group"),
            layout: &self.path_renderer.pipeline().bind_group_layouts().clips,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.path_clips_buffer.as_entire_binding(),
            }],
        });

        self.path_clips_slab_capacity = new_capacity;
        self.path_clips_slot_stride = new_stride;
    }

    /// SPEC-HAL-084: Advance frame epoch after successful GPU submission.
    ///
    /// This MUST be called by the platform layer after:
    /// 1. queue.submit() with the command buffer
    /// 2. queue.present(surface_output)
    ///
    /// # SPEC-HAL-040 Semantics
    ///
    /// - Epoch advances exactly once per successful GPU frame
    /// - Epoch is the canonical source of truth for resource lifetimes
    /// - Metadata GC runs after epoch advancement
    pub fn advance_frame(&mut self) {
        // SPEC-HAL-040 Stage 1: Advance epoch exactly once per successful GPU frame.
        self.frame_epoch = self.frame_epoch.next();

        self.poll_staging_recycles();
        // HAL385: reuse the existing nonblocking completion maintenance.
        self.frosted_glass.maintain();

        // SPEC-HAL-040 Stage 3: Run metadata GC after advancing epoch.
        // Removes metadata entries whose destruction epoch has matured.
        self.gpu_arena.collect_garbage(self.frame_epoch);

        // SPEC-HAL-264: Evict stale mesh cache entries.
        // Paths with changing data (sparklines, animated shapes) generate new
        // cache keys every frame. Without eviction, the cache grows unboundedly.
        self.frame_counter += 1;
        #[cfg(feature = "render-audit")]
        super::cadence_audit::record_renderer_frame_advance();
        self.mesh_cache.evict_stale(self.frame_counter, 120, 64);
    }

    /// SPEC-HAL-264: Access the mesh cache for passing to DrawPlanBuilder::build_multi().
    /// INV-VMP-001: MeshCache is renderer-owned.
    pub fn mesh_cache(&mut self) -> &mut MeshCache {
        &mut self.mesh_cache
    }

    /// SPEC-HAL-264: Current frame counter for mesh cache LRU tracking.
    pub fn frame_counter(&self) -> u64 {
        self.frame_counter
    }

    /// SPEC-HAL-084 / INV-RG-013: Submit a rendered output and present it.
    ///
    /// Submits the command buffer to the GPU queue and presents the surface texture.
    /// This replaces direct queue access — the renderer owns all GPU handle interactions.
    pub fn submit_and_present(&self, output: MultiTargetRenderOutput) {
        if let Err(error)=self.submit_and_present_with_sdf_custody(output, Vec::new()) {
            tracing::error!(?error,"HAL385 output submission refused");
        }
    }

    /// Only the admitted runtime frame owner may supply sampled-generation guards.
    pub(super) fn submit_and_present_with_sdf_custody(
        &self,
        output: MultiTargetRenderOutput,
        custody: Vec<SdfImageCpuUse>,
    ) -> Result<(), SdfImageCustodyError> {
        self.submit_and_present_after(output, custody, Vec::new(), None, || {})
    }

    #[allow(
        dead_code,
        reason = "Retained startup submission entry with first-frame completion and reveal ordering"
    )]
    pub(crate) fn submit_and_present_startup(
        &self,
        output: MultiTargetRenderOutput,
        reveal: impl FnOnce(),
    ) {
        if let Err(error)=self.submit_and_present_startup_with_sdf_custody(output, Vec::new(), reveal) {
            tracing::error!(?error,"HAL385 startup output submission refused");
        }
    }

    #[allow(
        dead_code,
        reason = "Retained startup submission entry with first-frame completion and reveal ordering"
    )]
    fn submit_and_present_startup_with_sdf_custody(
        &self,
        output: MultiTargetRenderOutput,
        custody: Vec<SdfImageCpuUse>,
        reveal: impl FnOnce(),
    ) -> Result<(), SdfImageCustodyError> {
        self.submit_and_present_after(output, custody, Vec::new(), None, || {
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("SPEC-HAL-018 §16: first-frame GPU completion failed");
            reveal();
        })
    }

    pub(crate) fn submit_sdf_output(
        &self,
        output: SdfRenderOutput,
    ) -> Result<(), SdfFrameCustodyError> {
        self.submit_and_present_after(
            output.output,
            output.custody,
            output.presence,
            output.companion,
            || {},
        )
        .map_err(Into::into)
    }

    pub(crate) fn submit_sdf_output_startup(
        &self,
        output: SdfRenderOutput,
        reveal: impl FnOnce(),
    ) -> Result<(), SdfFrameCustodyError> {
        self.submit_and_present_after(
            output.output,
            output.custody,
            output.presence,
            output.companion,
            || {
                self.device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .expect("SPEC-HAL-018 §16: first-frame GPU completion failed");
                reveal();
            },
        )
        .map_err(Into::into)
    }

    fn submit_and_present_after(
        &self,
        output: MultiTargetRenderOutput,
        custody: Vec<SdfImageCpuUse>,
        presence: Vec<PresenceFrameUse>,
        companion: Option<spatial_hud::PresentationCompanionUse>,
        before_present: impl FnOnce(),
    ) -> Result<(), SdfImageCustodyError> {
        let (commands,surface_output)=output.into_submission_parts();
        let submitted =
            self.submit_frame_command_with_receipt(commands, custody, || {
                if let Some(companion) = companion {
                    companion.retire_submitted(&self.queue);
                }
            });
        if !presence.is_empty() {
            // Register at the actual submit owner before presentation, including
            // a typed post-submit SDF failure. No read lease escapes early.
            crate::renderer::on_submitted_work_done(&self.queue, move || drop(presence));
        }
        submitted?;
        #[cfg(feature = "render-audit")]
        super::cadence_audit::record_surface_submission();
        before_present();
        self.queue.present(surface_output);
        #[cfg(feature = "render-audit")]
        super::cadence_audit::record_surface_presentation();
        Ok(())
    }

    /// The command is the ordinary frame output, never a separate Image submit.
    /// Frame owners supply only the exact generations sampled by that command.
    #[cfg(test)]
    pub(super) fn submit_frame_command_with_sdf_custody(
        &self,
        command: crate::renderer::frosted_glass::EncodedTargetCommands,
        custody: Vec<SdfImageCpuUse>,
    ) -> Result<Vec<SdfImageSamplingCompletion>, SdfImageCustodyError> {
        self.submit_frame_command_with_receipt(command, custody, || {})
    }

    fn submit_frame_command_with_receipt(
        &self,
        command: crate::renderer::frosted_glass::EncodedTargetCommands,
        mut custody: Vec<SdfImageCpuUse>,
        submitted: impl FnOnce(),
    ) -> Result<Vec<SdfImageSamplingCompletion>, SdfImageCustodyError> {
        command.validate_submission(&self.queue)?;
        for usage in &custody {
            usage.validate_submission(&self.queue)?;
        }
        let mut completions = Vec::with_capacity(custody.len());
        for usage in &mut custody {
            usage.begin_submission();
        }
        command.submit(&self.queue)?;
        submitted();
        for usage in custody {
            // An error drops remaining handoffs as Uncertified, never as
            // no-submit cancellations of work already sent to the queue.
            completions.push(usage.register_submitted(&self.queue)?);
        }
        Ok(completions)
    }

    fn align_buffer_upload_size(value: u64, alignment: u64) -> u64 {
        debug_assert!(alignment.is_power_of_two());
        (value + alignment - 1) & !(alignment - 1)
    }

    /// SPEC-HAL-040 Stage 4: Stage a CPU→GPU buffer upload without blocking the frame.
    fn stage_buffer_upload(
        &mut self,
        destination: wgpu::Buffer,
        destination_offset: u64,
        data: &[u8],
    ) {
        stage_buffer_upload_parts(
            &self.device,
            &self.queue,
            &mut self.gpu_arena,
            &mut self.staging_pool,
            self.frame_epoch,
            &mut self.staging_upload_encoder,
            &mut self.staging_upload_handles,
            &destination,
            destination_offset,
            data,
        );
    }

    fn flush_staging_uploads(&mut self) {
        let Some(encoder) = self.staging_upload_encoder.take() else {
            debug_assert!(self.staging_upload_handles.is_empty());
            return;
        };

        let handles = std::mem::take(&mut self.staging_upload_handles);
        if handles.is_empty() {
            return;
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        let done = Arc::new(AtomicBool::new(false));
        let done_for_callback = done.clone();
        crate::renderer::on_submitted_work_done(&self.queue, move || {
            done_for_callback.store(true, Ordering::Release);
        });

        self.pending_staging_submissions
            .push(PendingStagingSubmission { done, handles });
    }

    fn poll_staging_recycles(&mut self) {
        let _ = self.device.poll(wgpu::PollType::Poll);

        let mut pending_submissions = Vec::new();
        for pending in self.pending_staging_submissions.drain(..) {
            if pending.done.load(Ordering::Acquire) {
                for handle in pending.handles {
                    let done = Arc::new(AtomicBool::new(false));
                    let succeeded = Arc::new(AtomicBool::new(false));
                    let done_for_callback = done.clone();
                    let succeeded_for_callback = succeeded.clone();

                    handle.buffer.slice(0..handle.size).map_async(
                        wgpu::MapMode::Write,
                        move |result| {
                            succeeded_for_callback.store(result.is_ok(), Ordering::Release);
                            done_for_callback.store(true, Ordering::Release);
                        },
                    );

                    self.pending_staging_remaps.push(PendingStagingRemap {
                        done,
                        succeeded,
                        handle,
                    });
                }
            } else {
                pending_submissions.push(pending);
            }
        }
        self.pending_staging_submissions = pending_submissions;

        let _ = self.device.poll(wgpu::PollType::Poll);

        let mut pending_remaps = Vec::new();
        for pending in self.pending_staging_remaps.drain(..) {
            if pending.done.load(Ordering::Acquire) {
                if pending.succeeded.load(Ordering::Acquire) {
                    self.staging_pool.recycle(pending.handle);
                }
            } else {
                pending_remaps.push(pending);
            }
        }
        self.pending_staging_remaps = pending_remaps;
    }

    /// SPEC-HAL-039: Decide whether we can reuse GPU buffers or must rebuild.
    ///
    /// Strategy v1 ("full_overwrite_same_layout"):
    /// - If there is no previous frame: FullRebuild.
    /// - If batch count or any batch key differs: FullRebuild.
    /// - If vertex/index counts per batch differ: FullRebuild.
    /// - Otherwise: OverwriteRange starting at 0.
    fn build_gpu_update(
        &self,
        prev: Option<&FrameBatches>,
        curr: &FrameBatches,
        vertices: Vec<RectVertex>,
        indices: Vec<u16>,
    ) -> GpuUpdate {
        // No previous layout: full rebuild.
        let Some(prev_batches) = prev else {
            return GpuUpdate::FullRebuild { vertices, indices };
        };

        // Quick structural checks.
        if prev_batches.batches.len() != curr.batches.len() {
            return GpuUpdate::FullRebuild { vertices, indices };
        }

        if prev_batches.total_vertices != curr.total_vertices
            || prev_batches.total_indices != curr.total_indices
        {
            return GpuUpdate::FullRebuild { vertices, indices };
        }

        // Per-batch compatibility.
        for (prev_batch, curr_batch) in prev_batches.batches.iter().zip(curr.batches.iter()) {
            // Batch key must be identical.
            if prev_batch.key != curr_batch.key {
                return GpuUpdate::FullRebuild { vertices, indices };
            }

            // Geometry span must match.
            if prev_batch.vertex_range.count != curr_batch.vertex_range.count
                || prev_batch.index_range.count != curr_batch.index_range.count
            {
                return GpuUpdate::FullRebuild { vertices, indices };
            }
        }

        // Layout is identical: we can reuse buffers and overwrite in-place.
        GpuUpdate::OverwriteRange {
            vertex_offset: 0,
            vertices,
            index_offset: 0,
            indices,
        }
    }

    /// SPEC-HAL-039: Apply a GPU update to persistent buffers.
    ///
    /// - FullRebuild: (re)allocate buffers sized exactly to the data.
    /// - OverwriteRange: write into existing buffers via the staging upload path.
    fn apply_gpu_update(&mut self, update: GpuUpdate) {
        match update {
            GpuUpdate::FullRebuild { vertices, indices } => {
                let vertex_count = vertices.len() as u32;
                let index_count = indices.len() as u32;

                // If there is nothing to draw, drop buffers and return.
                if vertex_count == 0 || index_count == 0 {
                    self.vertex_buffer = None;
                    self.index_buffer = None;
                    self.vertex_capacity = 0;
                    self.index_capacity = 0;
                    return;
                }

                let vertex_size = std::mem::size_of::<RectVertex>() as u64;
                let index_size = std::mem::size_of::<u16>() as u64;

                let vertex_buffer_size = vertex_size * vertex_count as u64;
                let index_buffer_size = index_size * index_count as u64;

                self.vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Batched Rect Vertex Buffer"),
                    size: vertex_buffer_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));

                self.index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Batched Rect Index Buffer"),
                    size: index_buffer_size,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));

                if let Some(vb) = self.vertex_buffer.clone() {
                    self.stage_buffer_upload(vb, 0, bytemuck::cast_slice(&vertices));
                }
                if let Some(ib) = self.index_buffer.clone() {
                    self.stage_buffer_upload(ib, 0, bytemuck::cast_slice(&indices));
                }

                self.vertex_capacity = vertex_count;
                self.index_capacity = index_count;
            }

            GpuUpdate::OverwriteRange {
                vertex_offset,
                vertices,
                index_offset,
                indices,
            } => {
                // If there are no buffers yet or capacity is insufficient, fall back.
                let vertex_count = vertices.len() as u32;
                let index_count = indices.len() as u32;

                let need_vertex = vertex_offset + vertex_count;
                let need_index = index_offset + index_count;

                let can_reuse = matches!(
                    (&self.vertex_buffer, &self.index_buffer),
                    (Some(_), Some(_))
                ) && need_vertex <= self.vertex_capacity
                    && need_index <= self.index_capacity;

                if !can_reuse {
                    // Fallback: treat as full rebuild.
                    return self.apply_gpu_update(GpuUpdate::FullRebuild { vertices, indices });
                }

                if let Some(vb) = self.vertex_buffer.clone() {
                    let byte_offset =
                        vertex_offset as u64 * std::mem::size_of::<RectVertex>() as u64;
                    self.stage_buffer_upload(vb, byte_offset, bytemuck::cast_slice(&vertices));
                }

                if let Some(ib) = self.index_buffer.clone() {
                    let byte_offset = index_offset as u64 * std::mem::size_of::<u16>() as u64;
                    self.stage_buffer_upload(ib, byte_offset, bytemuck::cast_slice(&indices));
                }
            }
        }
    }

    fn extend_rebased_u16_indices(target: &mut Vec<u16>, indices: Vec<u16>, vertex_base: u32) {
        target.extend(indices.into_iter().map(|index| {
            let rebased = index as u32 + vertex_base;
            debug_assert!(
                rebased <= u16::MAX as u32,
                "multi-surface u16 index overflow: {} + {}",
                index,
                vertex_base
            );
            rebased as u16
        }));
    }

    fn extend_rebased_u32_indices(target: &mut Vec<u32>, indices: Vec<u32>, vertex_base: u32) {
        target.extend(indices.into_iter().map(|index| index + vertex_base));
    }

    /// SPEC-HAL-029: Build text vertex and index buffers from text quads.
    /// SPEC-HAL-064 Phase 5: Now computes screen_uv on CPU for pixel-perfect background sampling.
    /// INV-SIZE-001: Takes explicit parameters for per-target rendering.
    fn build_text_buffers(
        text_quads: &[TextQuad],
        scale_factor: f32,
        physical_width: u32,
        physical_height: u32,
    ) -> (Vec<TextVertex>, Vec<u32>) {
        let mut vertices = Vec::with_capacity(text_quads.len() * 4);
        let mut indices = Vec::with_capacity(text_quads.len() * 6);

        let phys_w = physical_width as f32;
        let phys_h = physical_height as f32;

        for quad in text_quads {
            // Use text_quad_to_ndc_for_target for pixel-perfect text rendering
            // This converts using physical dimensions to avoid rounding errors
            let (x0, y0, x1, y1) = Self::text_quad_to_ndc_for_target(
                &quad.pos,
                scale_factor,
                physical_width,
                physical_height,
            );
            let base = vertices.len() as u32;
            let color_array = [quad.color.r, quad.color.g, quad.color.b, quad.color.a];

            // UV coordinates (atlas space)
            let u0 = quad.uv.x;
            let v0 = quad.uv.y;
            let u1 = quad.uv.x + quad.uv.width;
            let v1 = quad.uv.y + quad.uv.height;

            // SPEC-HAL-064 Phase 5: Compute screen_uv on CPU for pixel-perfect background sampling
            // Convert quad position to physical pixels, then normalize to [0,1]
            let px_x = quad.pos.x * scale_factor;
            let px_y = quad.pos.y * scale_factor;
            let px_w = quad.pos.width * scale_factor;
            let px_h = quad.pos.height * scale_factor;

            let su0 = px_x / phys_w;
            let sv0 = px_y / phys_h;
            let su1 = (px_x + px_w) / phys_w;
            let sv1 = (px_y + px_h) / phys_h;

            // Add vertices (top-left, top-right, bottom-right, bottom-left)
            // R-010B FIX: Include per-glyph downsample_u
            vertices.push(TextVertex {
                position: [x0, y0],
                uv: [u0, v0],
                color: color_array,
                screen_uv: [su0, sv0],
                downsample_u: quad.downsample_u,
            });
            vertices.push(TextVertex {
                position: [x1, y0],
                uv: [u1, v0],
                color: color_array,
                screen_uv: [su1, sv0],
                downsample_u: quad.downsample_u,
            });
            vertices.push(TextVertex {
                position: [x1, y1],
                uv: [u1, v1],
                color: color_array,
                screen_uv: [su1, sv1],
                downsample_u: quad.downsample_u,
            });
            vertices.push(TextVertex {
                position: [x0, y1],
                uv: [u0, v1],
                color: color_array,
                screen_uv: [su0, sv1],
                downsample_u: quad.downsample_u,
            });

            // Add indices for two triangles
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        (vertices, indices)
    }

    #[cfg(test)]
    pub(crate) fn build_text_index_values_for_test(
        text_quads: &[TextQuad],
        scale_factor: f32,
        physical_width: u32,
        physical_height: u32,
    ) -> (usize, Vec<u32>) {
        let (vertices, indices) =
            Self::build_text_buffers(text_quads, scale_factor, physical_width, physical_height);
        (vertices.len(), indices.into_iter().map(u32::from).collect())
    }

    /// SPEC-HAL-029: Update text GPU buffers.
    fn update_text_gpu_buffers(&mut self, vertices: &[TextVertex], indices: &[u32]) {
        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        // Skip if nothing to render
        if vertex_count == 0 || index_count == 0 {
            return;
        }

        // Check if we need to reallocate
        let need_realloc = self.text_vertex_buffer.is_none()
            || vertex_count > self.text_vertex_capacity
            || index_count > self.text_index_capacity;

        if need_realloc {
            // Allocate with some extra capacity to avoid frequent reallocations
            let new_vertex_capacity = (vertex_count * 3 / 2).max(256);
            let new_index_capacity = (index_count * 3 / 2).max(512);

            self.text_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Text Vertex Buffer"),
                size: (new_vertex_capacity as usize * std::mem::size_of::<TextVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.text_index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Text Index Buffer"),
                size: (new_index_capacity as usize * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.text_vertex_capacity = new_vertex_capacity;
            self.text_index_capacity = new_index_capacity;
        }

        // Upload data
        if let Some(vb) = self.text_vertex_buffer.clone() {
            self.stage_buffer_upload(vb, 0, bytemuck::cast_slice(vertices));
        }
        if let Some(ib) = self.text_index_buffer.clone() {
            self.stage_buffer_upload(ib, 0, bytemuck::cast_slice(indices));
        }
    }

    fn add_rect(
        &self,
        rect: &RectF,
        color: &Color,
        vertices: &mut Vec<RectVertex>,
        indices: &mut Vec<u16>,
    ) {
        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.x + rect.width;
        let y1 = rect.y + rect.height;

        let base = vertices.len() as u16;
        let color_array = [color.r, color.g, color.b, color.a];

        vertices.push(RectVertex {
            position: [x0, y0],
            color: color_array,
        });
        vertices.push(RectVertex {
            position: [x1, y0],
            color: color_array,
        });
        vertices.push(RectVertex {
            position: [x1, y1],
            color: color_array,
        });
        vertices.push(RectVertex {
            position: [x0, y1],
            color: color_array,
        });

        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// SPEC-HAL-263: Emit 4 image vertices + 6 indices for one image quad.
    fn add_image_quad(
        rect: &RectF,
        uv_rect: &RectF,
        tint: &Color,
        vertices: &mut Vec<ImageVertex>,
        indices: &mut Vec<u16>,
    ) {
        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.x + rect.width;
        let y1 = rect.y + rect.height;

        let u0 = uv_rect.x;
        let v0 = uv_rect.y;
        let u1 = uv_rect.x + uv_rect.width;
        let v1 = uv_rect.y + uv_rect.height;

        let color = [tint.r, tint.g, tint.b, tint.a];
        let base = vertices.len() as u16;

        vertices.push(ImageVertex {
            position: [x0, y0],
            uv: [u0, v0],
            color,
        });
        vertices.push(ImageVertex {
            position: [x1, y0],
            uv: [u1, v0],
            color,
        });
        vertices.push(ImageVertex {
            position: [x1, y1],
            uv: [u1, v1],
            color,
        });
        vertices.push(ImageVertex {
            position: [x0, y1],
            uv: [u0, v1],
            color,
        });

        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// SPEC-HAL-263: Build image vertex/index buffers from a surface's primitives.
    fn build_image_buffers_from_surface_primitives(
        primitives: &[crate::renderer::batcher::FlatPrimitive],
    ) -> (Vec<ImageVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for prim in primitives.iter() {
            if let PrimitiveData::Image {
                rect,
                uv_rect,
                tint,
                ..
            } = &prim.data
            {
                Self::add_image_quad(rect, uv_rect, tint, &mut vertices, &mut indices);
            }
        }

        (vertices, indices)
    }

    /// SPEC-HAL-263: Upload image vertex/index data to dedicated GPU buffers.
    fn update_image_gpu_buffers(&mut self, vertices: &[ImageVertex], indices: &[u16]) {
        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        if vertex_count == 0 || index_count == 0 {
            return;
        }

        let need_realloc = self.image_vertex_buffer.is_none()
            || vertex_count > self.image_vertex_capacity
            || index_count > self.image_index_capacity;

        if need_realloc {
            let new_vertex_capacity = (vertex_count * 3 / 2).max(64);
            let new_index_capacity = (index_count * 3 / 2).max(128);

            self.image_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Image Vertex Buffer"),
                size: (new_vertex_capacity as usize * std::mem::size_of::<ImageVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.image_index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Image Index Buffer"),
                size: (new_index_capacity as usize * std::mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.image_vertex_capacity = new_vertex_capacity;
            self.image_index_capacity = new_index_capacity;
        }

        if let Some(vb) = self.image_vertex_buffer.clone() {
            self.stage_buffer_upload(vb, 0, bytemuck::cast_slice(vertices));
        }
        if let Some(ib) = self.image_index_buffer.clone() {
            self.stage_buffer_upload(ib, 0, bytemuck::cast_slice(indices));
        }
    }

    /// SPEC-R-013-v2: Build shadow vertex/index buffers AND collect unique shadow params.
    fn build_shadow_buffers_from_surface_primitives(
        primitives: &[crate::renderer::batcher::FlatPrimitive],
    ) -> (Vec<ShadowVertex>, Vec<u16>, Vec<(u64, ShadowParamsUniform)>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut unique_params: std::collections::HashMap<u64, ShadowParamsUniform> =
            std::collections::HashMap::new();

        for prim in primitives.iter() {
            if let PrimitiveData::Shadow {
                quad_rect,
                color,
                offset,
                params,
            } = &prim.data
            {
                // Collect unique params
                let hash = params.batch_hash();
                unique_params
                    .entry(hash)
                    .or_insert_with(|| ShadowParamsUniform {
                        shape_center: [params.shape_center.0, params.shape_center.1],
                        shape_half_size: [params.shape_half_size.0, params.shape_half_size.1],
                        shape_radius: params.shape_radius,
                        sigma: params.sigma,
                        mode: params.mode,
                        spread: params.spread,
                    });

                let base = vertices.len() as u16;
                let c = [
                    color.r * color.a,
                    color.g * color.a,
                    color.b * color.a,
                    color.a,
                ];
                let off = [offset.0, offset.1];

                // 4 vertices: top-left, top-right, bottom-right, bottom-left
                vertices.push(ShadowVertex::new([quad_rect.x, quad_rect.y], c, off));
                vertices.push(ShadowVertex::new(
                    [quad_rect.x + quad_rect.width, quad_rect.y],
                    c,
                    off,
                ));
                vertices.push(ShadowVertex::new(
                    [
                        quad_rect.x + quad_rect.width,
                        quad_rect.y + quad_rect.height,
                    ],
                    c,
                    off,
                ));
                vertices.push(ShadowVertex::new(
                    [quad_rect.x, quad_rect.y + quad_rect.height],
                    c,
                    off,
                ));

                // 2 triangles
                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
                indices.push(base + 2);
                indices.push(base + 3);
                indices.push(base);
            }
        }

        let params_vec: Vec<(u64, ShadowParamsUniform)> = unique_params.into_iter().collect();
        (vertices, indices, params_vec)
    }

    /// SPEC-R-013-v2: Upload shadow vertex/index data to dedicated GPU buffers.
    fn update_shadow_gpu_buffers(&mut self, vertices: &[ShadowVertex], indices: &[u16]) {
        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        if vertex_count == 0 || index_count == 0 {
            return;
        }

        let need_realloc = self.shadow_vertex_buffer.is_none()
            || vertex_count > self.shadow_vertex_capacity
            || index_count > self.shadow_index_capacity;

        if need_realloc {
            let new_vertex_capacity = (vertex_count * 3 / 2).max(64);
            let new_index_capacity = (index_count * 3 / 2).max(128);

            self.shadow_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Shadow Vertex Buffer"),
                size: (new_vertex_capacity as usize * std::mem::size_of::<ShadowVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.shadow_index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Shadow Index Buffer"),
                size: (new_index_capacity as usize * std::mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.shadow_vertex_capacity = new_vertex_capacity;
            self.shadow_index_capacity = new_index_capacity;
        }

        if let Some(vb) = self.shadow_vertex_buffer.clone() {
            self.stage_buffer_upload(vb, 0, bytemuck::cast_slice(vertices));
        }
        if let Some(ib) = self.shadow_index_buffer.clone() {
            self.stage_buffer_upload(ib, 0, bytemuck::cast_slice(indices));
        }
    }

    /// SPEC-R-013-v2: Upload unique shadow params to uniform buffer.
    /// Returns hash→byte_offset map for dynamic binding.
    fn upload_shadow_params(
        &mut self,
        unique_params: &[(u64, ShadowParamsUniform)],
    ) -> std::collections::HashMap<u64, u32> {
        use std::collections::HashMap;

        let mut offsets: HashMap<u64, u32> = HashMap::new();
        let mut buffer_data: Vec<u8> = Vec::new();

        for (idx, (hash, uniform)) in unique_params.iter().enumerate() {
            if idx >= MAX_SHADOW_PARAMS_PER_FRAME {
                break;
            }

            let offset = (idx as u64 * ROUNDED_CLIP_UNIFORM_ALIGNMENT) as u32;
            offsets.insert(*hash, offset);

            let uniform_bytes = bytemuck::bytes_of(uniform);
            buffer_data.extend_from_slice(uniform_bytes);
            // Pad to alignment
            let aligned = ((buffer_data.len() as u64 + ROUNDED_CLIP_UNIFORM_ALIGNMENT - 1)
                / ROUNDED_CLIP_UNIFORM_ALIGNMENT
                * ROUNDED_CLIP_UNIFORM_ALIGNMENT) as usize;
            buffer_data.resize(aligned, 0);
        }

        if !buffer_data.is_empty() {
            self.stage_buffer_upload(self.shadow_params_buffer.clone(), 0, &buffer_data);
        }

        offsets
    }

    /// SPEC-R-012-v2: Build gradient vertex/index buffers AND collect unique gradient params.
    fn build_gradient_buffers_from_surface_primitives(
        primitives: &[crate::renderer::batcher::FlatPrimitive],
        clip_aa_radius: f32,
    ) -> (
        Vec<GradientVertex>,
        Vec<u16>,
        Vec<(u64, GradientParamsUniform)>,
    ) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut unique_params: std::collections::HashMap<u64, GradientParamsUniform> =
            std::collections::HashMap::new();

        for prim in primitives.iter() {
            if let PrimitiveData::GradientRect { rect, params, .. } = &prim.data {
                // Collect unique params
                let hash = params.batch_hash_with_clip(prim.clip_state.hash);
                unique_params.entry(hash).or_insert_with(|| {
                    let mut stops = [GradientStopUniform {
                        color: [0.0; 4],
                        offset: 0.0,
                        _pad0: 0.0,
                        _pad1: 0.0,
                        _pad2: 0.0,
                    }; 8];
                    for (i, stop) in params.stops.iter().take(8).enumerate() {
                        stops[i] = GradientStopUniform {
                            color: stop.color,
                            offset: stop.offset,
                            _pad0: 0.0,
                            _pad1: 0.0,
                            _pad2: 0.0,
                        };
                    }
                    GradientParamsUniform {
                        rect_origin: [params.rect_origin.0, params.rect_origin.1],
                        rect_size: [params.rect_size.0, params.rect_size.1],
                        p0: [params.p0.0, params.p0.1],
                        p1: [params.p1.0, params.p1.1],
                        corner_radius: params.corner_radius,
                        kind: params.kind,
                        stop_count: params.stops.len().min(8) as u32,
                        clip_enabled: u32::from(!prim.clip_state.is_none()),
                        stops,
                        clip: RoundedClipUniform::new(
                            [
                                prim.clip_state.rect.x,
                                prim.clip_state.rect.y,
                                prim.clip_state.rect.width,
                                prim.clip_state.rect.height,
                            ],
                            prim.clip_state
                                .rounded
                                .as_ref()
                                .map_or([0.0; 4], |r| [r.tl, r.tr, r.bl, r.br]),
                            clip_aa_radius,
                        ),
                    }
                });

                // Build quad vertices
                let base = vertices.len() as u16;
                vertices.push(GradientVertex::new([rect.x, rect.y]));
                vertices.push(GradientVertex::new([rect.x + rect.width, rect.y]));
                vertices.push(GradientVertex::new([
                    rect.x + rect.width,
                    rect.y + rect.height,
                ]));
                vertices.push(GradientVertex::new([rect.x, rect.y + rect.height]));

                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
                indices.push(base + 2);
                indices.push(base + 3);
                indices.push(base);
            }
        }

        let mut params_vec: Vec<(u64, GradientParamsUniform)> = unique_params.into_iter().collect();
        params_vec.sort_by_key(|(hash, _)| *hash);
        (vertices, indices, params_vec)
    }

    /// SPEC-R-012-v2: Upload gradient vertex/index data to dedicated GPU buffers.
    fn update_gradient_gpu_buffers(&mut self, vertices: &[GradientVertex], indices: &[u16]) {
        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        if vertex_count == 0 || index_count == 0 {
            return;
        }

        let need_realloc = self.gradient_vertex_buffer.is_none()
            || vertex_count > self.gradient_vertex_capacity
            || index_count > self.gradient_index_capacity;

        if need_realloc {
            let new_vertex_capacity = (vertex_count * 3 / 2).max(64);
            let new_index_capacity = (index_count * 3 / 2).max(128);

            self.gradient_vertex_buffer =
                Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Gradient Vertex Buffer"),
                    size: (new_vertex_capacity as usize * std::mem::size_of::<GradientVertex>())
                        as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));

            self.gradient_index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Gradient Index Buffer"),
                size: (new_index_capacity as usize * std::mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.gradient_vertex_capacity = new_vertex_capacity;
            self.gradient_index_capacity = new_index_capacity;
        }

        if let Some(vb) = self.gradient_vertex_buffer.clone() {
            self.stage_buffer_upload(vb, 0, bytemuck::cast_slice(vertices));
        }
        if let Some(ib) = self.gradient_index_buffer.clone() {
            self.stage_buffer_upload(ib, 0, bytemuck::cast_slice(indices));
        }
    }

    /// SPEC-R-012-v2: Upload unique gradient params to uniform buffer.
    fn upload_gradient_params(
        &mut self,
        unique_params: &[(u64, GradientParamsUniform)],
    ) -> std::collections::HashMap<u64, u32> {
        use std::collections::HashMap;

        let mut offsets: HashMap<u64, u32> = HashMap::new();
        let mut buffer_data: Vec<u8> = Vec::new();

        for (idx, (hash, uniform)) in unique_params.iter().enumerate() {
            if idx >= MAX_GRADIENT_PARAMS_PER_FRAME {
                break;
            }

            let offset = (idx as u64 * GRADIENT_PARAMS_UNIFORM_ALIGNMENT) as u32;
            offsets.insert(*hash, offset);

            let uniform_bytes = bytemuck::bytes_of(uniform);
            buffer_data.extend_from_slice(uniform_bytes);
            let aligned = ((buffer_data.len() as u64 + GRADIENT_PARAMS_UNIFORM_ALIGNMENT - 1)
                / GRADIENT_PARAMS_UNIFORM_ALIGNMENT
                * GRADIENT_PARAMS_UNIFORM_ALIGNMENT) as usize;
            buffer_data.resize(aligned, 0);
        }

        if !buffer_data.is_empty() {
            self.stage_buffer_upload(self.gradient_params_buffer.clone(), 0, &buffer_data);
        }

        offsets
    }

    /// SPEC-HAL-272: Build CPU-side vertex/index buffers and params map for all
    /// ColorPickerSv primitives in the given primitive list.
    fn build_color_picker_sv_buffers_from_surface_primitives(
        primitives: &[crate::renderer::batcher::FlatPrimitive],
        scale_factor: f32,
    ) -> (
        Vec<ColorPickerSvVertex>,
        Vec<u32>,
        std::collections::HashMap<u64, (u32, ColorPickerSvParamsUniform)>,
    ) {
        use crate::renderer::batcher::PrimitiveData;

        let mut vertices: Vec<ColorPickerSvVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut params_map: std::collections::HashMap<u64, (u32, ColorPickerSvParamsUniform)> =
            std::collections::HashMap::new();
        let mut param_slot: u32 = 0;

        let sx = scale_factor;
        let sy = scale_factor;

        for prim in primitives {
            if let PrimitiveData::ColorPickerSv { rect, hue, radius } = &prim.data {
                let hue_bits = hue.to_bits() as u64;
                params_map.entry(hue_bits).or_insert_with(|| {
                    let slot = param_slot;
                    param_slot += 1;
                    let params = ColorPickerSvParamsUniform {
                        rect_origin: [rect.x * sx, rect.y * sy],
                        rect_size: [rect.width * sx, rect.height * sy],
                        hue: *hue,
                        corner_radius: *radius * sx,
                        _pad: [0.0; 2],
                    };
                    (slot, params)
                });

                let base = vertices.len() as u32;
                vertices.extend_from_slice(&[
                    ColorPickerSvVertex {
                        position: [rect.x * sx, rect.y * sy],
                    },
                    ColorPickerSvVertex {
                        position: [(rect.x + rect.width) * sx, rect.y * sy],
                    },
                    ColorPickerSvVertex {
                        position: [(rect.x + rect.width) * sx, (rect.y + rect.height) * sy],
                    },
                    ColorPickerSvVertex {
                        position: [rect.x * sx, (rect.y + rect.height) * sy],
                    },
                ]);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }

        (vertices, indices, params_map)
    }

    /// SPEC-HAL-272: Upload ColorPickerSv vertex/index data to GPU, lazily reallocating.
    fn update_color_picker_sv_gpu_buffers(
        &mut self,
        vertices: &[ColorPickerSvVertex],
        indices: &[u32],
    ) {
        if vertices.is_empty() {
            return;
        }
        let needed_verts = vertices.len() as u32;
        let needed_idxs = indices.len() as u32;

        if self.color_picker_sv_vertex_buffer.is_none()
            || self.color_picker_sv_vertex_capacity < needed_verts
        {
            let cap = (needed_verts * 3 / 2).max(64);
            self.color_picker_sv_vertex_buffer =
                Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ColorPicker SV Vertex Buffer"),
                    size: cap as u64 * std::mem::size_of::<ColorPickerSvVertex>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.color_picker_sv_vertex_capacity = cap;
        }
        if self.color_picker_sv_index_buffer.is_none()
            || self.color_picker_sv_index_capacity < needed_idxs
        {
            let cap = (needed_idxs * 3 / 2).max(64);
            self.color_picker_sv_index_buffer =
                Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ColorPicker SV Index Buffer"),
                    size: cap as u64 * std::mem::size_of::<u32>() as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.color_picker_sv_index_capacity = cap;
        }
        self.stage_buffer_upload(
            self.color_picker_sv_vertex_buffer.as_ref().unwrap().clone(),
            0,
            bytemuck::cast_slice(vertices),
        );
        self.stage_buffer_upload(
            self.color_picker_sv_index_buffer.as_ref().unwrap().clone(),
            0,
            bytemuck::cast_slice(indices),
        );
    }

    /// SPEC-HAL-272: Write ColorPickerSv params to the uniform buffer.
    /// Returns hue_bits → dynamic_offset_bytes map for draw-time binding.
    fn upload_color_picker_sv_params(
        &mut self,
        params_map: &std::collections::HashMap<u64, (u32, ColorPickerSvParamsUniform)>,
    ) -> std::collections::HashMap<u64, u32> {
        let mut offsets: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        for (hue_bits, (slot, params)) in params_map {
            let byte_offset = *slot as u64 * COLOR_PICKER_SV_PARAMS_UNIFORM_ALIGNMENT;
            let mut padded = [0u8; COLOR_PICKER_SV_PARAMS_UNIFORM_ALIGNMENT as usize];
            let src = bytemuck::bytes_of(params);
            padded[..src.len()].copy_from_slice(src);
            self.stage_buffer_upload(
                self.color_picker_sv_params_buffer.clone(),
                byte_offset,
                &padded,
            );
            offsets.insert(*hue_bits, byte_offset as u32);
        }
        offsets
    }

    /// SPEC-HAL-278: Build CPU-side vertex/index buffers and params map for all
    /// RadialGauge primitives in the given primitive list.
    fn build_radial_gauge_buffers_from_surface_primitives(
        primitives: &[crate::renderer::batcher::FlatPrimitive],
        _scale_factor: f32,
    ) -> (
        Vec<RadialGaugeVertex>,
        Vec<u32>,
        std::collections::HashMap<u64, (u32, RadialGaugeParamsUniform)>,
    ) {
        use crate::renderer::batcher::PrimitiveData;

        let mut vertices: Vec<RadialGaugeVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut params_map: std::collections::HashMap<u64, (u32, RadialGaugeParamsUniform)> =
            std::collections::HashMap::new();
        let mut param_slot: u32 = 0;

        for prim in primitives {
            if let PrimitiveData::RadialGauge {
                rect,
                fill,
                thickness,
                track_color,
                fill_color,
                fill_color_negative,
                start_angle,
                sweep_angle,
                origin_fill,
                mode,
                zone_count,
                zones,
                fill_style,
                major_divisions,
                minor_per_major,
                tick_length_major,
                tick_length_minor,
                tick_color,
            } = &prim.data
            {
                let param_hash = prim.key.texture_id.unwrap_or(0);
                params_map.entry(param_hash).or_insert_with(|| {
                    let slot = param_slot;
                    param_slot += 1;
                    let z = zones;
                    let params = RadialGaugeParamsUniform {
                        track_color: *track_color,
                        fill_color: *fill_color,
                        rect_origin: [rect.x, rect.y],
                        rect_size: [rect.width, rect.height],
                        fill: *fill,
                        thickness: *thickness,
                        start_angle: *start_angle,
                        sweep_angle: *sweep_angle,
                        origin_fill: *origin_fill,
                        zone_count: *zone_count,
                        mode: *mode,
                        fill_style: *fill_style,
                        zone0_range: [z[0].0, z[0].1, 0.0, 0.0],
                        zone0_color: z[0].2,
                        zone1_range: [z[1].0, z[1].1, 0.0, 0.0],
                        zone1_color: z[1].2,
                        zone2_range: [z[2].0, z[2].1, 0.0, 0.0],
                        zone2_color: z[2].2,
                        zone3_range: [z[3].0, z[3].1, 0.0, 0.0],
                        zone3_color: z[3].2,
                        major_divisions: *major_divisions,
                        minor_per_major: *minor_per_major,
                        tick_length_major: *tick_length_major,
                        tick_length_minor: *tick_length_minor,
                        tick_color: *tick_color,
                        fill_color_negative: *fill_color_negative,
                    };
                    (slot, params)
                });

                let base = vertices.len() as u32;
                vertices.extend_from_slice(&[
                    RadialGaugeVertex {
                        position: [0.0, 0.0],
                    },
                    RadialGaugeVertex {
                        position: [1.0, 0.0],
                    },
                    RadialGaugeVertex {
                        position: [1.0, 1.0],
                    },
                    RadialGaugeVertex {
                        position: [0.0, 1.0],
                    },
                ]);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }

        (vertices, indices, params_map)
    }

    /// SPEC-HAL-278: Upload RadialGauge vertex/index data to GPU, lazily reallocating.
    fn update_radial_gauge_gpu_buffers(&mut self, vertices: &[RadialGaugeVertex], indices: &[u32]) {
        if vertices.is_empty() {
            return;
        }
        let needed_verts = vertices.len() as u32;
        let needed_idxs = indices.len() as u32;

        if self.radial_gauge_vertex_buffer.is_none()
            || self.radial_gauge_vertex_capacity < needed_verts
        {
            let cap = (needed_verts * 3 / 2).max(64);
            self.radial_gauge_vertex_buffer =
                Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("RadialGauge Vertex Buffer"),
                    size: cap as u64 * std::mem::size_of::<RadialGaugeVertex>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.radial_gauge_vertex_capacity = cap;
        }
        if self.radial_gauge_index_buffer.is_none()
            || self.radial_gauge_index_capacity < needed_idxs
        {
            let cap = (needed_idxs * 3 / 2).max(64);
            self.radial_gauge_index_buffer =
                Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("RadialGauge Index Buffer"),
                    size: cap as u64 * std::mem::size_of::<u32>() as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.radial_gauge_index_capacity = cap;
        }
        self.stage_buffer_upload(
            self.radial_gauge_vertex_buffer.as_ref().unwrap().clone(),
            0,
            bytemuck::cast_slice(vertices),
        );
        self.stage_buffer_upload(
            self.radial_gauge_index_buffer.as_ref().unwrap().clone(),
            0,
            bytemuck::cast_slice(indices),
        );
    }

    /// SPEC-HAL-278: Write RadialGauge params to the uniform buffer.
    /// Returns param_hash → dynamic_offset_bytes map for draw-time binding.
    fn upload_radial_gauge_params(
        &mut self,
        params_map: &std::collections::HashMap<u64, (u32, RadialGaugeParamsUniform)>,
    ) -> std::collections::HashMap<u64, u32> {
        let mut offsets: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        for (param_hash, (slot, params)) in params_map {
            let byte_offset = *slot as u64 * RADIAL_GAUGE_PARAMS_UNIFORM_ALIGNMENT;
            let mut padded = [0u8; RADIAL_GAUGE_PARAMS_UNIFORM_ALIGNMENT as usize];
            let src = bytemuck::bytes_of(params);
            padded[..src.len()].copy_from_slice(src);
            self.stage_buffer_upload(
                self.radial_gauge_params_buffer.clone(),
                byte_offset,
                &padded,
            );
            offsets.insert(*param_hash, byte_offset as u32);
        }
        offsets
    }

    /// SPEC-R-022: Create the SDF rounded rect pipeline.
    /// Only uses globals at group(0) — all shape params are per-vertex.
    fn create_sdf_rect_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDF Rect Shader"),
            source: wgpu::ShaderSource::Wgsl(SDF_RECT_SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SDF Rect Pipeline Layout"),
            bind_group_layouts: &[Some(globals_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SDF Rect Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(SdfRectVertex::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    /// SPEC-R-022: Build SDF rect vertex/index data from surface primitives.
    fn build_sdf_rect_buffers_from_surface_primitives(
        primitives: &[crate::renderer::batcher::FlatPrimitive],
    ) -> (Vec<SdfRectVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for prim in primitives.iter() {
            if let PrimitiveData::SdfRect {
                quad_rect,
                fill_color,
                border_color,
                rect_center,
                rect_half_size,
                radius,
                border_width,
            } = &prim.data
            {
                let base = vertices.len() as u16;
                let fc = [fill_color.r, fill_color.g, fill_color.b, fill_color.a];
                let bc = [
                    border_color.r,
                    border_color.g,
                    border_color.b,
                    border_color.a,
                ];
                let center = [rect_center.0, rect_center.1];
                let half = [rect_half_size.0, rect_half_size.1];

                let make_vert = |pos: [f32; 2]| SdfRectVertex {
                    position: pos,
                    fill_color: fc,
                    border_color: bc,
                    rect_center: center,
                    rect_half_size: half,
                    radius: *radius,
                    border_width: *border_width,
                };

                vertices.push(make_vert([quad_rect.x, quad_rect.y]));
                vertices.push(make_vert([quad_rect.x + quad_rect.width, quad_rect.y]));
                vertices.push(make_vert([
                    quad_rect.x + quad_rect.width,
                    quad_rect.y + quad_rect.height,
                ]));
                vertices.push(make_vert([quad_rect.x, quad_rect.y + quad_rect.height]));

                indices.push(base);
                indices.push(base + 1);
                indices.push(base + 2);
                indices.push(base + 2);
                indices.push(base + 3);
                indices.push(base);
            }
        }

        (vertices, indices)
    }

    /// SPEC-R-022: Upload SDF rect vertex/index data to dedicated GPU buffers.
    fn update_sdf_rect_gpu_buffers(&mut self, vertices: &[SdfRectVertex], indices: &[u16]) {
        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        if vertex_count == 0 || index_count == 0 {
            return;
        }

        let need_realloc = self.sdf_rect_vertex_buffer.is_none()
            || vertex_count > self.sdf_rect_vertex_capacity
            || index_count > self.sdf_rect_index_capacity;

        if need_realloc {
            let new_vertex_capacity = (vertex_count * 3 / 2).max(64);
            let new_index_capacity = (index_count * 3 / 2).max(128);

            self.sdf_rect_vertex_buffer =
                Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("SDF Rect Vertex Buffer"),
                    size: (new_vertex_capacity as usize * std::mem::size_of::<SdfRectVertex>())
                        as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));

            self.sdf_rect_index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SDF Rect Index Buffer"),
                size: (new_index_capacity as usize * std::mem::size_of::<u16>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.sdf_rect_vertex_capacity = new_vertex_capacity;
            self.sdf_rect_index_capacity = new_index_capacity;
        }

        if let Some(vb) = self.sdf_rect_vertex_buffer.clone() {
            self.stage_buffer_upload(vb, 0, bytemuck::cast_slice(vertices));
        }
        if let Some(ib) = self.sdf_rect_index_buffer.clone() {
            self.stage_buffer_upload(ib, 0, bytemuck::cast_slice(indices));
        }
    }

    fn push_shape_quad(
        vertices: &mut Vec<ShapeVertex>,
        indices: &mut Vec<u32>,
        rect: RectF,
        color: [f32; 4],
        params0: [f32; 4],
        params1: [f32; 4],
        shape_type: ShapeType,
    ) {
        let base = vertices.len() as u32;
        let p0 = [rect.x, rect.y];
        let p1 = [rect.x + rect.width, rect.y];
        let p2 = [rect.x + rect.width, rect.y + rect.height];
        let p3 = [rect.x, rect.y + rect.height];

        vertices.push(ShapeVertex::new(
            p0, p0, color, params0, params1, shape_type,
        ));
        vertices.push(ShapeVertex::new(
            p1, p1, color, params0, params1, shape_type,
        ));
        vertices.push(ShapeVertex::new(
            p2, p2, color, params0, params1, shape_type,
        ));
        vertices.push(ShapeVertex::new(
            p3, p3, color, params0, params1, shape_type,
        ));

        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    fn push_luminous_contour_region(
        vertices: &mut Vec<ShapeVertex>,
        indices: &mut Vec<u32>,
        region: LuminousContourRegion,
        core_width: f32,
        color: [f32; 4],
        appearance: crate::core::paint::LuminousContourAppearance,
        scroll_offset: (f32, f32),
    ) {
        const AA_PAD: f32 = 1.0;
        let a = crate::core::math::Point::new(
            region.start.x - scroll_offset.0,
            region.start.y - scroll_offset.1,
        );
        let b = crate::core::math::Point::new(
            region.end.x - scroll_offset.0,
            region.end.y - scroll_offset.1,
        );
        let half_width = core_width * 0.5;
        let extent = half_width + appearance.halo_radius() + AA_PAD;
        let min_x = a.x.min(b.x) - extent;
        let min_y = a.y.min(b.y) - extent;
        let max_x = a.x.max(b.x) + extent;
        let max_y = a.y.max(b.y) + extent;
        let params0 = [a.x, a.y, b.x, b.y];
        let params1 = [
            half_width,
            appearance.halo_radius(),
            appearance.halo_intensity(),
            appearance.halo_falloff(),
        ];
        let params2 = [
            region.start_ownership_normal.x,
            region.start_ownership_normal.y,
            region.end_ownership_normal.x,
            region.end_ownership_normal.y,
        ];
        let params3 = [
            appearance.core_luminance_lift(),
            appearance.inner_aura_radius(),
            appearance.inner_aura_intensity(),
            appearance.front_opacity_boost(),
        ];
        let contour_gains = [region.start_gain, region.end_gain];
        let positions = [
            [min_x, min_y],
            [max_x, min_y],
            [max_x, max_y],
            [min_x, max_y],
        ];
        let base = vertices.len() as u32;
        for position in positions {
            vertices.push(ShapeVertex::new_luminous_contour(
                position,
                color,
                params0,
                params1,
                params2,
                params3,
                contour_gains,
            ));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }

    fn build_shape_buffers_from_surface_primitives(
        primitives: &[crate::renderer::batcher::FlatPrimitive],
    ) -> (Vec<ShapeVertex>, Vec<u32>) {
        const AA_PAD: f32 = 1.0;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for prim in primitives.iter() {
            match &prim.data {
                PrimitiveData::Circle {
                    center,
                    radius,
                    color,
                } if *radius > 0.0 && color.a >= 0.001 => {
                    let extent = *radius + AA_PAD;
                    let quad = RectF::new(
                        center.x - extent,
                        center.y - extent,
                        extent * 2.0,
                        extent * 2.0,
                    );
                    let color = [color.r, color.g, color.b, color.a];
                    let params0 = [center.x, center.y, *radius, 0.0];
                    Self::push_shape_quad(
                        &mut vertices,
                        &mut indices,
                        quad,
                        color,
                        params0,
                        [0.0; 4],
                        ShapeType::Circle,
                    );
                }
                PrimitiveData::Ellipse {
                    center,
                    radii,
                    color,
                } if radii.x > 0.0 && radii.y > 0.0 && color.a >= 0.001 => {
                    let quad = RectF::new(
                        center.x - radii.x - AA_PAD,
                        center.y - radii.y - AA_PAD,
                        (radii.x + AA_PAD) * 2.0,
                        (radii.y + AA_PAD) * 2.0,
                    );
                    let color = [color.r, color.g, color.b, color.a];
                    let params0 = [center.x, center.y, radii.x, radii.y];
                    Self::push_shape_quad(
                        &mut vertices,
                        &mut indices,
                        quad,
                        color,
                        params0,
                        [0.0; 4],
                        ShapeType::Ellipse,
                    );
                }
                PrimitiveData::Line { a, b, width, color } if *width > 0.0 && color.a >= 0.001 => {
                    let half_width = *width * 0.5;
                    let extent = half_width + AA_PAD;
                    let min_x = a.x.min(b.x) - extent;
                    let min_y = a.y.min(b.y) - extent;
                    let max_x = a.x.max(b.x) + extent;
                    let max_y = a.y.max(b.y) + extent;
                    let quad = RectF::new(min_x, min_y, max_x - min_x, max_y - min_y);
                    let color = [color.r, color.g, color.b, color.a];
                    let params0 = [a.x, a.y, b.x, b.y];
                    let params1 = [half_width, 0.0, 0.0, 0.0];
                    Self::push_shape_quad(
                        &mut vertices,
                        &mut indices,
                        quad,
                        color,
                        params0,
                        params1,
                        ShapeType::Line,
                    );
                }
                PrimitiveData::Polyline {
                    points,
                    width,
                    color,
                } if *width > 0.0 && color.a >= 0.001 && points.len() >= 2 => {
                    let half_width = *width * 0.5;
                    let extent = half_width + AA_PAD;
                    let color = [color.r, color.g, color.b, color.a];
                    let params1 = [half_width, 0.0, 0.0, 0.0];
                    for segment in points.windows(2) {
                        let a = segment[0];
                        let b = segment[1];
                        let min_x = a.x.min(b.x) - extent;
                        let min_y = a.y.min(b.y) - extent;
                        let max_x = a.x.max(b.x) + extent;
                        let max_y = a.y.max(b.y) + extent;
                        let quad = RectF::new(min_x, min_y, max_x - min_x, max_y - min_y);
                        let params0 = [a.x, a.y, b.x, b.y];
                        Self::push_shape_quad(
                            &mut vertices,
                            &mut indices,
                            quad,
                            color,
                            params0,
                            params1,
                            ShapeType::Polyline,
                        );
                    }
                }
                PrimitiveData::LocalAmbientGlow {
                    rect,
                    bounds,
                    appearance,
                } => {
                    let mut centers = [[0.0; 4]; 2];
                    let mut colors = [[0.0; 4]; 2];
                    let mut rotations = [0.0; 4];
                    let mut intensities = [0.0; 2];
                    for (index, lobe) in appearance.lobes().iter().copied().enumerate() {
                        let uv = lobe.center_uv();
                        let radii = lobe.radii();
                        let color = lobe.color();
                        let rotation = lobe.rotation_cos_sin();
                        centers[index] = [
                            rect.x + rect.width * uv.x,
                            rect.y + rect.height * uv.y,
                            radii.x,
                            radii.y,
                        ];
                        colors[index] = [color.r, color.g, color.b, color.a];
                        rotations[index * 2] = rotation.x;
                        rotations[index * 2 + 1] = rotation.y;
                        intensities[index] = lobe.intensity();
                    }
                    let first = vertices.len();
                    Self::push_shape_quad(
                        &mut vertices,
                        &mut indices,
                        *bounds,
                        colors[0],
                        centers[0],
                        centers[1],
                        ShapeType::LocalAmbientGlow,
                    );
                    for vertex in &mut vertices[first..] {
                        vertex.params2 = colors[1];
                        vertex.params3 = rotations;
                        vertex.contour_gains = intensities;
                    }
                }
                PrimitiveData::LuminousContour {
                    points,
                    core_width,
                    color,
                    appearance,
                    fronts,
                    total_length,
                    scroll_offset,
                } if color.a >= 0.001 || appearance.front_opacity_boost() > 0.0 => {
                    let base_color = [color.r, color.g, color.b, color.a];
                    visit_luminous_contour_regions(points, *total_length, fronts, |region| {
                        Self::push_luminous_contour_region(
                            &mut vertices,
                            &mut indices,
                            region,
                            *core_width,
                            base_color,
                            *appearance,
                            *scroll_offset,
                        );
                    });
                }
                _ => {}
            }
        }

        (vertices, indices)
    }

    fn update_shape_gpu_buffers(&mut self, vertices: &[ShapeVertex], indices: &[u32]) {
        let vertex_count = vertices.len() as u32;
        let index_count = indices.len() as u32;

        if vertex_count == 0 || index_count == 0 {
            return;
        }

        let need_realloc = self.shape_vertex_buffer.is_none()
            || vertex_count > self.shape_vertex_capacity
            || index_count > self.shape_index_capacity;

        if need_realloc {
            let new_vertex_capacity = (vertex_count * 3 / 2).max(64);
            let new_index_capacity = (index_count * 3 / 2).max(128);

            self.shape_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Shape Vertex Buffer"),
                size: (new_vertex_capacity as usize * std::mem::size_of::<ShapeVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.shape_index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Shape Index Buffer"),
                size: (new_index_capacity as usize * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            self.shape_vertex_capacity = new_vertex_capacity;
            self.shape_index_capacity = new_index_capacity;
        }

        if let Some(vb) = self.shape_vertex_buffer.clone() {
            self.stage_buffer_upload(vb, 0, bytemuck::cast_slice(vertices));
        }
        if let Some(ib) = self.shape_index_buffer.clone() {
            self.stage_buffer_upload(ib, 0, bytemuck::cast_slice(indices));
        }
    }

    /// Convert text quad to NDC using physical coordinates.
    ///
    /// For text rendering, we need pixel-perfect alignment between atlas bitmaps
    /// and screen pixels. Using physical coordinates directly avoids rounding
    /// errors introduced by logical→physical→logical conversion.
    ///
    /// INV-SIZE-001: Takes explicit parameters for per-target rendering.
    ///
    /// The input rect from GlyphQuad has:
    /// - Positions in logical pixels (already scaled back from physical)
    /// - Dimensions in logical pixels (bitmap_px / scale_factor)
    ///
    /// We convert everything to physical pixels and then to NDC using
    /// physical_width/height to ensure exact pixel mapping.
    fn text_quad_to_ndc_for_target(
        rect: &RectF,
        scale_factor: f32,
        physical_width: u32,
        physical_height: u32,
    ) -> (f32, f32, f32, f32) {
        // Convert logical coordinates to physical pixels
        // Position: logical → physical
        let px_x = rect.x * scale_factor;
        let px_y = rect.y * scale_factor;
        // Dimensions: logical → physical (restores raw bitmap pixel size)
        let px_w = rect.width * scale_factor;
        let px_h = rect.height * scale_factor;

        // Convert physical pixels to NDC using physical viewport
        // This avoids rounding errors from logical dimension computation
        let phys_w = physical_width as f32;
        let phys_h = physical_height as f32;

        let x0 = (px_x / phys_w) * 2.0 - 1.0;
        let y0 = 1.0 - (px_y / phys_h) * 2.0;
        let x1 = ((px_x + px_w) / phys_w) * 2.0 - 1.0;
        let y1 = 1.0 - ((px_y + px_h) / phys_h) * 2.0;

        (x0, y0, x1, y1)
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn text_engine_mut(&mut self) -> &mut BackendTextEngine<CoreTextBackend> {
        &mut self.text_engine
    }

    #[cfg(target_os = "windows")]
    pub fn text_engine_mut(&mut self) -> &mut BackendTextEngine<DirectWriteBackend> {
        &mut self.text_engine
    }

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios")),
        not(target_arch = "wasm32")
    ))]
    pub fn text_engine_mut(&mut self) -> &mut BackendTextEngine<FreeTypeBackend> {
        &mut self.text_engine
    }

    #[cfg(target_arch = "wasm32")]
    pub fn text_engine_mut(&mut self) -> &mut BackendTextEngine<GlyphonBackend> {
        &mut self.text_engine
    }

    /// INV-SCALE-001: Get the scale factor for a specific surface.
    ///
    /// Returns the per-target scale factor, or 1.0 if the surface is not registered.
    pub fn scale_factor(&self, surface_id: crate::compositor::SurfaceId) -> f32 {
        self.multi_targets.scale_factor(surface_id).unwrap_or(1.0)
    }

    pub(crate) fn custom_render_target_snapshot(
        &self,
        surface_id: crate::compositor::SurfaceId,
    ) -> Result<CustomRenderTargetSnapshot, PixelCoverageError> {
        let (width, height) = self
            .multi_targets
            .configured_size_px(surface_id)
            .ok_or(PixelCoverageError::TargetNotRegistered)?;
        if !self.multi_targets.has_active_extent(surface_id) {
            return Err(PixelCoverageError::InvalidPhysicalTargetExtent);
        }
        let scale_factor = self
            .multi_targets
            .scale_factor(surface_id)
            .ok_or(PixelCoverageError::TargetNotRegistered)?;
        CustomRenderTargetSnapshot::new(
            surface_id,
            PhysicalExtent::new(width, height),
            scale_factor,
        )
    }

    pub(crate) fn target_has_active_extent(
        &self,
        surface_id: crate::compositor::SurfaceId,
    ) -> bool {
        self.multi_targets.has_active_extent(surface_id)
    }

    // ========================================================================
    // SPEC-HAL-295: GPU Resource Accessors for Custom Rendering
    // ========================================================================

    /// SPEC-HAL-295: Get a clone of the shared Device handle.
    ///
    /// Used by widgets that perform custom GPU rendering (Plot, custom
    /// render passes). The Arc clone is cheap and allows the widget to
    /// create pipelines, buffers, and textures independently.
    pub fn device(&self) -> std::sync::Arc<wgpu::Device> {
        self.device.clone()
    }

    /// SPEC-HAL-295: Get a clone of the shared Queue handle.
    ///
    /// Used by widgets that perform custom GPU rendering. The Arc clone
    /// allows the widget to submit command buffers independently of
    /// Halcyon's frame loop.
    pub fn queue(&self) -> std::sync::Arc<wgpu::Queue> {
        self.queue.clone()
    }

    /// SPEC-HAL-295: Get the surface texture format.
    ///
    /// Used by widgets creating custom render pipelines — the pipeline's
    /// color target format must match the surface format.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    /// SPEC-HAL-295: Get all GPU resources needed for custom rendering.
    ///
    /// Convenience method that bundles device, queue, and format into
    /// a single struct for widget initialization.
    pub fn gpu_resources(&self) -> GpuResources {
        self.pending_rgba8_registrations
            .get_or_init(Default::default)
            .presence_layout
            .get_or_init(|| loading_presence_output::PresenceImageLayout {
                globals: self.rect_globals_layout.clone(),
                image: self.image_bind_group_layout.clone(),
                format: self.surface_format,
            });
        GpuResources {
            device: self.device.clone(),
            queue: self.queue.clone(),
            format: self.surface_format,
            adapter: self.adapter.clone(),
            pending_registrations: self.pending_registrations.clone(),
            pending_unregistrations: self.pending_unregistrations.clone(),
            pending_rgba8_registrations: self
                .pending_rgba8_registrations
                .get_or_init(Default::default)
                .clone(),
            image_admission: self.image_admission.get_or_init(Default::default).clone(),
            scale_factor: 1.0,
        }
    }

    #[cfg(test)]
    pub(crate) fn headless_adapter_for_test(&self) -> &wgpu::AdapterInfo {
        self.headless_adapter_info
            .as_ref()
            .expect("actual headless adapter/device")
    }

    /// The adapter captured by this headless renderer, without another adapter
    /// request or Device. Window-backed renderers have no headless capture.
    #[cfg(feature = "test-harness")]
    pub fn headless_adapter_info_for_test_harness(&self) -> Option<wgpu::AdapterInfo> {
        self.headless_adapter_info.clone()
    }

    #[cfg(test)]
    pub(crate) fn set_hdr_removal_protected_for_test(&mut self, id: u64, protected: bool) {
        if protected {
            self.protected_sdf_texture_ids.insert(id);
        } else {
            self.protected_sdf_texture_ids.remove(&id);
        }
    }

    /// SPEC-HAL-040 Stage 1: Get the current frame epoch.
    ///
    /// This is a read-only view of the monotonically increasing frame counter.
    /// The epoch advances by exactly 1 via `advance_frame()` after each successful
    /// frame that submits GPU work. It does NOT advance on skipped frames (unfocused,
    /// surface error before submit).
    ///
    /// Later stages of SPEC-HAL-040 will use this for:
    /// - Tagging resource creation epoch
    /// - Tracking last-used epoch
    /// - Determining safe destruction epoch
    pub fn frame_epoch(&self) -> FrameEpoch {
        self.frame_epoch
    }

    /// SPEC-HAL-040 Stage 2: Get read-only access to the GPU resource arena.
    ///
    /// The arena holds metadata for all registered GPU resources, including
    /// their creation epoch and last-used epoch. This is useful for debugging
    /// and profiling resource lifetimes.
    pub fn gpu_arena(&self) -> &GpuResourceArena {
        &self.gpu_arena
    }

    /// SPEC-HAL-040 Stage 2: Get mutable access to the GPU resource arena.
    ///
    /// Used internally for registering new resources and updating last-used epochs.
    /// Stage 2 does NOT call this anywhere - actual integration with resource
    /// creation happens in Stage 4+.
    #[allow(
        dead_code,
        reason = "Retained renderer-owned mutable resource arena access"
    )]
    fn gpu_arena_mut(&mut self) -> &mut GpuResourceArena {
        &mut self.gpu_arena
    }

    /// SPEC-HAL-040 Stage 4: Get mutable access to the staging buffer pool.
    ///
    /// Used internally for allocating and recycling staging buffers during
    /// CPU→GPU uploads.
    #[allow(dead_code)]
    fn staging_pool_mut(&mut self) -> &mut StagingBufferPool {
        &mut self.staging_pool
    }

    #[cfg(test)]
    pub(crate) fn staging_pool_total_bytes_for_test(&self) -> u64 {
        self.staging_pool.total_bytes()
    }

    #[cfg(test)]
    pub(crate) fn pending_staging_work_for_test(&self) -> (usize, usize) {
        (
            self.pending_staging_submissions.len(),
            self.pending_staging_remaps.len(),
        )
    }

    #[cfg(test)]
    pub(crate) fn wait_for_staging_recycles_for_test(&mut self) {
        for _ in 0..3 {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
            self.poll_staging_recycles();
        }
        assert!(self.pending_staging_submissions.is_empty());
        assert!(self.pending_staging_remaps.is_empty());
    }

    // ========================================================================
    // SPEC-HAL-263: Image Texture Registration API
    // ========================================================================

    pub(crate) fn set_image_admission_wake(&self, wake: Box<dyn Fn() + Send + Sync>) {
        self.image_admission
            .get_or_init(Default::default)
            .set_wake(wake);
    }

    pub(crate) fn has_pending_image_admissions(&self) -> bool {
        self.image_admission
            .get()
            .is_some_and(|mailbox| mailbox.has_pending())
            || self
                .pending_rgba8_registrations
                .get()
                .is_some_and(|queue| queue.has_items.load(std::sync::atomic::Ordering::Acquire))
    }

    pub(crate) fn has_pending_image_unregistrations(&self) -> bool {
        !self
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty()
    }

    #[cfg(test)]
    pub(crate) fn pending_image_rgba8_registrations_for_test(&self) -> Option<usize> {
        self.pending_rgba8_registrations.get().and_then(|queue| {
            queue
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .items
                .as_ref()
                .map(Vec::len)
        })
    }

    /// Issues CPU-only evidence for the actual current registration generation.
    /// The value neither registers a resource nor extends its GPU lifetime.
    pub fn admit_image(
        &mut self,
        texture_id: u64,
    ) -> Result<
        crate::renderer::image_binding::ImageBinding,
        crate::renderer::image_binding::ImageAdmissionError,
    > {
        use crate::renderer::image_binding::{
            ImageAdmissionError, ImageBinding, ImageBindingIdentity, ImageBindingSampling,
            ImageBindingState,
        };
        if !self.image_textures.contains_key(&texture_id) {
            return Err(ImageAdmissionError::MissingRegistration(texture_id));
        }
        let device = DeviceLifetimeObservation::for_device(&self.device);
        let entry = self
            .image_textures
            .get_mut(&texture_id)
            .expect("checked current Image entry");
        let state = entry
            .admission
            .get_or_insert_with(|| {
                ImageBindingState::new(
                    ImageBindingIdentity::new(texture_id, entry.generation),
                    match entry.sampling {
                        ImageSampling::Linear => ImageBindingSampling::Linear,
                        ImageSampling::Nearest => ImageBindingSampling::Nearest,
                    },
                    device,
                )
            })
            .clone();
        let binding = ImageBinding { state };
        binding
            .availability()
            .map_err(ImageAdmissionError::Unavailable)?;
        Ok(binding)
    }

    fn assert_unprotected_image_mutation(&self, texture_id: u64, operation: &str) {
        assert!(
            !self.protected_sdf_texture_ids.contains(&texture_id),
            "protected SDF texture ID {texture_id}: unauthenticated {operation}"
        );
    }

    #[cfg(test)]
    fn protect_sdf_image_for_test(&mut self, texture_id: u64) {
        assert!(self.image_textures.contains_key(&texture_id));
        assert!(self.protected_sdf_texture_ids.insert(texture_id));
    }

    fn insert_image_entry(&mut self, texture_id: u64, entry: ImageTextureEntry) {
        self.assert_unprotected_image_mutation(texture_id, "image entry insertion");
        if let Some(previous) = self.image_textures.insert(texture_id, entry) {
            self.invalidate_compile_preparation();
            previous.revoke(crate::renderer::image_binding::ImageBindingFailure::Replaced);
        }
    }

    /// SPEC-HAL-263: Register an image texture for use by the Image pipeline.
    ///
    /// This is the ONLY legal way to make a `texture_id` usable by ImageNode.
    /// Must be called before any scene references this texture_id.
    ///
    /// Creates a single ImageTextureEntry owning Texture + TextureView + BindGroup.
    /// This low-level boundary performs no content conversion: the texture view
    /// must already yield premultiplied-linear RGBA when sampled.
    ///
    /// # Arguments
    ///
    /// * `texture_id` - Application-chosen u64 identifier (INV-IMG-002: identity by handle)
    /// * `texture` - The wgpu::Texture to register (caller transfers ownership)
    pub fn register_image_texture(&mut self, texture_id: u64, texture: wgpu::Texture) {
        self.assert_unprotected_image_mutation(texture_id, "texture registration");
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = Self::create_image_bind_group(
            &self.device,
            &self.image_bind_group_layout,
            &view,
            &self.image_sampler,
            &self.rounded_clip_buffer,
        );

        self.insert_image_entry(
            texture_id,
            ImageTextureEntry {
                _texture: Some(texture),
                view,
                sampling: ImageSampling::Linear,
                bind_group,
                generation: crate::renderer::image_binding::next_image_generation(),
                admission: None,
            },
        );
    }

    /// SPEC-HAL-295: Register an externally-owned texture for image display.
    ///
    /// Unlike `register_image_texture`, this does NOT take ownership of the
    /// wgpu::Texture. The caller (e.g., Plot) keeps the texture alive.
    /// The renderer stores only the TextureView and BindGroup for sampling.
    ///
    /// The caller must ensure the source texture outlives this registration.
    /// Call `unregister_image()` before dropping the source texture.
    /// The view must already yield premultiplied-linear RGBA when sampled; this
    /// low-level boundary performs no content conversion.
    pub fn register_image_from_view(&mut self, texture_id: u64, view: wgpu::TextureView) {
        self.assert_unprotected_image_mutation(texture_id, "view registration");
        let bind_group = Self::create_image_bind_group(
            &self.device,
            &self.image_bind_group_layout,
            &view,
            &self.image_sampler,
            &self.rounded_clip_buffer,
        );

        self.insert_image_entry(
            texture_id,
            ImageTextureEntry {
                _texture: None,
                view,
                sampling: ImageSampling::Linear,
                bind_group,
                generation: crate::renderer::image_binding::next_image_generation(),
                admission: None,
            },
        );
    }

    /// SPEC-HAL-295: Register externally-owned texture with nearest filtering.
    ///
    /// Same as register_image_from_view but uses Nearest sampler instead
    /// of Linear. Use for pixel-perfect content like charts where linear
    /// filtering would blur candle edges.
    /// Nearest filtering does not change the premultiplied-linear precondition.
    pub fn register_image_from_view_nearest(&mut self, texture_id: u64, view: wgpu::TextureView) {
        self.assert_unprotected_image_mutation(texture_id, "nearest view registration");
        let bind_group = Self::create_image_bind_group(
            &self.device,
            &self.image_bind_group_layout,
            &view,
            &self.nearest_sampler,
            &self.rounded_clip_buffer,
        );

        self.insert_image_entry(
            texture_id,
            ImageTextureEntry {
                _texture: None,
                view,
                sampling: ImageSampling::Nearest,
                bind_group,
                generation: crate::renderer::image_binding::next_image_generation(),
                admission: None,
            },
        );
    }

    /// SPEC-HAL-295: Drain pending texture registrations.
    ///
    /// Called by the frame loop between build_scene and compile_multi.
    /// Processes all registrations queued by widgets during layout without
    /// converting their premultiplied-linear sampled content.
    pub fn drain_pending_image_registrations(&mut self) {
        self.drain_loading_presence();
        // Validate one locked mutation batch before taking any entries. In
        // particular, a rejected removal must keep its front textures queued.
        let (removals, entries, rgba8_entries) = {
            let mut removals = self
                .pending_unregistrations
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut entries = self
                .pending_registrations
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let rgba8_queue = self
                .pending_rgba8_registrations
                .get()
                .filter(|queue| queue.has_items.load(std::sync::atomic::Ordering::Acquire));
            let mut rgba8_pending = rgba8_queue.map(|queue| {
                queue
                    .pending
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
            });
            let conflict = removals
                .iter()
                .find(|entry| self.protected_sdf_texture_ids.contains(&entry.texture_id))
                .map(|entry| (entry.texture_id, "queued unregistration"))
                .or_else(|| {
                    entries
                        .iter()
                        .find(|entry| self.protected_sdf_texture_ids.contains(&entry.texture_id))
                        .map(|entry| (entry.texture_id, "queued view registration"))
                })
                .or_else(|| {
                    rgba8_pending
                        .as_ref()
                        .and_then(|pending| pending.items.as_ref())
                        .and_then(|items| {
                            items.iter().find(|entry| {
                                self.protected_sdf_texture_ids
                                    .contains(&entry.admission.texture_id)
                            })
                        })
                        .map(|entry| (entry.admission.texture_id, "queued RGBA8 registration"))
                });
            if let Some((texture_id, operation)) = conflict {
                // Release guards before panic: retain entries and avoid poisoning
                // shared queues while an existing sampled front remains alive.
                drop(rgba8_pending);
                drop(entries);
                drop(removals);
                panic!("protected SDF texture ID {texture_id}: unauthenticated {operation}");
            }
            let hdr = self
                .pending_rgba8_registrations
                .get()
                .and_then(|shared| shared.hdr_finals.get())
                .map(|state| state.lock().unwrap_or_else(|e| e.into_inner()));
            let invalid = hdr.as_ref().and_then(|state| {
                state.entries.iter().find_map(|(id, record)| {
                    let count = removals
                        .iter()
                        .filter(|removal| removal.texture_id == *id)
                        .count();
                    ((record.pending
                        && (count != 1
                            || removals.iter().any(|removal| {
                                removal.texture_id == *id
                                    && !record.matches_textures(&removal.retained_textures)
                            })))
                        || (!record.pending && count != 0))
                        .then_some(*id)
                })
            });
            if let Some(id) = invalid {
                drop(hdr);
                drop(rgba8_pending);
                drop(entries);
                drop(removals);
                panic!("unmatched/duplicate HDR final removal {id}");
            }
            let removals = std::mem::take(&mut *removals);
            let entries = std::mem::take(&mut *entries);
            let rgba8_entries = rgba8_pending
                .as_mut()
                .map(|pending| pending.items.take().unwrap_or_default())
                .unwrap_or_default();
            if let Some(queue) = rgba8_queue {
                queue
                    .has_items
                    .store(false, std::sync::atomic::Ordering::Release);
            }
            (removals, entries, rgba8_entries)
        };

        for removal in removals {
            let PendingImageUnregistration {
                texture_id,
                retained_textures,
            } = removal;
            self.unregister_image(texture_id);
            // Move the already-reserved custody only AFTER actual removal. This
            // requires no second map allocation in the retirement path.
            let custody = self
                .pending_rgba8_registrations
                .get()
                .and_then(|shared| shared.hdr_finals.get())
                .and_then(|state| {
                    let _removals = self
                        .pending_unregistrations
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                    let matches = state.entries.get(&texture_id).is_some_and(|record| {
                        record.pending && record.matches_textures(&retained_textures)
                    });
                    if matches {
                        state.entries.remove(&texture_id)
                    } else {
                        None
                    }
                });
            if let Some(custody) = custody {
                crate::renderer::on_submitted_work_done(&self.queue, move || drop((retained_textures, custody)));
            } else {
                drop(retained_textures);
            }
        }
        for entry in entries {
            if entry.nearest {
                self.register_image_from_view_nearest(entry.texture_id, entry.view);
            } else {
                self.register_image_from_view(entry.texture_id, entry.view);
            }
        }
        let requests = self
            .image_admission
            .get()
            .map(|mailbox| mailbox.take())
            .unwrap_or_default();
        let mut delivered = false;
        for entry in rgba8_entries {
            let texture_id = entry.admission.texture_id;
            self.register_image_rgba8_srgb(texture_id, entry.width, entry.height, &entry.rgba);
            // Issue this generation before the next entry can replace it.
            let result = self.admit_image(texture_id);
            delivered |= entry.admission.sender.send(result).is_ok();
        }
        for mut request in requests {
            if let Some((entry, current)) = request.plot_front.take() {
                if !current.load(std::sync::atomic::Ordering::Acquire) {
                    delivered |= request
                        .sender
                        .send(Err(
                            super::image_binding::ImageAdmissionError::RequestCancelled,
                        ))
                        .is_ok();
                    continue;
                }
                if entry.nearest {
                    self.register_image_from_view_nearest(entry.texture_id, entry.view);
                } else {
                    self.register_image_from_view(entry.texture_id, entry.view);
                }
            }
            let result = if request.expected_view.as_ref().is_some_and(|expected| {
                self.image_textures
                    .get(&request.texture_id)
                    .is_none_or(|entry| entry.view != *expected)
            }) {
                Err(
                    crate::renderer::image_binding::ImageAdmissionError::MissingRegistration(
                        request.texture_id,
                    ),
                )
            } else {
                self.admit_image(request.texture_id)
            };
            delivered |= request.sender.send(result).is_ok();
        }
        if delivered {
            self.image_admission
                .get()
                .expect("requested Image admission mailbox")
                .notify();
        }
    }

    /// SPEC-HAL-263: Unregister an image texture, releasing all GPU resources.
    ///
    /// Removes the Texture + TextureView + BindGroup immediately.
    /// Any subsequent frame that references this texture_id will trigger
    /// INV-IMG-005 (debug: panic, release: skip).
    ///
    /// Safe to call even if texture_id is not currently registered (no-op).
    pub fn unregister_image(&mut self, texture_id: u64) {
        self.assert_unprotected_image_mutation(texture_id, "unregistration");
        if let Some(entry) = self.image_textures.remove(&texture_id) {
            self.invalidate_compile_preparation();
            entry.revoke(crate::renderer::image_binding::ImageBindingFailure::Unregistered);
        }
    }

    /// SPEC-HAL-263: Register an image texture from raw RGBA8 sRGB pixel data.
    ///
    /// Creates a GPU texture internally, converts straight-sRGB input to
    /// premultiplied-linear content encoded as sRGB bytes, uploads it, and
    /// registers it for use by the Image pipeline. This is the preferred
    /// high-level API — callers never touch wgpu handles directly.
    ///
    /// # Arguments
    ///
    /// * `texture_id` - Application-chosen u64 identifier (INV-IMG-002)
    /// * `w` - Texture width in pixels
    /// * `h` - Texture height in pixels
    /// * `rgba` - Straight-sRGB RGBA8 data (must be exactly `w * h * 4` bytes)
    ///
    /// # Panics
    ///
    /// Panics if the checked `w * h * 4` byte count overflows or differs from `rgba.len()`.
    pub fn register_image_rgba8_srgb(&mut self, texture_id: u64, w: u32, h: u32, rgba: &[u8]) {
        self.assert_unprotected_image_mutation(texture_id, "RGBA8 registration");
        let expected =
            image_rgba8_byte_count(w, h).expect("SPEC-HAL-263: RGBA8 byte count overflow");
        assert_eq!(
            rgba.len(),
            expected,
            "SPEC-HAL-263: RGBA8 data length mismatch: expected {}x{}x4={}, got {}",
            w,
            h,
            expected,
            rgba.len()
        );
        let normalized = Self::normalize_straight_srgb_rgba8(rgba);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Image Texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &normalized,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.register_image_texture(texture_id, texture);
    }

    fn normalize_straight_srgb_rgba8(rgba: &[u8]) -> Vec<u8> {
        assert_eq!(
            rgba.len() % 4,
            0,
            "SPEC-HAL-263A: RGBA8 input must contain complete texels"
        );

        let mut normalized = Vec::with_capacity(rgba.len());
        for texel in rgba.chunks_exact(4) {
            let alpha_byte = texel[3];
            if alpha_byte == 0 {
                normalized.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            if alpha_byte == u8::MAX {
                normalized.extend_from_slice(texel);
                continue;
            }

            let alpha = f64::from(alpha_byte) / 255.0;
            for channel in &texel[..3] {
                let srgb = f64::from(*channel) / 255.0;
                let linear = if srgb <= 0.04045 {
                    srgb / 12.92
                } else {
                    ((srgb + 0.055) / 1.055).powf(2.4)
                };
                let premultiplied = (linear * alpha).clamp(0.0, 1.0);
                let encoded = if premultiplied <= 0.003_130_8 {
                    12.92 * premultiplied
                } else {
                    1.055 * premultiplied.powf(1.0 / 2.4) - 0.055
                };
                normalized.push((encoded.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
            normalized.push(alpha_byte);
        }
        normalized
    }

    #[cfg(test)]
    pub(crate) fn normalize_image_rgba8_srgb_for_test(rgba: &[u8]) -> Vec<u8> {
        Self::normalize_straight_srgb_rgba8(rgba)
    }

    #[cfg(test)]
    pub(crate) fn render_registered_image_pixel_for_test(
        &self,
        texture_id: u64,
        uv: [f32; 2],
        tint: [f32; 4],
        background: wgpu::Color,
    ) -> [u8; 4] {
        self.render_registered_image_pixel_checked_for_test(texture_id, uv, tint, background)
            .expect("SPEC-HAL-263A: production image readback")
    }

    /// Samples an already registered image with the unchanged production Image
    /// pipeline. Drain pending image registrations after publishing a front and
    /// before calling this diagnostic. No source/front is written or published.
    #[cfg(feature = "test-harness")]
    pub fn registered_image_pixel_for_test_harness(
        &self,
        image_id: u64,
        uv: [f32; 2],
        tint: [f32; 4],
        background: [f64; 4],
    ) -> Result<[u8; 4], TestHarnessImageProofError> {
        self.render_registered_image_pixel_checked_for_test(
            image_id,
            uv,
            tint,
            wgpu::Color {
                r: background[0],
                g: background[1],
                b: background[2],
                a: background[3],
            },
        )
    }

    #[cfg(any(test, feature = "test-harness"))]
    fn render_registered_image_pixel_checked_for_test(
        &self,
        texture_id: u64,
        uv: [f32; 2],
        tint: [f32; 4],
        background: wgpu::Color,
    ) -> Result<[u8; 4], TestHarnessImageProofError> {
        const SIZE: u32 = 4;
        if uv.into_iter().chain(tint).any(|v| !v.is_finite())
            || [background.r, background.g, background.b, background.a]
                .into_iter()
                .any(|v| !v.is_finite())
        {
            return Err(TestHarnessImageProofError::NonfiniteInput);
        }
        if !matches!(
            self.surface_format,
            wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
                | wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
        ) {
            return Err(TestHarnessImageProofError::UnsupportedFormat(
                self.surface_format,
            ));
        }
        let lifetime = self.gpu_resources().observe_device_lifetime();
        if lifetime.failure().is_some() {
            return Err(TestHarnessImageProofError::DeviceUnavailable);
        }
        let registration_pending = self
            .pending_registrations
            .lock()
            .map_err(|_| TestHarnessImageProofError::RegistrationLock)?
            .iter()
            .any(|entry| entry.texture_id == texture_id);
        let removal_pending = self
            .pending_unregistrations
            .lock()
            .map_err(|_| TestHarnessImageProofError::RegistrationLock)?
            .iter()
            .any(|entry| entry.texture_id == texture_id);
        if registration_pending || removal_pending {
            return Err(TestHarnessImageProofError::RegistrationPending(texture_id));
        }
        let entry = self
            .image_textures
            .get(&texture_id)
            .ok_or(TestHarnessImageProofError::MissingImage(texture_id))?;
        let globals = Globals::new(SIZE as f32, SIZE as f32, SIZE as f32, SIZE as f32, false);
        let globals_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SPEC-HAL-263A probe globals"),
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let globals_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SPEC-HAL-263A probe globals bind group"),
            layout: &self.rect_globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &globals_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap(),
                    ),
                }),
            }],
        });
        let vertices = [
            ImageVertex {
                position: [0.0, 0.0],
                uv,
                color: tint,
            },
            ImageVertex {
                position: [SIZE as f32, 0.0],
                uv,
                color: tint,
            },
            ImageVertex {
                position: [SIZE as f32, SIZE as f32],
                uv,
                color: tint,
            },
            ImageVertex {
                position: [0.0, SIZE as f32],
                uv,
                color: tint,
            },
        ];
        let indices = [0u16, 1, 2, 0, 2, 3];
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SPEC-HAL-263A probe image vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SPEC-HAL-263A probe image indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SPEC-HAL-263A production image target"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let padded_bytes_per_row = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SPEC-HAL-263A production image readback"),
            size: u64::from(padded_bytes_per_row * SIZE),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SPEC-HAL-263A production image encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SPEC-HAL-263A production image pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.image_pipeline);
            pass.set_bind_group(0, &globals_bind_group, &[0]);
            pass.set_bind_group(1, &entry.bind_group, &[0]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(SIZE),
                },
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let data = mapped_test_pixel_bytes(&self.device, &readback)?;
        if lifetime.failure().is_some() {
            return Err(TestHarnessImageProofError::DeviceUnavailable);
        }
        let offset = (2 * padded_bytes_per_row + 2 * 4) as usize;
        let stored = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ];
        match self.surface_format {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                Ok([stored[2], stored[1], stored[0], stored[3]])
            }
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => Ok(stored),
            format => Err(TestHarnessImageProofError::UnsupportedFormat(format)),
        }
    }

    #[cfg(test)]
    pub(crate) fn render_registered_image_coverage_for_test(
        &self,
        texture_id: u64,
        logical_rect: RectF,
        logical_clip: Option<RectF>,
        physical_extent: PhysicalExtent,
        scale_factor: f32,
    ) -> Option<PhysicalRect> {
        let entry = self
            .image_textures
            .get(&texture_id)
            .expect("SPEC-HAL-266A: coverage texture must be registered");
        let logical_width = physical_extent.width as f32 / scale_factor;
        let logical_height = physical_extent.height as f32 / scale_factor;
        let globals = Globals::new(
            logical_width,
            logical_height,
            physical_extent.width as f32,
            physical_extent.height as f32,
            false,
        );
        let globals_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SPEC-HAL-266A coverage globals"),
                contents: bytemuck::bytes_of(&globals),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let globals_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SPEC-HAL-266A coverage globals bind group"),
            layout: &self.rect_globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &globals_buffer,
                    offset: 0,
                    size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap(),
                    ),
                }),
            }],
        });
        let vertices = [
            ImageVertex {
                position: [logical_rect.left(), logical_rect.top()],
                uv: [0.0, 0.0],
                color: [1.0; 4],
            },
            ImageVertex {
                position: [logical_rect.right(), logical_rect.top()],
                uv: [1.0, 0.0],
                color: [1.0; 4],
            },
            ImageVertex {
                position: [logical_rect.right(), logical_rect.bottom()],
                uv: [1.0, 1.0],
                color: [1.0; 4],
            },
            ImageVertex {
                position: [logical_rect.left(), logical_rect.bottom()],
                uv: [0.0, 1.0],
                color: [1.0; 4],
            },
        ];
        let indices = [0u16, 1, 2, 0, 2, 3];
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SPEC-HAL-266A coverage image vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SPEC-HAL-266A coverage image indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SPEC-HAL-266A production image coverage target"),
            size: wgpu::Extent3d {
                width: physical_extent.width,
                height: physical_extent.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let padded_bytes_per_row = physical_extent
            .width
            .saturating_mul(4)
            .div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .saturating_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SPEC-HAL-266A production coverage readback"),
            size: u64::from(padded_bytes_per_row) * u64::from(physical_extent.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let constants = crate::renderer::render_graph::FrameConstants {
            viewport_size: [physical_extent.width as f32, physical_extent.height as f32],
            physical_width: physical_extent.width,
            physical_height: physical_extent.height,
            logical_width,
            logical_height,
            scale_factor,
            clear_color: Color::TRANSPARENT,
        };
        let (sx, sy, sw, sh) = constants.clip_rect_to_scissor(logical_clip.as_ref());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SPEC-HAL-266A production coverage encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SPEC-HAL-266A production image coverage pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            if sw > 0 && sh > 0 {
                pass.set_pipeline(&self.image_pipeline);
                pass.set_bind_group(0, &globals_bind_group, &[0]);
                pass.set_bind_group(1, &entry.bind_group, &[0]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(physical_extent.height),
                },
            },
            wgpu::Extent3d {
                width: physical_extent.width,
                height: physical_extent.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let done = Arc::new(AtomicBool::new(false));
        let succeeded = Arc::new(AtomicBool::new(false));
        let done_for_callback = done.clone();
        let succeeded_for_callback = succeeded.clone();
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            succeeded_for_callback.store(result.is_ok(), Ordering::Release);
            done_for_callback.store(true, Ordering::Release);
        });
        while !done.load(Ordering::Acquire) {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }
        assert!(
            succeeded.load(Ordering::Acquire),
            "SPEC-HAL-266A: production coverage readback failed"
        );

        let data = slice
            .get_mapped_range()
            .expect("mapped production coverage readback range");
        let mut min_x = physical_extent.width;
        let mut min_y = physical_extent.height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut covered = false;
        for y in 0..physical_extent.height {
            for x in 0..physical_extent.width {
                let offset = (y * padded_bytes_per_row + x * 4) as usize;
                if data[offset + 3] == 0 {
                    continue;
                }
                covered = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
            }
        }
        drop(data);
        readback.unmap();
        covered.then(|| PhysicalRect::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }

    /// Update the scale factor (DPI scaling) for a specific surface.
    ///
    /// INV-SCALE-001: Scale factor is per-target, not renderer-global.
    /// Called when the window moves between displays with different scale factors.
    /// Invalidates the glyph atlas since all glyphs are rasterized at a specific scale.
    ///
    /// # Arguments
    ///
    /// * `surface_id` - The surface to update
    /// * `scale_factor` - New scale factor (f64 from winit, converted to f32)
    pub fn set_scale_factor(
        &mut self,
        surface_id: crate::compositor::SurfaceId,
        scale_factor: f64,
    ) {
        self.set_target_scale_factor(surface_id, scale_factor as f32, true);
    }

    /// The App identifies the target that supplies its mounted tree's shaping scale.
    pub(crate) fn set_target_scale_factor(
        &mut self,
        surface_id: SurfaceId,
        sf: f32,
        sets_shaping_scale: bool,
    ) {
        // Get current scale factor for this surface to check if it changed
        let current_sf = self.multi_targets.scale_factor(surface_id).unwrap_or(1.0);
        if current_sf == sf {
            return;
        }

        tracing::info!(
            "Scale factor changed for surface {:?}: {} -> {}",
            surface_id.raw(),
            current_sf,
            sf
        );

        // INV-SCALE-001: Update per-target scale factor in registry
        if let Err(e) = self.multi_targets.set_scale_factor(surface_id, sf) {
            tracing::warn!(
                "Failed to set scale factor for surface {:?}: {}",
                surface_id.raw(),
                e
            );
            return;
        }

        // Invalidate glyph atlas - all glyphs are scale-dependent
        self.invalidate_compile_preparation();
        self.glyph_atlas.clear(&self.queue);

        // SPEC-HAL-081B: Per-surface text_bind_groups need recreation (atlas cleared).
        // Trigger pipeline validation which will recreate all per-surface bind groups.
        self.pipeline_generation += 1;

        // Compile selects each output's scale temporarily. The mounted layout
        // engine keeps the primary scale between preparations.
        if sets_shaping_scale {
            self.text_engine.set_scale(sf);
        }
    }

    // ========================================================================
    // SPEC-R-020 Stage 14: Rounded Clip Ring Buffer Preparation
    // ========================================================================

    /// Prepare rounded clips for a frame: collect unique clips, upload to ring buffer.
    ///
    /// Returns a HashMap mapping clip_hash to the byte offset in the buffer.
    /// This offset is used with `set_bind_group(..., &[offset])` for dynamic binding.
    ///
    /// # Arguments
    ///
    /// * `frame` - Frame batches containing clips
    /// * `logical_width` - Target logical width for fullscreen/no-clip rects
    /// * `logical_height` - Target logical height for fullscreen/no-clip rects
    fn prepare_rounded_clips(
        &mut self,
        frame: &FrameBatches,
        logical_width: f32,
        logical_height: f32,
    ) -> std::collections::HashMap<u64, u32> {
        use std::collections::HashMap;

        let mut clip_offsets: HashMap<u64, u32> = HashMap::new();
        let mut clip_data: Vec<u8> = Vec::new();
        let mut current_index: usize = 0;

        // Always include a "no clip" entry at index 0 for batches without clips
        // INV-SCISSOR-001: Use per-target dimensions, not renderer globals
        let no_clip_uniform = RoundedClipUniform::new(
            [0.0, 0.0, logical_width, logical_height],
            [0.0, 0.0, 0.0, 0.0],
            self.rounded_clip_aa_radius,
        );
        clip_offsets.insert(0, 0); // hash 0 = no clip = offset 0
        let uniform_bytes = bytemuck::bytes_of(&no_clip_uniform);
        clip_data.extend_from_slice(uniform_bytes);
        // Pad to alignment
        clip_data.resize(ROUNDED_CLIP_UNIFORM_ALIGNMENT as usize, 0);
        current_index += 1;

        // Collect unique clips from batches (Rect + Image pipelines use rounded clip UBO)
        for batch in &frame.batches {
            if !matches!(
                batch.key.pipeline,
                PipelineId::Rect | PipelineId::Image | PipelineId::CanonicalOverNeutral
            ) {
                continue;
            }

            let hash = batch.clip_state.hash;
            if clip_offsets.contains_key(&hash) {
                continue; // Already have this clip
            }

            if current_index >= MAX_ROUNDED_CLIPS_PER_FRAME {
                // Exceeded capacity - use offset 0 (no clip) for remaining batches
                // This is a fallback; in practice, 64 unique clips per frame is plenty
                continue;
            }

            // INV-SCISSOR-001: Use per-target dimensions, not renderer globals
            let (rect_logical, radii_logical) = if batch.clip_state.is_none() {
                (
                    [0.0, 0.0, logical_width, logical_height],
                    [0.0, 0.0, 0.0, 0.0],
                )
            } else if let Some(rounded) = &batch.clip_state.rounded {
                (
                    [
                        batch.clip_state.rect.x,
                        batch.clip_state.rect.y,
                        batch.clip_state.rect.width,
                        batch.clip_state.rect.height,
                    ],
                    [rounded.tl, rounded.tr, rounded.bl, rounded.br],
                )
            } else {
                (
                    [
                        batch.clip_state.rect.x,
                        batch.clip_state.rect.y,
                        batch.clip_state.rect.width,
                        batch.clip_state.rect.height,
                    ],
                    [0.0, 0.0, 0.0, 0.0],
                )
            };

            let uniform =
                RoundedClipUniform::new(rect_logical, radii_logical, self.rounded_clip_aa_radius);
            let offset = (current_index as u64 * ROUNDED_CLIP_UNIFORM_ALIGNMENT) as u32;
            clip_offsets.insert(hash, offset);

            let uniform_bytes = bytemuck::bytes_of(&uniform);
            clip_data.extend_from_slice(uniform_bytes);
            // Pad to alignment
            clip_data.resize(
                (current_index + 1) * ROUNDED_CLIP_UNIFORM_ALIGNMENT as usize,
                0,
            );
            current_index += 1;
        }

        // Upload all clips at once
        if !clip_data.is_empty() {
            self.stage_buffer_upload(self.rounded_clip_buffer.clone(), 0, &clip_data);
        }

        clip_offsets
    }

    // ========================================================================
    // SPEC-R-004: Path Pipeline Uniform Uploads
    // ========================================================================

    /// SPEC-HAL-085: Upload shared Globals to path globals buffer.
    /// Uses same Globals struct as rect pipeline for consistency.
    ///
    /// # Arguments
    ///
    /// * `globals_offset` - Byte offset into path globals buffer (for dynamic binding)
    /// * `logical_width` - Target logical width
    /// * `logical_height` - Target logical height
    /// * `physical_width` - Target physical width
    /// * `physical_height` - Target physical height
    ///
    /// SPEC-HAL-081B: Takes per-target offset and dimensions for multi-surface support.
    fn upload_path_globals(
        &mut self,
        globals_offset: u32,
        logical_width: f32,
        logical_height: f32,
        physical_width: f32,
        physical_height: f32,
    ) {
        let globals = Globals::new(
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            self.gpu_rounded_clips_enabled,
        );
        // Pad to GLOBALS_UNIFORM_ALIGNMENT bytes (same as rect globals)
        let mut globals_data = [0u8; GLOBALS_UNIFORM_ALIGNMENT as usize];
        globals_data[..std::mem::size_of::<Globals>()]
            .copy_from_slice(bytemuck::bytes_of(&globals));
        self.stage_buffer_upload(
            self.path_globals_buffer.clone(),
            globals_offset as u64,
            &globals_data,
        );
    }

    /// SPEC-HAL-081B: Upload path clip rects for a specific target to GPU slab buffer.
    ///
    /// - Index 0 is always the fullscreen clip (for paths with clip_id=0, i.e., no clipping).
    /// - Indices 1..N correspond to path_clips[0..N-1].
    /// - Writes to the specified offset in the slab buffer.
    ///
    /// # Arguments
    ///
    /// * `slot_offset` - Byte offset into the slab buffer for this target
    /// * `path_clips` - Clip rects from the draw plan
    /// * `physical_width` - Target physical width for fullscreen clip
    /// * `physical_height` - Target physical height for fullscreen clip
    /// * `scale_factor` - Scale factor for logical→physical conversion
    ///
    /// # Panics
    ///
    /// Debug assertion if total clips exceed PATH_CLIPS_SLOT_SIZE capacity.
    fn upload_path_clips_at_offset(
        &mut self,
        slot_offset: u64,
        path_clips: &[crate::core::math::RectF],
        physical_width: f32,
        physical_height: f32,
        scale_factor: f32,
    ) {
        // Total clips = 1 (fullscreen) + path_clips.len()
        let total_clips = 1 + path_clips.len();
        #[cfg(debug_assertions)]
        let total_bytes = total_clips * std::mem::size_of::<ClipRectUniform>();

        #[cfg(debug_assertions)]
        debug_assert!(
            total_bytes as u64 <= self.path_clips_slot_stride,
            "INV-PATH-CLIPS: {} clips ({} bytes) exceeds slot stride {} bytes",
            total_clips,
            total_bytes,
            self.path_clips_slot_stride
        );

        // Build clip data: index 0 = fullscreen, then path_clips
        let mut clip_data: Vec<ClipRectUniform> = Vec::with_capacity(total_clips);

        // Index 0: Fullscreen clip (no clipping)
        // INV-SCISSOR-001: Use per-target dimensions
        clip_data.push(ClipRectUniform {
            min: [0.0, 0.0],
            max: [physical_width, physical_height],
        });

        // Indices 1..N: Actual clip rects from frame
        // Convert from logical coordinates to physical coordinates
        for clip_rect in path_clips {
            clip_data.push(ClipRectUniform {
                min: [clip_rect.x * scale_factor, clip_rect.y * scale_factor],
                max: [
                    (clip_rect.x + clip_rect.width) * scale_factor,
                    (clip_rect.y + clip_rect.height) * scale_factor,
                ],
            });
        }

        // Upload to GPU at the specified offset
        self.stage_buffer_upload(
            self.path_clips_buffer.clone(),
            slot_offset,
            bytemuck::cast_slice(&clip_data),
        );
    }

    /// SPEC-R-004 Stage 12: Build path batch from frame's path draws.
    ///
    /// Iterates over all PathDraw entries in the frame and flattens them into
    /// the path renderer's owned batch (vertices, indices, instances).
    ///
    /// Each PathDraw produces:
    /// - N vertices (from mesh.vertices)
    /// - M indices (from mesh.indices, rebased by vertex offset)
    /// - 1 instance (color, transform, clip_id, stroke_flag)
    /// - 1 draw command (for correct per-path rendering)
    fn build_path_batch_from_frame(&mut self, frame: &FrameBatches) {
        use crate::renderer::pipelines::path_pipeline::PathDrawCommand;

        let batch = self.path_renderer.batch_mut();
        batch.clear();

        if frame.paths.is_empty() {
            return;
        }

        let mut vertex_offset: u32 = 0;
        let mut index_offset: u32 = 0;
        let mut instance_id: u32 = 0;

        for path_draw in &frame.paths {
            // Skip empty meshes
            if path_draw.mesh.vertices.is_empty() || path_draw.mesh.indices.is_empty() {
                continue;
            }

            let path_index_count = path_draw.mesh.indices.len() as u32;

            // 1) Push vertices
            for v in &path_draw.mesh.vertices {
                batch.vertices.push(PathVertex { position: *v });
            }

            // 2) Push indices (rebased by current vertex offset)
            for &idx in &path_draw.mesh.indices {
                batch.indices.push(idx + vertex_offset);
            }

            // 3) Push instance (Stage 20: use to_instance() for clean CPU→GPU packaging)
            batch.instances.push(path_draw.to_instance());

            // 4) Push draw command for per-path rendering (fixes hover bleed bug)
            batch.draw_commands.push(PathDrawCommand {
                start_index: index_offset,
                index_count: path_index_count,
                instance_id,
            });

            vertex_offset += path_draw.mesh.vertices.len() as u32;
            index_offset += path_index_count;
            instance_id += 1;
        }
    }

    // ========================================================================
    // SPEC-HAL-081B: Multi-Surface Rendering APIs
    // ========================================================================

    /// SPEC-HAL-081B: Create a wgpu::Surface for an OS window.
    ///
    /// This is transport infrastructure for multi-window rendering. The returned
    /// surface can be passed to `register_render_target()` to enable rendering
    /// to the window.
    ///
    /// # Arguments
    ///
    /// * `window` - Arc-wrapped winit window handle
    ///
    /// # Returns
    ///
    /// A wgpu::Surface bound to the window, ready for configuration and registration.
    ///
    /// # Panics
    ///
    /// Panics if surface creation fails (e.g., unsupported platform).
    pub fn create_surface_for_window(
        &self,
        window: std::sync::Arc<winit::window::Window>,
    ) -> wgpu::Surface<'static> {
        self.instance
            .create_surface(window)
            .expect("SPEC-HAL-081B: Failed to create surface for window")
    }

    /// SPEC-HAL-081B: Register a render target for rendering.
    ///
    /// # Arguments
    ///
    /// * `surface_id` - The compositor SurfaceId for this target
    /// * `surface` - The WGPU surface (created from OS window handle)
    /// * `physical_size` - (width, height) in physical pixels
    /// * `scale_factor` - HiDPI scale factor
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Target registered successfully
    /// * `Err(DuplicateRegistration)` - SurfaceId already registered (INV-RT-002)
    pub fn register_render_target(
        &mut self,
        surface_id: crate::compositor::SurfaceId,
        surface: wgpu::Surface<'static>,
        physical_size: (u32, u32),
        scale_factor: f32,
    ) -> Result<(), crate::renderer::render_target::RenderTargetError> {
        self.invalidate_compile_preparation();
        // Build configuration (renderer is the single authority).
        let config = window_surface_configuration(
            self.surface_format,
            physical_size,
            self.default_present_mode,
            self.default_alpha_mode,
        );

        // Configure surface before registration
        surface.configure(&self.device, &config);

        // Register bundle with the configured size (INV-SCISSOR-001) and scale factor (INV-SCALE-001)
        let bundle = crate::renderer::render_target::SurfaceBundle::new(surface);
        self.multi_targets
            .register(surface_id, bundle, physical_size, scale_factor)
    }

    /// SPEC-HAL-081B: Unregister a render target.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Target removed successfully
    /// * `Err(NotRegistered)` - SurfaceId not found
    /// * `Err(UnregisterWhileAcquired)` - Target is currently acquired (INV-RT-009)
    pub fn unregister_render_target(
        &mut self,
        surface_id: crate::compositor::SurfaceId,
    ) -> Result<(), crate::renderer::render_target::RenderTargetError> {
        self.multi_targets.unregister(surface_id)?;
        self.invalidate_compile_preparation();
        self.frosted_glass.retire_surface(surface_id);
        Ok(())
    }

    /// SPEC-HAL-081B: Resize a registered render target.
    ///
    /// Renderer builds the full configuration and passes it to the registry.
    /// The registry applies it verbatim.
    ///
    /// INV-RSZ-001: Also updates renderer's cached dimension fields so that
    /// scissor/viewport/NDC computations use correct values for the next frame.
    ///
    /// # Arguments
    ///
    /// * `surface_id` - The target to resize
    /// * `physical_size` - New (width, height) in physical pixels
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Target resized successfully
    /// * `Err(NotRegistered)` - SurfaceId not found
    /// * `Err(ResizeWhileAcquired)` - Target is currently acquired (INV-RT-004)
    pub fn resize_render_target(
        &mut self,
        surface_id: crate::compositor::SurfaceId,
        physical_size: (u32, u32),
    ) -> Result<(), crate::renderer::render_target::RenderTargetError> {
        self.multi_targets
            .observe_size_px(surface_id, physical_size)?;
        self.invalidate_compile_preparation();
        if physical_size.0 == 0 || physical_size.1 == 0 {
            return Ok(());
        }

        let resize_start = web_time::Instant::now();

        // Build configuration (renderer is the single authority).
        let config = window_surface_configuration(
            self.surface_format,
            physical_size,
            self.default_present_mode,
            self.default_alpha_mode,
        );

        // Registry applies config verbatim
        let configure_start = web_time::Instant::now();
        self.multi_targets
            .configure(surface_id, &self.device, &config)?;
        let configure_us = configure_start.elapsed().as_micros();

        // INV-RESIZE-EFFICIENT: Do NOT increment pipeline_generation on resize.
        // Pipeline recreation is format-dependent, not size-dependent.

        let total_us = resize_start.elapsed().as_micros();
        tracing::debug!(
            surface = surface_id.raw(),
            w = physical_size.0,
            h = physical_size.1,
            configure_us,
            total_us,
            "resize_render_target"
        );

        Ok(())
    }

    /// SPEC-HAL-081B: Get the number of registered render targets (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should not reason about registry cardinality.
    #[cfg(debug_assertions)]
    pub fn render_target_count(&self) -> usize {
        self.multi_targets.len()
    }

    /// SPEC-HAL-081B: Check if a render target is registered (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should not query registration state.
    #[cfg(debug_assertions)]
    pub fn has_render_target(&self, surface_id: crate::compositor::SurfaceId) -> bool {
        self.multi_targets.contains(surface_id)
    }

    /// SPEC-HAL-081B: Compile draw plan for rendering.
    ///
    /// Acquires textures from all registered render targets that appear in the
    /// compositor snapshot. One window is simply N=1 targets.
    ///
    /// # Arguments
    ///
    /// * `draw_plan` - The draw plan with MultiSurfaceContract snapshot
    ///
    /// # Returns
    ///
    /// * `Ok(Some(plan))` - Successfully compiled, ready for render_multi()
    /// * `Ok(None)` - Skipped (unfocused or surface error)
    /// * `Err(e)` - Fatal surface error (out of memory)
    ///
    /// # Panics
    ///
    /// * In debug builds, panics if a SurfaceId from the snapshot has no registered render target
    ///
    /// # SPEC-HAL-163: Surface-Scoped Scene Partitioning
    ///
    /// Each surface's scene is compiled independently (INV-SCENE-004: Renderer never sees a mixed scene).
    /// Per-surface data is stored in `MultiTargetDrawPlan.surface_data`.
    pub fn compile_multi(
        &mut self,
        draw_plan: &crate::renderer::draw_plan::DrawPlan<crate::compositor::MultiSurfaceContract>,
    ) -> Result<crate::renderer::RendererCompileOutcome<crate::renderer::compiled_draw_plan::MultiTargetDrawPlan>, crate::renderer::RendererCompileError>
    {
        let candidate = self
            .compile_multi_admitted(draw_plan, None, None, |_, _, _| ())?
            .map(|(compiled, ())| compiled);
        self.publish_multi_compile(candidate)
    }

    /// SPEC-HAL-024 E.5: App-owned per-target geometry readiness, without changing
    /// compositor registration/visibility or waiting for another target's output.
    #[allow(
        dead_code,
        reason = "HAL-385 8.3 retains the per-surface Compile entry and error envelope"
    )]
    pub(crate) fn compile_multi_for_surfaces(
        &mut self,
        draw_plan: &crate::renderer::draw_plan::DrawPlan<crate::compositor::MultiSurfaceContract>,
        surfaces: &std::collections::HashSet<crate::compositor::SurfaceId>,
    ) -> Result<crate::renderer::RendererCompileOutcome<crate::renderer::compiled_draw_plan::MultiTargetDrawPlan>, crate::renderer::RendererCompileError>
    {
        let candidate = self
            .compile_multi_admitted(draw_plan, Some(surfaces), None, |_, _, _| ())?
            .map(|(compiled, ())| compiled);
        self.publish_multi_compile(candidate)
    }

    pub(crate) fn compile_sdf_frame(
        &mut self,
        frame: SdfFramePlan,
        surfaces: Option<&std::collections::HashSet<SurfaceId>>,
    ) -> Result<crate::renderer::RendererCompileOutcome<SdfCompiledFrame>, crate::renderer::RendererCompileError> {
        let SdfFramePlan { plan, mut custody } = frame;
        let Some((compiled, ())) = self.compile_multi_admitted(
            &plan,
            surfaces,
            (!custody.is_empty()).then_some(&custody),
            |_, _, _| (),
        )?
        else {
            return Ok(crate::renderer::RendererCompileOutcome::NoTargets);
        };
        custody.retain_compiled(&compiled);
        self.publish_sdf_compile(SdfCompiledFrame {
            plan: compiled,
            custody,
            companions: None,
        })
    }

    pub(crate) fn compile_sdf_frame_with_presentation_bindings(
        &mut self,
        frame: SdfFramePlan,
        surfaces: Option<&std::collections::HashSet<SurfaceId>>,
        bindings: crate::renderer::custom_render_prepare::FrozenPresentationBindings<'_>,
    ) -> Result<crate::renderer::RendererCompileOutcome<SdfCompiledFrame>, crate::renderer::RendererCompileError> {
        if !bindings.has_recorded_companions() {
            return self.compile_sdf_frame(frame, surfaces);
        }
        self.compile_sdf_frame_with_companions(frame, surfaces, bindings)
    }

    fn validate_image_batch(
        &self,
        batch: &crate::renderer::batcher::DrawBatch,
        surface_id: SurfaceId,
        sdf_custody: Option<&SdfSceneCustody>,
    ) -> Result<(), ImageBatchRefusal> {
        let id = batch.key.texture_id.ok_or(ImageBatchRefusal::Unavailable)?;
        let entry = self
            .image_textures
            .get(&id)
            .ok_or(ImageBatchRefusal::Unavailable)?;
        if !self.loading_presence_view_matches(id, &entry.view) {
            return Err(ImageBatchRefusal::PresenceChanged);
        }
        if batch
            .key
            .image_generation
            .is_some_and(|generation| generation != entry.generation)
            || entry.admission.as_ref().is_some_and(|state| {
                crate::renderer::image_binding::ImageBinding {
                    state: state.clone(),
                }
                .availability()
                .is_err()
            })
            || self.protected_sdf_texture_ids.contains(&id)
                && !batch.key.image_generation.is_some_and(|generation| {
                    sdf_custody.is_some_and(|custody| {
                        custody.contains(
                            surface_id,
                            crate::renderer::image_binding::ImageBindingIdentity::new(
                                id, generation,
                            ),
                        )
                    })
                })
        {
            return Err(ImageBatchRefusal::Unavailable);
        }
        Ok(())
    }

    pub(crate) fn compiled_plot_image(
        &self,
        compiled: &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
        surface: SurfaceId,
        texture_id: u64,
    ) -> Option<crate::renderer::image_binding::ImageBindingIdentity> {
        use crate::renderer::batcher::PipelineId;
        use crate::renderer::compiled_draw_plan::SurfaceDrawCommand;
        let entry = self.image_textures.get(&texture_id)?;
        let data = compiled.surface_data.get(&surface)?;
        data.draw_commands.iter().any(|draw| {
            let SurfaceDrawCommand::Batch { batch_index, .. } = draw else { return false };
            data.frame_batches.batches.get(*batch_index).is_some_and(|batch| {
                batch.key.pipeline == PipelineId::Image
                    && batch.key.texture_id == Some(texture_id)
                    && batch.index_range.count > 0
                    && self.validate_image_batch(batch, surface, None).is_ok()
            })
        }).then(|| crate::renderer::image_binding::ImageBindingIdentity::new(texture_id, entry.generation))
    }

    fn compile_multi_admitted<C>(
        &mut self,
        draw_plan: &crate::renderer::draw_plan::DrawPlan<crate::compositor::MultiSurfaceContract>,
        surfaces: Option<&std::collections::HashSet<SurfaceId>>,
        sdf_custody: Option<&SdfSceneCustody>,
        after_required_text: impl FnOnce(
            &mut Self,
            &std::collections::HashMap<SurfaceId, SurfaceTextDemand>,
            &mut std::collections::HashSet<crate::renderer::glyph_atlas::GlyphKey>,
        ) -> C,
    ) -> Result<
        Option<(crate::renderer::compiled_draw_plan::MultiTargetDrawPlan, C)>,
        crate::renderer::RendererCompileError,
    > {
        use crate::compositor::Contracted;
        use crate::renderer::compiled_draw_plan::{MultiTargetDrawPlan, SurfaceCompiledData};
        use crate::renderer::render_graph::{
            AttachmentDesc, AttachmentSize, MainPassExecutor, OverlayPassExecutor,
            RenderGraphBuilder, RenderPassNode, ResourceRef,
        };
        use crate::renderer::render_target::TargetDescriptor;
        use std::collections::HashMap;

        // SPEC-HAL-082A: Focus gating removed from renderer (sovereignty fix).
        self.invalidate_compile_preparation();
        // The renderer is execution-only; "should we render" decisions belong
        // in the App layer where InteractionDomains / focus events live.

        // SPEC-HAL-081B: Ensure globals slab can hold all targets from snapshot.
        // No fixed upper bound - grows dynamically to any size until alloc fails.
        for visible in draw_plan.surface_stack.iter()
            .filter(|surface| surface.is_visible() && surfaces.is_none_or(|ids| ids.contains(&surface.id)))
        {
            if let Some(bloom) = draw_plan.surface_data.get(&visible.id).and_then(|scene| scene.bloom.as_ref()) {
                if self.multi_targets.configured_size_px(visible.id).is_none() {
                    return Err(bloom.error(crate::renderer::BloomCompileRefusal::TargetUnavailable));
                }
                if !self.multi_targets.has_active_extent(visible.id) {
                    return Err(bloom.error(crate::renderer::BloomCompileRefusal::InvalidTarget));
                }
            }
        }
        let output_surfaces: Vec<_> = draw_plan
            .surface_stack
            .iter()
            .filter(|surface| surface.is_visible())
            .filter(|surface| surfaces.is_none_or(|ids| ids.contains(&surface.id)))
            .filter(|surface| {
                self.multi_targets.configured_size_px(surface.id).is_none()
                    || self.multi_targets.has_active_extent(surface.id)
            })
            .collect();
        let required_slots = output_surfaces.len();
        for surface in &output_surfaces {
            if draw_plan.surface_data.get(&surface.id).is_some_and(|scene|scene.primitives.iter()
                .any(|p|matches!(p.data,PrimitiveData::FrostedGlass{..})))
                && self.multi_targets.configured_size_px(surface.id).is_none() {
                return Err(crate::renderer::frosted_glass::error(surface.id,None,
                    crate::renderer::FrostedGlassCompileRefusal::TargetUnavailable));
            }
        }
        self.ensure_globals_slab_capacity(required_slots);
        // SPEC-HAL-081B / INV-PATH-CLIPS: size each per-target clip slot to the
        // frame's largest clip count (1 fullscreen + that target's path clips) so
        // no target overruns its slot into the next. Grows on demand; never capped.
        let frame_max_clips = output_surfaces
            .iter()
            .filter_map(|d| draw_plan.surface_data.get(&d.id))
            .map(|scene| 1 + scene.path_clips.len())
            .max()
            .unwrap_or(1);
        let required_clip_stride = {
            let bytes = (frame_max_clips * std::mem::size_of::<ClipRectUniform>()) as u64;
            let aligned = (bytes + PATH_CLIPS_SLAB_ALIGNMENT - 1) / PATH_CLIPS_SLAB_ALIGNMENT
                * PATH_CLIPS_SLAB_ALIGNMENT;
            aligned.max(PATH_CLIPS_SLOT_SIZE)
        };
        self.ensure_path_clips_slab_capacity(required_slots, required_clip_stride);

        // SPEC-HAL-084: Track compile_multi() invocation for phase ordering enforcement
        #[cfg(debug_assertions)]
        {
            self.last_compile_epoch.set(Some(self.frame_epoch.value()));
            self.compile_consumed.set(false);
        }

        // INV-TEXT-RESIDENCY: All atlas admission precedes final UV lowering.
        let preparation = Arc::new(());
        self.current_preparation = Some(preparation.clone());
        let mut text_demands = HashMap::new();
        let mut gradient_preparations = HashMap::new();
        let mut gradient_params = HashMap::new();
        for surface in output_surfaces.iter().copied() {
            let scale = self.multi_targets.scale_factor(surface.id).unwrap_or(1.0);
            let demand = if let Some(scene) = draw_plan.surface_data.get(&surface.id) {
                let (vertices, indices, params) =
                    Self::build_gradient_buffers_from_surface_primitives(
                        &scene.primitives,
                        self.rounded_clip_aa_radius,
                    );
                if !params.is_empty() {
                    let mut hashes = Vec::with_capacity(params.len());
                    for (hash, uniform) in params {
                        if !gradient_params.contains_key(&hash)
                            && gradient_params.len() == MAX_GRADIENT_PARAMS_PER_FRAME
                        {
                            tracing::error!(
                                capacity = MAX_GRADIENT_PARAMS_PER_FRAME,
                                "Gradient parameter capacity exceeded before GPU preparation"
                            );
                            return Err(crate::renderer::RendererSurfaceError::OutOfMemory.into());
                        }
                        gradient_params.entry(hash).or_insert(uniform);
                        hashes.push(hash);
                    }
                    gradient_preparations.insert(surface.id, (vertices, indices, hashes));
                }
                SurfaceTextDemand::gather(&scene.primitives, &mut self.text_engine, scale)
            } else {
                SurfaceTextDemand::default()
            };
            text_demands.insert(surface.id, demand);
        }
        // Every target refers to the same immutable frame-wide parameter table.
        let mut gradient_params: Vec<_> = gradient_params.into_iter().collect();
        gradient_params.sort_unstable_by_key(|(hash, _)| *hash);
        let gradient_params_offsets = self.upload_gradient_params(&gradient_params);
        let mut protected = std::collections::HashSet::new();
        for surface in output_surfaces.iter().copied() {
            let scale = self.multi_targets.scale_factor(surface.id).unwrap_or(1.0);
            let rasterizations = text_demands.get_mut(&surface.id).unwrap().admit(
                &mut self.text_engine,
                scale,
                &mut self.glyph_atlas,
                &mut protected,
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
        }
        let optional_working_set = after_required_text(self, &text_demands, &mut protected);
        if self.glyph_atlas.texture_id != self.last_atlas_texture_id {
            self.pipeline_generation += 1;
            self.last_atlas_texture_id = self.glyph_atlas.texture_id;
        }

        // SPEC-HAL-163: Build per-surface compiled data
        let mut surface_data: HashMap<SurfaceId, SurfaceCompiledData> = HashMap::new();

        // Build target descriptors for each base surface in the snapshot.
        // INV-MT-003: Only base surfaces are in the snapshot - enforced by type system
        // (SurfaceStackSnapshot<MultiSurfaceContract> iterates SurfaceDescriptor<BaseSurfaceTag>)
        //
        // NOTE: No texture acquisition here. Acquisition is an execution-time concern
        // handled exclusively by render_multi(). compile_multi() produces purely
        // declarative GPU work descriptions.
        //
        // NOTE: TargetDescriptor is built from compositor snapshot (logical bounds)
        // and registry (physical size). Registry lookups for contains() are debug-only.
        // INV-SCISSOR-001: Physical size comes from authoritative REGISTRY storage
        // (the value passed to Surface::configure()), NOT from compositor snapshot,
        // and NOT derived from bounds × scale_factor (which causes float drift).
        let mut target_descriptors = Vec::new();

        // SPEC-HAL-163: Track vertex/index offsets for shared buffer accumulation
        let mut global_vertex_offset: u32 = 0;
        let mut global_index_offset: u32 = 0;
        // SPEC-HAL-263: Per-pipeline offset tracking for image buffer
        let mut global_image_vertex_offset: u32 = 0;
        let mut global_image_index_offset: u32 = 0;
        let mut all_vertices: Vec<RectVertex> = Vec::new();
        let mut all_indices: Vec<u16> = Vec::new();
        // SPEC-HAL-263: Separate image vertex/index accumulation
        let mut all_image_vertices: Vec<ImageVertex> = Vec::new();
        let mut all_image_indices: Vec<u16> = Vec::new();
        // SPEC-R-013-v2: Separate shadow vertex/index accumulation
        let mut global_shadow_vertex_offset: u32 = 0;
        let mut global_shadow_index_offset: u32 = 0;
        let mut all_shadow_vertices: Vec<ShadowVertex> = Vec::new();
        let mut all_shadow_indices: Vec<u16> = Vec::new();
        // SPEC-R-012-v2: Separate gradient vertex/index accumulation
        let mut global_gradient_vertex_offset: u32 = 0;
        let mut global_gradient_index_offset: u32 = 0;
        let mut all_gradient_vertices: Vec<GradientVertex> = Vec::new();
        let mut all_gradient_indices: Vec<u16> = Vec::new();
        let mut all_analytic_vertices: Vec<GradientVertex> = Vec::new();
        let mut all_analytic_indices: Vec<u16> = Vec::new();
        let mut all_analytic_params = Vec::new();
        // SPEC-R-022: Separate SDF rect vertex/index accumulation
        let mut global_sdf_rect_vertex_offset: u32 = 0;
        let mut global_sdf_rect_index_offset: u32 = 0;
        let mut all_sdf_rect_vertices: Vec<SdfRectVertex> = Vec::new();
        let mut all_sdf_rect_indices: Vec<u16> = Vec::new();
        // SPEC-HAL-272: Separate ColorPicker SV vertex/index accumulation
        let mut global_color_picker_sv_vertex_offset: u32 = 0;
        let mut global_color_picker_sv_index_offset: u32 = 0;
        let mut all_color_picker_sv_vertices: Vec<ColorPickerSvVertex> = Vec::new();
        let mut all_color_picker_sv_indices: Vec<u32> = Vec::new();
        // SPEC-HAL-278: Separate RadialGauge vertex/index accumulation
        let mut global_radial_gauge_vertex_offset: u32 = 0;
        let mut global_radial_gauge_index_offset: u32 = 0;
        let mut all_radial_gauge_vertices: Vec<RadialGaugeVertex> = Vec::new();
        let mut all_radial_gauge_indices: Vec<u32> = Vec::new();
        // SPEC-R-003 Stage 2B: Separate shape vertex/index accumulation
        let mut global_shape_vertex_offset: u32 = 0;
        let mut global_shape_index_offset: u32 = 0;
        let mut all_shape_vertices: Vec<ShapeVertex> = Vec::new();
        let mut all_shape_indices: Vec<u32> = Vec::new();
        let mut global_text_quad_offset: u32 = 0;
        let mut global_path_draw_offset: usize = 0;

        for surface_desc in output_surfaces.iter().copied() {
            let surface_id = surface_desc.id;

            // Debug-only: verify target is registered
            // In release builds, no registry access - we trust compositor snapshot validity.
            // If a target is missing, render_multi() will handle it via acquire() error.
            #[cfg(debug_assertions)]
            {
                if !self.multi_targets.contains(surface_id) {
                    panic!(
                        "SPEC-HAL-081B: No render target registered for SurfaceId {:?}. \
                         All surfaces in compositor snapshot must have registered targets.",
                        surface_id
                    );
                }
            }

            // Build sizes from compositor snapshot data only (no registry.get())
            // Logical size comes from compositor surface bounds
            let logical_w = surface_desc.bounds.width;
            let logical_h = surface_desc.bounds.height;
            let logical_size = (logical_w, logical_h);

            // INV-SCISSOR-001: Physical size from authoritative REGISTRY storage.
            // This is the exact (u32, u32) last passed to Surface::configure().
            // FORBIDDEN: deriving physical size from bounds × scale_factor (causes float drift).
            // The registry is the single source of truth because it calls Surface::configure().
            let physical_size = self
                .multi_targets
                .configured_size_px(surface_id)
                .unwrap_or_else(|| {
                    // This should never happen if register_render_target was called.
                    debug_assert!(
                        false,
                        "INV-SCISSOR-001: configured_size_px not available for surface {:?}. \
                         Target must be registered before compile_multi().",
                        surface_id
                    );
                    (0, 0)
                });

            // Debug-only invariants for compile_multi (INV-CM-001 through INV-CM-004)
            #[cfg(debug_assertions)]
            {
                // INV-CM-001: physical_size must be non-zero
                debug_assert!(
                    physical_size.0 > 0 && physical_size.1 > 0,
                    "INV-CM-001: physical_size must be non-zero for surface {:?}, got ({}, {})",
                    surface_id,
                    physical_size.0,
                    physical_size.1
                );

                // INV-CM-002: logical_size must be non-zero
                debug_assert!(
                    logical_w > 0.0 && logical_h > 0.0,
                    "INV-CM-002: logical_size must be non-zero for surface {:?}, got ({}, {})",
                    surface_id,
                    logical_w,
                    logical_h
                );

                // INV-CM-003: physical_size must match registry (already guaranteed by configured_size_px)
                // This is a sanity check that the value we got is the same if re-fetched
                let re_fetched = self.multi_targets.configured_size_px(surface_id);
                debug_assert!(
                    re_fetched == Some(physical_size),
                    "INV-CM-003: physical_size mismatch for surface {:?}: stored={:?} vs fetched={:?}",
                    surface_id,
                    physical_size,
                    re_fetched
                );

                // INV-CM-004: scale_factor must be positive
                let sf = self.multi_targets.scale_factor(surface_id).unwrap_or(1.0);
                debug_assert!(
                    sf > 0.0,
                    "INV-CM-004: scale_factor must be positive for surface {:?}, got {}",
                    surface_id,
                    sf
                );
            }

            // INV-SCALE-001: Use per-target scale factor from registry
            let scale_factor = self.multi_targets.scale_factor(surface_id).unwrap_or(1.0);

            // SPEC-HAL-081B: Compute offset for this target's globals in the slab buffer
            // Capacity was ensured above via ensure_globals_slab_capacity()
            let target_index = target_descriptors.len();
            let globals_offset = (target_index as u64 * GLOBALS_UNIFORM_ALIGNMENT) as u32;

            // INV-GLOBALS-OFFSET: Dynamic offset must be aligned and within buffer bounds
            #[cfg(debug_assertions)]
            {
                debug_assert!(
                    globals_offset as u64 % GLOBALS_UNIFORM_ALIGNMENT == 0,
                    "INV-GLOBALS-OFFSET: globals_offset {} not aligned to {}",
                    globals_offset,
                    GLOBALS_UNIFORM_ALIGNMENT
                );
                let globals_end = globals_offset as u64 + std::mem::size_of::<Globals>() as u64;
                let buffer_size = self.globals_slab_capacity as u64 * GLOBALS_UNIFORM_ALIGNMENT;
                debug_assert!(
                    globals_end <= buffer_size,
                    "INV-GLOBALS-OFFSET: globals_offset {} + sizeof(Globals) {} = {} exceeds buffer size {}",
                    globals_offset,
                    std::mem::size_of::<Globals>(),
                    globals_end,
                    buffer_size
                );
            }

            // SPEC-HAL-081B: Upload this target's globals to its distinct offset
            // This must happen during compile, not render, because all command buffers
            // will be submitted together and must reference different buffer regions.
            let proj_logical_w = physical_size.0 as f32 / scale_factor;
            let proj_logical_h = physical_size.1 as f32 / scale_factor;
            let globals = Globals::new(
                proj_logical_w,
                proj_logical_h,
                physical_size.0 as f32,
                physical_size.1 as f32,
                self.gpu_rounded_clips_enabled,
            );
            // Pad to GLOBALS_UNIFORM_ALIGNMENT bytes
            let mut globals_data = [0u8; GLOBALS_UNIFORM_ALIGNMENT as usize];
            globals_data[..std::mem::size_of::<Globals>()]
                .copy_from_slice(bytemuck::bytes_of(&globals));
            self.stage_buffer_upload(
                self.rect_globals_buffer.clone(),
                globals_offset as u64,
                &globals_data,
            );

            // SPEC-HAL-081B: Upload path globals at same offset (both use Globals struct)
            self.upload_path_globals(
                globals_offset,
                proj_logical_w,
                proj_logical_h,
                physical_size.0 as f32,
                physical_size.1 as f32,
            );

            // SPEC-HAL-163: Get this surface's scene data (INV-SCENE-001: surface-scoped)
            let surface_scene = draw_plan.surface_data.get(&surface_id);

            // SPEC-HAL-081B: Upload path clips for this target at its slot offset
            let path_clips_offset = (target_index as u64 * self.path_clips_slot_stride) as u32;
            if let Some(scene) = surface_scene {
                self.upload_path_clips_at_offset(
                    path_clips_offset as u64,
                    &scene.path_clips,
                    physical_size.0 as f32,
                    physical_size.1 as f32,
                    scale_factor,
                );
            }

            target_descriptors.push(TargetDescriptor {
                surface_id,
                physical_size,
                logical_size,
                scale_factor,
                globals_offset,
                path_clips_offset,
            });

            // SPEC-HAL-163: Build per-surface vertices/indices and adjust batch ranges
            if let Some(scene) = surface_scene {
                let analytic_vertex_offset = all_analytic_vertices.len() as u32;
                let analytic_index_offset = all_analytic_indices.len() as u32;
                let mut analytic_surface_params_offsets = HashMap::new();
                if scene.batches.iter().any(|b| {
                    b.key.pipeline == crate::renderer::batcher::PipelineId::AnalyticSurface
                }) {
                    use crate::renderer::pipelines::analytic_surface::{
                        AnalyticSurfaceParams, AnalyticSurfacePipeline, CAPACITY,
                    };
                    if self.analytic_surface.is_none() {
                        self.analytic_surface = Some(AnalyticSurfacePipeline::try_new(
                            &self.device,
                            self.surface_format,
                            &self.rect_globals_layout,
                        )?);
                    }
                    let stride = self.analytic_surface.as_ref().unwrap().stride;
                    for primitive in &scene.primitives {
                        let PrimitiveData::AnalyticSurface {
                            rect,
                            corners,
                            material,
                            ancestor,
                            ordinal,
                            ..
                        } = &primitive.data
                        else {
                            continue;
                        };
                        if all_analytic_params.len() == CAPACITY {
                            return Err(crate::renderer::RendererSurfaceError::OutOfMemory.into());
                        }
                        let slot = all_analytic_params.len() as u32;
                        let previous =
                            analytic_surface_params_offsets.insert(*ordinal, slot * stride);
                        assert!(
                            previous.is_none(),
                            "HAL386: duplicate surface-local analytic ordinal"
                        );
                        all_analytic_params.push(AnalyticSurfaceParams::new(
                            *rect,
                            *corners,
                            material,
                            *ancestor,
                            self.rounded_clip_aa_radius,
                        ));
                        let margin = material.halo_extent() + 2.0 / scale_factor;
                        let [x0, y0, x1, y1] = [
                            rect.x - margin,
                            rect.y - margin,
                            rect.x + rect.width + margin,
                            rect.y + rect.height + margin,
                        ];
                        let vertex = u16::try_from(all_analytic_vertices.len())
                            .map_err(|_| crate::renderer::RendererSurfaceError::OutOfMemory)?;
                        all_analytic_vertices.extend(
                            [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(GradientVertex::new),
                        );
                        all_analytic_indices.extend([
                            vertex,
                            vertex + 1,
                            vertex + 2,
                            vertex,
                            vertex + 2,
                            vertex + 3,
                        ]);
                    }
                }
                // Build vertices/indices from this surface's primitives
                let (surf_vertices, surf_indices) =
                    self.build_buffers_from_surface_primitives(&scene.primitives, scale_factor);

                // Adjust batch ranges for global buffer offset and clone
                // SPEC-HAL-263: Per-pipeline offset adjustment
                let mut adjusted_batches = scene.batches.clone();
                for batch in &mut adjusted_batches {
                    match batch.key.pipeline {
                        crate::renderer::batcher::PipelineId::Image
                        | crate::renderer::batcher::PipelineId::CanonicalOverNeutral => {
                            batch.vertex_range.start += global_image_vertex_offset;
                            batch.index_range.start += global_image_index_offset;
                        }
                        crate::renderer::batcher::PipelineId::Shadow => {
                            batch.vertex_range.start += global_shadow_vertex_offset;
                            batch.index_range.start += global_shadow_index_offset;
                        }
                        crate::renderer::batcher::PipelineId::Gradient => {
                            batch.vertex_range.start += global_gradient_vertex_offset;
                            batch.index_range.start += global_gradient_index_offset;
                        }
                        crate::renderer::batcher::PipelineId::AnalyticSurface => {
                            batch.vertex_range.start += analytic_vertex_offset;
                            batch.index_range.start += analytic_index_offset;
                        }
                        crate::renderer::batcher::PipelineId::ColorPickerSv => {
                            batch.vertex_range.start += global_color_picker_sv_vertex_offset;
                            batch.index_range.start += global_color_picker_sv_index_offset;
                        }
                        crate::renderer::batcher::PipelineId::RadialGauge => {
                            batch.vertex_range.start += global_radial_gauge_vertex_offset;
                            batch.index_range.start += global_radial_gauge_index_offset;
                        }
                        crate::renderer::batcher::PipelineId::SdfRect => {
                            batch.vertex_range.start += global_sdf_rect_vertex_offset;
                            batch.index_range.start += global_sdf_rect_index_offset;
                        }
                        crate::renderer::batcher::PipelineId::Shape => {
                            batch.vertex_range.start += global_shape_vertex_offset;
                            batch.index_range.start += global_shape_index_offset;
                        }
                        _ => {
                            batch.vertex_range.start += global_vertex_offset;
                            batch.index_range.start += global_index_offset;
                        }
                    }
                }

                // Compute overlay_start_batch_idx for this surface
                let overlay_start_batch_idx = adjusted_batches
                    .iter()
                    .position(|b| b.key.z_layer >= crate::core::paint::LAYER_OVERLAY)
                    .unwrap_or(adjusted_batches.len());

                // Build FrameBatches for this surface
                let frame_batches = FrameBatches {
                    batches: adjusted_batches,
                    total_vertices: scene.total_vertices,
                    total_indices: scene.total_indices,
                    total_image_vertices: scene.total_image_vertices,
                    total_image_indices: scene.total_image_indices,
                    paths: scene.path_draws.clone(),
                    path_clips: scene.path_clips.clone(),
                    overlay_start_batch_idx,
                };

                let demand = text_demands.get_mut(&surface_id).unwrap();
                let (text_quads, text_draw_commands) = match self
                    .build_text_quads_from_surface_primitives(
                        &scene.primitives,
                        demand,
                        scale_factor,
                    ) {
                    Ok(prepared) => { demand.markers_prepared = true; prepared },
                    Err(failure) => {
                        demand.failure = Some(failure);
                        (Vec::new(), Vec::new())
                    }
                };
                let text_realization = demand.realization(surface_id, self.frame_epoch);
                let text_quad_offset = global_text_quad_offset;
                let text_quad_count = text_quads.len() as u32;
                let path_draw_offset = global_path_draw_offset;
                let path_draw_count = frame_batches
                    .paths
                    .iter()
                    .filter(|path| !path.mesh.vertices.is_empty() && !path.mesh.indices.is_empty())
                    .count();

                // Prepare rounded clips for this surface
                let rounded_clip_offsets =
                    self.prepare_rounded_clips(&frame_batches, logical_w, logical_h);

                // Convert clear color
                let clear_color = wgpu::Color {
                    r: scene.clear_color.r as f64,
                    g: scene.clear_color.g as f64,
                    b: scene.clear_color.b as f64,
                    a: scene.clear_color.a as f64,
                };

                // Accumulate into global buffers. Batch ranges were shifted above;
                // index values must be rebased too or later surfaces draw surface 0 geometry.
                let surf_vertex_count = surf_vertices.len() as u32;
                let surf_index_count = surf_indices.len() as u32;
                all_vertices.extend(surf_vertices);
                Self::extend_rebased_u16_indices(
                    &mut all_indices,
                    surf_indices,
                    global_vertex_offset,
                );

                // SPEC-HAL-263: Build and accumulate image vertices for this surface
                let (surf_image_verts, surf_image_idxs) =
                    Self::build_image_buffers_from_surface_primitives(&scene.primitives);
                let surf_image_vertex_count = surf_image_verts.len() as u32;
                let surf_image_index_count = surf_image_idxs.len() as u32;
                all_image_vertices.extend(surf_image_verts);
                Self::extend_rebased_u16_indices(
                    &mut all_image_indices,
                    surf_image_idxs,
                    global_image_vertex_offset,
                );

                // SPEC-R-013-v2: Build and accumulate shadow vertices for this surface
                let (surf_shadow_verts, surf_shadow_idxs, surf_shadow_params) =
                    Self::build_shadow_buffers_from_surface_primitives(&scene.primitives);
                let shadow_params_offsets = self.upload_shadow_params(&surf_shadow_params);
                let surf_shadow_vertex_count = surf_shadow_verts.len() as u32;
                let surf_shadow_index_count = surf_shadow_idxs.len() as u32;
                all_shadow_vertices.extend(surf_shadow_verts);
                Self::extend_rebased_u16_indices(
                    &mut all_shadow_indices,
                    surf_shadow_idxs,
                    global_shadow_vertex_offset,
                );

                // SPEC-R-012-v2: Build and accumulate gradient vertices for this surface
                let (surf_gradient_verts, surf_gradient_idxs, surf_gradient_hashes) =
                    gradient_preparations
                        .remove(&surface_id)
                        .unwrap_or_default();
                let surface_gradient_params_offsets = surf_gradient_hashes
                    .into_iter()
                    .map(|hash| (hash, gradient_params_offsets[&hash]))
                    .collect();
                let surf_gradient_vertex_count = surf_gradient_verts.len() as u32;
                let surf_gradient_index_count = surf_gradient_idxs.len() as u32;
                all_gradient_vertices.extend(surf_gradient_verts);
                Self::extend_rebased_u16_indices(
                    &mut all_gradient_indices,
                    surf_gradient_idxs,
                    global_gradient_vertex_offset,
                );

                // SPEC-HAL-272: Build and accumulate color picker SV vertices for this surface
                let (surf_cp_sv_verts, surf_cp_sv_idxs, surf_cp_sv_params) =
                    Self::build_color_picker_sv_buffers_from_surface_primitives(
                        &scene.primitives,
                        scale_factor,
                    );
                let color_picker_sv_params_offsets =
                    self.upload_color_picker_sv_params(&surf_cp_sv_params);
                let surf_cp_sv_vertex_count = surf_cp_sv_verts.len() as u32;
                let surf_cp_sv_index_count = surf_cp_sv_idxs.len() as u32;
                all_color_picker_sv_vertices.extend(surf_cp_sv_verts);
                Self::extend_rebased_u32_indices(
                    &mut all_color_picker_sv_indices,
                    surf_cp_sv_idxs,
                    global_color_picker_sv_vertex_offset,
                );

                // SPEC-HAL-278: Build and accumulate radial gauge vertices for this surface
                let (surf_rg_verts, surf_rg_idxs, surf_rg_params) =
                    Self::build_radial_gauge_buffers_from_surface_primitives(
                        &scene.primitives,
                        scale_factor,
                    );
                let radial_gauge_params_offsets = self.upload_radial_gauge_params(&surf_rg_params);
                let surf_rg_vertex_count = surf_rg_verts.len() as u32;
                let surf_rg_index_count = surf_rg_idxs.len() as u32;
                all_radial_gauge_vertices.extend(surf_rg_verts);
                Self::extend_rebased_u32_indices(
                    &mut all_radial_gauge_indices,
                    surf_rg_idxs,
                    global_radial_gauge_vertex_offset,
                );

                // SPEC-R-022: Build and accumulate SDF rect vertices for this surface
                let (surf_sdf_verts, surf_sdf_idxs) =
                    Self::build_sdf_rect_buffers_from_surface_primitives(&scene.primitives);
                let surf_sdf_vertex_count = surf_sdf_verts.len() as u32;
                let surf_sdf_index_count = surf_sdf_idxs.len() as u32;
                all_sdf_rect_vertices.extend(surf_sdf_verts);
                Self::extend_rebased_u16_indices(
                    &mut all_sdf_rect_indices,
                    surf_sdf_idxs,
                    global_sdf_rect_vertex_offset,
                );

                // SPEC-R-003 Stage 2B: Build and accumulate shape SDF vertices for this surface
                let (surf_shape_verts, surf_shape_idxs) =
                    Self::build_shape_buffers_from_surface_primitives(&scene.primitives);
                let surf_shape_vertex_count = surf_shape_verts.len() as u32;
                let surf_shape_index_count = surf_shape_idxs.len() as u32;
                all_shape_vertices.extend(surf_shape_verts);
                Self::extend_rebased_u32_indices(
                    &mut all_shape_indices,
                    surf_shape_idxs,
                    global_shape_vertex_offset,
                );

                // Update global offsets for next surface
                global_vertex_offset += surf_vertex_count;
                global_index_offset += surf_index_count;
                global_image_vertex_offset += surf_image_vertex_count;
                global_image_index_offset += surf_image_index_count;
                global_shadow_vertex_offset += surf_shadow_vertex_count;
                global_shadow_index_offset += surf_shadow_index_count;
                global_gradient_vertex_offset += surf_gradient_vertex_count;
                global_gradient_index_offset += surf_gradient_index_count;
                global_color_picker_sv_vertex_offset += surf_cp_sv_vertex_count;
                global_color_picker_sv_index_offset += surf_cp_sv_index_count;
                global_radial_gauge_vertex_offset += surf_rg_vertex_count;
                global_radial_gauge_index_offset += surf_rg_index_count;
                global_sdf_rect_vertex_offset += surf_sdf_vertex_count;
                global_sdf_rect_index_offset += surf_sdf_index_count;
                global_shape_vertex_offset += surf_shape_vertex_count;
                global_shape_index_offset += surf_shape_index_count;
                global_text_quad_offset += text_quad_count;
                global_path_draw_offset += path_draw_count;

                // Store per-surface compiled data
                surface_data.insert(
                    surface_id,
                    SurfaceCompiledData {
                        text_realization,
                        frame_batches,
                        text_quads,
                        text_quad_offset,
                        path_draw_offset,
                        draw_commands: text_draw_commands,
                        clear_color,
                        rounded_clip_offsets,
                        shadow_params_offsets,
                        gradient_params_offsets: surface_gradient_params_offsets,
                        analytic_surface_params_offsets,
                        color_picker_sv_params_offsets,
                        radial_gauge_params_offsets,
                    },
                );
            } else {
                // INV-SCENE-005: Empty surface is valid
                let empty_frame_batches = FrameBatches {
                    batches: Vec::new(),
                    total_vertices: 0,
                    total_indices: 0,
                    total_image_vertices: 0,
                    total_image_indices: 0,
                    paths: Vec::new(),
                    path_clips: Vec::new(),
                    overlay_start_batch_idx: 0,
                };
                surface_data.insert(
                    surface_id,
                    SurfaceCompiledData {
                        text_realization: SurfaceTextRealization::new(
                            surface_id,
                            self.frame_epoch,
                            text_demands[&surface_id].outcome(),
                        ),
                        frame_batches: empty_frame_batches,
                        text_quads: Vec::new(),
                        text_quad_offset: global_text_quad_offset,
                        path_draw_offset: global_path_draw_offset,
                        draw_commands: Vec::new(),
                        clear_color: wgpu::Color::BLACK,
                        rounded_clip_offsets: HashMap::new(),
                        shadow_params_offsets: HashMap::new(),
                        gradient_params_offsets: HashMap::new(),
                        analytic_surface_params_offsets: HashMap::new(),
                        color_picker_sv_params_offsets: HashMap::new(),
                        radial_gauge_params_offsets: HashMap::new(),
                    },
                );
            }
        }

        if target_descriptors.is_empty() {
            return Ok(None);
        }

        let mut render_graph_builder = RenderGraphBuilder::new();
        let render_graph_output = render_graph_builder.declare_attachment(AttachmentDesc {
            label: "RenderGraph Output Attachment",
            size: AttachmentSize::SWAPCHAIN_FULL,
            format: self.surface_format,
            samples: 1,
            clear_color: None,
            load_op: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store_op: wgpu::StoreOp::Store,
        });
        let main_pass = render_graph_builder.add_pass(
            RenderPassNode::new(
                "main_pass",
                Box::new(MainPassExecutor::new(render_graph_output)),
            )
            .write(ResourceRef::Attachment(render_graph_output)),
        );
        render_graph_builder.add_pass(
            RenderPassNode::new(
                "overlay_pass",
                Box::new(OverlayPassExecutor::new(render_graph_output)),
            )
            .read(ResourceRef::Attachment(render_graph_output))
            .write(ResourceRef::Attachment(render_graph_output))
            .depends_on(main_pass),
        );
        let render_graph_topology = render_graph_builder
            .build(draw_plan)
            .expect("SPEC-HAL-084: renderer-owned RenderGraph topology must be valid");

        // SPEC-HAL-040 Stage 7: Validate pipelines before buffer operations
        self.validate_pipelines();

        // SPEC-HAL-263B / INV-IMG-005: Validate the final executable state,
        // after same-compile pipeline and bind-group revalidation. In release,
        // remove rejected image batches rather than leaving Execute to discover
        // an unavailable registration or obsolete admitted generation.
        for target in &target_descriptors {
            let Some(surface_compiled) = surface_data.get_mut(&target.surface_id) else {
                continue;
            };
            let last_frost=draw_plan.surface_data.get(&target.surface_id).and_then(|scene|scene.primitives.iter()
                .filter(|p|matches!(p.data,PrimitiveData::FrostedGlass{..}))
                .map(|p|p.key.effective_order()).max());
            let mut incomplete_prefix=false;
            surface_compiled.frame_batches.batches.retain(|batch| {
                if !matches!(batch.key.pipeline,PipelineId::Image|PipelineId::CanonicalOverNeutral) {
                    return true;
                }

                let texture_id = batch.key.texture_id;
                if last_frost.as_ref().is_some_and(|last|batch.key.effective_order()<*last)
                    && self.validate_image_batch(batch,target.surface_id,sdf_custody).is_err() {
                    incomplete_prefix=true;return false;
                }
                match self.validate_image_batch(batch, target.surface_id, sdf_custody) {
                    Ok(()) => return true,
                    Err(ImageBatchRefusal::PresenceChanged) => {
                        tracing::error!(?texture_id,"HAL381 private presentation registration changed; withdrawing the unqualified image");
                        return false;
                    }
                    Err(ImageBatchRefusal::Unavailable) => {}
                }

                #[cfg(debug_assertions)]
                panic!(
                    "INV-IMG-005: Image batch references texture_id={texture_id:?} with unavailable registration, missing SDF custody or generation {:?}. \
                     Register the texture before submitting ImageNodes to the scene."
                    , batch.key.image_generation
                );

                #[cfg(not(debug_assertions))]
                {
                    tracing::warn!(
                        texture_id = ?texture_id,
                        image_generation = ?batch.key.image_generation,
                        "INV-IMG-005: Skipping image batch — registration or admitted generation unavailable"
                    );
                    false
                }
            });
            if incomplete_prefix {
                return Err(crate::renderer::frosted_glass::error(target.surface_id,None,
                    crate::renderer::FrostedGlassCompileRefusal::IncompletePrefix));
            }
            surface_compiled.frame_batches.overlay_start_batch_idx = surface_compiled
                .frame_batches
                .batches
                .iter()
                .position(|batch| batch.key.z_layer >= crate::core::paint::LAYER_OVERLAY)
                .unwrap_or(surface_compiled.frame_batches.batches.len());

            // SPEC-HAL-359: Compile one immutable per-surface execution schedule
            // after every admission/revalidation step that can remove a batch.
            // Equal-Z ordering deliberately preserves the historical broad
            // family order; semantic overlap requires distinct Z values.
            let text_commands = std::mem::take(&mut surface_compiled.draw_commands);
            surface_compiled.draw_commands =
                build_surface_draw_commands(&surface_compiled.frame_batches, text_commands);
        }

        // Close every optical topology and the combined plan budget before
        // reserving any shared HAL385/395 physical backing.
        use crate::renderer::{
            BloomCompileRefusal, bloom::BloomPlan,
            frosted_glass::{self, FrostedGlassCompileRefusal as Refusal},
        };
        let mut frosted_surfaces = HashMap::new();
        let mut optical_bytes = 0_u64;
        for target in &target_descriptors {
            let id = target.surface_id;
            let Some(scene) = draw_plan.surface_data.get(&id) else { continue };
            let has_frost = scene.primitives.iter().any(|p| matches!(p.data, PrimitiveData::FrostedGlass { .. }));
            if !has_frost && scene.bloom.is_none() { continue; }
            let surface = surface_data.get_mut(&id).expect("current compiled surface");
            if !surface.text_realization().is_prepared_for_output() {
                return Err(if has_frost {
                    frosted_glass::error(id, None, Refusal::IncompletePrefix)
                } else {
                    scene.bloom.as_ref().unwrap().error(BloomCompileRefusal::MissingCompiledSource)
                });
            }
            let mut occurrences = Vec::new();
            if has_frost {
                if !frosted_glass::parameters::supported_format(self.surface_format) {
                    return Err(frosted_glass::error(id, None, Refusal::UnsupportedFormat));
                }
                let features = self.adapter.get_texture_format_features(self.surface_format);
                let half = self.adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float);
                let sampled_attachment = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;
                if !features.allowed_usages.contains(sampled_attachment | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST) || !half.allowed_usages.contains(sampled_attachment)
                {
                    return Err(frosted_glass::error(id, None, Refusal::UnsupportedFormat));
                }
                let clear = surface.clear_color;
                if ![clear.r, clear.g, clear.b, clear.a].into_iter().all(|v| (0.0..=1.0).contains(&v))
                    || [clear.r, clear.g, clear.b].into_iter().any(|v| v > clear.a)
                {
                    return Err(frosted_glass::error(id, None, Refusal::NonPremultipliedClear));
                }
                for p in &scene.primitives {
                    let node_key = match &p.data { PrimitiveData::FrostedGlass { node_key, .. } => *node_key, _ => None };
                    if let Some(o) = frosted_glass::parameters::occurrence(p, target, self.rounded_clip_aa_radius)
                        .map_err(|reason| frosted_glass::error(id, node_key, reason))?
                    {
                        occurrences.push(o);
                    }
                }
                surface.draw_commands.retain(|cmd| match cmd {
                    SurfaceDrawCommand::FrostedGlass { ordinal, .. } => occurrences.iter().any(|o| o.ordinal == *ordinal),
                    _ => true,
                });
            }
            let bloom = if let Some(source) = &scene.bloom {
                Some(BloomPlan::qualify(source, scene, surface, target, self.surface_format,
                    &self.adapter, &self.device.limits(), self.rounded_clip_aa_radius)
                    .map_err(|reason| source.error(reason))?)
            } else {
                None
            };
            if occurrences.is_empty() && bloom.is_none() { continue; }
            let optical_error = |reason| {
                if let Some(source) = &scene.bloom {
                    match reason {
                        Refusal::ResourceBudget { required_bytes, limit_bytes } =>
                            return source.error(BloomCompileRefusal::ResourceBudget { required_bytes, limit_bytes }),
                        Refusal::Graph(error) => return source.error(BloomCompileRefusal::Graph(error)),
                        _ => {}
                    }
                }
                frosted_glass::error(id, None, reason)
            };
            let plan = frosted_glass::FrostedSurfacePlan::new(
                draw_plan, &surface.draw_commands, occurrences, bloom, target.physical_size, self.surface_format,
            ).map_err(&optical_error)?;
            optical_bytes = optical_bytes.saturating_add(plan.bytes);
            if optical_bytes > frosted_glass::PLAN_BUDGET {
                return Err(optical_error(Refusal::ResourceBudget {
                    required_bytes: optical_bytes, limit_bytes: frosted_glass::PLAN_BUDGET,
                }));
            }
            frosted_surfaces.insert(id, plan);
        }
        #[cfg(target_arch = "wasm32")]
        if !frosted_surfaces.is_empty() {
            self.preparation_gate = Some(crate::renderer::RendererPreparationIdentity::new(preparation.clone()));
        }
        for target in &target_descriptors {
            if let Some(plan)=frosted_surfaces.get(&target.surface_id) {
                if let Err(error) = self.frosted_glass.prepare(
                    &self.device, &self.queue, &self.rect_globals_layout, target.surface_id, plan,
                    self.preparation_gate.as_ref(),
                ) {
                    self.invalidate_compile_preparation();
                    return Err(error);
                }
            }
        }

        // ========================================================================
        // SPEC-HAL-163: TRANSITIONAL COMPATIBILITY LAYER
        // ========================================================================
        //
        // This combined FrameBatches exists ONLY to feed the legacy GPU buffer
        // update path (`build_gpu_update` / `apply_gpu_update`), which expects
        // a single FrameBatches for the entire frame.
        //
        // **This is explicitly transitional.**
        //
        // Semantic limitations of combined batches:
        // - `overlay_start_batch_idx = 0` is NOT meaningful globally; overlay
        //   ordering is per-surface, computed in SurfaceCompiledData.
        // - Batch ordering across surfaces is now implicit (HashMap iteration
        //   order). Per-surface ordering is preserved within each surface.
        // - Path clip indices are surface-local; combined clips may have
        //   duplicates across surfaces.
        //
        // Future: GPU buffer management should be refactored to be per-surface
        // or explicitly multi-surface aware. This combined path is a bridge.
        // ========================================================================
        let mut combined_batches = Vec::new();
        let mut combined_paths = Vec::new();
        let mut combined_path_clips = Vec::new();
        // ====================================================================
        // SPEC-HAL-163 (INV-SCENE-004): DETERMINISTIC ITERATION REQUIRED
        // ====================================================================
        // MUST iterate in surface_stack order, NOT HashMap::values().
        // HashMap iteration is nondeterministic and violates INV-SCENE-004.
        // Reverting to `surface_data.values()` is an invariant violation.
        // ====================================================================
        for surface_desc in draw_plan.surface_stack.iter() {
            if let Some(surface_compiled) = surface_data.get(&surface_desc.id) {
                combined_batches.extend(surface_compiled.frame_batches.batches.clone());
                combined_paths.extend(surface_compiled.frame_batches.paths.clone());
                combined_path_clips.extend(surface_compiled.frame_batches.path_clips.clone());
            }
        }
        let combined_frame_batches = FrameBatches {
            batches: combined_batches,
            total_vertices: global_vertex_offset,
            total_indices: global_index_offset,
            total_image_vertices: global_image_vertex_offset,
            total_image_indices: global_image_index_offset,
            paths: combined_paths,
            path_clips: combined_path_clips,
            // TRANSITIONAL: Not meaningful for combined batches; per-surface
            // overlay ordering is in SurfaceCompiledData.frame_batches
            overlay_start_batch_idx: 0,
        };

        // SPEC-HAL-039: Update GPU buffers with combined data
        let update = self.build_gpu_update(
            self.last_batches.as_ref(),
            &combined_frame_batches,
            all_vertices,
            all_indices,
        );
        self.apply_gpu_update(update);

        // SPEC-HAL-263: Upload image geometry to dedicated GPU buffers
        self.update_image_gpu_buffers(&all_image_vertices, &all_image_indices);

        // SPEC-R-013-v2: Upload shadow geometry to dedicated GPU buffers
        self.update_shadow_gpu_buffers(&all_shadow_vertices, &all_shadow_indices);

        // SPEC-R-012-v2: Upload gradient geometry to dedicated GPU buffers
        self.update_gradient_gpu_buffers(&all_gradient_vertices, &all_gradient_indices);
        if !all_analytic_params.is_empty() {
            let resources = self
                .analytic_surface
                .as_ref()
                .expect("HAL386: admitted analytic resources");
            let (params, vertices, indices, stride) = (
                resources.params.clone(),
                resources.vertices.clone(),
                resources.indices.clone(),
                resources.stride as usize,
            );
            let mut bytes = vec![0u8; stride * all_analytic_params.len()];
            for (slot, value) in all_analytic_params.iter().enumerate() {
                let record = bytemuck::bytes_of(value);
                bytes[slot * stride..slot * stride + record.len()].copy_from_slice(record);
            }
            self.stage_buffer_upload(params, 0, &bytes);
            self.stage_buffer_upload(vertices, 0, bytemuck::cast_slice(&all_analytic_vertices));
            self.stage_buffer_upload(indices, 0, bytemuck::cast_slice(&all_analytic_indices));
        }

        // SPEC-HAL-272: Upload ColorPicker SV geometry to dedicated GPU buffers
        self.update_color_picker_sv_gpu_buffers(
            &all_color_picker_sv_vertices,
            &all_color_picker_sv_indices,
        );

        // SPEC-HAL-278: Upload RadialGauge geometry to dedicated GPU buffers
        self.update_radial_gauge_gpu_buffers(&all_radial_gauge_vertices, &all_radial_gauge_indices);

        // SPEC-R-022: Upload SDF rect geometry to dedicated GPU buffers
        self.update_sdf_rect_gpu_buffers(&all_sdf_rect_vertices, &all_sdf_rect_indices);

        // SPEC-R-003 Stage 2B: Upload analytic shape geometry to dedicated GPU buffers
        self.update_shape_gpu_buffers(&all_shape_vertices, &all_shape_indices);

        // SPEC-HAL-163 / SPEC-HAL-266: Text vertices are still uploaded through
        // one legacy GPU buffer, but each surface's quads must be converted using
        // that surface's physical size and scale factor before concatenation.
        let mut text_vertices = Vec::new();
        let mut text_indices = Vec::new();
        for surface_desc in draw_plan.surface_stack.iter() {
            let Some(surface_compiled) = surface_data.get(&surface_desc.id) else {
                continue;
            };
            if surface_compiled.text_quads.is_empty() {
                continue;
            }
            let Some(target) = target_descriptors
                .iter()
                .find(|target| target.surface_id == surface_desc.id)
            else {
                continue;
            };

            let vertex_base = text_vertices.len() as u32;
            let (surface_text_vertices, surface_text_indices) = Self::build_text_buffers(
                &surface_compiled.text_quads,
                target.scale_factor,
                target.physical_size.0,
                target.physical_size.1,
            );
            text_vertices.extend(surface_text_vertices);
            Self::extend_rebased_u32_indices(&mut text_indices, surface_text_indices, vertex_base);
        }

        // Build and upload text vertex/index buffers
        self.update_text_gpu_buffers(&text_vertices, &text_indices);

        // SPEC-R-004: Prepare path batch (clips already uploaded per-target above)
        self.build_path_batch_from_frame(&combined_frame_batches);
        {
            let device = self.device.as_ref();
            let queue = self.queue.as_ref();
            let gpu_arena = &mut self.gpu_arena;
            let staging_pool = &mut self.staging_pool;
            let frame_epoch = self.frame_epoch;
            let staging_upload_encoder = &mut self.staging_upload_encoder;
            let staging_upload_handles = &mut self.staging_upload_handles;

            self.path_renderer
                .prepare_frame_with_upload(device, |destination, data| {
                    stage_buffer_upload_parts(
                        device,
                        queue,
                        gpu_arena,
                        staging_pool,
                        frame_epoch,
                        staging_upload_encoder,
                        staging_upload_handles,
                        destination,
                        0,
                        data,
                    );
                });
        }

        self.flush_staging_uploads();

        // Remember batch layout for next frame diff
        self.last_batches = Some(combined_frame_batches);

        let presence = self.loading_presence_frame_uses(&surface_data);
        Ok(Some((
            MultiTargetDrawPlan {
                frosted_surfaces,
                presence,
                preparation,
                preparation_epoch: self.frame_epoch,
                target_descriptors,
                surface_data,
                surface_stack: draw_plan.surface_stack.clone(),
                render_graph_topology,
                render_graph_output,
                _phase: Contracted::new(),
            },
            optional_working_set,
        )))
    }

    /// SPEC-HAL-163: Build vertex/index buffers from a surface's primitives.
    ///
    /// INV-SCENE-001: Each surface's primitives are processed independently.
    fn build_buffers_from_surface_primitives(
        &self,
        primitives: &[crate::renderer::batcher::FlatPrimitive],
        scale_factor: f32,
    ) -> (Vec<RectVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for prim in primitives.iter() {
            match &prim.data {
                PrimitiveData::Rect { rect, color } => {
                    self.add_rect(rect, color, &mut vertices, &mut indices);
                }
                PrimitiveData::BorderRect {
                    rect,
                    fill_color,
                    border_color,
                    border_width,
                    radius: _,
                } => {
                    self.add_rect(rect, fill_color, &mut vertices, &mut indices);
                    let bw = *border_width;
                    if bw > 0.0 {
                        // SPEC-HAL-266: Quantize stroke geometry to the device-pixel
                        // grid. Snap each outer edge to a whole device pixel and round
                        // the stroke thickness to a whole number of device pixels (>= 1).
                        // Without this, a `bw`-logical edge scales to `bw * scale_factor`
                        // device px at a fractional origin and rasterizes as 1px or 2px
                        // depending on sub-pixel phase, producing uneven top/bottom
                        // borders that drift with scroll at fractional DPI.
                        let sf = scale_factor;
                        let x0 = snap_to_physical(rect.x, sf);
                        let y0 = snap_to_physical(rect.y, sf);
                        let x1 = snap_to_physical(rect.x + rect.width, sf);
                        let y1 = snap_to_physical(rect.y + rect.height, sf);
                        let bw = ((bw * sf).round().max(1.0)) / sf;
                        let w = x1 - x0;
                        let h = y1 - y0;
                        let top = RectF::new(x0, y0, w, bw);
                        self.add_rect(&top, border_color, &mut vertices, &mut indices);
                        let bottom = RectF::new(x0, y1 - bw, w, bw);
                        self.add_rect(&bottom, border_color, &mut vertices, &mut indices);
                        let left = RectF::new(x0, y0 + bw, bw, h - 2.0 * bw);
                        self.add_rect(&left, border_color, &mut vertices, &mut indices);
                        let right = RectF::new(x1 - bw, y0 + bw, bw, h - 2.0 * bw);
                        self.add_rect(&right, border_color, &mut vertices, &mut indices);
                    }
                }
                PrimitiveData::RoundedRect { rect, color, .. } => {
                    self.add_rect(rect, color, &mut vertices, &mut indices);
                }
                PrimitiveData::Text { .. } => {}
                PrimitiveData::Image { .. } => {} // SPEC-HAL-263: emitted via build_image_buffers
                PrimitiveData::Circle { .. } => {}
                PrimitiveData::Ellipse { .. } => {}
                PrimitiveData::Line { .. } => {}
                PrimitiveData::Polyline { .. } => {}
                PrimitiveData::LuminousContour { .. } => {}
                PrimitiveData::LocalAmbientGlow { .. } => {}
                PrimitiveData::Shadow { .. } => {} // SPEC-R-013-v2: separate shadow buffers
                PrimitiveData::GradientRect { .. } => {} // SPEC-R-012-v2: separate gradient buffers
                PrimitiveData::AnalyticSurface { .. } => {} // HAL386: dedicated bounded buffers
                PrimitiveData::FrostedGlass {..} => {} // HAL385: graph barrier, no ordinary vertices
                PrimitiveData::ColorPickerSv { .. } => {} // SPEC-HAL-272: separate SV buffers
                PrimitiveData::RadialGauge { .. } => {} // SPEC-HAL-278: separate radial gauge buffers
                PrimitiveData::SdfRect { .. } => {}     // SPEC-R-022: separate SDF rect buffers
            }
        }

        (vertices, indices)
    }

    /// SPEC-HAL-163: Build text quads from a surface's primitives.
    ///
    /// INV-SCENE-001: Each surface's primitives are processed independently.
    /// INV-SIZE-001: Takes scale_factor parameter for per-target rendering.
    fn build_text_quads_from_surface_primitives(
        &self,
        primitives: &[crate::renderer::batcher::FlatPrimitive],
        demand: &SurfaceTextDemand,
        scale_factor: f32,
    ) -> Result<(Vec<TextQuad>, Vec<SurfaceDrawCommand>), TextRealizationFailure> {
        if let Some(failure) = demand.failure {
            return Err(failure);
        }
        let mut text_quads = Vec::new();
        let mut draw_commands = Vec::new();
        for run in &demand.runs {
            let prim = &primitives[run.primitive_index];
            let PrimitiveData::Text {
                position,
                color,
                backgrounds,
                ..
            } = &prim.data
            else {
                unreachable!("text demand is indexed into its frozen surface");
            };
            let quad_start = text_quads.len() as u32;
            let current_clip = (!prim.clip_state.is_none()).then_some(prim.clip_state.rect);
            for bg in backgrounds {
                text_quads.push(text_background_quad(
                    RectF::new(
                        position.x + bg.x_offset,
                        position.y + bg.y_offset,
                        bg.width,
                        bg.height,
                    ),
                    bg.color,
                    current_clip,
                ));
            }
            // HAL360 §7: geometry only, in the original contiguous text command.
            for &interval in &run.markers {
                let rects = crate::renderer::text_realization::missing_cluster_rects(
                    [position.x,position.y], interval, run.metrics, scale_factor,
                ).ok_or(TextRealizationFailure {
                    primitive_index: run.primitive_index,
                    glyph_index: run.keys.iter().position(|key|key.glyph_id==0),
                    reason: TextRealizationFailureReason::InvalidTypographyInput(
                        crate::core::text_engine::TextInputError::PositionOverflow),
                })?;
                for rect in rects {
                    if rect.width>0.0 && rect.height>0.0 {
                        text_quads.push(text_background_quad(rect,*color,current_clip));
                    }
                }
            }
            let quad_params = GlyphToQuadParams {
                origin_phys_x: (position.x * scale_factor).floor(),
                origin_phys_y: (position.y * scale_factor).floor(),
                metrics: run.metrics,
                scale_factor,
                color: *color,
            };
            for (glyph_index, (glyph, key)) in
                run.glyph_run.glyphs.iter().zip(&run.keys).enumerate()
            {
                let ink = run.ink.get(glyph_index).copied().flatten();
                if run.suppressed.get(glyph_index) == Some(&true) {
                    continue;
                }
                if demand.no_ink.contains(key) {
                    if ink.is_some() {
                        return Err(TextRealizationFailure {
                            primitive_index: run.primitive_index,
                            glyph_index: Some(glyph_index),
                            reason: TextRealizationFailureReason::InvalidBitmap,
                        });
                    }
                    continue;
                }
                let failure = TextRealizationFailure {
                    primitive_index: run.primitive_index,
                    glyph_index: Some(glyph_index),
                    reason: TextRealizationFailureReason::AtlasResidency,
                };
                let entry = self.glyph_atlas.get(key).ok_or(failure)?;
                let quad = build_quad_from_glyph(glyph, &entry, &quad_params).ok_or(failure)?;
                let mut rect = RectF::new(quad.x, quad.y, quad.width, quad.height);
                let mut foreground = *color;
                if let Some(ink) = ink {
                    let origin = crate::core::math::Point::new(
                        quad_params.origin_phys_x / scale_factor,
                        quad_params.origin_phys_y / scale_factor,
                    );
                    rect = ink.project(rect, origin).ok_or(TextRealizationFailure {
                        primitive_index: run.primitive_index,
                        glyph_index: Some(glyph_index),
                        reason: TextRealizationFailureReason::InvalidTypographyInput(
                            crate::core::text_engine::TextInputError::PositionOverflow,
                        ),
                    })?;
                    foreground = ink.color.into();
                }
                text_quads.push(TextQuad {
                    pos: rect,
                    uv: entry.uv_rect,
                    color: foreground,
                    clip_rect: current_clip,
                    downsample_u: quad.downsample_u,
                });
            }
            let quad_count = text_quads.len() as u32 - quad_start;
            if quad_count > 0 {
                draw_commands.push(SurfaceDrawCommand::Text {
                    order: prim.key.effective_order(),
                    z_index: prim.key.z_layer,
                    quad_start,
                    quad_count,
                });
            }
        }
        Ok((text_quads, draw_commands))
    }

    fn encode_compiled_target(
        &self,
        compiled: &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
        descriptor: &crate::renderer::render_target::TargetDescriptor,
        view: &wgpu::TextureView,
    ) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands> {
        self.encode_compiled_target_with_graph(
            compiled,
            descriptor,
            view,
            &compiled.render_graph_topology,
            compiled.render_graph_output,
        )
    }

    fn encode_compiled_target_with_graph(
        &self,
        compiled: &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
        descriptor: &crate::renderer::render_target::TargetDescriptor,
        view: &wgpu::TextureView,
        topology: &crate::renderer::render_graph::RenderGraphTopology,
        output: crate::renderer::render_graph::AttachmentHandle,
    ) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands> {
        use crate::core::scene::Color;
        use crate::renderer::render_graph::{
            FrameConstants, FrameDrawData, PipelineBuffers, RenderGraph, RenderResources,
            TargetDrawContext,
        };
        use std::collections::HashMap;

        let surface_id = descriptor.surface_id;
        let surface_compiled = compiled.surface_data.get(&surface_id)?;
        if !self.is_current_preparation(compiled)
            || !surface_compiled.text_realization().is_prepared_for_output()
        {
            return None;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(&format!("Render Encoder (Surface {:?})", surface_id)),
            });
        let target_scene_key = compiled
            .surface_stack
            .iter()
            .find(|surface| surface.id == surface_id)
            .map(|surface| surface.scene_key)
            .unwrap_or(0);
        let (topology,output)=compiled.frosted_surfaces.get(&surface_id)
            .map(|p|(&p.topology,p.output)).unwrap_or((topology,output));
        let mut attachments = HashMap::new();
        attachments.insert(output, view.clone());
        let constants = FrameConstants {
            viewport_size: [
                descriptor.physical_size.0 as f32,
                descriptor.physical_size.1 as f32,
            ],
            physical_width: descriptor.physical_size.0,
            physical_height: descriptor.physical_size.1,
            logical_width: descriptor.logical_size.0,
            logical_height: descriptor.logical_size.1,
            scale_factor: descriptor.scale_factor,
            clear_color: Color {
                r: surface_compiled.clear_color.r as f32,
                g: surface_compiled.clear_color.g as f32,
                b: surface_compiled.clear_color.b as f32,
                a: surface_compiled.clear_color.a as f32,
            },
        };
        let mut graph = RenderGraph::compile(topology, constants, attachments);
        let frost_use=if compiled.frosted_surfaces.contains_key(&surface_id) {
            Some(self.frosted_glass.bind(surface_id,&mut graph.bound_resources)?)
        } else {None};
        let image_bind_groups: HashMap<u64, &wgpu::BindGroup> = self
            .image_textures
            .iter()
            .map(|(id, entry)| (*id, &entry.bind_group))
            .collect();
        let draw_data = FrameDrawData {
            target: TargetDrawContext {
                scene_key: target_scene_key,
                globals_offset: descriptor.globals_offset,
                path_clips_offset: descriptor.path_clips_offset,
            },
            frame_batches: &surface_compiled.frame_batches,
            rounded_clip_offsets: &surface_compiled.rounded_clip_offsets,
            shadow_params_offsets: &surface_compiled.shadow_params_offsets,
            gradient_params_offsets: &surface_compiled.gradient_params_offsets,
            analytic_surface_params_offsets: &surface_compiled.analytic_surface_params_offsets,
            color_picker_sv_params_offsets: &surface_compiled.color_picker_sv_params_offsets,
            radial_gauge_params_offsets: &surface_compiled.radial_gauge_params_offsets,
            text_quads: &surface_compiled.text_quads,
            text_quad_offset: surface_compiled.text_quad_offset,
            path_draw_offset: surface_compiled.path_draw_offset,
            draw_commands: &surface_compiled.draw_commands,
        };
        let presence_programs = self.loading_presence_terminal_pipelines();
        let presence_terminal_pipelines = presence_programs
            .iter()
            .map(|(id, p)| (*id, &p.pipeline))
            .collect();
        let render_resources = RenderResources {
            rect_pipeline: &self.rect_pipeline,
            rect_globals_bind_group: &self.rect_globals_bind_group,
            rounded_clip_bind_group: &self.rounded_clip_bind_group,
            shadow_pipeline: &self.shadow_pipeline,
            shadow_params_bind_group: &self.shadow_params_bind_group,
            gradient_pipeline: &self.gradient_pipeline,
            gradient_params_bind_group: &self.gradient_params_bind_group,
            analytic_surface_pipeline: self.analytic_surface.as_ref().map(|r| &r.pipeline),
            analytic_surface_params_bind_group: self
                .analytic_surface
                .as_ref()
                .map(|r| &r.bind_group),
            color_picker_sv_pipeline: &self.color_picker_sv_pipeline,
            color_picker_sv_params_bind_group: &self.color_picker_sv_params_bind_group,
            radial_gauge_pipeline: &self.radial_gauge_pipeline,
            radial_gauge_params_bind_group: &self.radial_gauge_params_bind_group,
            sdf_rect_pipeline: self.sdf_rect_pipeline.as_ref(),
            shape_pipeline: self.shape_pipeline.pipeline(),
            image_pipeline: &self.image_pipeline,
            presence_terminal_pipelines: &presence_terminal_pipelines,
            image_bind_groups: &image_bind_groups,
            image_index_capacity: self.image_index_capacity,
            path_globals_bind_group: &self.path_globals_bind_group,
            path_clips_bind_group: &self.path_clips_bind_group,
            path_renderer: &self.path_renderer,
            text_pipeline: &self.text_pipeline,
            text_bind_group: &self.text_bind_group,
        };
        let buffers = PipelineBuffers {
            rect_vertex_buffer: self.vertex_buffer.as_ref(),
            rect_index_buffer: self.index_buffer.as_ref(),
            shadow_vertex_buffer: self.shadow_vertex_buffer.as_ref(),
            shadow_index_buffer: self.shadow_index_buffer.as_ref(),
            gradient_vertex_buffer: self.gradient_vertex_buffer.as_ref(),
            gradient_index_buffer: self.gradient_index_buffer.as_ref(),
            analytic_surface_vertex_buffer: self.analytic_surface.as_ref().map(|r| &r.vertices),
            analytic_surface_index_buffer: self.analytic_surface.as_ref().map(|r| &r.indices),
            color_picker_sv_vertex_buffer: self.color_picker_sv_vertex_buffer.as_ref(),
            color_picker_sv_index_buffer: self.color_picker_sv_index_buffer.as_ref(),
            radial_gauge_vertex_buffer: self.radial_gauge_vertex_buffer.as_ref(),
            radial_gauge_index_buffer: self.radial_gauge_index_buffer.as_ref(),
            sdf_rect_vertex_buffer: self.sdf_rect_vertex_buffer.as_ref(),
            sdf_rect_index_buffer: self.sdf_rect_index_buffer.as_ref(),
            shape_vertex_buffer: self.shape_vertex_buffer.as_ref(),
            shape_index_buffer: self.shape_index_buffer.as_ref(),
            image_vertex_buffer: self.image_vertex_buffer.as_ref(),
            image_index_buffer: self.image_index_buffer.as_ref(),
            text_vertex_buffer: self.text_vertex_buffer.as_ref(),
            text_index_buffer: self.text_index_buffer.as_ref(),
        };

        if let Err(error)=RenderGraph::execute(&graph, &mut encoder, draw_data, render_resources, buffers) {
            // The still-unsubmitted encoder has been disposed; its exact use
            // can now be released through the same command-owning boundary.
            // An unwinding executor instead leaves its use conservatively
            // Unconfirmed, never free or falsely completed.
            crate::renderer::frosted_glass::EncodedTargetCommands::cancel_encoding(encoder,frost_use);
            panic!("SPEC-HAL-084: RenderGraph execution failed: {error:?}");
        }
        Some(crate::renderer::frosted_glass::EncodedTargetCommands::finish(encoder,frost_use,self.queue.clone()))
    }

    #[cfg(test)]
    pub(crate) fn encode_compiled_target_for_test(
        &self,
        compiled: &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
        surface_id: SurfaceId,
        view: &wgpu::TextureView,
    ) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands> {
        let descriptor = compiled.get_target(surface_id)?;
        self.encode_compiled_target(compiled, descriptor, view)
    }

    #[cfg(test)]
    pub(crate) fn submit_compiled_target_for_test(&self,command:crate::renderer::frosted_glass::EncodedTargetCommands) {
        self.submit_frame_command_with_receipt(command,Vec::new(),|| {}).expect("owned test submission custody");
    }

    #[cfg(test)]
    pub(crate) fn frosted_pool_for_test(&mut self)->(u64,u64) {
        self.frosted_glass.maintain();
        (self.frosted_glass.allocations,self.frosted_glass.resident_bytes())
    }
    #[cfg(test)]
    pub(crate) fn bloom_textures_for_test(&self, surface: SurfaceId) -> Option<[wgpu::Texture; 3]> {
        self.frosted_glass.bloom_textures(surface)
    }
    #[cfg(test)]
    pub(crate) fn frosted_pool_states_for_test(&self)->[usize;8] {
        self.frosted_glass.state_counts()
    }

    /// SPEC-HAL-081B: Render multi-target draw plan.
    ///
    /// Produces one command buffer per render target. The caller must submit
    /// each command buffer and present each surface.
    ///
    /// # Arguments
    ///
    /// * `compiled` - The MultiTargetDrawPlan from compile_multi()
    ///
    /// # Returns
    ///
    /// A vector of MultiTargetRenderOutput, one per target in z-order.
    pub fn render_multi(
        &mut self,
        compiled: crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
    ) -> Vec<crate::renderer::compiled_draw_plan::MultiTargetRenderOutput> {
        // HAL381 sampled generations require the canonical consuming native
        // output wrapper. Ordinary outputs do not carry HAL381 presence uses.
        if !compiled.presence.is_empty() {
            tracing::error!("HAL381 output requires canonical frame custody");
            return Vec::new();
        }
        self.render_multi_owned(compiled, Self::encode_compiled_target)
    }

    fn render_multi_owned(
        &mut self,
        compiled: crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
        mut encode: impl FnMut(
            &Self,
            &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
            &crate::renderer::render_target::TargetDescriptor,
            &wgpu::TextureView,
        ) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands>,
    ) -> Vec<crate::renderer::compiled_draw_plan::MultiTargetRenderOutput> {
        use crate::renderer::compiled_draw_plan::MultiTargetRenderOutput;

        if !self.is_current_preparation(&compiled) {
            tracing::warn!("SPEC-HAL-360: refusing a superseded or foreign compiled plan");
            return Vec::new();
        }

        // SPEC-HAL-264: Advance frame counter and evict stale mesh cache entries
        self.frame_counter += 1;
        self.mesh_cache.evict_stale(self.frame_counter, 120, 16);

        // SPEC-HAL-084: Phase ordering enforcement (debug only)
        #[cfg(debug_assertions)]
        {
            let current_epoch = self.frame_epoch.value();
            debug_assert!(
                self.last_compile_epoch.get() == Some(current_epoch),
                "SPEC-HAL-084 VIOLATION: render_multi() called without compile_multi() for epoch {}",
                current_epoch
            );
            debug_assert!(
                !self.compile_consumed.get(),
                "SPEC-HAL-084 VIOLATION: render_multi() called twice for epoch {}",
                current_epoch
            );
            self.compile_consumed.set(true);
        }

        let mut outputs = Vec::with_capacity(compiled.target_descriptors.len());

        for descriptor in compiled.target_descriptors.iter() {
            let surface_id = descriptor.surface_id;
            if !compiled
                .text_realization(surface_id)
                .is_some_and(|result| result.is_prepared_for_output())
            {
                tracing::warn!(?surface_id, outcome = ?compiled.text_realization(surface_id),
                    "INV-TEXT-ADMISSION: incomplete surface skipped before acquisition");
                continue;
            }

            // INV-RT-005/008: Acquire the target via Registry before encoding.
            // This is the ONLY place where texture acquisition happens.
            // The guard is held for the entire encoding of this target.
            let guard = self.multi_targets.acquire(surface_id).expect(
                "INV-RT-008: render_multi() target must be registered; \
                 compile_multi() should have verified this",
            );

            // Debug assertion: target is now acquired.
            // INV-RT-004: While acquired, configure() will fail with ResizeWhileAcquired,
            // enforcing swapchain immutability during encoding.
            #[cfg(debug_assertions)]
            debug_assert!(
                self.multi_targets.is_acquired(surface_id),
                "INV-RT-008: Target {:?} must be acquired before encoding render passes",
                surface_id
            );

            // Acquire the surface texture just-in-time via the guard
            let surface_output = match guard.get_current_texture() {
                Ok(output) => output,
                Err(crate::renderer::RendererSurfaceError::Lost | crate::renderer::RendererSurfaceError::Outdated) => {
                    // Surface needs reconfiguration - skip this target
                    tracing::warn!(
                        "SPEC-HAL-081B: Surface {:?} lost/outdated, skipping",
                        surface_id
                    );
                    continue;
                }
                Err(crate::renderer::RendererSurfaceError::Timeout | crate::renderer::RendererSurfaceError::Occluded) => {
                    // The target is temporarily unavailable.
                    continue;
                }
                Err(e) => {
                    // Fatal error - log and skip
                    tracing::error!("Surface {:?} error: {:?}", surface_id, e);
                    continue;
                }
            };

            let view = surface_output
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    #[cfg(target_arch = "wasm32")]
                    format: Some(self.surface_format),
                    ..Default::default()
                });
            let Some(commands) = encode(self, &compiled, descriptor, &view) else {
                continue;
            };

            outputs.push(MultiTargetRenderOutput::from_encoded(surface_id,commands,surface_output));

            // guard drops here, releasing the target (INV-RT-007)
        }

        // Debug assertion: all guards dropped, no targets left acquired
        #[cfg(debug_assertions)]
        for output in &outputs {
            debug_assert!(
                !self.multi_targets.is_acquired(output.surface_id),
                "INV-RT-007: Target {:?} must be released after render_multi() completes",
                output.surface_id
            );
        }

        outputs
    }

    pub(crate) fn render_sdf_frame(
        &mut self,
        frame: SdfCompiledFrame,
    ) -> Result<Vec<SdfRenderOutput>, SdfFrameCustodyError> {
        let SdfCompiledFrame {
            plan,
            custody,
            companions,
        } = frame;
        if let Some(companions) = companions {
            return self.render_sdf_frame_with_companions(plan, custody, companions);
        }
        self.render_sdf_frame_outputs(plan, custody, Self::encode_compiled_target, |_| None)
    }

    fn render_sdf_frame_outputs(
        &mut self,
        plan: crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
        custody: SdfSceneCustody,
        encode: impl FnMut(
            &Self,
            &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
            &crate::renderer::render_target::TargetDescriptor,
            &wgpu::TextureView,
        ) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands>,
        companion_for_output: impl Fn(SurfaceId) -> Option<spatial_hud::PresentationCompanionUse>,
    ) -> Result<Vec<SdfRenderOutput>, SdfFrameCustodyError> {
        let presence = plan.presence.clone();
        // Retained CPU references stay alive through all command encoding. A
        // failed/skipped target has no output; its queued use cancels normally.
        let outputs = self.render_multi_owned(plan, encode);
        outputs
            .into_iter()
            .map(|output| {
                let uses = custody.output_uses(output.surface_id)?;
                Ok(SdfRenderOutput {
                    presence: presence
                        .get(&output.surface_id)
                        .cloned()
                        .unwrap_or_default(),
                    companion: companion_for_output(output.surface_id),
                    output,
                    custody: uses,
                })
            })
            .collect()
    }

    fn invalidate_compile_preparation(&mut self) {
        if let Some(identity) = self.preparation_gate.take() {
            identity.cancel();
        }
        self.suspended_compile = None;
        self.current_preparation = None;
        self.frosted_glass.begin_preparation();
    }

    /// Cancel only the named suspended preparation, never another caller's work.
    pub fn cancel_pending_preparation(&mut self, identity: &crate::renderer::RendererPreparationIdentity) -> bool {
        if self.suspended_compile.is_some() && self.preparation_gate.as_ref() == Some(identity) {
            self.invalidate_compile_preparation();
            true
        } else {
            false
        }
    }

    fn publish_multi_compile(&mut self, candidate: Option<crate::renderer::MultiTargetDrawPlan>)
        -> Result<crate::renderer::RendererCompileOutcome<crate::renderer::MultiTargetDrawPlan>, crate::renderer::RendererCompileError> {
        use crate::renderer::RendererCompileOutcome as Outcome;
        let Some(plan) = candidate else {
            return Ok(Outcome::NoTargets);
        };
        let Some(identity) = self.preparation_gate.clone() else {
            return Ok(Outcome::Ready(plan));
        };
        match identity.qualification() {
            Ok(true) => Ok(Outcome::Ready(plan)),
            Ok(false) => {
                self.suspended_compile = Some(SuspendedCompile::Multi(plan));
                Ok(Outcome::Pending(identity))
            }
            Err(error) => {
                self.invalidate_compile_preparation();
                Err(error)
            }
        }
    }

    fn publish_sdf_compile(&mut self, frame: SdfCompiledFrame)
        -> Result<crate::renderer::RendererCompileOutcome<SdfCompiledFrame>, crate::renderer::RendererCompileError> {
        use crate::renderer::RendererCompileOutcome as Outcome;
        let Some(identity) = self.preparation_gate.clone() else {
            return Ok(Outcome::Ready(frame));
        };
        match identity.qualification() {
            Ok(true) => Ok(Outcome::Ready(frame)),
            Ok(false) => {
                self.suspended_compile = Some(SuspendedCompile::Sdf(frame));
                Ok(Outcome::Pending(identity))
            }
            Err(error) => {
                self.invalidate_compile_preparation();
                Err(error)
            }
        }
    }

    fn take_completed_preparation(&mut self, identity: &crate::renderer::RendererPreparationIdentity)
        -> Result<Option<SuspendedCompile>, crate::renderer::RendererCompileError> {
        if self.preparation_gate.as_ref() != Some(identity)
            || self
                .current_preparation
                .as_ref()
                .is_none_or(|current| !Arc::ptr_eq(current, identity.token()))
        {
            return Err(crate::renderer::RendererSurfaceError::Outdated.into());
        }
        let Some(suspended) = self.suspended_compile.as_ref() else {
            return Err(crate::renderer::RendererSurfaceError::Outdated.into());
        };
        let plan = suspended.plan();
        let valid = plan.preparation_epoch == self.frame_epoch
            && plan.target_descriptors.iter().all(|target| {
                self.multi_targets.configured_size_px(target.surface_id)
                    == Some(target.physical_size)
                    && self.multi_targets.scale_factor(target.surface_id)
                        == Some(target.scale_factor)
                    && self.multi_targets.has_active_extent(target.surface_id)
            });
        if !valid {
            self.invalidate_compile_preparation();
            return Err(crate::renderer::RendererSurfaceError::Outdated.into());
        }
        if self
            .gpu_resources()
            .observe_device_lifetime()
            .failure()
            .is_some()
        {
            self.invalidate_compile_preparation();
            return Err(crate::renderer::RendererSurfaceError::Lost.into());
        }
        match identity.qualification() {
            Ok(false) => Ok(None),
            Ok(true) => Ok(self.suspended_compile.take()),
            Err(error) => {
                self.invalidate_compile_preparation();
                Err(error)
            }
        }
    }

    /// Resume the exact public Compile preparation without compiling or opening
    /// another allocation scope. Stale identities never expose an executable.
    pub fn resume_compile(&mut self, identity: &crate::renderer::RendererPreparationIdentity)
        -> Result<crate::renderer::RendererCompileOutcome<crate::renderer::MultiTargetDrawPlan>, crate::renderer::RendererCompileError> {
        use crate::renderer::RendererCompileOutcome as Outcome;
        match self.take_completed_preparation(identity)? {
            None => Ok(Outcome::Pending(identity.clone())),
            Some(SuspendedCompile::Multi(plan)) => Ok(Outcome::Ready(plan)),
            Some(other) => {
                self.suspended_compile = Some(other);
                Err(crate::renderer::RendererSurfaceError::Outdated.into())
            }
        }
    }

    pub(crate) fn resume_sdf_compile(&mut self, identity: &crate::renderer::RendererPreparationIdentity)
        -> Result<crate::renderer::RendererCompileOutcome<SdfCompiledFrame>, crate::renderer::RendererCompileError> {
        use crate::renderer::RendererCompileOutcome as Outcome;
        match self.take_completed_preparation(identity)? {
            None => Ok(Outcome::Pending(identity.clone())),
            Some(SuspendedCompile::Sdf(frame)) => Ok(Outcome::Ready(frame)),
            Some(other) => {
                self.suspended_compile = Some(other);
                Err(crate::renderer::RendererSurfaceError::Outdated.into())
            }
        }
    }

    fn is_current_preparation(&self, compiled: &crate::renderer::MultiTargetDrawPlan) -> bool {
        self.suspended_compile.is_none()
            && compiled.preparation_epoch == self.frame_epoch
            && self
                .current_preparation
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &compiled.preparation))
    }

    #[cfg(test)]
    pub(crate) fn set_text_atlas_limits_for_test(&mut self, initial_size: u32, max_size: u32) {
        self.gpu_arena.schedule_destroy(self.glyph_atlas.texture_id);
        self.glyph_atlas = GlyphAtlas::new(
            &self.device,
            initial_size,
            initial_size,
            &mut self.gpu_arena,
            self.frame_epoch,
        );
        self.glyph_atlas.set_max_size_for_test(max_size);
        self.last_atlas_texture_id = self.glyph_atlas.texture_id;
        self.pipeline_generation += 1;
        self.invalidate_compile_preparation();
    }

    #[cfg(test)]
    pub(crate) fn text_atlas_size_for_test(&self) -> (u32, u32) {
        self.glyph_atlas.dimensions_for_test()
    }

    #[cfg(all(test, debug_assertions))]
    pub(crate) fn text_rasterizations_for_test(&self) -> u32 {
        self.rasterization_count.get()
    }

    #[cfg(test)]
    pub(crate) fn text_atlas_contains_for_test(
        &self,
        key: &crate::renderer::glyph_atlas::GlyphKey,
    ) -> bool {
        self.glyph_atlas.get(key).is_some()
    }
}

#[cfg(test)]
#[path = "../tests/wgpu_renderer/unit.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "../tests/wgpu_renderer/gradient_dither.rs"]
mod gradient_dither_tests;
