// SPEC-HAL-300: DepthBarSeries — Horizontal Depth Bars
//
// Projection-only. Does not own book state, reconcile updates,
// track history, or classify levels. Consumer's book engine
// provides pre-classified BookSide snapshots; this Series renders them.
//
// Horizontal bars at each price level. Bids extend left from center,
// asks extend right. Separate sides, independent updates.

use super::{PhysicalRect, Series};
use crate::core::paint::Color;
use crate::plot::series::candle::{OVERLAY_SHADER, Uniforms, Vertex};
use crate::plot::transform::Transform;

pub use crate::model::order_book::{BookLevel, BookSide, BookSideKind, LevelClassification};

/// Visual treatment for a LevelClassification.
#[derive(Debug, Clone)]
pub struct ClassificationStyle {
    /// Outline color. None = no outline.
    pub outline_color: Option<Color>,
    /// Bar color override. None = use side default.
    pub bar_color: Option<Color>,
    /// Opacity multiplier (applied to bar color alpha).
    pub opacity: f32,
}

impl Default for ClassificationStyle {
    fn default() -> Self {
        Self {
            outline_color: None,
            bar_color: None,
            opacity: 1.0,
        }
    }
}

/// Style configuration for order book rendering.
#[derive(Debug, Clone)]
pub struct DepthBarStyle {
    /// Bid bar color (default: green).
    pub bid_color: Color,
    /// Ask bar color (default: red).
    pub ask_color: Color,
    /// Bar height as fraction of tick spacing. Default 0.8.
    pub bar_height_fraction: f32,
    /// Style overrides per classification.
    pub wall_style: ClassificationStyle,
    pub spoof_style: ClassificationStyle,
    pub persistent_style: ClassificationStyle,
    pub iceberg_style: ClassificationStyle,
    pub own_order_style: ClassificationStyle,
}

impl Default for DepthBarStyle {
    fn default() -> Self {
        Self {
            bid_color: Color::from_hex(0x2ECC71).with_alpha(0.5),
            ask_color: Color::from_hex(0xF05454).with_alpha(0.5),
            bar_height_fraction: 0.8,
            wall_style: ClassificationStyle {
                outline_color: Some(Color::from_hex(0xFFFF00).with_alpha(0.9)),
                bar_color: None,
                opacity: 1.0,
            },
            spoof_style: ClassificationStyle {
                outline_color: Some(Color::from_hex(0xFF8000).with_alpha(0.7)),
                bar_color: None,
                opacity: 0.7,
            },
            persistent_style: ClassificationStyle {
                outline_color: Some(Color::from_srgb(0.5, 0.5, 0.8, 0.6)),
                bar_color: None,
                opacity: 1.0,
            },
            iceberg_style: ClassificationStyle {
                outline_color: Some(Color::from_srgb(0.3, 0.8, 1.0, 0.8)),
                bar_color: None,
                opacity: 1.0,
            },
            own_order_style: ClassificationStyle {
                outline_color: Some(Color::from_srgb(0.0, 0.8, 1.0, 1.0)),
                bar_color: Some(Color::from_srgb(0.0, 0.6, 0.8, 0.8)),
                opacity: 1.0,
            },
        }
    }
}

/// Events emitted by order book interaction.
#[derive(Debug, Clone)]
pub enum BookEvent {
    LevelHoverEnter {
        price: f64,
        side: BookSideKind,
        quantity: f64,
        classification: LevelClassification,
    },
    LevelHoverLeave,
    LevelClicked {
        price: f64,
        side: BookSideKind,
        quantity: f64,
        classification: LevelClassification,
    },
}

pub struct DepthBarSeries {
    bids: BookSide,
    asks: BookSide,
    style: DepthBarStyle,
    z_order: u32,
    theme_style: bool,

    // GPU state
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buffer: Option<wgpu::Buffer>,
    uniform_bind_group: Option<wgpu::BindGroup>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_count: u32,

    // Interaction state
    pending_events: Vec<BookEvent>,
}

impl DepthBarSeries {
    pub fn new() -> Self {
        Self {
            bids: BookSide::new(),
            asks: BookSide::new(),
            style: DepthBarStyle::default(),
            z_order: 0,
            theme_style: true,
            pipeline: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            vertex_buffer: None,
            vertex_count: 0,
            pending_events: Vec::new(),
        }
    }

    pub fn with_style(mut self, style: DepthBarStyle) -> Self {
        self.style = style;
        self.theme_style = false;
        self
    }

    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z_order = z;
        self
    }

    pub fn set_bids(&mut self, bids: BookSide) {
        self.bids = bids;
    }

    pub fn set_asks(&mut self, asks: BookSide) {
        self.asks = asks;
    }

    pub fn set_both(&mut self, bids: BookSide, asks: BookSide) {
        self.bids = bids;
        self.asks = asks;
    }

    /// Get pending book events (drains queue).
    pub fn take_events(&mut self) -> Vec<BookEvent> {
        std::mem::take(&mut self.pending_events)
    }

    fn infer_tick_spacing(&self) -> f64 {
        let mut prices: Vec<f64> = self
            .bids
            .levels
            .iter()
            .chain(self.asks.levels.iter())
            .map(|l| l.price)
            .collect();

        if prices.len() < 2 {
            return 1.0;
        }

        prices.sort_by(|a, b| a.partial_cmp(b).unwrap());
        prices.dedup();

        let mut min_diff = f64::INFINITY;
        for i in 1..prices.len() {
            let diff = prices[i] - prices[i - 1];
            if diff > f64::EPSILON && diff < min_diff {
                min_diff = diff;
            }
        }

        if min_diff.is_finite() { min_diff } else { 1.0 }
    }

    fn resolve_classification_style(
        &self,
        classification: &LevelClassification,
        base_color: Color,
    ) -> ([f32; 4], Option<[f32; 4]>) {
        let style = match classification {
            LevelClassification::Normal => return (base_color.to_array(), None),
            LevelClassification::Wall => &self.style.wall_style,
            LevelClassification::SuspectedSpoof => &self.style.spoof_style,
            LevelClassification::Persistent => &self.style.persistent_style,
            LevelClassification::Iceberg { .. } => &self.style.iceberg_style,
            LevelClassification::OwnOrder => &self.style.own_order_style,
        };

        let color = if let Some(override_color) = style.bar_color {
            override_color.to_array()
        } else {
            base_color
                .with_alpha(base_color.a * style.opacity)
                .to_array()
        };

        (color, style.outline_color.map(|c| c.to_array()))
    }

    fn push_rect(verts: &mut Vec<Vertex>, x1: f32, y1: f32, x2: f32, y2: f32, color: [f32; 4]) {
        verts.push(Vertex {
            position: [x1, y1],
            color,
        });
        verts.push(Vertex {
            position: [x2, y1],
            color,
        });
        verts.push(Vertex {
            position: [x1, y2],
            color,
        });
        verts.push(Vertex {
            position: [x1, y2],
            color,
        });
        verts.push(Vertex {
            position: [x2, y1],
            color,
        });
        verts.push(Vertex {
            position: [x2, y2],
            color,
        });
    }

    fn build_vertices(&self, transform: &Transform) -> Vec<Vertex> {
        if self.bids.levels.is_empty() && self.asks.levels.is_empty() {
            return Vec::new();
        }

        let tick_h = self.infer_tick_spacing();
        let bar_half_h = tick_h * self.style.bar_height_fraction as f64 / 2.0;

        let mut vertices = Vec::new();
        let Some(x_center) = transform.x_to_px_checked(0.0) else {
            return Vec::new();
        };
        let outline_t = 1.0_f32;

        // Bids: bars extend LEFT from center (0 to -quantity)
        for level in &self.bids.levels {
            let (Some(y_top), Some(y_bot), Some(x_left)) = (
                transform.y_to_px_checked(level.price + bar_half_h),
                transform.y_to_px_checked(level.price - bar_half_h),
                transform.x_to_px_checked(-level.quantity),
            ) else {
                continue;
            };

            let (bar_color, outline) =
                self.resolve_classification_style(&level.classification, self.style.bid_color);

            Self::push_rect(&mut vertices, x_left, y_top, x_center, y_bot, bar_color);

            if let Some(oc) = outline {
                Self::push_rect(
                    &mut vertices,
                    x_left,
                    y_top,
                    x_center,
                    y_top + outline_t,
                    oc,
                );
                Self::push_rect(
                    &mut vertices,
                    x_left,
                    y_bot - outline_t,
                    x_center,
                    y_bot,
                    oc,
                );
                Self::push_rect(&mut vertices, x_left, y_top, x_left + outline_t, y_bot, oc);
                Self::push_rect(
                    &mut vertices,
                    x_center - outline_t,
                    y_top,
                    x_center,
                    y_bot,
                    oc,
                );
            }

            if let LevelClassification::Iceberg { inferred_total } = level.classification {
                let Some(x_ext) = transform.x_to_px_checked(-inferred_total) else {
                    continue;
                };
                let ext_color = [
                    bar_color[0],
                    bar_color[1],
                    bar_color[2],
                    bar_color[3] * 0.25,
                ];
                Self::push_rect(&mut vertices, x_ext, y_top, x_left, y_bot, ext_color);
            }
        }

        // Asks: bars extend RIGHT from center (0 to +quantity)
        for level in &self.asks.levels {
            let (Some(y_top), Some(y_bot), Some(x_right)) = (
                transform.y_to_px_checked(level.price + bar_half_h),
                transform.y_to_px_checked(level.price - bar_half_h),
                transform.x_to_px_checked(level.quantity),
            ) else {
                continue;
            };

            let (bar_color, outline) =
                self.resolve_classification_style(&level.classification, self.style.ask_color);

            Self::push_rect(&mut vertices, x_center, y_top, x_right, y_bot, bar_color);

            if let Some(oc) = outline {
                Self::push_rect(
                    &mut vertices,
                    x_center,
                    y_top,
                    x_right,
                    y_top + outline_t,
                    oc,
                );
                Self::push_rect(
                    &mut vertices,
                    x_center,
                    y_bot - outline_t,
                    x_right,
                    y_bot,
                    oc,
                );
                Self::push_rect(
                    &mut vertices,
                    x_center,
                    y_top,
                    x_center + outline_t,
                    y_bot,
                    oc,
                );
                Self::push_rect(
                    &mut vertices,
                    x_right - outline_t,
                    y_top,
                    x_right,
                    y_bot,
                    oc,
                );
            }

            if let LevelClassification::Iceberg { inferred_total } = level.classification {
                let Some(x_ext) = transform.x_to_px_checked(inferred_total) else {
                    continue;
                };
                let ext_color = [
                    bar_color[0],
                    bar_color[1],
                    bar_color[2],
                    bar_color[3] * 0.25,
                ];
                Self::push_rect(&mut vertices, x_right, y_top, x_ext, y_bot, ext_color);
            }
        }

        vertices
    }
}

impl Series for DepthBarSeries {
    fn init(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DepthBarSeries Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DepthBarSeries Bind Group Layout"),
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
            label: Some("DepthBarSeries Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DepthBarSeries Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DepthBarSeries Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("DepthBarSeries Pipeline"),
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
                        label: Some("DepthBarSeries Vertex Buffer"),
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
            self.style.bid_color = scheme.depth_bid;
            self.style.ask_color = scheme.depth_ask;
        }
    }

    fn contributes_to_y_autoscale(&self) -> bool {
        true
    }

    fn data_y_range(&self, _x_min: f64, _x_max: f64) -> Option<(f64, f64)> {
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for l in &self.bids.levels {
            min_y = min_y.min(l.price);
            max_y = max_y.max(l.price);
        }
        for l in &self.asks.levels {
            min_y = min_y.min(l.price);
            max_y = max_y.max(l.price);
        }
        if min_y <= max_y {
            Some((min_y, max_y))
        } else {
            None
        }
    }

    fn hit_test(&self, x: f32, y: f32, transform: &Transform) -> Option<Box<dyn std::any::Any>> {
        let world_y = transform.px_to_y_checked(y)?;
        let world_x = transform.px_to_x_checked(x)?;

        let (side, levels) = if world_x < 0.0 {
            (BookSideKind::Bid, &self.bids.levels)
        } else {
            (BookSideKind::Ask, &self.asks.levels)
        };

        let tick = self.infer_tick_spacing();
        let threshold = tick * 0.6;

        levels
            .iter()
            .min_by(|a, b| {
                (a.price - world_y)
                    .abs()
                    .partial_cmp(&(b.price - world_y).abs())
                    .unwrap()
            })
            .filter(|l| (l.price - world_y).abs() <= threshold)
            .map(|l| -> Box<dyn std::any::Any> {
                Box::new((l.price, side, l.quantity, l.classification))
            })
    }

    fn hit_proxy(&self) -> crate::plot::hit_proxy::HitProxy {
        crate::plot::hit_proxy::HitProxy::Depth {
            bids: self.bids.levels.clone(),
            asks: self.asks.levels.clone(),
            tick_spacing: self.infer_tick_spacing(),
        }
    }

    fn apply_data(&mut self, data: &super::SeriesData) -> bool {
        match data {
            super::SeriesData::BookUpdate(super::BookUpdateKind::Bids(bids)) => {
                self.bids = bids.clone();
                true
            }
            super::SeriesData::BookUpdate(super::BookUpdateKind::Asks(asks)) => {
                self.asks = asks.clone();
                true
            }
            super::SeriesData::BookUpdate(super::BookUpdateKind::Snapshot { bids, asks }) => {
                self.bids = bids.clone();
                self.asks = asks.clone();
                true
            }
            _ => false,
        }
    }
}
