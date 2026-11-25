//! SPEC-HAL-161 Signal Semantics Demo
//!
//! This demo exercises **real cross-window signal semantics**, not just multi-window rendering.
//!
//! # Signal Semantics Demonstrated
//!
//! 1. **Explicit window-to-window addressing**

#![allow(deprecated)]
//!    - Child window addresses parent via `WindowId`, not implicit "current" or "parent" pointer
//!    - Source/target printed on every delivery to prove addressing works
//!
//! 2. **Cancel vs Confirm state divergence**
//!    - DialogConfirm: mutates application state (record.name updated)
//!    - DialogCancel: preserves application state (record.name unchanged)
//!    - Console shows "STATE MUTATION" vs "STATE PRESERVED"
//!
//! 3. **Typed payload routing**
//!    - `SignalPayload::DialogConfirm { value }` → update handler
//!    - `SignalPayload::DialogCancel` → preserve handler
//!    - Distinct code paths, not string matching
//!
//! 4. **Phase-correct delivery**
//!    - `deliver_signals()` called AFTER input, BEFORE layout/paint
//!    - No re-entrancy into compositor/renderer from signal handler
//!
//! # UI Rendering Note
//!
//! This demo uses **scene-only rendering** (empty scene with clear_color) rather than
//! widgets. Per-window widget UI requires surface-scoped scene partitioning, which is
//! not yet implemented. The architectural boundary is:
//!
//! - Layout is engine-global (SINGLE-LAYOUT-PER-FRAME invariant)
//! - Text requires background prepass (INV-BG-PREPASS)
//! - Per-surface text requires per-surface background prepass
//!
//! Widget-based per-window UI is deferred to a future SPEC. The signal semantics
//! demonstrated here are fully functional regardless of rendering approach.
//!
//! # Architecture Compliance
//!
//! - ❌ No blocking
//! - ❌ No global modal state
//! - ❌ No focus stealing
//! - ❌ No cross-window input routing
//! - ❌ No compositor calls from signal handler
//! - ❌ No "parent pointer" stored in child
//!
//! All coordination goes through `SignalBus`.
//!
//! # Run with:
//! ```
//! cargo run --release --example signal_dialog
//! ```

use halcyon::compositor::{Compositor, MultiSurfaceContract, SceneKey};
use halcyon::core::event::WindowId;
use halcyon::core::lifecycle::LifecycleRegistry;
use halcyon::core::math::RectF;
use halcyon::core::resize_settling::ResizeSettlingManager;
use halcyon::core::scene::SceneGraph;
use halcyon::core::signal_bus::{SignalBus, SignalEnvelope, SignalKind, SignalPayload};
use halcyon::core::theme::Theme;
use halcyon::platform::window::Window;
use halcyon::renderer::DrawPlanBuilder;
use halcyon::renderer::wgpu_renderer::WgpuRenderer;
use halcyon::winit;
use std::collections::HashMap;
use std::sync::Arc;
use winit::event::{ElementState, Event as WinitEvent, WindowEvent as WinitWindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId as WinitWindowId;

/// Application state — the record being edited
struct Record {
    name: String,
}

/// Child window state (when dialog is logically open)
struct ChildWindowState {
    /// Current text being edited
    edit_buffer: String,
    /// Parent's WindowId for sending signals
    parent_window_id: WindowId,
}

fn main() {
    println!("=== SPEC-HAL-161: Signal Dialog Demo ===");
    println!();
    println!("This demo validates cross-window signaling with a real dialog flow.");
    println!();
    println!("Parent window:");
    println!("  - Shows current record name");
    println!("  - Press SPACE to open Edit dialog");
    println!("  - Press ESCAPE to exit");
    println!();
    println!("Child dialog:");
    println!("  - Shows edit buffer");
    println!("  - Type letters A-Z to append to edit buffer");
    println!("  - Press BACKSPACE to delete");
    println!("  - Press ENTER to Save (sends DialogConfirm)");
    println!("  - Press ESCAPE to Cancel (sends DialogCancel)");
    println!();
    println!("Watch the console for signal delivery messages.");
    println!("=========================================");

    // Application state
    let mut record = Record {
        name: "Initial Value".to_string(),
    };
    println!();
    println!("[State] Record name: '{}'", record.name);

    // SPEC-HAL-161: Create signal bus for cross-window communication
    let mut signal_bus = SignalBus::new();

    // SPEC-HAL-162: Create lifecycle registry for target validation
    let mut lifecycle = LifecycleRegistry::new();

    // Create event loop
    let event_loop = EventLoop::new().unwrap();

    // Create parent window
    let parent_window_obj = Window::new(&event_loop, "Parent Window - SPEC-HAL-161", 400, 200);
    let parent_window = Arc::new(parent_window_obj.winit_window);
    let parent_winit_id = parent_window.id();
    let parent_window_id = WindowId(0);
    let scale_factor_parent = parent_window.scale_factor() as f32;
    let physical_size_parent = parent_window.inner_size();

    // SPEC-HAL-162: Register and activate parent window
    lifecycle.register_window(parent_window_id);
    lifecycle.activate_window(parent_window_id);

    // Create child window upfront (winit API limitation)
    let child_window_obj = Window::new(&event_loop, "Edit Dialog - SPEC-HAL-161", 350, 150);
    let child_window = Arc::new(child_window_obj.winit_window);
    let child_winit_id = child_window.id();
    let child_window_id = WindowId(1);
    let scale_factor_child = child_window.scale_factor() as f32;
    let physical_size_child = child_window.inner_size();

    // SPEC-HAL-162: Register and activate child window
    lifecycle.register_window(child_window_id);
    lifecycle.activate_window(child_window_id);

    // Create renderer
    let mut renderer = pollster::block_on(WgpuRenderer::new(
        parent_window.clone(),
        physical_size_parent.width,
        physical_size_parent.height,
    ));

    // Create wgpu surfaces for both windows
    let parent_wgpu_surface = renderer.create_surface_for_window(parent_window.clone());
    let child_wgpu_surface = renderer.create_surface_for_window(child_window.clone());

    // Compositor with both surfaces
    let viewport_parent = RectF::new(
        0.0,
        0.0,
        physical_size_parent.width as f32 / scale_factor_parent,
        physical_size_parent.height as f32 / scale_factor_parent,
    );
    let mut compositor = Compositor::new(viewport_parent);
    let parent_surface_id = compositor.create_surface(0, viewport_parent, SceneKey::new(1));

    let viewport_child = RectF::new(
        0.0,
        0.0,
        physical_size_child.width as f32 / scale_factor_child,
        physical_size_child.height as f32 / scale_factor_child,
    );
    let child_surface_id = compositor.create_surface(1, viewport_child, SceneKey::new(2));

    // Register render targets
    renderer
        .register_render_target(
            parent_surface_id,
            parent_wgpu_surface,
            (physical_size_parent.width, physical_size_parent.height),
            scale_factor_parent,
        )
        .expect("Failed to register parent surface");
    renderer
        .register_render_target(
            child_surface_id,
            child_wgpu_surface,
            (physical_size_child.width, physical_size_child.height),
            scale_factor_child,
        )
        .expect("Failed to register child surface");

    // Dialog state - child window always exists but is only "logically open" when this is Some
    let mut child_state: Option<ChildWindowState> = None;

    // Resize settling manager — coalesces resize events per surface
    let mut resize_manager = ResizeSettlingManager::new();
    resize_manager.register(
        parent_surface_id,
        (physical_size_parent.width, physical_size_parent.height),
    );
    resize_manager.register(
        child_surface_id,
        (physical_size_child.width, physical_size_child.height),
    );

    // Map winit window ID to surface ID for resize routing
    let mut winit_to_surface: HashMap<WinitWindowId, halcyon::compositor::SurfaceId> =
        HashMap::new();
    winit_to_surface.insert(parent_winit_id, parent_surface_id);
    winit_to_surface.insert(child_winit_id, child_surface_id);

    // DrawPlanBuilder for multi-surface scene construction
    let mut draw_plan_builder: DrawPlanBuilder<MultiSurfaceContract> = DrawPlanBuilder::new();

    // Theme for clear_color
    let theme = Theme::dark();

    // Resize redraw throttling: skip N-1 out of N redraw requests during resize
    // to reduce redraw pressure. Simple modulo, no timers.
    let mut resize_redraw_counter: u32 = 0;
    const RESIZE_REDRAW_DIVISOR: u32 = 3; // redraw every 3rd frame during resize

    println!();
    println!("[Ready] Press SPACE in parent window to open dialog...");

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                WinitEvent::WindowEvent { window_id, event } => {
                    let is_parent = window_id == parent_winit_id;
                    let is_child = window_id == child_winit_id;

                    match event {
                        WinitWindowEvent::CloseRequested => {
                            if is_parent {
                                println!("[Exit] Parent window closed.");
                                elwt.exit();
                            } else if is_child && child_state.is_some() {
                                // Child closed via chrome while dialog open → Cancel
                                println!("[Child] Closed via chrome → sending DialogCancel");
                                signal_bus.enqueue(SignalEnvelope::window_to_window(
                                    child_window_id,
                                    parent_window_id,
                                    SignalKind::Response,
                                    SignalPayload::DialogCancel,
                                ));
                                child_state = None;
                            }
                        }

                        WinitWindowEvent::Resized(size) => {
                            if size.width > 0 && size.height > 0 {
                                // Route resize to appropriate surface via resize settling manager
                                if let Some(&surface_id) = winit_to_surface.get(&window_id) {
                                    let immediate =
                                        resize_manager.record_pending(surface_id, (size.width, size.height));

                                    // If threshold exceeded, apply immediately
                                    if immediate {
                                        let sf = if is_parent { scale_factor_parent } else { scale_factor_child };
                                        let logical_w = size.width as f32 / sf;
                                        let logical_h = size.height as f32 / sf;

                                        let _ = renderer.resize_render_target(surface_id, (size.width, size.height));
                                        compositor.set_surface_bounds(
                                            surface_id,
                                            RectF::new(0.0, 0.0, logical_w, logical_h),
                                        );
                                        resize_manager.mark_applied(surface_id);
                                    }
                                }
                            }
                        }

                        WinitWindowEvent::KeyboardInput { event: key_event, .. } => {
                            if key_event.state != ElementState::Pressed {
                                return;
                            }

                            if is_parent {
                                match key_event.physical_key {
                                    PhysicalKey::Code(KeyCode::Escape) => {
                                        println!("[Exit] Escape in parent.");
                                        elwt.exit();
                                    }
                                    PhysicalKey::Code(KeyCode::Space) if child_state.is_none() => {
                                        // Open dialog
                                        println!();
                                        println!("[Parent] Opening dialog...");
                                        child_state = Some(ChildWindowState {
                                            edit_buffer: record.name.clone(),
                                            parent_window_id,
                                        });
                                        println!("[Child] Dialog opened with buffer: '{}'", record.name);
                                        println!("[Child] Type A-Z, BACKSPACE. ENTER=Save, ESC=Cancel");
                                    }
                                    _ => {}
                                }
                            } else if is_child {
                                if let Some(ref mut state) = child_state {
                                    match key_event.physical_key {
                                        PhysicalKey::Code(KeyCode::Escape) => {
                                            println!();
                                            println!("[Child] Cancel → sending DialogCancel");
                                            signal_bus.enqueue(SignalEnvelope::window_to_window(
                                                child_window_id,
                                                state.parent_window_id,
                                                SignalKind::Response,
                                                SignalPayload::DialogCancel,
                                            ));
                                        }
                                        PhysicalKey::Code(KeyCode::Enter) => {
                                            println!();
                                            println!(
                                                "[Child] Save → sending DialogConfirm with: '{}'",
                                                state.edit_buffer
                                            );
                                            signal_bus.enqueue(SignalEnvelope::window_to_window(
                                                child_window_id,
                                                state.parent_window_id,
                                                SignalKind::Response,
                                                SignalPayload::DialogConfirm {
                                                    value: Some(state.edit_buffer.clone()),
                                                },
                                            ));
                                        }
                                        PhysicalKey::Code(KeyCode::Backspace) => {
                                            if !state.edit_buffer.is_empty() {
                                                state.edit_buffer.pop();
                                                println!("[Child] Buffer: '{}'", state.edit_buffer);
                                            }
                                        }
                                        PhysicalKey::Code(code) => {
                                            let c = match code {
                                                KeyCode::KeyA => Some('a'),
                                                KeyCode::KeyB => Some('b'),
                                                KeyCode::KeyC => Some('c'),
                                                KeyCode::KeyD => Some('d'),
                                                KeyCode::KeyE => Some('e'),
                                                KeyCode::KeyF => Some('f'),
                                                KeyCode::KeyG => Some('g'),
                                                KeyCode::KeyH => Some('h'),
                                                KeyCode::KeyI => Some('i'),
                                                KeyCode::KeyJ => Some('j'),
                                                KeyCode::KeyK => Some('k'),
                                                KeyCode::KeyL => Some('l'),
                                                KeyCode::KeyM => Some('m'),
                                                KeyCode::KeyN => Some('n'),
                                                KeyCode::KeyO => Some('o'),
                                                KeyCode::KeyP => Some('p'),
                                                KeyCode::KeyQ => Some('q'),
                                                KeyCode::KeyR => Some('r'),
                                                KeyCode::KeyS => Some('s'),
                                                KeyCode::KeyT => Some('t'),
                                                KeyCode::KeyU => Some('u'),
                                                KeyCode::KeyV => Some('v'),
                                                KeyCode::KeyW => Some('w'),
                                                KeyCode::KeyX => Some('x'),
                                                KeyCode::KeyY => Some('y'),
                                                KeyCode::KeyZ => Some('z'),
                                                KeyCode::Space => Some(' '),
                                                _ => None,
                                            };
                                            if let Some(ch) = c {
                                                state.edit_buffer.push(ch);
                                                println!("[Child] Buffer: '{}'", state.edit_buffer);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        WinitWindowEvent::RedrawRequested => {
                            // ============================================================
                            // SPEC-HAL-161/162: SIGNAL DELIVERY PHASE
                            // AFTER: input processing + teardown phase
                            // BEFORE: layout + paint
                            // ============================================================
                            halcyon::core::app::deliver_signals(&mut signal_bus, &lifecycle, |envelope| {
                                // SPEC-HAL-161: Explicit addressing visible in output
                                println!();
                                println!("╔════════════════════════════════════════════════════╗");
                                println!("║  SPEC-HAL-161 SIGNAL DELIVERY                       ║");
                                println!("╠════════════════════════════════════════════════════╣");
                                println!("║  Source: {:?}", envelope.source);
                                println!("║  Target: {:?}", envelope.target);
                                println!("║  Kind:   {:?}", envelope.kind);
                                println!("╠════════════════════════════════════════════════════╣");

                                match &envelope.payload {
                                    SignalPayload::DialogConfirm { value } => {
                                        println!("║  Payload: DialogConfirm");
                                        if let Some(new_name) = value {
                                            println!("║  Value: '{}'", new_name);
                                            let old_name = record.name.clone();
                                            record.name = new_name.clone();
                                            println!("╠════════════════════════════════════════════════════╣");
                                            println!("║  STATE MUTATION: '{}' → '{}'", old_name, record.name);
                                            // Close dialog after confirm
                                            child_state = None;
                                        }
                                    }
                                    SignalPayload::DialogCancel => {
                                        println!("║  Payload: DialogCancel");
                                        println!("╠════════════════════════════════════════════════════╣");
                                        println!("║  STATE PRESERVED: '{}'", record.name);
                                        // Close dialog after cancel
                                        child_state = None;
                                    }
                                    other => {
                                        println!("║  Payload: {:?} (unhandled)", other);
                                    }
                                }

                                println!("╚════════════════════════════════════════════════════╝");
                                println!();
                                println!("[State] Record: '{}'", record.name);
                                println!("[Ready] Press SPACE to open dialog again...");
                            });

                            // ============================================================
                            // RENDER PHASE - Scene-only (no widgets)
                            //
                            // SPEC-HAL-163: Surface-scoped scene partitioning.
                            // Each surface gets its own SceneGraph with its own clear_color.
                            // This demo validates signal semantics; visual rendering is secondary.
                            // ============================================================
                            let snapshot = compositor.snapshot_multi();

                            // SPEC-HAL-163: Create per-surface scenes (INV-SCENE-001)
                            let mut surface_scenes: HashMap<
                                halcyon::compositor::SurfaceId,
                                SceneGraph,
                            > = HashMap::new();

                            // Parent surface scene
                            let mut parent_scene = SceneGraph::new();
                            parent_scene.clear_color = theme.colors.surfaces.base0.into();
                            surface_scenes.insert(parent_surface_id, parent_scene);

                            // Child surface scene
                            let mut child_scene = SceneGraph::new();
                            child_scene.clear_color = theme.colors.surfaces.base0.into();
                            surface_scenes.insert(child_surface_id, child_scene);

                            let fc = renderer.frame_counter();
                            let surface_scale_factors: HashMap<_, _> = snapshot
                                .iter()
                                .map(|surface| (surface.id, renderer.scale_factor(surface.id)))
                                .collect();
                            let draw_plan = draw_plan_builder.build_multi(
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
                            ).expect("scene must satisfy DrawPlan Build admission");

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
                                    // No render needed
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
                                let should_redraw = resize_redraw_counter % RESIZE_REDRAW_DIVISOR == 0;
                                if should_redraw {
                                    if resize_manager.in_progress(parent_surface_id) {
                                        parent_window.request_redraw();
                                    }
                                    if resize_manager.in_progress(child_surface_id) {
                                        child_window.request_redraw();
                                    }
                                }
                            } else {
                                resize_redraw_counter = 0;
                                parent_window.request_redraw();
                                child_window.request_redraw();
                            }
                        }

                        _ => {}
                    }
                }

                WinitEvent::AboutToWait => {
                    // Apply any pending resizes (settling point)
                    for (surface_id, size) in resize_manager.drain_pending() {
                        let _ = renderer.resize_render_target(surface_id, size);
                        let sf = if surface_id == parent_surface_id {
                            scale_factor_parent
                        } else {
                            scale_factor_child
                        };
                        let logical_w = size.0 as f32 / sf;
                        let logical_h = size.1 as f32 / sf;
                        compositor.set_surface_bounds(surface_id, RectF::new(0.0, 0.0, logical_w, logical_h));
                        resize_manager.mark_applied(surface_id);
                    }

                    // Redraw policy during resize:
                    // 1. Suppress non-resizing surfaces (they freeze)
                    // 2. Throttle resizing surface (skip N-1 of N frames)
                    if resize_manager.any_in_progress() {
                        resize_redraw_counter = resize_redraw_counter.wrapping_add(1);
                        let should_redraw = resize_redraw_counter % RESIZE_REDRAW_DIVISOR == 0;
                        if should_redraw {
                            if resize_manager.in_progress(parent_surface_id) {
                                parent_window.request_redraw();
                            }
                            if resize_manager.in_progress(child_surface_id) {
                                child_window.request_redraw();
                            }
                        }
                    } else {
                        // No resize active — all windows redraw normally
                        resize_redraw_counter = 0;
                        parent_window.request_redraw();
                        child_window.request_redraw();
                    }
                }

                _ => {}
            }
        })
        .unwrap();
}
