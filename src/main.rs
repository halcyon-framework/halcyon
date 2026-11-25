// SPEC-HAL-044/045: Threaded UI Architecture Demo
//
// This demonstrates the separation of:
// - UI thread: owns widget tree, layout, event routing, scene construction
// - Platform thread: owns winit event loop, GPU device, renders scenes
//
// SPEC-HAL-045: Scene handoff via triple-buffer swapchain (SceneSwapchain)

use halcyon::core::app::{UiThreadConfig, spawn_ui_thread_with_cursor_output};
use halcyon::core::event::{Modifiers, WindowId};
use halcyon::core::scene::SceneSwapchain;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::platform::window::{HalcyonEvent, convert_modifiers, convert_window_event};
use halcyon::prelude::*;
use halcyon::renderer::DrawPlanBuilder;
use halcyon::widgets::screen_host::ScreenHost;
use std::sync::Arc;
use std::sync::mpsc;
use winit::event::{Event as WinitEvent, StartCause, WindowEvent as WinitWindowEvent};
use winit::event_loop::ControlFlow;

fn main() {
    tracing_subscriber::fmt::init();

    let event_loop = create_event_loop();
    let window_obj = Window::new(
        &event_loop,
        "Halcyon UI Framework - SPEC-044 Threaded",
        1280,
        720,
    );
    let mut scale_factor = window_obj.scale_factor;
    let window = Arc::new(window_obj.winit_window);

    // SPEC-HAL-024: Pass physical size to renderer
    let physical_size = window.inner_size();
    let mut renderer = pollster::block_on(WgpuRenderer::new(
        window.clone(),
        physical_size.width,
        physical_size.height,
    ));

    // SPEC-HAL-081B: Multi-surface setup
    // Create wgpu surface, but defer registration until we receive the first frame
    // from the UI thread to get the correct surface_id.
    let mut wgpu_surface = Some(renderer.create_surface_for_window(window.clone()));
    let mut render_target_registered = false;
    let mut base_surface_id = None;

    // SPEC-HAL-084: DrawPlanBuilder owns Build phase (consumes SceneGraph, produces DrawPlan)
    let mut draw_plan_builder = DrawPlanBuilder::new();

    // ========================================================================
    // SPEC-HAL-044/045: Channel and Swapchain Setup
    // ========================================================================
    // Input channel: unbounded (platform never blocks on send)
    let (input_tx, input_rx) = mpsc::channel::<RawInputEvent>();
    // SPEC-HAL-045: Scene swapchain (triple-buffer, non-blocking)
    let scene_swapchain = SceneSwapchain::new();

    // ========================================================================
    // SPEC-HAL-044: UI Thread Configuration (Send-able)
    // ========================================================================
    let logical_size = window.inner_size();
    let initial_constraints = Constraints::loose(Size::new(
        logical_size.width as f32 / scale_factor,
        logical_size.height as f32 / scale_factor,
    ));

    let config = UiThreadConfig {
        ui_interaction_policy: Default::default(),
        theme: Theme::light(),
        text_engine: Box::new(FontManagerTextEngine::new_with_system_ui()),
        initial_constraints,
        // No initial screen - this is a minimal harness.
        // For the full demo, run: cargo run --example SPEC018_demo
        initial_screen: None,
        // SPEC-HAL-266: Pass initial DPR from OS window
        initial_scale_factor: window_obj.scale_factor,
    };

    // ========================================================================
    // SPEC-HAL-044/045: Spawn UI Thread with Builder Closure
    // SPEC-HAL-018 Stage 11: Builder now creates ScreenHost as root.
    // The actual UI content is built by MainScreen via rebuild_active_view().
    // ========================================================================
    let (_ui_thread, mut cursor_output) = spawn_ui_thread_with_cursor_output(
        config,
        || {
            // SPEC-HAL-018 Stage 11: Create ScreenHost as the tree root.
            // MainScreen will mount its content under this ScreenHost each frame.
            let mut widget_tree = WidgetTree::new();
            let screen_host = Box::new(ScreenHost::new());
            let root = widget_tree.create_root(screen_host, None, 0);

            (widget_tree, root)
        },
        input_rx,
        scene_swapchain.clone(),
    );

    // ========================================================================
    // SPEC-HAL-044/045: Platform Thread (winit event loop)
    // ========================================================================
    let mut last_cursor_pos = (0.0, 0.0);
    let mut current_modifiers = Modifiers::default();

    #[allow(deprecated)]
    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll); // Use Poll for responsiveness
            // This host has no RelativeLocked backend; ordinary cursor output is live.
            cursor_output.apply_to_window(window.as_ref(), false);

            match event {
                WinitEvent::WindowEvent {
                    event: win_event, ..
                } => {
                    // Track focus state for macOS
                    if let WinitWindowEvent::Focused(focused) = &win_event {
                        renderer.set_focused(*focused);
                    }

                    // Track modifier key state
                    if let WinitWindowEvent::ModifiersChanged(mods) = &win_event {
                        current_modifiers = convert_modifiers(&mods.state());
                    }

                    // Track cursor position for delta calculation
                    if let WinitWindowEvent::CursorMoved { position, .. } = &win_event {
                        let new_pos = (
                            position.x as f32 / scale_factor,
                            position.y as f32 / scale_factor,
                        );

                        // Convert and send to UI thread
                        for halcyon_event in convert_window_event(
                            &win_event,
                            WindowId::PRIMARY,
                            scale_factor,
                            last_cursor_pos,
                            current_modifiers,
                        ) {
                            if let HalcyonEvent::RawInput(raw) = halcyon_event {
                                let _ = input_tx.send(raw);
                            }
                        }

                        last_cursor_pos = new_pos;
                    } else {
                        for halcyon_event in convert_window_event(
                            &win_event,
                            WindowId::PRIMARY,
                            scale_factor,
                            last_cursor_pos,
                            current_modifiers,
                        ) {
                            // SPEC-HAL-044: Send raw input to UI thread
                            if let HalcyonEvent::RawInput(raw) = halcyon_event {
                                let _ = input_tx.send(raw);
                            }
                        }
                    }

                    match win_event {
                        WinitWindowEvent::CloseRequested => {
                            elwt.exit();
                        }
                        WinitWindowEvent::Resized(size) => {
                            // Resize render target if registered
                            if let Some(id) = base_surface_id {
                                let _ =
                                    renderer.resize_render_target(id, (size.width, size.height));
                            }

                            // SPEC-HAL-044: Send resize to UI thread for constraint update
                            // INV-SCALE-001: Use per-target scale_factor from registry
                            let sf = if let Some(id) = base_surface_id {
                                renderer.scale_factor(id)
                            } else {
                                scale_factor
                            };
                            let logical_w = size.width as f32 / sf;
                            let logical_h = size.height as f32 / sf;
                            let _ = input_tx.send(RawInputEvent::Resize {
                                window_id: WindowId::PRIMARY,
                                width: logical_w,
                                height: logical_h,
                            });

                            // INV-RSZ-002: Do NOT request_redraw() here. The UI thread must
                            // process the resize event and update compositor bounds before
                            // rendering. AboutToWait will trigger the next frame.
                        }
                        WinitWindowEvent::ScaleFactorChanged {
                            scale_factor: new_scale_factor,
                            ..
                        } => {
                            // INV-SCALE-001: Update per-target scale factor (clears glyph atlas)
                            if let Some(id) = base_surface_id {
                                renderer.set_scale_factor(id, new_scale_factor);
                            }

                            // Update local scale factor for coordinate conversion
                            scale_factor = new_scale_factor as f32;

                            // Recompute logical size from current physical size
                            let physical_size = window.inner_size();
                            let logical_w = physical_size.width as f32 / scale_factor;
                            let logical_h = physical_size.height as f32 / scale_factor;

                            // SPEC-HAL-266: Send scale factor change to UI thread
                            let _ = input_tx.send(RawInputEvent::ScaleFactorChanged {
                                window_id: WindowId::PRIMARY,
                                scale_factor: new_scale_factor as f32,
                            });

                            // Send resize to UI thread so layout updates
                            let _ = input_tx.send(RawInputEvent::Resize {
                                window_id: WindowId::PRIMARY,
                                width: logical_w,
                                height: logical_h,
                            });

                            // Request redraw to re-render at new scale
                            window.request_redraw();
                        }
                        WinitWindowEvent::RedrawRequested => {
                            // SPEC-HAL-045: Get latest frame from swapchain
                            if let Some(frame) = scene_swapchain.latest() {
                                // SPEC-HAL-081B: Register render target on first frame
                                // We use the surface_id from the UI thread's compositor to ensure IDs match.
                                if !render_target_registered {
                                    if let Some(first_surface) = frame.surface_stack.iter().next() {
                                        let id = first_surface.id;
                                        let physical_size = window.inner_size();
                                        renderer
                                            .register_render_target(
                                                id,
                                                wgpu_surface
                                                    .take()
                                                    .expect("wgpu_surface already consumed"),
                                                (physical_size.width, physical_size.height),
                                                scale_factor,
                                            )
                                            .expect(
                                                "SPEC-HAL-081B: Failed to register render target",
                                            );
                                        base_surface_id = Some(id);
                                        render_target_registered = true;
                                    }
                                }

                                // SPEC-HAL-084 + SPEC-HAL-163: Build phase — consume scene to produce DrawPlan
                                // SPEC-HAL-081B: Use frame's surface_stack (MultiSurfaceContract)
                                // SPEC-HAL-163: Convert single scene to per-surface HashMap
                                let mut surface_scenes = std::collections::HashMap::new();
                                for surface_desc in frame.surface_stack.iter() {
                                    surface_scenes.insert(surface_desc.id, frame.scene.clone());
                                }
                                let fc = renderer.frame_counter();
                                let surface_scale_factors: std::collections::HashMap<_, _> = frame
                                    .surface_stack
                                    .iter()
                                    .map(|desc| (desc.id, renderer.scale_factor(desc.id)))
                                    .collect();
                                let draw_plan = match draw_plan_builder.build_multi(
                                    &surface_scenes,
                                    frame.surface_stack.clone(),
                                    renderer.mesh_cache(),
                                    |surface_id| {
                                        surface_scale_factors
                                            .get(&surface_id)
                                            .copied()
                                            .unwrap_or(1.0)
                                    },
                                    fc,
                                ) {
                                    Ok(plan) => plan,
                                    Err(error) => {
                                        tracing::error!(%error, "DrawPlan Build refused");
                                        return;
                                    }
                                };
                                // SPEC-HAL-084: Compile phase (multi-target) — GPU buffer uploads
                                match renderer.compile_multi(&draw_plan) {
                                    Ok(halcyon::renderer::RendererCompileOutcome::Ready(compiled)) => {
                                        // SPEC-HAL-084: Execute phase (multi-target) — command encoding
                                        let outputs = renderer.render_multi(compiled);
                                        // Platform-layer submission (INV-RG-013) for each target
                                        for output in outputs {
                                            renderer.submit_and_present(output);
                                        }
                                        renderer.advance_frame();
                                    }
                                    Ok(halcyon::renderer::RendererCompileOutcome::NoTargets) => {
                                        // Frame skipped (no registered targets)
                                    }
                                    Ok(halcyon::renderer::RendererCompileOutcome::Pending(identity)) => {
                                        tracing::warn!(?identity, "Compile pending; no frame encoded by this native single-frame driver");
                                    }
                                    Err(e) => {
                                        tracing::error!("Multi-surface compile error: {:?}", e);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                WinitEvent::NewEvents(StartCause::Init) => {
                    // Request initial redraw immediately on startup
                    window.request_redraw();
                }
                WinitEvent::AboutToWait => {
                    // Request redraw if we might have new frames
                    window.request_redraw();
                }
                WinitEvent::LoopExiting => cursor_output.restore_window(window.as_ref()),
                _ => {}
            }
        })
        .unwrap();
}
