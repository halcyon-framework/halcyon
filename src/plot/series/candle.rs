// SPEC-HAL-296: CandleSeries — GPU-Instanced Candlestick Rendering
//
// Ported from the original chart renderer. Candle bodies are instanced quads with
// shader-based borders. Wicks are vertex-based line quads rendered
// behind bodies via the overlay pipeline.

use super::{PhysicalRect, Series};
use crate::core::scene::Color;
use crate::plot::transform::Transform;

// ========================================================================
// Data Types
// ========================================================================

/// Geometry-ready candle data. Consumer creates these from domain data.
/// All prices f64, all times f64 (generalized from i64 microseconds).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderCandle {
    /// X-axis start (time, index, or arbitrary f64).
    pub x_open: f64,
    /// X-axis end.
    pub x_close: f64,
    /// Highest Y value (wick top).
    pub high: f64,
    /// Lowest Y value (wick bottom).
    pub low: f64,
    /// Body top = max(open_price, close_price). Consumer computes this.
    pub body_top: f64,
    /// Body bottom = min(open_price, close_price). Consumer computes this.
    pub body_bottom: f64,
    /// True if bullish (close >= open). Drives color selection.
    pub is_up: bool,
    /// Volume (optional — 0.0 if unavailable).
    pub volume: f64,
    /// Optional body color override. When Some, replaces the
    /// default up/down color for this candle's body fill.
    /// Wicks still use the standard color. Consumer uses this
    /// for confidence coloring, interpolation markers, etc.
    pub body_color_override: Option<Color>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CandleInstance {
    pub body_rect: [f32; 4],    // x1, y1, x2, y2 in screen pixels
    pub wick_x: f32,            // wick center x in pixels
    pub wick_top: f32,          // wick top y in pixels
    pub wick_bottom: f32,       // wick bottom y in pixels
    pub body_color: [f32; 4],   // candle body RGBA
    pub border_color: [f32; 4], // border RGBA (alpha 0 = no border)
    pub border_px: f32,         // border width in pixels (0 = no border)
}

impl CandleInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        static ATTRS: &[wgpu::VertexAttribute] = &[
            // location 2: body_rect (vec4<f32>)
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            // location 3: wick_data (vec3<f32>) — wick_x, wick_top, wick_bottom
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x3,
            },
            // location 4: body_color (vec4<f32>)
            wgpu::VertexAttribute {
                offset: 28,
                shader_location: 4,
                format: wgpu::VertexFormat::Float32x4,
            },
            // location 5: border_color (vec4<f32>)
            wgpu::VertexAttribute {
                offset: 44,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32x4,
            },
            // location 6: border_px (f32)
            wgpu::VertexAttribute {
                offset: 60,
                shader_location: 6,
                format: wgpu::VertexFormat::Float32,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CandleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2], // screen-space pixels
    pub color: [f32; 4],    // RGBA
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        static ATTRS: &[wgpu::VertexAttribute] = &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub viewport_size: [f32; 2],
}

// ========================================================================
// Shaders
// ========================================================================

const CANDLE_BODY_SHADER: &str = r#"
struct Uniforms {
    viewport_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
}

struct InstanceInput {
    @location(2) body_rect: vec4<f32>,
    @location(3) wick_data: vec3<f32>,
    @location(4) body_color: vec4<f32>,
    @location(5) border_color: vec4<f32>,
    @location(6) border_px: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) body_color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) border_px_norm: f32,
    @location(4) body_size_px: vec2<f32>,
    @location(5) part: f32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    var uv: vec2<f32>;
    var pos: vec2<f32>;

    let part = vertex.vertex_index / 6u;
    let local_idx = vertex.vertex_index % 6u;

    let x1 = instance.body_rect.x;
    let y1 = instance.body_rect.y;
    let x2 = instance.body_rect.z;
    let y2 = instance.body_rect.w;

    if (part == 0u) {
        // Body quad
        switch local_idx {
            case 0u: { pos = vec2<f32>(x1, y1); uv = vec2<f32>(0.0, 0.0); }
            case 1u: { pos = vec2<f32>(x2, y1); uv = vec2<f32>(1.0, 0.0); }
            case 2u: { pos = vec2<f32>(x1, y2); uv = vec2<f32>(0.0, 1.0); }
            case 3u: { pos = vec2<f32>(x1, y2); uv = vec2<f32>(0.0, 1.0); }
            case 4u: { pos = vec2<f32>(x2, y1); uv = vec2<f32>(1.0, 0.0); }
            default: { pos = vec2<f32>(x2, y2); uv = vec2<f32>(1.0, 1.0); }
        }
    } else {
        // Wick quad — thin line when body is wide, full-width when narrow
        let body_w = x2 - x1;
        var wick_half_w: f32;
        if (body_w < 4.0) {
            // Narrow candle: wick matches body width (single bar look)
            wick_half_w = body_w / 2.0;
        } else {
            wick_half_w = clamp(body_w * 0.15, 0.5, 1.5);
        }
        let wick_x = instance.wick_data.x;
        let wl = wick_x - wick_half_w;
        let wr = wick_x + wick_half_w;

        var wt: f32;
        var wb: f32;
        if (part == 1u) {
            wt = instance.wick_data.y;
            wb = y1;
        } else {
            wt = y2;
            wb = instance.wick_data.z;
        }

        switch local_idx {
            case 0u: { pos = vec2<f32>(wl, wt); }
            case 1u: { pos = vec2<f32>(wr, wt); }
            case 2u: { pos = vec2<f32>(wl, wb); }
            case 3u: { pos = vec2<f32>(wl, wb); }
            case 4u: { pos = vec2<f32>(wr, wt); }
            default: { pos = vec2<f32>(wr, wb); }
        }
        uv = vec2<f32>(0.5, 0.5);
    }

    let ndc_x = (pos.x / uniforms.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pos.y / uniforms.viewport_size.y) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = uv;
    out.body_color = instance.body_color;
    out.border_color = instance.border_color;
    out.border_px_norm = instance.border_px;
    out.body_size_px = vec2<f32>(x2 - x1, y2 - y1);
    out.part = f32(part);

    return out;
}

struct FragmentInput {
    @location(0) uv: vec2<f32>,
    @location(1) body_color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) border_px_norm: f32,
    @location(4) body_size_px: vec2<f32>,
    @location(5) part: f32,
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    if (in.part > 0.5) {
        // Wick: solid body color
        return in.body_color;
    }
    // Body: border logic
    let body_width_px = in.body_size_px.x;
    let border_threshold = in.border_px_norm / body_width_px;
    let dist_left = in.uv.x;
    let dist_right = 1.0 - in.uv.x;

    if (dist_left < border_threshold || dist_right < border_threshold) {
        return in.border_color;
    }
    return in.body_color;
}
"#;

pub(crate) const OVERLAY_SHADER: &str = r#"
struct Uniforms {
    viewport_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let ndc_x = (in.position.x / uniforms.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / uniforms.viewport_size.y) * 2.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

struct FragmentInput {
    @location(0) color: vec4<f32>,
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

// ========================================================================
// CandleSeries
// ========================================================================

pub struct CandleSeries {
    // Data
    candles: Vec<RenderCandle>,

    // GPU resources (created in init)
    candle_pipeline: Option<wgpu::RenderPipeline>,
    overlay_pipeline: Option<wgpu::RenderPipeline>,
    uniform_buffer: Option<wgpu::Buffer>,
    uniform_bind_group: Option<wgpu::BindGroup>,

    // Per-frame buffers (recreated/resized in prepare)
    instance_buffer: Option<wgpu::Buffer>,
    instance_count: u32,

    // Colors
    up_color: [f32; 4],
    down_color: [f32; 4],
    theme_colors: bool,
}

impl CandleSeries {
    pub fn new() -> Self {
        Self {
            candles: Vec::new(),
            candle_pipeline: None,
            overlay_pipeline: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            instance_buffer: None,
            instance_count: 0,
            up_color: [0.0, 1.0, 0.0, 1.0],   // green
            down_color: [1.0, 0.0, 0.0, 1.0], // red
            theme_colors: true,
        }
    }

    /// Override the default theme-derived candle colors.
    pub fn with_colors(
        mut self,
        up: crate::core::paint::Color,
        down: crate::core::paint::Color,
    ) -> Self {
        self.up_color = up.to_array();
        self.down_color = down.to_array();
        self.theme_colors = false;
        self
    }

    pub fn set_candles(&mut self, candles: Vec<RenderCandle>) {
        self.candles = candles;
    }

    #[cfg(test)]
    pub(crate) fn colors_for_test(&self) -> ([f32; 4], [f32; 4]) {
        (self.up_color, self.down_color)
    }

    /// Get the overlay pipeline (for external crosshair/box rendering).
    pub fn overlay_pipeline(&self) -> Option<&wgpu::RenderPipeline> {
        self.overlay_pipeline.as_ref()
    }

    /// Get the uniform bind group (for external overlay rendering).
    pub fn uniform_bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.uniform_bind_group.as_ref()
    }

    fn build_instances(&self, transform: &Transform) -> Vec<CandleInstance> {
        // Viewport culling: collect visible candles
        let visible: Vec<&RenderCandle> = self
            .candles
            .iter()
            .filter(|c| c.x_close >= transform.x_min && c.x_open <= transform.x_max)
            .collect();

        // Cap visible count to prevent buffer overflow
        // Max GPU buffer = 256MB. Wick vertices = 12 × 24 bytes = 288 bytes/candle.
        // 256MB / 288 = ~930K candles max for wicks. Use 500K as safe limit.
        const MAX_VISIBLE: usize = 500_000;

        let step = if visible.len() > MAX_VISIBLE {
            (visible.len() as f64 / MAX_VISIBLE as f64).ceil() as usize
        } else {
            1
        };

        let mut instances = Vec::with_capacity(visible.len().min(MAX_VISIBLE));

        for (i, c) in visible.iter().enumerate() {
            if step > 1 && i % step != 0 {
                continue;
            }

            let (Some(x1), Some(x2)) = (
                transform.x_to_px_checked(c.x_open),
                transform.x_to_px_checked(c.x_close),
            ) else {
                continue;
            };
            let (Some(body_top_px), Some(body_bottom_px), Some(wick_top_px), Some(wick_bottom_px)) = (
                transform.y_to_px_checked(c.body_top),
                transform.y_to_px_checked(c.body_bottom),
                transform.y_to_px_checked(c.high),
                transform.y_to_px_checked(c.low),
            ) else {
                continue;
            };

            // Ensure minimum 1px body height (doji)
            let body_h = (body_bottom_px - body_top_px).abs();
            let (by1, by2) = if body_h < 1.0 {
                let mid = (body_top_px + body_bottom_px) / 2.0;
                (mid - 0.5, mid + 0.5)
            } else {
                (body_top_px, body_bottom_px)
            };

            let base_color = if c.is_up {
                self.up_color
            } else {
                self.down_color
            };
            let color = match c.body_color_override {
                Some(ov) => [ov.r, ov.g, ov.b, ov.a],
                None => base_color,
            };

            let wick_x = (x1 + x2) / 2.0;

            instances.push(CandleInstance {
                body_rect: [x1, by1, x2, by2],
                wick_x,
                wick_top: wick_top_px,
                wick_bottom: wick_bottom_px,
                body_color: color,
                border_color: [color[0], color[1], color[2], 0.0],
                border_px: {
                    let body_w = (x2 - x1).abs();
                    if body_w < 6.0 { 0.0 } else { 2.0 }
                },
            });
        }

        instances
    }

    /// Query the Y range of visible candles for auto-scale.
    ///
    /// Returns (min_low, max_high) of candles that overlap the given
    /// X range. Returns None if no candles are visible.
    pub fn visible_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut found = false;

        for c in &self.candles {
            if c.x_close >= x_min && c.x_open <= x_max {
                lo = lo.min(c.low);
                hi = hi.max(c.high);
                found = true;
            }
        }

        if found { Some((lo, hi)) } else { None }
    }
}

impl Series for CandleSeries {
    fn init(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        use wgpu::util::DeviceExt;

        // Uniform buffer
        let uniform_data = Uniforms {
            viewport_size: [1.0, 1.0],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CandleSeries Uniforms"),
            contents: bytemuck::bytes_of(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CandleSeries BGL"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CandleSeries BG"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CandleSeries Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // Alpha blend state (shared)
        let blend_state = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let color_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(blend_state),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        // Candle body pipeline (instanced, no vertex buffer)
        let candle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Candle Body Shader"),
            source: wgpu::ShaderSource::Wgsl(CANDLE_BODY_SHADER.into()),
        });

        let candle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Candle Body Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &candle_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(CandleInstance::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &candle_shader,
                entry_point: Some("fs_main"),
                targets: &color_target,
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

        // Overlay pipeline (vertex-based, for wicks)
        let overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });

        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &overlay_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(Vertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &overlay_shader,
                entry_point: Some("fs_main"),
                targets: &color_target,
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

        self.candle_pipeline = Some(candle_pipeline);
        self.overlay_pipeline = Some(overlay_pipeline);
        self.uniform_buffer = Some(uniform_buffer);
        self.uniform_bind_group = Some(bind_group);
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform: &Transform,
        viewport: PhysicalRect,
    ) {
        use wgpu::util::DeviceExt;

        // Update uniforms
        if let Some(ref buf) = self.uniform_buffer {
            let uniforms = Uniforms {
                viewport_size: [viewport.width as f32, viewport.height as f32],
            };
            queue.write_buffer(buf, 0, bytemuck::bytes_of(&uniforms));
        }

        // Build instances
        let instances = self.build_instances(transform);
        self.instance_count = instances.len() as u32;

        if !instances.is_empty() {
            let data = bytemuck::cast_slice(&instances);
            let needed = data.len() as u64;

            match &self.instance_buffer {
                Some(buf) if buf.size() >= needed => {
                    queue.write_buffer(buf, 0, data);
                }
                _ => {
                    self.instance_buffer = Some(device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Candle Instance Buffer"),
                            contents: data,
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        },
                    ));
                }
            }
        } else {
            self.instance_buffer = None;
        }
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, _viewport: PhysicalRect) {
        // Single draw call: 18 vertices per instance (body + top wick + bottom wick)
        if let (Some(pipeline), Some(bind_group), Some(buffer)) = (
            &self.candle_pipeline,
            &self.uniform_bind_group,
            &self.instance_buffer,
        ) {
            if self.instance_count > 0 {
                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..18, 0..self.instance_count);
            }
        }
    }

    fn z_order(&self) -> u32 {
        0
    }

    fn apply_color_scheme(&mut self, scheme: &crate::plot::PlotColorScheme) {
        if self.theme_colors {
            self.up_color = scheme.candle_up.to_array();
            self.down_color = scheme.candle_down.to_array();
        }
    }

    fn hit_test(&self, x: f32, y: f32, transform: &Transform) -> Option<Box<dyn std::any::Any>> {
        let world_x = transform.px_to_x_checked(x)?;
        let world_y = transform.px_to_y_checked(y)?;

        for (i, c) in self.candles.iter().enumerate() {
            if world_x >= c.x_open && world_x <= c.x_close {
                if world_y >= c.low && world_y <= c.high {
                    return Some(Box::new(i));
                }
            }
        }

        None
    }

    fn selection_rect(&self, hit: &dyn std::any::Any, transform: &Transform) -> Option<[f32; 4]> {
        let idx = hit.downcast_ref::<usize>()?;
        let c = self.candles.get(*idx)?;

        // Check visibility
        if c.x_close < transform.x_min || c.x_open > transform.x_max {
            return None;
        }

        let x1 = transform.x_to_px_checked(c.x_open)?;
        let x2 = transform.x_to_px_checked(c.x_close)?;
        let y_top = transform.y_to_px_checked(c.high)?;
        let y_bot = transform.y_to_px_checked(c.low)?;

        Some([x1, y_top, x2, y_bot])
    }

    fn objects_in_range(
        &self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
    ) -> Vec<Box<dyn std::any::Any>> {
        let mut results: Vec<Box<dyn std::any::Any>> = Vec::new();
        for (i, c) in self.candles.iter().enumerate() {
            // Candle overlaps range if its X span intersects and its
            // high-low span intersects the Y range
            if c.x_close >= x_min && c.x_open <= x_max && c.high >= y_min && c.low <= y_max {
                results.push(Box::new(i));
            }
        }
        results
    }

    fn interaction(&self) -> super::SeriesInteraction {
        super::SeriesInteraction::Selectable
    }

    fn data_y_range(&self, x_min: f64, x_max: f64) -> Option<(f64, f64)> {
        self.visible_y_range(x_min, x_max)
    }

    fn hit_proxy(&self) -> crate::plot::hit_proxy::HitProxy {
        crate::plot::hit_proxy::HitProxy::Candle {
            candles: self.candles.clone(),
        }
    }

    fn apply_data(&mut self, data: &super::SeriesData) -> bool {
        if let super::SeriesData::Candles(candles) = data {
            self.candles = candles.clone();
            true
        } else {
            false
        }
    }

    fn update_last(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(candle) = data.downcast::<RenderCandle>() {
            if let Some(last) = self.candles.last_mut() {
                *last = *candle;
            }
        }
    }

    fn append_data(&mut self, data: Box<dyn std::any::Any>) {
        if let Ok(candle) = data.downcast::<RenderCandle>() {
            self.candles.push(*candle);
        }
    }
}
