// SPEC-HAL-296: BarSeries — Vertical Bar Rendering
//
// Renders data points as vertical bars anchored to a baseline.
// Used for volume, histograms, divergence indicators.
// Shares Vertex, Uniforms, and OVERLAY_SHADER with CandleSeries.

use super::{PhysicalRect, Series};
use crate::core::paint::Color;
use crate::plot::series::candle::{OVERLAY_SHADER, Uniforms, Vertex};
use crate::plot::transform::Transform;

/// A single bar data point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarPoint {
    /// X-axis center of the bar (time, index, etc.).
    pub x: f64,
    /// Bar value (height extends from baseline to this value).
    pub y: f64,
    /// Optional per-bar color override. None = use series default.
    pub color: Option<Color>,
}

/// Configuration for bar appearance.
#[derive(Debug, Clone)]
pub struct BarStyle {
    /// Default bar color RGBA (used when BarPoint.color is None).
    pub color: Color,
    /// Bar width as a fraction of the interval between adjacent bars.
    /// 1.0 = bars touch, 0.8 = 20% gap between bars. Default 0.8.
    pub width_fraction: f32,
    /// Y value where bars start. Default 0.0 (bars grow upward from zero).
    pub baseline: f64,
    /// If set, bars ignore the chart's Y axis and instead normalize
    /// to occupy this fraction of the viewport height, anchored to
    /// the bottom. E.g. 0.2 = bars fill the bottom 20% of the chart.
    /// None = use world Y coordinates (default).
    pub viewport_fraction: Option<f32>,
}

impl Default for BarStyle {
    fn default() -> Self {
        Self {
            color: Color::from_srgb(0.3, 0.5, 1.0, 0.7),
            width_fraction: 0.8,
            baseline: 0.0,
            viewport_fraction: None,
        }
    }
}

pub struct BarSeries {
    // Data
    points: Vec<BarPoint>,
    style: BarStyle,
    z: u32,
    auto_scale: bool,
    theme_style: bool,
    volume_ratios: Option<Vec<f32>>,
    viewport_scrim_color: Color,
    theme_viewport_scrim: bool,

    // GPU resources (created in init)
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buffer: Option<wgpu::Buffer>,
    uniform_bind_group: Option<wgpu::BindGroup>,

    // Per-frame buffers
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_count: u32,
}

impl BarSeries {
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            style: BarStyle::default(),
            z: 1,
            auto_scale: true,
            theme_style: true,
            volume_ratios: None,
            viewport_scrim_color: Color::from_linear(0.0, 0.0, 0.0, 0.65),
            theme_viewport_scrim: true,
            pipeline: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            vertex_buffer: None,
            vertex_count: 0,
        }
    }

    pub fn with_style(mut self, style: BarStyle) -> Self {
        self.style = style;
        self.theme_style = false;
        self.volume_ratios = None;
        self
    }

    /// Set viewport-relative height while retaining the theme-derived bar color.
    pub fn with_viewport_fraction(mut self, fraction: f32) -> Self {
        self.style.viewport_fraction = Some(fraction);
        self
    }

    /// Override the theme-derived scrim behind viewport-relative bars.
    pub fn with_viewport_scrim_color(mut self, color: Color) -> Self {
        self.viewport_scrim_color = color;
        self.theme_viewport_scrim = false;
        self
    }

    pub(crate) fn with_volume_ratios(mut self, ratios: Vec<f32>) -> Self {
        self.volume_ratios = Some(ratios);
        self
    }

    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z = z;
        self
    }

    /// Set whether this series contributes to auto-scale Y.
    /// Default: true. Set false for overlay bars (volume behind candles).
    pub fn with_auto_scale_y(mut self, contribute: bool) -> Self {
        self.auto_scale = contribute;
        self
    }

    pub fn set_points(&mut self, points: Vec<BarPoint>) {
        self.points = points;
    }

    #[cfg(test)]
    pub(crate) fn color_for_test(&self) -> Color {
        self.style.color
    }

    #[cfg(test)]
    pub(crate) fn viewport_scrim_color_for_test(&self) -> Color {
        self.viewport_scrim_color
    }

    /// Compute bar width in world units from the data interval.
    fn bar_world_width(&self) -> f64 {
        if self.points.len() < 2 {
            return 1.0;
        }
        let mut intervals: Vec<f64> = self
            .points
            .windows(2)
            .map(|w| (w[1].x - w[0].x).abs())
            .filter(|&d| d > 0.0)
            .collect();

        if intervals.is_empty() {
            return 1.0;
        }
        intervals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        intervals[intervals.len() / 2] * self.style.width_fraction as f64
    }

    fn build_vertices(
        &self,
        transform: &Transform,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Vec<Vertex> {
        if self.points.is_empty() {
            return Vec::new();
        }

        let half_w = self.bar_world_width() * 0.5;
        let viewport_mode = self.style.viewport_fraction;

        // For viewport-relative mode: find max visible |y - baseline|
        let max_visible_value = if viewport_mode.is_some() {
            self.points
                .iter()
                .filter(|p| p.x + half_w >= transform.x_min && p.x - half_w <= transform.x_max)
                .map(|p| (p.y - self.style.baseline).abs())
                .fold(0.0_f64, f64::max)
                .max(1e-15)
        } else {
            1.0
        };

        let mut verts = Vec::with_capacity(self.points.len() * 6 + 6);

        // Theme-resolved scrim behind viewport-relative bars.
        if let Some(frac) = viewport_mode {
            let scrim_top = viewport_height * (1.0 - frac);
            let scrim_color = self.viewport_scrim_color.to_array();
            verts.push(Vertex {
                position: [0.0, scrim_top],
                color: scrim_color,
            });
            verts.push(Vertex {
                position: [viewport_width, scrim_top],
                color: scrim_color,
            });
            verts.push(Vertex {
                position: [0.0, viewport_height],
                color: scrim_color,
            });
            verts.push(Vertex {
                position: [0.0, viewport_height],
                color: scrim_color,
            });
            verts.push(Vertex {
                position: [viewport_width, scrim_top],
                color: scrim_color,
            });
            verts.push(Vertex {
                position: [viewport_width, viewport_height],
                color: scrim_color,
            });
        }

        for p in &self.points {
            if p.x + half_w < transform.x_min || p.x - half_w > transform.x_max {
                continue;
            }

            let (Some(x1), Some(x2)) = (
                transform.x_to_px_checked(p.x - half_w),
                transform.x_to_px_checked(p.x + half_w),
            ) else {
                continue;
            };

            let (top, bot) = if let Some(frac) = viewport_mode {
                let normalized = ((p.y - self.style.baseline) / max_visible_value) as f32;
                let bar_height = normalized.abs() * frac * viewport_height;
                let bottom = viewport_height;
                let top_y = bottom - bar_height;
                (top_y, bottom)
            } else {
                let (Some(y_val), Some(y_base)) = (
                    transform.y_to_px_checked(p.y),
                    transform.y_to_px_checked(self.style.baseline),
                ) else {
                    continue;
                };
                (y_val.min(y_base), y_val.max(y_base))
            };

            if (bot - top) < 0.5 {
                continue;
            }

            let color = p.color.unwrap_or(self.style.color).to_array();

            verts.push(Vertex {
                position: [x1, top],
                color,
            });
            verts.push(Vertex {
                position: [x2, top],
                color,
            });
            verts.push(Vertex {
                position: [x1, bot],
                color,
            });
            verts.push(Vertex {
                position: [x1, bot],
                color,
            });
            verts.push(Vertex {
                position: [x2, top],
                color,
            });
            verts.push(Vertex {
                position: [x2, bot],
                color,
            });
        }

        verts
    }
}

impl Series for BarSeries {
    fn init(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BarSeries Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BarSeries Bind Group Layout"),
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
            label: Some("BarSeries Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BarSeries Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BarSeries Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BarSeries Pipeline"),
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

        let verts = self.build_vertices(transform, viewport.width as f32, viewport.height as f32);
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
                        label: Some("BarSeries Vertex Buffer"),
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
        if self.theme_viewport_scrim {
            self.viewport_scrim_color = scheme.volume_scrim;
        }

        if self.theme_style {
            self.style.color = scheme.bar;
        }
        if self.theme_style
            && let Some(ratios) = &self.volume_ratios
        {
            for (point, ratio) in self.points.iter_mut().zip(ratios.iter().copied()) {
                let (start, end, t) = if ratio >= 0.5 {
                    (
                        scheme.volume_neutral_bullish,
                        scheme.volume_bullish,
                        (ratio - 0.5) * 2.0,
                    )
                } else {
                    (
                        scheme.volume_bearish,
                        scheme.volume_neutral_bearish,
                        ratio * 2.0,
                    )
                };
                point.color = Some(crate::core::paint::Color::from_linear(
                    start.r + (end.r - start.r) * t,
                    start.g + (end.g - start.g) * t,
                    start.b + (end.b - start.b) * t,
                    start.a + (end.a - start.a) * t,
                ));
            }
        }
    }

    fn hit_test(&self, _x: f32, _y: f32, _transform: &Transform) -> Option<Box<dyn std::any::Any>> {
        None
    }

    fn data_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        let half_w = self.bar_world_width() * 0.5;
        let mut lo = self.style.baseline;
        let mut hi = self.style.baseline;
        let mut found = false;

        for p in &self.points {
            if p.x + half_w >= x_min && p.x - half_w <= x_max {
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

    fn hit_proxy(&self) -> crate::plot::hit_proxy::HitProxy {
        crate::plot::hit_proxy::HitProxy::Bar {
            points: self.points.clone(),
        }
    }

    fn apply_data(&mut self, data: &super::SeriesData) -> bool {
        if let super::SeriesData::Bars(points) = data {
            self.points = points.clone();
            true
        } else {
            false
        }
    }

    fn update_last(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(point) = data.downcast::<BarPoint>() {
            if let Some(last) = self.points.last_mut() {
                *last = *point;
            }
        }
    }

    fn append_data(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(point) = data.downcast::<BarPoint>() {
            self.points.push(*point);
        }
    }
}
