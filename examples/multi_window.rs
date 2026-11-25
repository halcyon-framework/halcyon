//! SPEC-HAL-081B: Multi-Window End-to-End Demo
//!
//! This is a **truth test** - exercising the multi-surface infrastructure
//! with two real OS windows using MultiSurfaceContract.
//!
//! # What This Proves
//!
//! 1. Two OS windows can be created and managed

#![allow(deprecated)]
//! 2. create_surface_for_window() works (surface creation for windows)
//! 3. register_render_target() works (surface registration)
//! 4. Both windows render via compile_multi()/render_multi()
//! 5. Resize/close isolation works
//! 6. Resize settling coalesces burst events (ResizeSettlingManager)
//!
//! # MultiSurfaceContract Usage
//!
//! This demo uses `compositor.snapshot_multi()` which returns
//! `SurfaceStackSnapshot<MultiSurfaceContract>`. This permits multiple
//! base surfaces, unlike `snapshot()` which enforces single-surface.
//!
//! # Architecture Compliance
//!
//! - Window is transport-only (no semantic authority)
//! - Renderer is execution-only (no policy decisions)
//! - Compositor is surface-authority only
//! - No WindowManager abstraction
//!
//! # Run with:
//! ```
//! cargo run --release --example multi_window
//! ```

use halcyon::compositor::{Compositor, MultiSurfaceContract, SceneKey};
use halcyon::core::math::RectF;
use halcyon::core::resize_settling::ResizeSettlingManager;
use halcyon::platform::window::Window;
use halcyon::renderer::DrawPlanBuilder;
use halcyon::renderer::wgpu_renderer::WgpuRenderer;
use halcyon::winit;
use std::collections::HashMap;
use std::sync::Arc;
use winit::event::{Event as WinitEvent, WindowEvent as WinitWindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowId as WinitWindowId;

fn main() {
    println!("=== SPEC-HAL-081B: Multi-Window End-to-End Demo ===");
    println!();
    println!("This demo creates TWO OS windows rendering simultaneously.");
    println!("Both windows render via compile_multi()/render_multi().");
    println!();
    println!("Expected behavior:");
    println!("  - Window 1: 800x600");
    println!("  - Window 2: 400x300");
    println!("  - Both render simultaneously (same clear color for now)");
    println!("  - Resizing one window does NOT affect the other");
    println!("  - Closing one window does NOT crash the other");
    println!("  - Resize events are coalesced (ResizeSettlingManager)");
    println!();
    println!("Press Escape or close both windows to exit.");
    println!("================================================");

    // Create event loop
    let event_loop = EventLoop::new().unwrap();

    // Create Window 1
    let window1_obj = Window::new(&event_loop, "Window 1 (SPEC-HAL-081B)", 800, 600);
    let window1 = Arc::new(window1_obj.winit_window);
    let scale_factor1 = window1.scale_factor() as f32;
    let physical_size1 = window1.inner_size();

    // Create Window 2
    let window2_obj = Window::new(&event_loop, "Window 2 (SPEC-HAL-081B)", 400, 300);
    let window2 = Arc::new(window2_obj.winit_window);
    let scale_factor2 = window2.scale_factor() as f32;
    let physical_size2 = window2.inner_size();

    // Map winit WindowId to our window tracking
    let winit_id1 = window1.id();
    let winit_id2 = window2.id();

    // Create renderer (primary surface created here but unused for multi-window)
    let mut renderer = pollster::block_on(WgpuRenderer::new(
        window1.clone(),
        physical_size1.width,
        physical_size1.height,
    ));

    // Create wgpu surfaces for BOTH windows via create_surface_for_window
    // Note: Window 1 gets a second surface (the one from new() is unused)
    let surface1 = renderer.create_surface_for_window(window1.clone());
    let surface2 = renderer.create_surface_for_window(window2.clone());

    // Create compositor
    let viewport1 = RectF::new(
        0.0,
        0.0,
        physical_size1.width as f32 / scale_factor1,
        physical_size1.height as f32 / scale_factor1,
    );
    let mut compositor = Compositor::new(viewport1);

    // Create compositor surface for Window 1 (SceneKey = 1)
    let surface_id1 = compositor.create_surface(0, viewport1, SceneKey::new(1));

    // Create compositor surface for Window 2 (SceneKey = 2)
    let viewport2 = RectF::new(
        0.0,
        0.0,
        physical_size2.width as f32 / scale_factor2,
        physical_size2.height as f32 / scale_factor2,
    );
    let surface_id2 = compositor.create_surface(1, viewport2, SceneKey::new(2));

    // Register BOTH windows in multi_targets registry
    renderer
        .register_render_target(
            surface_id1,
            surface1,
            (physical_size1.width, physical_size1.height),
            scale_factor1,
        )
        .expect("SPEC-HAL-081B: Failed to register surface 1");
    renderer
        .register_render_target(
            surface_id2,
            surface2,
            (physical_size2.width, physical_size2.height),
            scale_factor2,
        )
        .expect("SPEC-HAL-081B: Failed to register surface 2");

    // Resize settling manager — coalesces resize events per surface
    let mut resize_manager = ResizeSettlingManager::new();
    resize_manager.register(surface_id1, (physical_size1.width, physical_size1.height));
    resize_manager.register(surface_id2, (physical_size2.width, physical_size2.height));

    // Map winit window ID to surface ID for resize routing
    let mut winit_to_surface: HashMap<WinitWindowId, halcyon::compositor::SurfaceId> =
        HashMap::new();
    winit_to_surface.insert(winit_id1, surface_id1);
    winit_to_surface.insert(winit_id2, surface_id2);

    // Track which windows are still open
    let mut window1_open = true;
    let mut window2_open = true;

    // DrawPlanBuilder for multi-surface scene construction
    let mut draw_plan_builder: DrawPlanBuilder<MultiSurfaceContract> = DrawPlanBuilder::new();

    // Resize redraw throttling: skip N-1 out of N redraw requests during resize
    // to reduce redraw pressure. Simple modulo, no timers.
    let mut resize_redraw_counter: u32 = 0;
    const RESIZE_REDRAW_DIVISOR: u32 = 3; // redraw every 3rd frame during resize

    println!();
    println!("Windows created. Starting render loop...");

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                WinitEvent::WindowEvent { window_id, event } => {
                    match event {
                        WinitWindowEvent::CloseRequested => {
                            // INV-RSZ-003: Destroy compositor surface BEFORE unregistering
                            // render target, so snapshot doesn't include closed surfaces.
                            // Guard by *_open flag to handle CloseRequested exactly once per window.
                            if window_id == winit_id1 && window1_open {
                                println!("Window 1 close requested");
                                window1_open = false;
                                resize_manager.unregister(surface_id1);
                                compositor.destroy_surface(surface_id1);
                                let _ = renderer.unregister_render_target(surface_id1);
                            } else if window_id == winit_id2 && window2_open {
                                println!("Window 2 close requested");
                                window2_open = false;
                                resize_manager.unregister(surface_id2);
                                compositor.destroy_surface(surface_id2);
                                let _ = renderer.unregister_render_target(surface_id2);
                            }

                            // Exit if both windows closed
                            if !window1_open && !window2_open {
                                println!("Both windows closed. Exiting.");
                                elwt.exit();
                            }
                        }

                        WinitWindowEvent::Resized(size) => {
                            if size.width > 0 && size.height > 0 {
                                // Route resize to appropriate surface via resize settling manager
                                if let Some(&surface_id) = winit_to_surface.get(&window_id) {
                                    let immediate = resize_manager
                                        .record_pending(surface_id, (size.width, size.height));

                                    // If threshold exceeded, apply immediately
                                    if immediate {
                                        apply_resize(
                                            surface_id,
                                            (size.width, size.height),
                                            &mut renderer,
                                            &mut compositor,
                                            &mut resize_manager,
                                        );
                                    }
                                }
                            }
                        }

                        WinitWindowEvent::KeyboardInput { event, .. } => {
                            if event.physical_key
                                == winit::keyboard::PhysicalKey::Code(
                                    winit::keyboard::KeyCode::Escape,
                                )
                            {
                                println!("Escape pressed. Exiting.");
                                elwt.exit();
                            }
                        }

                        WinitWindowEvent::RedrawRequested => {
                            // Only render if at least one window is open
                            if !window1_open && !window2_open {
                                return;
                            }

                            // Use MultiSurfaceContract snapshot for multi-window
                            let snapshot = compositor.snapshot_multi();

                            // INTENTIONAL: Empty scene with default clear_color (BLACK).
                            // This demo tests multi-surface infrastructure only, not UI rendering.
                            // Black windows are EXPECTED — see docs/MWA-AUDIT-REPORT.md § 4.
                            // To render UI: create WidgetTree + run_single_frame() per window.
                            let scene = halcyon::core::scene::SceneGraph::new();
                            // SPEC-HAL-163: Create per-surface scene map
                            let surface_scenes: HashMap<_, _> =
                                snapshot.iter().map(|s| (s.id, scene.clone())).collect();
                            let fc = renderer.frame_counter();
                            let surface_scale_factors: HashMap<_, _> = snapshot
                                .iter()
                                .map(|surface| (surface.id, renderer.scale_factor(surface.id)))
                                .collect();
                            let draw_plan = draw_plan_builder
                                .build_multi(
                                    &surface_scenes,
                                    snapshot,
                                    renderer.mesh_cache(),
                                    |surface_id| {
                                        surface_scale_factors
                                            .get(&surface_id)
                                            .copied()
                                            .unwrap_or(1.0)
                                    },
                                    fc,
                                )
                                .expect("scene must satisfy DrawPlan Build admission");

                            // Render via compile_multi/render_multi
                            match renderer.compile_multi(&draw_plan) {
                                Ok(halcyon::renderer::RendererCompileOutcome::Ready(compiled)) => {
                                    let outputs = renderer.render_multi(compiled);
                                    for output in outputs {
                                        renderer.submit_and_present(output);
                                    }
                                    renderer.advance_frame();
                                }
                                Ok(halcyon::renderer::RendererCompileOutcome::NoTargets) => {
                                    // No render needed (e.g., no registered targets)
                                }
                                Ok(halcyon::renderer::RendererCompileOutcome::Pending(identity)) => {
                                    tracing::warn!(?identity, "Compile pending; no frame encoded by this native single-frame driver");
                                }
                                Err(e) => {
                                    eprintln!("Multi-surface render error: {:?}", e);
                                }
                            }

                            // Redraw policy during resize:
                            // 1. Suppress non-resizing surfaces (they freeze)
                            // 2. Throttle resizing surface (skip N-1 of N frames)
                            if resize_manager.any_in_progress() {
                                resize_redraw_counter = resize_redraw_counter.wrapping_add(1);
                                let should_redraw =
                                    resize_redraw_counter % RESIZE_REDRAW_DIVISOR == 0;
                                if should_redraw {
                                    if window1_open && resize_manager.in_progress(surface_id1) {
                                        window1.request_redraw();
                                    }
                                    if window2_open && resize_manager.in_progress(surface_id2) {
                                        window2.request_redraw();
                                    }
                                }
                            } else {
                                resize_redraw_counter = 0;
                                if window1_open {
                                    window1.request_redraw();
                                }
                                if window2_open {
                                    window2.request_redraw();
                                }
                            }
                        }

                        _ => {}
                    }
                }

                WinitEvent::AboutToWait => {
                    // Apply any pending resizes (settling point)
                    for (surface_id, size) in resize_manager.drain_pending() {
                        apply_resize(
                            surface_id,
                            size,
                            &mut renderer,
                            &mut compositor,
                            &mut resize_manager,
                        );
                    }

                    // Redraw policy during resize:
                    // 1. Suppress non-resizing surfaces (they freeze)
                    // 2. Throttle resizing surface (skip N-1 of N frames)
                    if resize_manager.any_in_progress() {
                        resize_redraw_counter = resize_redraw_counter.wrapping_add(1);
                        let should_redraw = resize_redraw_counter % RESIZE_REDRAW_DIVISOR == 0;
                        if should_redraw {
                            if window1_open && resize_manager.in_progress(surface_id1) {
                                window1.request_redraw();
                            }
                            if window2_open && resize_manager.in_progress(surface_id2) {
                                window2.request_redraw();
                            }
                        }
                    } else {
                        // No resize active — all windows redraw normally
                        resize_redraw_counter = 0;
                        if window1_open {
                            window1.request_redraw();
                        }
                        if window2_open {
                            window2.request_redraw();
                        }
                    }
                }

                _ => {}
            }
        })
        .unwrap();
}

/// Apply a resize to renderer and compositor.
fn apply_resize(
    surface_id: halcyon::compositor::SurfaceId,
    size: (u32, u32),
    renderer: &mut WgpuRenderer,
    compositor: &mut Compositor,
    resize_manager: &mut ResizeSettlingManager,
) {
    let _ = renderer.resize_render_target(surface_id, size);
    let sf = renderer.scale_factor(surface_id);
    let logical_w = size.0 as f32 / sf;
    let logical_h = size.1 as f32 / sf;
    compositor.set_surface_bounds(surface_id, RectF::new(0.0, 0.0, logical_w, logical_h));
    resize_manager.mark_applied(surface_id);
}
