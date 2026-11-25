//! SPEC-R-003: GPU Shape Rendering Pipeline
//!
//! This module provides the analytic SDF shape pipeline for the constrained
//! primitive set allowed by SPEC-R-003.

use bytemuck::{Pod, Zeroable};
use std::sync::Arc;

const SHAPE_SHADER_SRC: &str = include_str!("../shaders/shape.wgsl");

/// Shape type identifiers for batching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ShapeType {
    Circle = 0,
    Ellipse = 1,
    Line = 2,
    Polyline = 3,
    LuminousContour = 4,
    LocalAmbientGlow = 5,
}

/// Vertex format for analytic SDF shape rendering.
///
/// `position` is the quad vertex position in logical coordinates. Shape
/// evaluation also occurs in logical coordinates so the shader can share the
/// renderer's normal globals/view-projection contract.
/// HAL-388 type 5 reuses this ABI: color/params2 hold the two straight colors,
/// params0/1 their centers and radii, params3 their cos/sin pairs, and
/// contour_gains their intensities. Absent lobe lanes and padding stay zero.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShapeVertex {
    pub position: [f32; 2],
    pub local_pos: [f32; 2],
    pub color: [f32; 4],
    /// Circle/Ellipse: center/radius. Line/Polyline: one segment start/end.
    pub params0: [f32; 4],
    /// Line/Polyline: `[half_width, halo_radius, halo_intensity, halo_falloff]`.
    /// Ordinary shapes keep the last three lanes at zero. Polylines are expanded
    /// into per-segment vertices by the renderer integration pass.
    pub params1: [f32; 4],
    /// Luminous contours: start/end ownership-plane normals. Ordinary shapes use zero.
    pub params2: [f32; 4],
    /// Luminous contours: `[core_lift, inner_radius, inner_intensity, front_opacity_boost]`.
    pub params3: [f32; 4],
    /// Luminous contours: front gain at the analytic region start/end.
    pub contour_gains: [f32; 2],
    pub shape_type: u32,
    _pad: u32,
}

impl ShapeVertex {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 9] = [
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
        wgpu::VertexAttribute {
            offset: 48,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: 64,
            shader_location: 5,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: 80,
            shader_location: 6,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: 96,
            shader_location: 7,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 104,
            shader_location: 8,
            format: wgpu::VertexFormat::Uint32,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub fn new(
        position: [f32; 2],
        local_pos: [f32; 2],
        color: [f32; 4],
        params0: [f32; 4],
        params1: [f32; 4],
        shape_type: ShapeType,
    ) -> Self {
        Self {
            position,
            local_pos,
            color,
            params0,
            params1,
            params2: [0.0; 4],
            params3: [0.0; 4],
            contour_gains: [0.0; 2],
            shape_type: shape_type as u32,
            _pad: 0,
        }
    }

    pub fn new_luminous_contour(
        position: [f32; 2],
        color: [f32; 4],
        params0: [f32; 4],
        params1: [f32; 4],
        params2: [f32; 4],
        params3: [f32; 4],
        contour_gains: [f32; 2],
    ) -> Self {
        Self {
            position,
            local_pos: position,
            color,
            params0,
            params1,
            params2,
            params3,
            contour_gains,
            shape_type: ShapeType::LuminousContour as u32,
            _pad: 0,
        }
    }
}

/// Shape pipeline for GPU path rendering
///
/// The pipeline is intentionally narrow: one triangle-list pipeline, one
/// globals bind group layout, and all shape parameters are per-vertex.
pub struct ShapePipeline {
    layout: Option<Arc<wgpu::PipelineLayout>>,
    pipeline: Option<wgpu::RenderPipeline>,
}

impl ShapePipeline {
    /// Create a new shape pipeline.
    ///
    /// # Arguments
    /// * `device` - The wgpu device for resource creation
    /// * `format` - The target surface format
    /// * `globals_layout` - Renderer globals layout at group(0)
    ///
    /// # Returns
    /// A ready ShapePipeline instance
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shape SDF Shader"),
            source: wgpu::ShaderSource::Wgsl(SHAPE_SHADER_SRC.into()),
        });

        let layout = Arc::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shape Pipeline Layout"),
                bind_group_layouts: &[Some(globals_layout)],
                immediate_size: 0,
            }),
        );

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shape SDF Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(ShapeVertex::desc())],
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

        Self {
            layout: Some(layout),
            pipeline: Some(pipeline),
        }
    }

    /// Check if the pipeline is ready for rendering
    pub fn is_ready(&self) -> bool {
        self.pipeline.is_some()
    }

    /// Prepare shape geometry for rendering.
    #[allow(dead_code)]
    pub fn prepare(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue) {
        // Geometry upload is owned by the future renderer integration pass.
    }

    /// Bind the shape pipeline for rendering.
    #[allow(dead_code)]
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if let Some(pipeline) = &self.pipeline {
            render_pass.set_pipeline(pipeline);
            // Draw calls require buffers from the future renderer integration pass.
        }
    }

    #[allow(dead_code)]
    pub fn pipeline(&self) -> Option<&wgpu::RenderPipeline> {
        self.pipeline.as_ref()
    }

    #[allow(dead_code)]
    pub fn layout(&self) -> Option<&wgpu::PipelineLayout> {
        self.layout.as_deref()
    }
}

impl Default for ShapePipeline {
    fn default() -> Self {
        Self {
            layout: None,
            pipeline: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_type_values() {
        assert_eq!(ShapeType::Circle as u8, 0);
        assert_eq!(ShapeType::Ellipse as u8, 1);
        assert_eq!(ShapeType::Line as u8, 2);
        assert_eq!(ShapeType::Polyline as u8, 3);
        assert_eq!(ShapeType::LuminousContour as u8, 4);
    }

    #[test]
    fn test_shape_pipeline_default() {
        let pipeline = ShapePipeline::default();
        assert!(!pipeline.is_ready());
    }

    #[test]
    fn test_shape_vertex_layout() {
        let desc = ShapeVertex::desc();
        assert_eq!(desc.array_stride, 112);
        assert_eq!(desc.step_mode, wgpu::VertexStepMode::Vertex);
        assert_eq!(desc.attributes.len(), 9);
        assert_eq!(
            desc.attributes[8],
            wgpu::VertexAttribute {
                offset: 104,
                shader_location: 8,
                format: wgpu::VertexFormat::Uint32,
            }
        );
    }

    #[test]
    fn test_shape_vertex_constructor_sets_type_and_padding() {
        let vertex = ShapeVertex::new(
            [1.0, 2.0],
            [3.0, 4.0],
            [0.25, 0.5, 0.75, 1.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            ShapeType::Polyline,
        );

        assert_eq!(vertex.position, [1.0, 2.0]);
        assert_eq!(vertex.local_pos, [3.0, 4.0]);
        assert_eq!(vertex.shape_type, 3);
        assert_eq!(vertex.params2, [0.0; 4]);
        assert_eq!(vertex.params3, [0.0; 4]);
        assert_eq!(vertex.contour_gains, [0.0; 2]);
        assert_eq!(vertex._pad, 0);
    }

    #[test]
    fn luminous_shape_vertex_has_explicit_ownership_and_gain_lanes() {
        let vertex = ShapeVertex::new_luminous_contour(
            [1.0, 2.0],
            [0.1, 0.2, 0.3, 0.4],
            [1.0, 2.0, 5.0, 6.0],
            [0.5, 3.0, 0.08, 4.0],
            [1.0, 0.0, 0.0, 1.0],
            [0.4, 1.5, 0.6, 0.75],
            [0.25, 0.75],
        );

        assert_eq!(vertex.params2, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(vertex.params3, [0.4, 1.5, 0.6, 0.75]);
        assert_eq!(vertex.contour_gains, [0.25, 0.75]);
        assert_eq!(vertex.shape_type, ShapeType::LuminousContour as u32);
    }

    #[test]
    fn test_shape_shader_is_not_placeholder() {
        assert!(SHAPE_SHADER_SRC.contains("sd_circle"));
        assert!(SHAPE_SHADER_SRC.contains("sd_segment"));
        assert!(!SHAPE_SHADER_SRC.contains("Placeholder fragment shader"));
    }
}
