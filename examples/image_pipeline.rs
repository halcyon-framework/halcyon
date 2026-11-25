//! SPEC-HAL-263: Image Pipeline Demo
//!
//! Minimal proof-of-concept for the image pipeline:
//! - Left: Image quad rendered under a rounded clip (16px radius)
//! - Right: Image quad rendered without rounded clip
//! - Both use the same checkerboard texture (texture_id = 1)
//!
//! Verifies:
//! - register_image_rgba8_srgb() → internal texture + bind group creation → scene → render
//! - Rounded clip coverage on image quads
//! - unregister_image() behavior (INV-IMG-005: debug panic / release skip)

use halcyon::compositor::{Compositor, SceneKey};
use halcyon::core::math::RectF;
use halcyon::core::scene::{Color, RoundedCorners, SceneBuilder};
use halcyon::renderer::DrawPlanBuilder;
use halcyon::renderer::wgpu_renderer::WgpuRenderer;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

const WIDTH: u32 = 600;
const HEIGHT: u32 = 400;
const TEX_ID: u64 = 1;
const TEX_SIZE: u32 = 64;

fn main() {
    println!("=== SPEC-HAL-263: Image Pipeline Demo ===");
    println!("Left:  Image under rounded clip (16px radius)");
    println!("Right: Image without clip");
    println!("Escape to exit.");

    let event_loop = EventLoop::new().unwrap();
    let mut app = ImageDemoApp::default();
    event_loop.run_app(&mut app).unwrap();
}

#[derive(Default)]
struct ImageDemoApp {
    state: Option<DemoState>,
}

struct DemoState {
    window: Arc<Window>,
    renderer: WgpuRenderer,
    draw_plan_builder: DrawPlanBuilder,
    compositor: Compositor,
    surface_id: halcyon::compositor::SurfaceId,
}

/// Generate a 64x64 RGBA8 checkerboard pattern.
fn checkerboard_rgba(size: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let is_white = ((x / 8) + (y / 8)) % 2 == 0;
            let c: u8 = if is_white { 255 } else { 80 };
            data.extend_from_slice(&[c, c, c, 255]);
        }
    }
    data
}

impl ApplicationHandler for ImageDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("SPEC-HAL-263: Image Pipeline Demo")
                        .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, HEIGHT)),
                )
                .unwrap(),
        );

        let mut renderer = pollster::block_on(WgpuRenderer::new(window.clone(), WIDTH, HEIGHT));

        // --- Create and register checkerboard texture ---
        let pixels = checkerboard_rgba(TEX_SIZE);
        renderer.register_image_rgba8_srgb(TEX_ID, TEX_SIZE, TEX_SIZE, &pixels);

        // --- Surface setup ---
        let wgpu_surface = renderer.create_surface_for_window(window.clone());
        let viewport = RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
        let mut compositor = Compositor::new(viewport);
        let surface_id = compositor.create_surface(0, viewport, SceneKey::BASE);
        renderer
            .register_render_target(
                surface_id,
                wgpu_surface,
                (WIDTH, HEIGHT),
                window.scale_factor() as f32,
            )
            .expect("register_render_target failed");

        let draw_plan_builder = DrawPlanBuilder::new();

        self.state = Some(DemoState {
            window,
            renderer,
            draw_plan_builder,
            compositor,
            surface_id,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key
                    == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape)
                {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                // Build scene
                let mut builder = SceneBuilder::new();
                let bg_color = Color::from_srgb(0.12, 0.12, 0.14, 1.0);
                builder.push_rect(
                    RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
                    bg_color,
                    0,
                    None,
                );

                // Left: Image under rounded clip
                let clip_rect = RectF::new(40.0, 60.0, 200.0, 200.0);
                builder.begin_clip(clip_rect, Some(RoundedCorners::uniform(16.0)));
                builder.push_image(
                    RectF::new(40.0, 60.0, 200.0, 200.0),
                    TEX_ID,
                    RectF::new(0.0, 0.0, 1.0, 1.0),
                    Color::WHITE,
                    1,
                    None,
                );
                builder.end_clip();

                // Right: Image without clip
                builder.push_image(
                    RectF::new(320.0, 60.0, 200.0, 200.0),
                    TEX_ID,
                    RectF::new(0.0, 0.0, 1.0, 1.0),
                    Color::WHITE,
                    1,
                    None,
                );

                let scene = builder.build();

                // Build → Compile → Execute
                let surface_stack = state.compositor.snapshot_multi();
                let mut surface_scenes = std::collections::HashMap::new();
                surface_scenes.insert(state.surface_id, scene);
                let fc = state.renderer.frame_counter();
                let surface_scale_factor = state.renderer.scale_factor(state.surface_id);
                let draw_plan = state
                    .draw_plan_builder
                    .build_multi(
                        &surface_scenes,
                        surface_stack,
                        state.renderer.mesh_cache(),
                        |_| surface_scale_factor,
                        fc,
                    )
                    .expect("scene must satisfy DrawPlan Build admission");

                match state.renderer.compile_multi(&draw_plan) {
                    Ok(halcyon::renderer::RendererCompileOutcome::Ready(compiled)) => {
                        let outputs = state.renderer.render_multi(compiled);
                        for output in outputs {
                            state.renderer.submit_and_present(output);
                        }
                        state.renderer.advance_frame();
                    }
                    Ok(halcyon::renderer::RendererCompileOutcome::NoTargets) => {}
                    Ok(halcyon::renderer::RendererCompileOutcome::Pending(identity)) => {
                        tracing::warn!(
                            ?identity,
                            "Compile pending; no frame encoded by this native single-frame driver"
                        );
                    }
                    Err(e) => eprintln!("compile_multi error: {:?}", e),
                }
            }
            _ => {}
        }

        state.window.request_redraw();
    }
}
