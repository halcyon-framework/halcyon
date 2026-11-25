//! SPEC-HAL-084: DrawPlan — Build Phase Output
//!
//! SPEC-HAL-081A: DrawPlan (frozen contract).
//!
//! DrawPlan is the CPU-only output of the Build phase. It contains all the
//! data needed for rendering without any GPU handles.
//!
//! # Data Flow
//!
//! ```text
//! SceneGraph/SceneCache (upstream)
//!     ↓
//! Batcher.flatten_scene() + build_batches()
//!     ↓
//! DrawPlan (this struct)
//!     ↓
//! Compile phase (GPU buffer uploads)
//!     ↓
//! Execute phase (GPU command encoding)
//! ```
//!
//! # Invariants (SPEC-HAL-084)
//!
//! - INV-RG-001: DrawPlan contains only orchestration data, no geometry generation
//! - INV-RG-002: RenderGraph receives DrawPlan, never SceneGraph/SceneCache
//! - INV-RG-007: No GPU handles — all data is CPU-only

use crate::compositor::{BaseContract, Contract, SurfaceId, SurfaceStackSnapshot};
use crate::core::math::RectF;
use crate::core::scene::Color;
use crate::renderer::batcher::{DrawBatch, FlatPrimitive, PipelineId};
use crate::renderer::path::PathDraw;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

// ============================================================================
// RectVertex — CPU vertex data for rect pipeline
// ============================================================================

/// Vertex for rect quad rendering.
///
/// This is CPU-only data that gets uploaded to GPU buffers during Compile phase.
/// Position is in NDC (normalized device coordinates), color is linear RGBA.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RectVertex {
    /// Position in NDC (-1..1)
    pub position: [f32; 2],
    /// Color as packed RGBA (linear, not sRGB)
    pub color: [f32; 4],
}

impl RectVertex {
    /// Create a new RectVertex.
    pub fn new(position: [f32; 2], color: [f32; 4]) -> Self {
        Self { position, color }
    }

    /// Get the wgpu vertex buffer layout for RectVertex.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: vec2<f32> @ location(0)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color: vec4<f32> @ location(1)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// SPEC-R-013-v2: Shadow pipeline vertex.
/// Position in logical coords, premultiplied color, shadow offset.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShadowVertex {
    /// Quad corner position (logical coordinates)
    pub position: [f32; 2],
    /// Shadow color (premultiplied RGBA)
    pub color: [f32; 4],
    /// Shadow offset applied to SDF evaluation point
    pub shadow_offset: [f32; 2],
}

impl ShadowVertex {
    pub fn new(position: [f32; 2], color: [f32; 4], shadow_offset: [f32; 2]) -> Self {
        Self {
            position,
            color,
            shadow_offset,
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: vec2<f32> @ location(0)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color: vec4<f32> @ location(1)
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // shadow_offset: vec2<f32> @ location(2)
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// SPEC-R-012-v2: Gradient pipeline vertex.
/// Position only — all gradient computation in fragment shader from uniforms.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GradientVertex {
    /// Quad corner position (logical coordinates)
    pub position: [f32; 2],
}

impl GradientVertex {
    pub fn new(position: [f32; 2]) -> Self {
        Self { position }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GradientVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: vec2<f32> @ location(0)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// ============================================================================
// SPEC-HAL-163: Surface-Scoped Scene Data
// ============================================================================

/// SPEC-HAL-163: Per-surface scene data for surface-scoped rendering.
///
/// Each surface has its own scene data, enforcing:
/// - INV-SCENE-001: SceneGraphs are surface-scoped
/// - INV-SCENE-002: No cross-surface scene or derived-resource dependency
///
/// # Invariants
///
/// - No GPU handles — all data is CPU-only
/// - Surface-owned data never crosses into another surface's plan
#[derive(Debug, Clone)]
pub struct SurfaceSceneData {
    pub(crate) bloom: Option<crate::renderer::bloom::BloomOccurrence>,
    /// Flattened primitives from this surface's scene.
    pub(crate) primitives: Vec<FlatPrimitive>,
    /// Path draws for this surface.
    pub(crate) path_draws: Vec<PathDraw>,
    /// Clip rectangles for path rendering (index 0 = fullscreen).
    pub(crate) path_clips: Vec<RectF>,
    /// Draw batches with vertex/index ranges.
    pub(crate) batches: Vec<DrawBatch>,
    /// Total rect vertex count for this surface.
    pub(crate) total_vertices: u32,
    /// Total rect index count for this surface.
    pub(crate) total_indices: u32,
    /// SPEC-HAL-263: Total image vertex count for this surface.
    pub(crate) total_image_vertices: u32,
    /// SPEC-HAL-263: Total image index count for this surface.
    pub(crate) total_image_indices: u32,
    /// Clear color for this surface's background.
    pub(crate) clear_color: Color,
}

impl SurfaceSceneData {
    /// Check if this surface has any rect batches.
    pub fn has_rect_batches(&self) -> bool {
        self.batches
            .iter()
            .any(|b| b.key.pipeline == PipelineId::Rect)
    }

    /// Check if this surface has any path draws.
    pub fn has_paths(&self) -> bool {
        !self.path_draws.is_empty()
    }

    /// Check if this surface has any content (non-empty).
    pub fn has_content(&self) -> bool {
        !self.primitives.is_empty() || !self.path_draws.is_empty()
    }
}

// ============================================================================
// DrawPlan — Build Phase Output
// ============================================================================

/// SPEC-HAL-084: Build phase output containing all CPU-only data for rendering.
/// SPEC-HAL-163: Extended for surface-scoped scene partitioning.
///
/// This is the sole input to the Compile phase. It contains:
/// - Per-surface scene data (primitives, batches, paths, clear_color)
/// - Compositor surface stack for surface ordering
///
/// # SPEC-HAL-163: Surface-Scoped Scenes
///
/// Each surface has its own `SurfaceSceneData` containing:
/// - Primitives from that surface's SceneGraph
/// - Batches for that surface only
/// - Clear color for that surface
///
/// This enforces:
/// - INV-SCENE-001: SceneGraphs are surface-scoped
/// - INV-SCENE-002: No cross-surface background/text dependency
/// - INV-SCENE-004: Renderer never sees a mixed scene
///
/// # Invariants
///
/// - No GPU handles (wgpu::Buffer, wgpu::Texture, etc.)
/// - All data is CPU-only and can be freely cloned/moved
/// - Can be constructed without initializing GPU
/// - SceneGraph/SceneCache consumption happens upstream to produce this
///
/// # Text Rendering
///
/// Text quads are NOT stored here. Text info is in `primitives` as
/// `PrimitiveData::Text`. The renderer builds TextQuads during Compile
/// phase using the glyph atlas (which requires GPU).
///
/// # Contract Parameterization (SPEC-HAL-081A)
///
/// The `P` type parameter encodes the compositor contract at the type level.
/// Production code uses `DrawPlan<MultiSurfaceContract>` via `build_multi()`.
#[derive(Debug, Clone)]
pub struct DrawPlan<P: Contract = BaseContract> {
    // ========================================================================
    // SPEC-HAL-163: Per-Surface Scene Data
    // ========================================================================
    /// Per-surface scene data. Each surface has its own primitives, batches, etc.
    ///
    /// INV-SCENE-001: SceneGraphs are surface-scoped
    /// INV-SCENE-004: Renderer never sees a mixed scene
    pub(crate) surface_data: HashMap<SurfaceId, SurfaceSceneData>,

    // ========================================================================
    // SPEC-HAL-081A: Compositor Surface Stack (MANDATORY)
    // ========================================================================
    /// Compositor surface stack for surface-level composition.
    ///
    /// SPEC-HAL-081A: This field is MANDATORY, not optional.
    /// All rendering must go through the compositor path.
    ///
    /// The contract parameter `P` is propagated from the DrawPlanBuilder,
    /// ensuring type-level enforcement of contract constraints.
    ///
    /// # Structural Freeze (SPEC-HAL-081A)
    ///
    /// This field is `SurfaceStackSnapshot<P>`, NOT `Option<SurfaceStackSnapshot<P>>`.
    /// The type system enforces that surface_stack is always present.
    /// There is no escape path that allows `None`.
    pub(crate) surface_stack: SurfaceStackSnapshot<P>,
}

impl<P: Contract> DrawPlan<P> {
    /// Get scene data for a specific surface.
    pub fn surface_scene(&self, id: SurfaceId) -> Option<&SurfaceSceneData> {
        self.surface_data.get(&id)
    }

    /// Check if any surface has rect batches.
    pub fn has_rect_batches(&self) -> bool {
        self.surface_data.values().any(|s| s.has_rect_batches())
    }

    /// Check if any surface has path draws.
    pub fn has_paths(&self) -> bool {
        self.surface_data.values().any(|s| s.has_paths())
    }

    /// INV-SCENE-005: Check if a surface is empty (valid state).
    pub fn is_surface_empty(&self, id: SurfaceId) -> bool {
        self.surface_data
            .get(&id)
            .map_or(true, |s| !s.has_content())
    }
}
