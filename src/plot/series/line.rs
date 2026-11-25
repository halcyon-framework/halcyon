// SPEC-HAL-296: LineSeries — Connected Line Rendering
//
// Renders data points as connected line segments via the overlay
// pipeline. Each segment is a screen-space quad (2 triangles, 6 vertices).
// Shares Vertex, Uniforms, and OVERLAY_SHADER with CandleSeries.

use super::{PhysicalRect, Series};
use crate::core::paint::Color;
use crate::plot::series::candle::{OVERLAY_SHADER, Uniforms, Vertex};
use crate::plot::transform::Transform;

/// A single data point for LineSeries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinePoint {
    pub x: f64,
    pub y: f64,
}

/// Configuration for line appearance.
#[derive(Debug, Clone)]
pub struct LineStyle {
    /// Line color RGBA.
    pub color: Color,
    /// Line width in pixels.
    pub width: f32,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            width: 1.5,
        }
    }
}

pub struct LineSeries {
    // Data
    points: Vec<LinePoint>,
    style: LineStyle,
    z: u32,
    auto_scale: bool,
    interaction: super::SeriesInteraction,
    theme_style: bool,

    // GPU resources (created in init)
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buffer: Option<wgpu::Buffer>,
    uniform_bind_group: Option<wgpu::BindGroup>,

    // Per-frame buffers
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_count: u32,
}

impl LineSeries {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            style: LineStyle::default(),
            z: 10,
            auto_scale: true,
            interaction: super::SeriesInteraction::None,
            theme_style: true,
            pipeline: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            vertex_buffer: None,
            vertex_count: 0,
        }
    }

    pub fn with_style(mut self, style: LineStyle) -> Self {
        self.style = style;
        self.theme_style = false;
        self
    }

    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z = z;
        self
    }

    /// Set whether this series contributes to auto-scale Y.
    pub fn with_auto_scale_y(mut self, contribute: bool) -> Self {
        self.auto_scale = contribute;
        self
    }

    /// Set interaction capability (None, Selectable, or Draggable).
    pub fn with_interaction(mut self, interaction: super::SeriesInteraction) -> Self {
        self.interaction = interaction;
        self
    }

    pub fn set_points(&mut self, points: Vec<LinePoint>) {
        self.points = points;
    }

    #[cfg(test)]
    pub(crate) fn color_for_test(&self) -> Color {
        self.style.color
    }

    /// Build line segment quads from visible data points.
    fn build_vertices(&self, transform: &Transform) -> Vec<Vertex> {
        if self.points.len() < 2 {
            return Vec::new();
        }

        let half_w = self.style.width * 0.5;
        let color = self.style.color.to_array();

        // Find visible range with one-point margin on each side
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

        let mut verts = Vec::with_capacity((draw_slice.len() - 1) * 6);

        for pair in draw_slice.windows(2) {
            let p0 = &pair[0];
            let p1 = &pair[1];

            let Some((x0, y0)) = transform.point_to_px_checked(p0.x, p0.y) else {
                continue;
            };
            let Some((x1, y1)) = transform.point_to_px_checked(p1.x, p1.y) else {
                continue;
            };

            // Line segment as a rotated quad
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = (dx * dx + dy * dy).sqrt();

            if len < 0.001 {
                continue;
            }

            // Normal perpendicular to segment direction
            let nx = -dy / len * half_w;
            let ny = dx / len * half_w;

            // Four corners
            let ax = x0 + nx;
            let ay = y0 + ny;
            let bx = x0 - nx;
            let by = y0 - ny;
            let cx = x1 + nx;
            let cy = y1 + ny;
            let ddx = x1 - nx;
            let ddy = y1 - ny;

            // Two triangles
            verts.push(Vertex {
                position: [ax, ay],
                color,
            });
            verts.push(Vertex {
                position: [cx, cy],
                color,
            });
            verts.push(Vertex {
                position: [bx, by],
                color,
            });
            verts.push(Vertex {
                position: [bx, by],
                color,
            });
            verts.push(Vertex {
                position: [cx, cy],
                color,
            });
            verts.push(Vertex {
                position: [ddx, ddy],
                color,
            });
        }

        verts
    }
}

impl Series for LineSeries {
    fn init(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LineSeries Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("LineSeries Bind Group Layout"),
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
            label: Some("LineSeries Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("LineSeries Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("LineSeries Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("LineSeries Pipeline"),
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
                        label: Some("LineSeries Vertex Buffer"),
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
        self.z
    }

    fn apply_color_scheme(&mut self, scheme: &crate::plot::PlotColorScheme) {
        if self.theme_style {
            self.style.color = scheme.line;
        }
    }

    fn hit_test(&self, x: f32, y: f32, transform: &Transform) -> Option<Box<dyn std::any::Any>> {
        if self.points.len() < 2 {
            return None;
        }

        let threshold = 10.0_f32;
        let mut best_dist = f32::INFINITY;
        let mut best_idx = 0usize;

        for (i, p) in self.points.iter().enumerate() {
            let Some((px, py)) = transform.point_to_px_checked(p.x, p.y) else {
                continue;
            };
            let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }

        if best_dist <= threshold {
            Some(Box::new(best_idx))
        } else {
            None
        }
    }

    fn selection_rect(&self, hit: &dyn std::any::Any, transform: &Transform) -> Option<[f32; 4]> {
        let idx = hit.downcast_ref::<usize>()?;
        let p = self.points.get(*idx)?;

        let (px, py) = transform.point_to_px_checked(p.x, p.y)?;

        // Small rect around the point (8px radius)
        let r = 8.0_f32;
        Some([px - r, py - r, px + r, py + r])
    }

    fn data_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut found = false;

        for p in &self.points {
            if p.x >= x_min && p.x <= x_max {
                lo = lo.min(p.y);
                hi = hi.max(p.y);
                found = true;
            }
        }

        if found { Some((lo, hi)) } else { None }
    }

    fn contributes_to_y_autoscale(&self) -> bool {
        self.auto_scale
    }

    fn interaction(&self) -> super::SeriesInteraction {
        self.interaction
    }

    fn hit_proxy(&self) -> crate::plot::hit_proxy::HitProxy {
        crate::plot::hit_proxy::HitProxy::Line {
            points: self.points.clone(),
        }
    }

    fn apply_data(&mut self, data: &super::SeriesData) -> bool {
        if let super::SeriesData::Lines(points) = data {
            self.points = points.clone();
            true
        } else {
            false
        }
    }

    fn update_last(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(point) = data.downcast::<LinePoint>() {
            if let Some(last) = self.points.last_mut() {
                *last = *point;
            }
        }
    }

    fn append_data(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(point) = data.downcast::<LinePoint>() {
            self.points.push(*point);
        }
    }
}
