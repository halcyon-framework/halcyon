//! Text vertex definition for text rendering pipeline.

use bytemuck::{Pod, Zeroable};

/// Vertex for text quad rendering (uses atlas texture)
///
/// `screen_uv` is retained for vertex-layout compatibility while the current
/// text shaders do not sample a background texture.
/// R-010B FIX: Added downsample_u for per-glyph downsample offset.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TextVertex {
    /// Position in NDC
    pub position: [f32; 2],
    /// UV coordinates in atlas space (0..1)
    pub uv: [f32; 2],
    /// Color as packed RGBA (r, g, b, a)
    pub color: [f32; 4],
    /// Screen-space UV retained for vertex-layout compatibility.
    /// Computed on CPU as (phys_x / surface_width, phys_y / surface_height)
    pub screen_uv: [f32; 2],
    /// R-010B FIX: Per-glyph downsample offset in UV space
    pub downsample_u: f32,
}

impl TextVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2, // position
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2, // uv
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4, // color
                },
                // screen_uv retained at shader location 3 for layout compatibility
                wgpu::VertexAttribute {
                    offset: 32, // 16 (color offset) + 16 (color size)
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2, // screen_uv
                },
                // R-010B FIX: downsample_u for per-glyph offset
                wgpu::VertexAttribute {
                    offset: 40, // 32 (screen_uv offset) + 8 (screen_uv size)
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32, // downsample_u
                },
            ],
        }
    }
}
