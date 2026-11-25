//! SPEC-HAL-085 Section 6.2: Canonical Vertex Formats
//!
//! All pipelines MUST use one of these two vertex input paths:
//! - Path A: QuadVertex + QuadInstance (for quads with instancing)
//! - Path B: BakedVertex (for pre-expanded geometry)

use bytemuck::{Pod, Zeroable};

// ============================================================================
// Path A: QuadVertex + QuadInstance (instanced quad rendering)
// ============================================================================

/// SPEC-HAL-085 Path A: Per-vertex data for instanced quads.
///
/// Used by: rect, image, clip application, debug overlays.
/// Contains only the corner position and UV within the unit quad.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadVertex {
    /// Local position within unit quad (typically 0..1, 0..1 for corners)
    pub local_pos: [f32; 2],
    /// UV coordinates (used by image/text, ignored by rect)
    pub uv: [f32; 2],
}

impl QuadVertex {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 8,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// Unit quad vertices (4 corners, CCW from bottom-left)
    pub const UNIT_QUAD: [QuadVertex; 4] = [
        QuadVertex {
            local_pos: [0.0, 0.0],
            uv: [0.0, 0.0],
        }, // bottom-left
        QuadVertex {
            local_pos: [1.0, 0.0],
            uv: [1.0, 0.0],
        }, // bottom-right
        QuadVertex {
            local_pos: [1.0, 1.0],
            uv: [1.0, 1.0],
        }, // top-right
        QuadVertex {
            local_pos: [0.0, 1.0],
            uv: [0.0, 1.0],
        }, // top-left
    ];

    /// Indices for two triangles forming a quad
    pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];
}

/// SPEC-HAL-085 Path A: Per-instance data for instanced quads.
///
/// Contains the rect bounds, color, and pipeline-specific extra data.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    /// Rect in Absolute Logical coords: (x, y, width, height)
    pub rect: [f32; 4],
    /// Premultiplied RGBA color
    pub color: [f32; 4],
    /// Pipeline-specific: radius, border width, etc.
    pub extra0: [f32; 4],
    /// Pipeline-specific: glyph params, etc.
    pub extra1: [f32; 4],
}

impl QuadInstance {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: 16,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: 32,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: 48,
            shader_location: 5,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

// ============================================================================
// Path B: BakedVertex (pre-expanded geometry)
// ============================================================================

/// SPEC-HAL-085 Path B: Pre-expanded vertex with all data baked in.
///
/// Used by: path/shape pipelines when geometry is already tessellated.
/// Each vertex is fully self-contained.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BakedVertex {
    /// Position in Absolute Logical coordinates
    pub pos: [f32; 2],
    /// UV coordinates (optional, pipeline-specific)
    pub uv: [f32; 2],
    /// Premultiplied RGBA color
    pub color: [f32; 4],
    /// Pipeline-specific extra data
    pub extra: [f32; 4],
}

impl BakedVertex {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 8,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 16,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: 32,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BakedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    /// Create a simple colored vertex (no UV, no extra)
    pub fn colored(pos: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            pos,
            uv: [0.0, 0.0],
            color,
            extra: [0.0; 4],
        }
    }
}

// ============================================================================
// Path B Variants (minimal BakedVertex for specific pipelines)
// ============================================================================

/// SPEC-HAL-085: RectVertex is a minimal Path B variant.
///
/// Uses only position and color (no UV, no extra), matching rect.wgsl:
/// - location(0): position vec2<f32>
/// - location(1): color vec4<f32>
///
/// This is valid Path B usage - pipelines may use subsets of BakedVertex
/// when extra fields are not needed.
pub type RectVertex = crate::renderer::draw_plan::RectVertex;

// ============================================================================
// SPEC-HAL-263: Image pipeline vertex
// ============================================================================

/// SPEC-HAL-263: Pre-expanded vertex for image quads.
///
/// Matches image.wgsl VertexInput:
/// - location(0): position vec2<f32>  (Absolute Logical coords)
/// - location(1): uv vec2<f32>        (texture coordinates)
/// - location(2): color vec4<f32>     (straight-linear tint RGBA)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ImageVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl ImageVertex {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 8,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 16,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ImageVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

// ============================================================================
// SPEC-R-022: SDF RoundedRect pipeline vertex
// ============================================================================

/// SPEC-R-022: Vertex for SDF rounded rect pipeline.
/// All shape params are per-vertex (flat interpolated) to enable
/// batching all rounded rects into one draw call.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SdfRectVertex {
    pub position: [f32; 2],
    pub fill_color: [f32; 4],
    pub border_color: [f32; 4],
    pub rect_center: [f32; 2],
    pub rect_half_size: [f32; 2],
    pub radius: f32,
    pub border_width: f32,
}

impl SdfRectVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // fill_color
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // border_color
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // rect_center
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // rect_half_size
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // radius
                wgpu::VertexAttribute {
                    offset: 56,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
                // border_width
                wgpu::VertexAttribute {
                    offset: 60,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}
