// SPEC-HAL-296A: CircleSeries — SDF Circle Rendering
//
// Renders data points as filled/bordered circles using an SDF circle
// shader. Each point is one quad (6 vertices, 2 triangles). The
// fragment shader evaluates distance(frag_pos, center) for smooth
// antialiased edges at any radius.
//
// INV-296-SC1: SDF rendering, not triangle-fan polygon approximation.
// INV-296-SC3: World coordinates are f64 until after Transform.

use super::{PhysicalRect, Series};
use crate::core::paint::Color;
use crate::plot::series::candle::Uniforms;
use crate::plot::transform::Transform;

/// A single data point for CircleSeries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    /// World X coordinate.
    pub x: f64,
    /// World Y coordinate.
    pub y: f64,
    /// Per-point radius override in pixels. None → use CircleStyle.radius.
    pub radius: Option<f32>,
    /// Per-point fill color override. None → use CircleStyle.fill_color.
    pub fill_color: Option<Color>,
}

/// Visual configuration for CircleSeries.
#[derive(Debug, Clone)]
pub struct CircleStyle {
    /// Default fill color for all points.
    pub fill_color: Color,
    /// Default radius in pixels.
    pub radius: f32,
    /// Border color. Alpha 0 = no border.
    pub border_color: Color,
    /// Border width in pixels. 0 = no border.
    pub border_width: f32,
}

impl Default for CircleStyle {
    fn default() -> Self {
        Self {
            fill_color: Color::WHITE,
            radius: 4.0,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
        }
    }
}

// ========================================================================
// GPU Types
// ========================================================================

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    /// Quad corner position in screen pixels.
    pub position: [f32; 2],
    /// Circle center in screen pixels.
    pub center: [f32; 2],
    /// Circle radius in pixels (outer edge of fill).
    pub radius: f32,
    /// Fill color RGBA.
    pub fill_color: [f32; 4],
    /// Border color RGBA (alpha 0 = no border).
    pub border_color: [f32; 4],
    /// Border width in pixels (0 = no border).
    pub border_width: f32,
}

impl CircleVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        static ATTRS: &[wgpu::VertexAttribute] = &[
            // location 0: position (vec2<f32>)
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            // location 1: center (vec2<f32>)
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
            // location 2: radius (f32)
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32,
            },
            // location 3: fill_color (vec4<f32>)
            wgpu::VertexAttribute {
                offset: 20,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            // location 4: border_color (vec4<f32>)
            wgpu::VertexAttribute {
                offset: 36,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32x4,
            },
            // location 5: border_width (f32)
            wgpu::VertexAttribute {
                offset: 52,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CircleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

// ========================================================================
// Shader
// ========================================================================

pub(crate) const CIRCLE_SHADER: &str = r#"
struct Uniforms {
    viewport_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) fill_color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) border_width: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) center: vec2<f32>,
    @location(1) radius: f32,
    @location(2) fill_color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
    @location(5) frag_pos: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let ndc_x = (in.position.x / uniforms.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / uniforms.viewport_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.center = in.center;
    out.radius = in.radius;
    out.fill_color = in.fill_color;
    out.border_color = in.border_color;
    out.border_width = in.border_width;
    out.frag_pos = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = distance(in.frag_pos, in.center);
    let outer_r = in.radius;
    let inner_r = in.radius - in.border_width;

    // Antialiased edge (1px smoothstep at outer boundary)
    let outer_alpha = 1.0 - smoothstep(outer_r - 0.5, outer_r + 0.5, dist);

    if outer_alpha <= 0.0 {
        discard;
    }

    // Border vs fill
    var color: vec4<f32>;
    if in.border_width > 0.0 && dist > inner_r - 0.5 {
        let border_blend = smoothstep(inner_r - 0.5, inner_r + 0.5, dist);
        color = mix(in.fill_color, in.border_color, border_blend);
    } else {
        color = in.fill_color;
    }

    color.a = color.a * outer_alpha;
    return color;
}
"#;

// ========================================================================
// CircleSeries
// ========================================================================

pub struct CircleSeries {
    // Data
    points: Vec<Circle>,
    style: CircleStyle,
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

impl CircleSeries {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            style: CircleStyle::default(),
            z: 15,
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

    pub fn with_style(mut self, style: CircleStyle) -> Self {
        self.style = style;
        self.theme_style = false;
        self
    }

    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z = z;
        self
    }

    pub fn with_auto_scale_y(mut self, contribute: bool) -> Self {
        self.auto_scale = contribute;
        self
    }

    pub fn with_interaction(mut self, interaction: super::SeriesInteraction) -> Self {
        self.interaction = interaction;
        self
    }

    pub fn set_points(&mut self, points: Vec<Circle>) {
        self.points = points;
    }

    /// SPEC-HAL-296A §2.5: Build SDF circle quads from visible points.
    fn build_vertices(&self, transform: &Transform) -> Vec<CircleVertex> {
        if self.points.is_empty() {
            return Vec::new();
        }

        let default_fill = self.style.fill_color.to_array();
        let border = self.style.border_color.to_array();
        let bw = self.style.border_width;
        let default_r = self.style.radius;

        // INV-296-SC4: viewport culling with radius margin
        let max_r = self
            .points
            .iter()
            .filter_map(|p| p.radius)
            .fold(default_r, f32::max);
        // Convert pixel margin to world-space for culling
        let world_margin_x = (max_r + 1.0) / ((transform.x_max - transform.x_min) as f32).max(1.0);
        let world_margin_y = (max_r + 1.0) / ((transform.y_max - transform.y_min) as f32).max(1.0);
        let x_lo = transform.x_min - world_margin_x as f64;
        let x_hi = transform.x_max + world_margin_x as f64;
        let y_lo = transform.y_min - world_margin_y as f64;
        let y_hi = transform.y_max + world_margin_y as f64;

        let mut verts = Vec::with_capacity(self.points.len() * 6);

        for p in &self.points {
            // Viewport culling
            if p.x < x_lo || p.x > x_hi || p.y < y_lo || p.y > y_hi {
                continue;
            }

            let Some((px, py)) = transform.point_to_px_checked(p.x, p.y) else {
                continue;
            };
            let r = p.radius.unwrap_or(default_r);
            let fill = p.fill_color.map_or(default_fill, |c| c.to_array());

            // Quad extends radius + 1px AA margin from center
            let margin = r + 1.0;
            let x0 = px - margin;
            let y0 = py - margin;
            let x1 = px + margin;
            let y1 = py + margin;

            let base = |pos: [f32; 2]| CircleVertex {
                position: pos,
                center: [px, py],
                radius: r,
                fill_color: fill,
                border_color: border,
                border_width: bw,
            };

            // Two triangles: TL-TR-BL, BL-TR-BR
            verts.push(base([x0, y0]));
            verts.push(base([x1, y0]));
            verts.push(base([x0, y1]));
            verts.push(base([x0, y1]));
            verts.push(base([x1, y0]));
            verts.push(base([x1, y1]));
        }

        verts
    }
}

impl Series for CircleSeries {
    fn init(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CircleSeries Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CircleSeries Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CircleSeries Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CircleSeries Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CircleSeries Circle Shader"),
            source: wgpu::ShaderSource::Wgsl(CIRCLE_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("CircleSeries Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(CircleVertex::desc())],
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
                        label: Some("CircleSeries Vertex Buffer"),
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
            self.style.fill_color = scheme.marker_fill;
            self.style.border_color = scheme.marker_border;
        }
    }

    /// INV-296-SC5: Hit testing accounts for per-point radius.
    fn hit_test(&self, x: f32, y: f32, transform: &Transform) -> Option<Box<dyn std::any::Any>> {
        let threshold = 2.0_f32;
        let mut best_effective = f32::INFINITY;
        let mut best_idx = 0usize;

        for (i, p) in self.points.iter().enumerate() {
            let Some((px, py)) = transform.point_to_px_checked(p.x, p.y) else {
                continue;
            };
            let r = p.radius.unwrap_or(self.style.radius);
            let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            let effective = dist - r; // negative = inside circle
            if effective < best_effective {
                best_effective = effective;
                best_idx = i;
            }
        }

        if best_effective <= threshold {
            Some(Box::new(best_idx))
        } else {
            None
        }
    }

    fn selection_rect(&self, hit: &dyn std::any::Any, transform: &Transform) -> Option<[f32; 4]> {
        let idx = hit.downcast_ref::<usize>()?;
        let p = self.points.get(*idx)?;
        let (px, py) = transform.point_to_px_checked(p.x, p.y)?;
        let r = p.radius.unwrap_or(self.style.radius) + 2.0;
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
        crate::plot::hit_proxy::HitProxy::Circles {
            circles: self.points.clone(),
            default_radius: self.style.radius,
        }
    }

    fn apply_data(&mut self, data: &super::SeriesData) -> bool {
        if let super::SeriesData::Circles(points) = data {
            self.points = points.clone();
            true
        } else {
            false
        }
    }

    fn update_last(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(point) = data.downcast::<Circle>() {
            if let Some(last) = self.points.last_mut() {
                *last = *point;
            }
        }
    }

    fn append_data(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(point) = data.downcast::<Circle>() {
            self.points.push(*point);
        }
    }
}
