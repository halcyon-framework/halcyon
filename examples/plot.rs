//! SPEC-HAL-296: Plot Demo — Direct Surface Rendering
//!
//! Standalone wgpu app. Renders CandleSeries directly to the window
//! surface — no offscreen texture, no Halcyon renderer, no sampler.
//! Pixel-perfect, same architecture as the original chart renderer.
//!
//! Left drag = pan, scroll = zoom, right drag = box zoom
//! Double-click = reset to latest, Escape = exit

use halcyon::plot::series::candle::{CandleSeries, RenderCandle, Vertex};
use halcyon::plot::series::{PhysicalRect, Series};
use halcyon::plot::transform::Transform;
use halcyon::plot::view_state::ViewState;
use halcyon::winit;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

fn main() {
    println!("=== SPEC-HAL-296: Plot Demo (Direct Surface) ===");
    println!("2M candles, pixel-perfect rendering");
    println!("Left drag = pan, scroll = zoom, right drag = box zoom");
    println!("Double-click = reset to latest, Escape = exit");

    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}

/// Generate mock candle data — exact port of the original chart renderer's MockSnapshotBuilder.
fn generate_candles(count: usize, base_price: f64) -> Vec<RenderCandle> {
    let mut candles = Vec::with_capacity(count);
    let mut price = base_price;
    let interval = 60.0;

    for i in 0..count {
        let x_open = i as f64 * interval;
        let x_close = x_open + interval;

        let rand1 = ((i * 1103515245 + 12345) % 2147483648) as f64 / 2147483648.0;
        let rand2 = ((i * 134775813 + 1) % 2147483648) as f64 / 2147483648.0;
        let rand3 = ((i * 1664525 + 1013904223) % 2147483648) as f64 / 2147483648.0;
        let rand4 = ((i * 22695477 + 1) % 2147483648) as f64 / 2147483648.0;

        let volatility = 0.005 + rand4 * 0.015;
        let open = price;
        let close = price * (1.0 + (rand1 - 0.5) * volatility);
        let body_top = open.max(close);
        let body_bottom = open.min(close);

        let (high, low) = if rand2 > 0.4 {
            let body_size = (body_top - body_bottom).abs().max(price * 0.0001);
            (
                body_top + body_size * rand3 * 3.0,
                body_bottom - body_size * rand4 * 3.0,
            )
        } else {
            (body_top, body_bottom)
        };

        candles.push(RenderCandle {
            x_open,
            x_close,
            high,
            low,
            body_top,
            body_bottom,
            is_up: close >= open,
            volume: 0.0,
            body_color_override: None,
        });

        price = close;
    }

    candles
}

/// Ensure buffer has sufficient capacity, growing with 1.5x factor.
fn ensure_buffer_capacity<T: bytemuck::Pod>(
    device: &wgpu::Device,
    buffer: &mut Option<wgpu::Buffer>,
    capacity: &mut usize,
    needed: usize,
    label: &str,
    usage: wgpu::BufferUsages,
) {
    if needed > *capacity {
        let new_capacity = ((needed as f32 * 1.5).ceil() as usize).max(needed);
        let size = (new_capacity * std::mem::size_of::<T>()) as u64;
        *buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        }));
        *capacity = new_capacity;
    }
}

struct App {
    // Window + GPU
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    config: Option<wgpu::SurfaceConfiguration>,
    size: PhysicalSize<u32>,

    // Data
    series: CandleSeries,
    view_state: ViewState,
    initial_view_state: ViewState,
    data_x_min: f64,
    data_x_max: f64,

    // Overlay buffers
    crosshair_buffer: Option<wgpu::Buffer>,
    crosshair_capacity: usize,
    box_buffer: Option<wgpu::Buffer>,
    box_capacity: usize,

    // Interaction
    mouse_pos: Option<(f32, f32)>,
    is_dragging: bool,
    prev_mouse_pos: Option<(f32, f32)>,
    box_zoom_active: bool,
    box_zoom_start: Option<(f32, f32)>,
    last_click_time: Option<std::time::Instant>,
}

impl App {
    fn new() -> Self {
        let candles = generate_candles(2_000_000, 100.0);

        let data_x_min = candles.first().map(|c| c.x_open).unwrap_or(0.0);
        let data_x_max = candles.last().map(|c| c.x_close).unwrap_or(1.0);

        // Initial view: first 1000 candles
        let initial_span = 1000.0 * 60.0;
        let y_min = candles
            .iter()
            .take_while(|c| c.x_open < initial_span)
            .map(|c| c.low)
            .fold(f64::INFINITY, f64::min);
        let y_max = candles
            .iter()
            .take_while(|c| c.x_open < initial_span)
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max);

        let mut view_state = ViewState::new(0.0, initial_span, y_min, y_max);
        view_state.apply_y_padding(0.05);
        let initial_view_state = view_state.clone();

        let mut series = CandleSeries::new();
        series.set_candles(candles);

        Self {
            window: None,
            surface: None,
            device: None,
            queue: None,
            config: None,
            size: PhysicalSize::new(1280, 800),
            series,
            view_state,
            initial_view_state,
            data_x_min,
            data_x_max,
            crosshair_buffer: None,
            crosshair_capacity: 0,
            box_buffer: None,
            box_capacity: 0,
            mouse_pos: None,
            is_dragging: false,
            prev_mouse_pos: None,
            box_zoom_active: false,
            box_zoom_start: None,
            last_click_time: None,
        }
    }

    fn constrain_view_state(&mut self) {
        let span = self.view_state.x_span();
        if self.view_state.x_min < self.data_x_min {
            self.view_state.x_min = self.data_x_min;
            self.view_state.x_max = self.data_x_min + span;
        }
        if self.view_state.x_max > self.data_x_max {
            self.view_state.x_max = self.data_x_max;
            self.view_state.x_min = self.data_x_max - span;
        }
    }

    fn auto_scale_y(&mut self) {
        if let Some((lo, hi)) = self
            .series
            .visible_y_range(self.view_state.x_min, self.view_state.x_max)
        {
            self.view_state.fit_y_to_range(lo, hi, 0.05);
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            if let (Some(surface), Some(device), Some(config)) =
                (&self.surface, &self.device, &mut self.config)
            {
                config.width = new_size.width;
                config.height = new_size.height;
                surface.configure(device, config);
            }
        }
    }

    fn render_frame(&mut self) {
        let surface = self.surface.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        let w = self.size.width as f32;
        let h = self.size.height as f32;

        // Transform + prepare
        let transform = Transform::from_view_state(&self.view_state, w, h);
        let viewport = PhysicalRect {
            x: 0,
            y: 0,
            width: self.size.width,
            height: self.size.height,
        };
        self.series.prepare(device, queue, &transform, viewport);

        // Build crosshair vertices
        let crosshair_verts: Option<Vec<Vertex>> = self.mouse_pos.map(|(mx, my)| {
            let lw = 1.0_f32;
            let color = [0.8, 0.8, 0.8, 0.7];
            vec![
                // Horizontal
                Vertex {
                    position: [0.0, my - lw * 0.5],
                    color,
                },
                Vertex {
                    position: [w, my - lw * 0.5],
                    color,
                },
                Vertex {
                    position: [0.0, my + lw * 0.5],
                    color,
                },
                Vertex {
                    position: [0.0, my + lw * 0.5],
                    color,
                },
                Vertex {
                    position: [w, my - lw * 0.5],
                    color,
                },
                Vertex {
                    position: [w, my + lw * 0.5],
                    color,
                },
                // Vertical
                Vertex {
                    position: [mx - lw * 0.5, 0.0],
                    color,
                },
                Vertex {
                    position: [mx + lw * 0.5, 0.0],
                    color,
                },
                Vertex {
                    position: [mx - lw * 0.5, h],
                    color,
                },
                Vertex {
                    position: [mx - lw * 0.5, h],
                    color,
                },
                Vertex {
                    position: [mx + lw * 0.5, 0.0],
                    color,
                },
                Vertex {
                    position: [mx + lw * 0.5, h],
                    color,
                },
            ]
        });

        // Build box selection vertices
        let box_verts: Option<Vec<Vertex>> = if self.box_zoom_active {
            if let (Some(start), Some(end)) = (self.box_zoom_start, self.mouse_pos) {
                let lw = 2.0_f32;
                let color = [0.2, 0.5, 1.0, 0.5];
                let left = start.0.min(end.0);
                let right = start.0.max(end.0);
                let top = start.1.min(end.1);
                let bottom = start.1.max(end.1);

                fn hrect(
                    verts: &mut Vec<Vertex>,
                    x1: f32,
                    x2: f32,
                    y: f32,
                    lw: f32,
                    color: [f32; 4],
                ) {
                    verts.push(Vertex {
                        position: [x1, y - lw * 0.5],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x2, y - lw * 0.5],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x1, y + lw * 0.5],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x1, y + lw * 0.5],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x2, y - lw * 0.5],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x2, y + lw * 0.5],
                        color,
                    });
                }
                fn vrect(
                    verts: &mut Vec<Vertex>,
                    x: f32,
                    y1: f32,
                    y2: f32,
                    lw: f32,
                    color: [f32; 4],
                ) {
                    verts.push(Vertex {
                        position: [x - lw * 0.5, y1],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x + lw * 0.5, y1],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x - lw * 0.5, y2],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x - lw * 0.5, y2],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x + lw * 0.5, y1],
                        color,
                    });
                    verts.push(Vertex {
                        position: [x + lw * 0.5, y2],
                        color,
                    });
                }

                let mut v = Vec::with_capacity(24);
                hrect(&mut v, left, right, top, lw, color);
                hrect(&mut v, left, right, bottom, lw, color);
                vrect(&mut v, left, top, bottom, lw, color);
                vrect(&mut v, right, top, bottom, lw, color);
                Some(v)
            } else {
                None
            }
        } else {
            None
        };

        // Ensure overlay buffer capacity
        if let Some(ref verts) = crosshair_verts {
            ensure_buffer_capacity::<Vertex>(
                device,
                &mut self.crosshair_buffer,
                &mut self.crosshair_capacity,
                verts.len(),
                "Crosshair Buffer",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            );
            queue.write_buffer(
                self.crosshair_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(verts),
            );
        }
        if let Some(ref verts) = box_verts {
            ensure_buffer_capacity::<Vertex>(
                device,
                &mut self.box_buffer,
                &mut self.box_capacity,
                verts.len(),
                "Box Select Buffer",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            );
            queue.write_buffer(
                self.box_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(verts),
            );
        }

        // Update series uniforms
        // (CandleSeries::prepare already updated its own uniform buffer)

        // Get surface texture
        let output = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost => {
                self.resize(self.size);
                return;
            }
            e => {
                eprintln!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Draw candles (wicks first, then bodies — handled internally)
            self.series.render(&mut pass, viewport);

            // Draw crosshair
            if let Some(ref verts) = crosshair_verts {
                if let (Some(pipeline), Some(bind_group), Some(buf)) = (
                    self.series.overlay_pipeline(),
                    self.series.uniform_bind_group(),
                    &self.crosshair_buffer,
                ) {
                    let data_len = (verts.len() * std::mem::size_of::<Vertex>()) as u64;
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, bind_group, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..data_len));
                    pass.draw(0..verts.len() as u32, 0..1);
                }
            }

            // Draw box selection
            if let Some(ref verts) = box_verts {
                if let (Some(pipeline), Some(bind_group), Some(buf)) = (
                    self.series.overlay_pipeline(),
                    self.series.uniform_bind_group(),
                    &self.box_buffer,
                ) {
                    let data_len = (verts.len() * std::mem::size_of::<Vertex>()) as u64;
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, bind_group, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..data_len));
                    pass.draw(0..verts.len() as u32, 0..1);
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        queue.present(output);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        event_loop.set_control_flow(ControlFlow::Poll);

        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("SPEC-HAL-296: Plot Demo")
                        .with_inner_size(PhysicalSize::new(1280u32, 800u32)),
                )
                .unwrap(),
        );

        let size = window.inner_size();
        self.size = size;

        // Initialize wgpu — same as the original chart renderer
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits {
                max_buffer_size: 4_294_967_296, // 4 GB
                ..wgpu::Limits::default()
            },
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: Default::default(),
            trace: Default::default(),
        }))
        .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .format_capabilities
            .iter()
            .map(|capability| capability.format)
            .filter(|format| surface_caps.formats.contains(format))
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Initialize CandleSeries pipelines
        self.series.init(&device, &queue, surface_format);

        self.window = Some(window);
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.config = Some(config);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key
                    == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape)
                {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(physical_size) => {
                self.resize(physical_size);
            }
            WindowEvent::RedrawRequested => {
                if self.device.is_some() {
                    self.render_frame();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32;
                let y = position.y as f32;

                if self.is_dragging {
                    if let Some((prev_x, _)) = self.prev_mouse_pos {
                        let dx = x - prev_x;
                        if dx.abs() > 0.0 {
                            let prev_x_min = self.view_state.x_min;
                            let w = self.size.width as f32;
                            let world_dx = -(dx as f64 / w as f64) * self.view_state.x_span();
                            self.view_state.pan(world_dx, 0.0);
                            self.constrain_view_state();
                            if self.view_state.x_min != prev_x_min {
                                self.auto_scale_y();
                            }
                        }
                    }
                }

                self.prev_mouse_pos = Some((x, y));
                self.mouse_pos = Some((x, y));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            let now = std::time::Instant::now();
                            let is_double = self
                                .last_click_time
                                .map(|t| now.duration_since(t).as_millis() < 300)
                                .unwrap_or(false);

                            if is_double {
                                // Reset to latest candles
                                let time_span = self.initial_view_state.x_span();
                                let latest = self.data_x_max;
                                let earliest = latest - time_span;
                                self.view_state.x_min = earliest;
                                self.view_state.x_max = latest;
                                self.auto_scale_y();
                                self.last_click_time = None;
                            } else {
                                self.last_click_time = Some(now);
                                self.is_dragging = true;
                            }
                        }
                        ElementState::Released => {
                            self.is_dragging = false;
                        }
                    }
                }

                if button == MouseButton::Right {
                    match state {
                        ElementState::Pressed => {
                            self.box_zoom_active = true;
                            self.box_zoom_start = self.mouse_pos;
                        }
                        ElementState::Released => {
                            if let (Some(start), Some(end)) = (self.box_zoom_start, self.mouse_pos)
                            {
                                let bw = (end.0 - start.0).abs();
                                let bh = (end.1 - start.1).abs();
                                if bw >= 10.0 && bh >= 10.0 {
                                    let w = self.size.width as f32;
                                    let h = self.size.height as f32;
                                    let transform =
                                        Transform::from_view_state(&self.view_state, w, h);
                                    let x1 = transform.px_to_x(start.0.min(end.0));
                                    let x2 = transform.px_to_x(start.0.max(end.0));
                                    let y_top = transform.px_to_y(start.1.min(end.1));
                                    let y_bot = transform.px_to_y(start.1.max(end.1));
                                    self.view_state.x_min = x1;
                                    self.view_state.x_max = x2;
                                    self.view_state.y_min = y_bot;
                                    self.view_state.y_max = y_top;
                                    self.constrain_view_state();
                                }
                            }
                            self.box_zoom_active = false;
                            self.box_zoom_start = None;
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y / 100.0) as f32,
                };

                let w = self.size.width as f32;
                let h = self.size.height as f32;
                let (cx, cy) = self.mouse_pos.unwrap_or((w / 2.0, h / 2.0));

                let transform = Transform::from_view_state(&self.view_state, w, h);
                let center_x = transform.px_to_x(cx);
                let center_y = transform.px_to_y(cy);

                let factor = if scroll_y > 0.0 { 1.2 } else { 0.8 };
                self.view_state.zoom(factor, center_x, center_y);
                self.constrain_view_state();
                self.auto_scale_y();
            }
            _ => {}
        }

        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }
}
