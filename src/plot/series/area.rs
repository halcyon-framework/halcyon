// SPEC-HAL-302: AreaSeries — Filled Region Between Two Curves
//
// Renders a triangle strip between upper and lower Y values at each
// X point. Uses the overlay pipeline (same Vertex + OVERLAY_SHADER
// as CandleSeries/LineSeries/BarSeries).

use super::{PhysicalRect, Series};
use crate::core::paint::Color;
use crate::plot::series::candle::{OVERLAY_SHADER, Uniforms, Vertex};
use crate::plot::transform::Transform;

/// A pair of Y values at a given X coordinate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaPoint {
    pub x: f64,
    pub y_upper: f64,
    pub y_lower: f64,
}

/// Configuration for area appearance.
#[derive(Debug, Clone)]
pub struct AreaStyle {
    /// Fill color RGBA (typically semi-transparent).
    pub fill_color: Color,
    /// Optional upper boundary line color. None = no line.
    pub upper_line_color: Option<Color>,
    /// Optional lower boundary line color. None = no line.
    pub lower_line_color: Option<Color>,
    /// Boundary line width in pixels.
    pub line_width: f32,
}

impl Default for AreaStyle {
    fn default() -> Self {
        Self {
            fill_color: Color::from_srgb(0.3, 0.5, 1.0, 0.15),
            upper_line_color: Some(Color::from_srgb(0.3, 0.5, 1.0, 0.4)),
            lower_line_color: Some(Color::from_srgb(0.3, 0.5, 1.0, 0.4)),
            line_width: 1.0,
        }
    }
}

pub struct AreaSeries {
    points: Vec<AreaPoint>,
    style: AreaStyle,
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

impl AreaSeries {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            style: AreaStyle::default(),
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

    pub fn with_style(mut self, style: AreaStyle) -> Self {
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

    pub fn set_points(&mut self, points: Vec<AreaPoint>) {
        self.points = points;
    }

    fn build_vertices(&self, transform: &Transform) -> Vec<Vertex> {
        if self.points.len() < 2 {
            return Vec::new();
        }

        // Find visible range with one-point margin
        let mut first_idx = None;
        let mut last_idx = None;
        for (i, p) in self.points.iter().enumerate() {
            if p.x >= transform.x_min && p.x <= transform.x_max {
                if first_idx.is_none() {
                    first_idx = Some(i);
                }
                last_idx = Some(i);
            }
        }

        let (start, end) = match (first_idx, last_idx) {
            (Some(f), Some(l)) => (f.saturating_sub(1), (l + 1).min(self.points.len() - 1)),
            _ => return Vec::new(),
        };

        let draw_slice = &self.points[start..=end];
        if draw_slice.len() < 2 {
            return Vec::new();
        }

        let mut vertices = Vec::new();

        // Fill: triangle pairs between consecutive points
        for pair in draw_slice.windows(2) {
            let p0 = &pair[0];
            let p1 = &pair[1];

            let (Some(x0), Some(x1)) = (
                transform.x_to_px_checked(p0.x),
                transform.x_to_px_checked(p1.x),
            ) else {
                continue;
            };
            let (Some(y0_upper), Some(y0_lower), Some(y1_upper), Some(y1_lower)) = (
                transform.y_to_px_checked(p0.y_upper),
                transform.y_to_px_checked(p0.y_lower),
                transform.y_to_px_checked(p1.y_upper),
                transform.y_to_px_checked(p1.y_lower),
            ) else {
                continue;
            };

            let color = self.style.fill_color.to_array();

            vertices.push(Vertex {
                position: [x0, y0_upper],
                color,
            });
            vertices.push(Vertex {
                position: [x1, y1_upper],
                color,
            });
            vertices.push(Vertex {
                position: [x0, y0_lower],
                color,
            });

            vertices.push(Vertex {
                position: [x0, y0_lower],
                color,
            });
            vertices.push(Vertex {
                position: [x1, y1_upper],
                color,
            });
            vertices.push(Vertex {
                position: [x1, y1_lower],
                color,
            });
        }

        // Upper boundary line
        if let Some(color) = self.style.upper_line_color.map(|c| c.to_array()) {
            let w = self.style.line_width;
            for pair in draw_slice.windows(2) {
                let p0 = &pair[0];
                let p1 = &pair[1];
                let Some((x0, y0)) = transform.point_to_px_checked(p0.x, p0.y_upper) else {
                    continue;
                };
                let Some((x1, y1)) = transform.point_to_px_checked(p1.x, p1.y_upper) else {
                    continue;
                };

                let dx = x1 - x0;
                let dy = y1 - y0;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 0.5 {
                    continue;
                }
                let nx = -dy / len * w / 2.0;
                let ny = dx / len * w / 2.0;

                vertices.push(Vertex {
                    position: [x0 + nx, y0 + ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x1 + nx, y1 + ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x0 - nx, y0 - ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x0 - nx, y0 - ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x1 + nx, y1 + ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x1 - nx, y1 - ny],
                    color,
                });
            }
        }

        // Lower boundary line
        if let Some(color) = self.style.lower_line_color.map(|c| c.to_array()) {
            let w = self.style.line_width;
            for pair in draw_slice.windows(2) {
                let p0 = &pair[0];
                let p1 = &pair[1];
                let Some((x0, y0)) = transform.point_to_px_checked(p0.x, p0.y_lower) else {
                    continue;
                };
                let Some((x1, y1)) = transform.point_to_px_checked(p1.x, p1.y_lower) else {
                    continue;
                };

                let dx = x1 - x0;
                let dy = y1 - y0;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 0.5 {
                    continue;
                }
                let nx = -dy / len * w / 2.0;
                let ny = dx / len * w / 2.0;

                vertices.push(Vertex {
                    position: [x0 + nx, y0 + ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x1 + nx, y1 + ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x0 - nx, y0 - ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x0 - nx, y0 - ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x1 + nx, y1 + ny],
                    color,
                });
                vertices.push(Vertex {
                    position: [x1 - nx, y1 - ny],
                    color,
                });
            }
        }

        vertices
    }
}

impl Series for AreaSeries {
    fn init(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("AreaSeries Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("AreaSeries Bind Group Layout"),
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
            label: Some("AreaSeries Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("AreaSeries Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("AreaSeries Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("AreaSeries Pipeline"),
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
                        label: Some("AreaSeries Vertex Buffer"),
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
            self.style.fill_color = scheme.area_fill;
            self.style.upper_line_color = Some(scheme.area_line);
            self.style.lower_line_color = Some(scheme.area_line);
        }
    }

    fn contributes_to_y_autoscale(&self) -> bool {
        self.contributes_to_y
    }

    fn data_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &self.points {
            if p.x >= x_min && p.x <= x_max {
                min_y = min_y.min(p.y_lower);
                max_y = max_y.max(p.y_upper);
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
        crate::plot::hit_proxy::HitProxy::Area {
            points: self.points.clone(),
        }
    }
}
