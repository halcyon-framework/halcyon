// SPEC-HAL-302: HistogramSeries — Positive/Negative Vertical Bars
//
// Renders bars from a zero baseline. Positive = above zero,
// negative = below zero. Color determined by sign. Consumer
// provides pre-computed HistogramPoint data.

use super::{PhysicalRect, Series};
use crate::core::paint::Color;
use crate::plot::series::candle::{OVERLAY_SHADER, Uniforms, Vertex};
use crate::plot::transform::Transform;

/// A single histogram bar. Consumer computes the Y value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistogramPoint {
    pub x: f64,
    /// Positive = above zero, negative = below zero.
    pub y: f64,
}

/// Configuration for histogram appearance.
#[derive(Debug, Clone)]
pub struct HistogramStyle {
    /// Color for positive bars (above zero).
    pub positive_color: Color,
    /// Color for negative bars (below zero).
    pub negative_color: Color,
    /// Bar width as fraction of interval. Default 0.8.
    pub width_fraction: f32,
}

impl Default for HistogramStyle {
    fn default() -> Self {
        Self {
            positive_color: Color::from_hex(0x2ECC71).with_alpha(0.7),
            negative_color: Color::from_hex(0xF05454).with_alpha(0.7),
            width_fraction: 0.8,
        }
    }
}

pub struct HistogramSeries {
    points: Vec<HistogramPoint>,
    style: HistogramStyle,
    z_order: u32,
    contributes_to_y: bool,
    theme_style: bool,

    // GPU state
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buffer: Option<wgpu::Buffer>,
    uniform_bind_group: Option<wgpu::BindGroup>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_count: u32,
}

impl HistogramSeries {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            style: HistogramStyle::default(),
            z_order: 5,
            contributes_to_y: true,
            theme_style: true,
            pipeline: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            vertex_buffer: None,
            vertex_count: 0,
        }
    }

    pub fn with_style(mut self, style: HistogramStyle) -> Self {
        self.style = style;
        self.theme_style = false;
        self
    }

    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z_order = z;
        self
    }

    pub fn with_auto_scale_y(mut self, contributes: bool) -> Self {
        self.contributes_to_y = contributes;
        self
    }

    pub fn set_points(&mut self, points: Vec<HistogramPoint>) {
        self.points = points;
    }

    fn build_vertices(&self, transform: &Transform) -> Vec<Vertex> {
        if self.points.is_empty() {
            return Vec::new();
        }

        let avg_interval = if self.points.len() >= 2 {
            let first_x = self.points.first().unwrap().x;
            let last_x = self.points.last().unwrap().x;
            (last_x - first_x) / (self.points.len() - 1) as f64
        } else {
            1.0
        };
        let bar_half_width = avg_interval * self.style.width_fraction as f64 / 2.0;
        let Some(zero_px) = transform.y_to_px_checked(0.0) else {
            return Vec::new();
        };

        let mut verts = Vec::with_capacity(self.points.len() * 6);

        for p in &self.points {
            if p.x + bar_half_width < transform.x_min || p.x - bar_half_width > transform.x_max {
                continue;
            }

            let (Some(x1), Some(x2), Some(y_px)) = (
                transform.x_to_px_checked(p.x - bar_half_width),
                transform.x_to_px_checked(p.x + bar_half_width),
                transform.y_to_px_checked(p.y),
            ) else {
                continue;
            };

            let color = if p.y >= 0.0 {
                self.style.positive_color.to_array()
            } else {
                self.style.negative_color.to_array()
            };

            let (top, bottom) = if y_px < zero_px {
                (y_px, zero_px)
            } else {
                (zero_px, y_px)
            };

            verts.push(Vertex {
                position: [x1, top],
                color,
            });
            verts.push(Vertex {
                position: [x2, top],
                color,
            });
            verts.push(Vertex {
                position: [x1, bottom],
                color,
            });
            verts.push(Vertex {
                position: [x1, bottom],
                color,
            });
            verts.push(Vertex {
                position: [x2, top],
                color,
            });
            verts.push(Vertex {
                position: [x2, bottom],
                color,
            });
        }

        verts
    }
}

impl Series for HistogramSeries {
    fn init(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HistogramSeries Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HistogramSeries Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HistogramSeries Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HistogramSeries Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HistogramSeries Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HistogramSeries Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(Vertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        self.pipeline = Some(pipeline);
        self.uniform_buffer = Some(uniform_buffer);
        self.uniform_bind_group = Some(uniform_bind_group);
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform: &Transform,
        viewport: PhysicalRect,
    ) {
        if let Some(ref buf) = self.uniform_buffer {
            let uniforms = Uniforms {
                viewport_size: [viewport.width as f32, viewport.height as f32],
            };
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&[uniforms]));
        }

        let verts = self.build_vertices(transform);
        self.vertex_count = verts.len() as u32;

        if verts.is_empty() {
            return;
        }

        let data = bytemuck::cast_slice(&verts);
        let needed = data.len() as u64;

        match &self.vertex_buffer {
            Some(buf) if buf.size() >= needed => {
                queue.write_buffer(buf, 0, data);
            }
            _ => {
                use wgpu::util::DeviceExt;
                self.vertex_buffer = Some(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("HistogramSeries Vertex Buffer"),
                        contents: data,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    },
                ));
            }
        }
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, _viewport: PhysicalRect) {
        if self.vertex_count == 0 {
            return;
        }
        if let (Some(pipeline), Some(bind_group), Some(buf)) = (
            &self.pipeline,
            &self.uniform_bind_group,
            &self.vertex_buffer,
        ) {
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.set_vertex_buffer(0, buf.slice(..));
            render_pass.draw(0..self.vertex_count, 0..1);
        }
    }

    fn z_order(&self) -> u32 {
        self.z_order
    }

    fn apply_color_scheme(&mut self, scheme: &crate::plot::PlotColorScheme) {
        if self.theme_style {
            self.style.positive_color = scheme.histogram_positive;
            self.style.negative_color = scheme.histogram_negative;
        }
    }

    fn contributes_to_y_autoscale(&self) -> bool {
        self.contributes_to_y
    }

    fn data_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        let mut min_y = 0.0_f64;
        let mut max_y = 0.0_f64;
        for p in &self.points {
            if p.x >= x_min && p.x <= x_max {
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }
        }
        if min_y <= max_y {
            Some((min_y, max_y))
        } else {
            None
        }
    }

    fn hit_test(&self, _x: f32, _y: f32, _transform: &Transform) -> Option<Box<dyn std::any::Any>> {
        None
    }

    fn hit_proxy(&self) -> crate::plot::hit_proxy::HitProxy {
        crate::plot::hit_proxy::HitProxy::Histogram {
            points: self.points.clone(),
        }
    }
}
