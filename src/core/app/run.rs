use super::*;

impl App {
    /// Create a new App with the given configuration
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    /// Run the application with the given initial screen.
    ///
    /// Native platforms block until the window is closed. The browser schedules
    /// asynchronous GPU initialization and transfers the same loop to its host.
    /// The screen's `build()` method will be called to construct the widget tree.
    pub fn run<S: Screen + 'static>(self, screen: S) {
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(self.run_async(screen));
        #[cfg(not(target_arch = "wasm32"))]
        pollster::block_on(self.run_async(screen));
    }

    pub(super) async fn run_async<S: Screen + 'static>(self, screen: S) {
        use crate::platform::window::{Window, create_app_event_loop};
        use std::sync::Arc;

        let event_loop = create_app_event_loop();
        #[cfg(target_os = "macos")]
        {
            use winit::event::{Event, StartCause};

            let event_loop_proxy = event_loop.create_proxy();
            let mut initial = Some((self, screen));
            let mut event_handler = None;
            #[allow(deprecated)]
            event_loop
                .run(move |event, active| {
                    if matches!(event, Event::NewEvents(StartCause::Init))
                        && let Some((app, screen)) = initial.take()
                    {
                        // Winit queues creation callbacks while it is dispatching Init.
                        let window = Window::create_hidden_on_active_loop(
                            active,
                            &app.config.title,
                            app.config.width as u32,
                            app.config.height as u32,
                            app.config.fullscreen,
                        );
                        event_handler = Some(pollster::block_on(app.create_event_handler(
                            screen,
                            Arc::new(window.winit_window),
                            event_loop_proxy.clone(),
                            window.scale_factor,
                        )));
                    }
                    let handler = event_handler
                        .as_mut()
                        .expect("macOS App must receive Init before other events");
                    handler(AppLoopEvent::Native(event), Some(active));
                })
                .unwrap();
        }
        #[cfg(not(target_os = "macos"))]
        {
            #[cfg(target_arch = "wasm32")]
            let choice_overlay = crate::platform::web::create_input_overlay();
            let window_obj = Window::with_endpoint_and_fullscreen(
                &event_loop,
                &self.config.title,
                self.config.width as u32,
                self.config.height as u32,
                None,
                self.config.fullscreen,
                false,
                #[cfg(target_arch = "wasm32")]
                choice_overlay.clone(),
            );
            #[cfg(not(target_arch = "wasm32"))]
            let platform_density = window_obj.scale_factor;
            let window = Arc::new(window_obj.winit_window);
            let event_loop_proxy = event_loop.create_proxy();
            #[cfg(target_arch = "wasm32")]
            let mut browser_loop = crate::platform::web::BrowserAppLoop::start(event_loop, window.id());
            #[cfg(target_arch = "wasm32")]
            browser_loop.wait_for_initial_size().await
                .expect("browser did not supply a primary viewport");
            #[cfg(target_arch = "wasm32")]
            let browser_window = window.clone();

            let event_handler = self.create_event_handler(
                screen,
                window,
                event_loop_proxy,
                #[cfg(not(target_arch = "wasm32"))]
                platform_density,
            ).await;
            #[cfg(target_arch = "wasm32")]
            if browser_loop.install(browser_window.as_ref(), choice_overlay, event_handler).is_err() {
                tracing::error!(target: "halcyon::core::app", "Browser input listener installation failed; App runtime not activated");
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut event_handler = event_handler;
                #[allow(deprecated)]
                event_loop.run(move |event, active| {
                    event_handler(AppLoopEvent::Native(event), Some(active));
                }).unwrap();
            }
        }
    }

    async fn create_event_handler<S: Screen + 'static>(
        mut self,
        screen: S,
        window: std::sync::Arc<winit::window::Window>,
        event_loop_proxy: winit::event_loop::EventLoopProxy<()>,
        #[cfg(not(target_arch = "wasm32"))] mut platform_density: f32,
    ) -> impl FnMut(AppLoopEvent, Option<&winit::event_loop::ActiveEventLoop>) + 'static {
        use crate::core::event::{InteractionDomainId, Modifiers, WindowId};
        use crate::core::math::Point;
        use crate::platform::window::{
            HalcyonEvent, convert_modifiers, convert_window_event, convert_window_event_at,
        };
        use crate::renderer::DrawPlanBuilder;
        use crate::renderer::wgpu_renderer::WgpuRenderer;
        use std::collections::HashMap;
        use std::sync::Arc;
        use web_time::Instant;
        use winit::event::{Event as WinitEvent, StartCause, WindowEvent as WinitWindowEvent};
        use winit::event_loop::ControlFlow;
        use winit::keyboard::{Key, NamedKey};
        use winit::window::WindowId as WinitWindowId;

        struct RuntimeChildWindow {
            window: Arc<winit::window::Window>,
            window_id: WindowId,
            parent_window_id: WindowId,
            surface_id: crate::compositor::SurfaceId,
            title: String,
            platform_density: f32,
            scale_factor: f32,
            physical_size: winit::dpi::PhysicalSize<u32>,
            edit_buffer: String,
            action_regions: Vec<ChildWindowActionRegion>,
            last_cursor_pos: Point,
            physical_cursor_pos: Option<(f64, f64)>,
            hovered_action: Option<ChildWindowAction>,
            armed_action: Option<ChildWindowAction>,
        }

        fn child_text_key(key: &Key) -> Option<char> {
            match key {
                Key::Character(text) => text
                    .chars()
                    .next()
                    .filter(|ch| ch.is_ascii_alphabetic() || *ch == ' '),
                Key::Named(NamedKey::Space) => Some(' '),
                _ => None,
            }
        }

        // Same primary-exit preparation for app actions, native Quit and primary
        // CloseRequested. Child close keeps its separate dialog operation.
        fn exit_primary_native_app(
            elwt: &winit::event_loop::ActiveEventLoop,
            window: &winit::window::Window,
            router: &mut EventRouter,
            tree: &mut WidgetTree,
            state: &mut crate::core::event::InteractionDomainState,
            fusion: &mut InputFusion,
            #[cfg(all(target_os = "macos", debug_assertions))]
            recorder: Option<&crate::platform::relative_pointer_macos_conformance::EngineRecorder>,
            _reason: &str,
        ) {
            if elwt.exiting() { return; }
            router.cancel_active_relative_with_tree(
                tree, state, RelativePointerCancellationReason::ApplicationShutdown, None,
            );
            fusion.discard_pending_relative_pointer();
            #[cfg(all(target_os = "macos", debug_assertions))]
            if let Some(recorder) = recorder { recorder.record_state(_reason, state); }
            window.set_cursor_visible(true);
            router.cursor_hidden_sticky = false;
            elwt.exit();
            tracing::info!(target: "halcyon::native_quit", exiting = elwt.exiting(), "HAL333_QUIT native_stop_called");
        }

        #[cfg(target_os = "ios")]
        let mut ios_choice = crate::platform::native_choice_ios::NativeChoiceHost::new(event_loop_proxy.clone());
        #[cfg(target_arch = "wasm32")]
        let mut platform_density = window.scale_factor() as f32;
        let mut scale_factor = crate::utils::coords::effective_scale_factor(
            platform_density,
            self.config.ui_scale.get(),
        )
        .expect("initial App UI scale cannot be represented at platform density");

        #[cfg(target_os = "ios")]
        if let Some(binding) = self.config.ios_window_binding.take() {
            binding(window.as_ref());
        }

        let physical_size = window.inner_size();
        let mut primary_physical_size = physical_size;
        let viewport_rect = startup_viewport_rect(physical_size, scale_factor);
        let mut renderer = WgpuRenderer::new(
            window.clone(),
            physical_size.width,
            physical_size.height,
        ).await;
        renderer
            .admit_font_assets(&self.config.font_assets)
            .expect("application font admission failed before Screen construction");

        // SPEC-HAL-081B: Create wgpu surface for the window and register as render target
        let wgpu_surface = renderer.create_surface_for_window(window.clone());

        // SPEC-HAL-084: DrawPlanBuilder owns Build phase (consumes SceneGraph, produces DrawPlan)
        let mut draw_plan_builder: DrawPlanBuilder<crate::compositor::MultiSurfaceContract> =
            DrawPlanBuilder::new();
        let mut recycled_primary_scene: Option<RecycledSceneStorage> = None;
        let image_wake = event_loop_proxy.clone();
        let ui_waker = UiWakeHandle {
            proxy: Arc::new(image_wake.clone()),
        };
        let admission_wake = image_wake.clone();
        renderer.set_image_admission_wake(Box::new(move || {
            let _ = admission_wake.send_event(());
        }));
        let mut retained_scene_cache = RetainedSceneCache {
            image_wake: Some(image_wake),
            ..RetainedSceneCache::default()
        };
        let mut custom_render_presentation_bindings = CustomRenderPresentationBindings::default();

        let mut theme = self.config.theme.clone();

        // Create widget tree with ScreenHost as root
        let mut widget_tree = WidgetTree::new();
        let screen_host = widget_tree.create_root(
            Box::new(crate::widgets::screen_host::ScreenHost::new()),
            None,
            0,
        );

        // Set up AppRoot with proper Screen infrastructure
        let mut gpu = renderer.gpu_resources();
        gpu.scale_factor = scale_factor;
        let mut app_context = AppContext::from_config(&self.config);
        app_context.ui_waker = Some(ui_waker);
        app_context.ui_scale = UiScaleState::initial(self.config.ui_scale);
        app_context.ui_scale_runtime = true;
        app_context.set_gpu_resources(gpu);
        app_context.frame_realization_observer = self.config.frame_realization_observer.take();
        if let Some(capacity) = self.config.flat_viewport3d_transfer_capacity {
            match crate::three_d::transfer::FlatViewport3DTransfers::new(capacity) {
                Ok(owner) => app_context.flat_viewport3d_transfers = Some(Box::new(owner)),
                Err(error) => app_context.flat_viewport3d_transfer_admission_error = Some(error),
            }
        }
        let mut frame_attempts = AppFrameAttempts::new();
        let mut required_text_faults = RequiredTextFaultGuard::default();
        let mut app_root = AppRoot::new(app_context, screen_host);

        // SPEC-HAL-333 Rev 4 Class G (INV-G1/G2): macOS projects to the native
        // main menu; other platforms keep the in-window bar host.
        match crate::platform::menu_host::resolve_menu_host_kind() {
            crate::platform::menu_host::MenuHostKind::MacOsNative => {
                #[cfg(target_os = "macos")]
                {
                    app_root.set_menu_host(
                        Box::new(crate::platform::native_menu_macos::MacOsMenuHost::new()),
                        None,
                    );
                }
                #[cfg(not(target_os = "macos"))]
                {
                    unreachable!("macOS native menu host selected on a non-macOS target");
                }
            }
            crate::platform::menu_host::MenuHostKind::InWindow => {
                use crate::widgets::menu_bar::{InWindowMenuHost, MenuBar, MenuBarMailbox};
                let mailbox = std::rc::Rc::new(std::cell::RefCell::new(MenuBarMailbox::default()));
                let bar = MenuBar::new(std::rc::Rc::clone(&mailbox));
                // z_order 100: above base content in sibling ordering,
                // below the toast overlay (200).
                let node = widget_tree.add_child(screen_host, Box::new(bar), None, 100);
                if let Some(n) = widget_tree.node_mut(screen_host) {
                    if let Some(sh) = n
                        .widget
                        .as_any_mut()
                        .downcast_mut::<crate::widgets::screen_host::ScreenHost>()
                    {
                        sh.set_menu_bar_child(Some(node));
                    }
                }
                app_root.set_menu_host(Box::new(InWindowMenuHost::new(mailbox)), Some(node));
            }
        }

        let mut initial_screen = Some(screen);
        #[cfg(target_arch = "wasm32")]
        let mut web_navigation_config = self.config.web_navigation.take();
        #[cfg(target_arch = "wasm32")]
        let bind_web_navigation = web_navigation_config.is_some();
        #[cfg(not(target_arch = "wasm32"))]
        let bind_web_navigation = false;
        if !bind_web_navigation {
            app_root.push_screen(Box::new(initial_screen.take().expect("initial Screen")));
            app_root.rebuild_active_view(&mut widget_tree);
        }

        let mut event_router = EventRouter::new();
        #[cfg(all(target_os = "macos", debug_assertions))]
        let engine_conformance = self.config.relative_pointer_engine_conformance.take();
        // SPEC-HAL-151: Interaction domains own per-window interaction state
        let mut interaction_domains = self.config.interaction_domains();
        #[cfg(target_os = "macos")]
        {
            let platform =
                crate::platform::relative_pointer_macos::MacOsRelativePointerPlatform::new(
                    WindowId::PRIMARY,
                    window.clone(),
                );
            #[cfg(debug_assertions)]
            if let Some(recorder) = engine_conformance.as_ref() {
                let platform =
                    crate::platform::relative_pointer_macos_conformance::EngineConformancePlatform::new(
                        platform,
                        window.clone(),
                        recorder.clone(),
                    );
                interaction_domains
                    .primary_state_mut()
                    .relative_pointer_authority = Some(Box::new(
                    crate::core::relative_pointer_authority::RelativePointerLeaseAuthority::new(
                        WindowId::PRIMARY,
                        InteractionDomainId::PRIMARY,
                        platform,
                    ),
                ));
            } else {
                interaction_domains
                    .primary_state_mut()
                    .relative_pointer_authority = Some(Box::new(
                    crate::core::relative_pointer_authority::RelativePointerLeaseAuthority::new(
                        WindowId::PRIMARY,
                        InteractionDomainId::PRIMARY,
                        platform,
                    ),
                ));
            }
            #[cfg(not(debug_assertions))]
            {
                interaction_domains
                    .primary_state_mut()
                    .relative_pointer_authority = Some(Box::new(
                    crate::core::relative_pointer_authority::RelativePointerLeaseAuthority::new(
                        WindowId::PRIMARY,
                        InteractionDomainId::PRIMARY,
                        platform,
                    ),
                ));
            }
        }
        // SPEC-HAL-162: Primary window/domain are alive for signal delivery in single-window mode.
        let mut lifecycle = crate::core::lifecycle::LifecycleRegistry::new();
        lifecycle.register_window(WindowId::PRIMARY);
        lifecycle.activate_window(WindowId::PRIMARY);
        lifecycle.register_domain(InteractionDomainId::PRIMARY);
        lifecycle.activate_domain(InteractionDomainId::PRIMARY);
        let mut input_fusion = crate::core::input_fusion::InputFusion::new();
        let mut ticker = EngineTicker::new(Instant::now());
        let mut frame_number: u64 = 0;
        let mut fused_events = Vec::new();
        let mut last_cursor_pos = (0.0, 0.0);
        let mut current_modifiers = Modifiers::default();
        let mut modifier_keys = crate::platform::window::ModifierKeyTracker::default();
        #[cfg(target_os = "macos")]
        let mut primary_os_focused = false;
        // SPEC-HAL-021 Stage 3: TransitionLayer owns animation timelines and policy
        // INV-TL-4: TransitionLayer is the sole animation policy owner
        let mut transition_layer = crate::core::transition::TransitionLayer::new();
        // SPEC-HAL-078 Stage 3: Engine-owned storage for active layout transitions
        // SPEC-HAL-078 FIX: Now mutable since binding happens inside build_scene
        let mut active_layout_transitions = ActiveLayoutTransitions::new();
        // SPEC-HAL-265 M8: Icon registry for paint-time icon lookup
        let icon_registry = crate::core::icons::builtin_registry();
        // SPEC-HAL-081B: Create compositor for surface-ordered rendering
        // Multi-surface contract: base surface covering viewport with scene_key=BASE
        let mut compositor = crate::compositor::Compositor::new(viewport_rect);
        let base_surface_id =
            compositor.create_surface(0, viewport_rect, crate::compositor::SceneKey::BASE);
        // SPEC-HAL-248: Main winit frame loop owns the same projection pull path as run_ui_loop.
        let shared_scene_key_allocator = SharedSceneKeyAllocator::new(SceneKey::BASE.get() + 1);
        #[cfg(target_arch = "wasm32")]
        let navigation_host = web_navigation_config.as_mut()
            .map(WebNavigation::make_host)
            .unwrap_or_else(|| NavigationHost::new(NavigationNodeId(1)));
        #[cfg(not(target_arch = "wasm32"))]
        let navigation_host = NavigationHost::new(NavigationNodeId(1));
        let nav_orchestrator = NavigationSurfaceOrchestrator::new_with_existing_root_surface(
            navigation_host,
            viewport_rect,
            base_surface_id,
            &mut compositor,
            Box::new(shared_scene_key_allocator.clone()),
        );
        #[cfg(target_arch = "wasm32")]
        let mut nav_orchestrator = if bind_web_navigation {
            nav_orchestrator.with_window_projection(base_surface_id)
        } else {
            nav_orchestrator
        };
        let overlay_domains = OverlayDomainCoordinator::new()
            .expect("primary overlay-domain coordinator identity must be available");
        overlay_domains
            .register_domain(
                &interaction_domains,
                crate::core::event::InteractionDomainId::PRIMARY,
            )
            .expect("primary interaction domain must admit frame projection");
        let focus_resolver = FocusResolver::new();
        let mut projection_bindings = FrameProjectionBindingRegistry::new(&overlay_domains);
        let screen_host_key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host);
        {
            let guard = overlay_domains
                .begin_domain_transition(crate::core::event::InteractionDomainId::PRIMARY)
                .expect("primary projection binding guard must be available");
            projection_bindings
                .install_or_replace(
                    FrameProjectionAuthorityAccess::new(
                        &interaction_domains,
                        &overlay_domains,
                        &focus_resolver,
                    ),
                    FrameProjectionWindowAccess::new(
                        WindowId::PRIMARY,
                        crate::core::event::InteractionDomainId::PRIMARY,
                        &compositor,
                        &mut widget_tree,
                    ),
                    screen_host_key,
                    FrameProjectionBinding::new(
                        crate::core::event::InteractionDomainId::PRIMARY,
                        vec![NavigationProjectionSource::standalone(
                            crate::core::event::InteractionDomainId::PRIMARY,
                            nav_orchestrator.projection_handle(),
                        )],
                    ),
                    &guard,
                )
                .expect("primary ScreenHost frame binding must install");
        }

        #[cfg(target_arch = "wasm32")]
        let mut web_navigation = web_navigation_config.map(|config| {
            let coordinator = crate::core::navigation_application::NavigationApplicationCoordinator::new(
                overlay_domains.clone(),
            );
            let participant = projection_bindings.guard_identity(screen_host_key);
            let mount = widget_tree.concrete_mount(
                widget_tree.node(screen_host).expect("primary ScreenHost").id,
            ).expect("primary ScreenHost mount");
            let navigation = super::web_navigation::AppWebNavigation::bind(
                config, &window, &mut app_root.context, mount, &coordinator,
                &mut nav_orchestrator, &mut compositor,
            );
            (navigation, coordinator, participant)
        });
        if let Some(screen) = initial_screen {
            app_root.push_screen(Box::new(screen));
            app_root.rebuild_active_view(&mut widget_tree);
        }

        // SPEC-HAL-081B: Register the render target for the base surface
        renderer
            .register_render_target(
                base_surface_id,
                wgpu_surface,
                (physical_size.width, physical_size.height),
                scale_factor as f32,
            )
            .expect("SPEC-HAL-081B: Failed to register render target");
        app_root.context.ui_scale.set_target(
            base_surface_id,
            platform_density,
            (physical_size.width, physical_size.height),
            scale_factor,
            UiScaleTargetOutput::Pending,
        );
        // SPEC-HAL-083 Phase-4: Identity-only snapshot for scene diff (no heavy data).
        let mut prev_scene_keys: Option<Vec<SceneNodeKey>> = None;

        // SPEC-HAL-083 Phase-16: Scene cache and consumers
        // Cache stores entries keyed by SceneNodeKey. Consumers process diff events.
        let mut scene_cache = SceneCache::new();
        let mut cache_consumers = {
            let mut consumers = CacheConsumers::new();
            consumers.register(Box::new(BasicCacheConsumer::new()));
            consumers
        };

        // Resize event coalescing: store pending resize, process once per frame
        // This eliminates the 100-200+ resize events/second on Linux during resize
        let mut pending_resize: Option<winit::dpi::PhysicalSize<u32>> = None;
        let mut pending_platform_density: Option<f32> = None;
        let mut last_physical_cursor_pos: Option<(f64, f64)> = None;
        let mut scale_geometry_pending = false;
        let mut deferred_scale_input = VecDeque::new();
        let mut runtime_child_windows: HashMap<WinitWindowId, RuntimeChildWindow> = HashMap::new();

        // Linux resize burst pacing: cap frame rate to 60Hz during active resize
        // to prevent compositor starvation that causes jank
        #[cfg(target_os = "linux")]
        let mut last_resize_time: Option<Instant> = None;
        #[cfg(target_os = "linux")]
        let mut in_resize_burst: bool = false;
        #[cfg(target_os = "linux")]
        let mut next_frame_tick: Option<Instant> = None;
        #[cfg(target_os = "linux")]
        const RESIZE_BURST_THRESHOLD_MS: u64 = 50; // Events <50ms apart = burst
        #[cfg(target_os = "linux")]
        const RESIZE_BURST_EXIT_MS: u64 = 150; // Exit burst after 150ms idle
        // Adaptive pacing bounds
        #[cfg(target_os = "linux")]
        const RESIZE_MIN_INTERVAL_MS: u64 = 7; // ~144Hz max
        #[cfg(target_os = "linux")]
        const RESIZE_MAX_INTERVAL_MS: u64 = 24; // ~42Hz min
        #[cfg(target_os = "linux")]
        const RESIZE_TARGET_UTILIZATION: f64 = 0.70;
        // Quantization bands to prevent interval oscillation
        #[cfg(target_os = "linux")]
        const RESIZE_INTERVAL_BANDS: [u64; 6] = [7, 8, 10, 12, 16, 24];

        // Linux-only: redraw gating to ignore compositor-injected RedrawRequested events
        // INV-LINUX-REDRAW-1: request_redraw() forbidden inside RedrawRequested on Linux;
        // all redraw scheduling must go through AboutToWait.
        // Latch: any widget/scroll/transition needs continuous animation frames (survives to AboutToWait)
        let mut animation_needed: bool = false;
        // Correlation metadata only; never participates in frame admission.
        let mut native_layout_trace_last_attempt = 0u64;

        // Single pending-render flag. Input event handlers set this; AboutToWait
        // translates it into at most one window.request_redraw() per vsync cadence.
        let mut pending_render: bool = true;
        let mut suspended_app_compile: Option<SuspendedAppCompile> = None;
        let mut primary_visible = false;
        #[cfg(target_os = "ios")]
        let mut ios_suspended = false;
        #[cfg(target_os = "ios")]
        let mut ios_restore_target = false;

        // SPEC-HAL-226A: Framework-level tooltip manager
        let mut tooltip_manager =
            crate::model::tooltip::TooltipManager::new(crate::model::tooltip::TooltipTiming {
                show_delay_ms: theme.tooltip.show_delay_ms,
                hide_delay_ms: theme.tooltip.hide_delay_ms,
                reshow_window_ms: theme.tooltip.reshow_window_ms,
            });
        let mut last_pointer_pos = Point::zero();

        #[cfg(target_os = "linux")]
        let mut linux_continuous_redraw_latch: bool = false;
        #[cfg(target_os = "linux")]
        let mut linux_redraw_pending: bool = false;
        #[cfg(target_os = "linux")]
        let mut linux_frame_cost_ema_ms: Option<f64> = None;
        #[cfg(not(target_os = "linux"))]
        let mut continuous_frame_pacer = ContinuousFramePacer::new(
            window
                .current_monitor()
                .and_then(|monitor| monitor.refresh_rate_millihertz()),
        );

        #[cfg(any(target_arch = "wasm32", test))]
        let mut browser = AppBrowserEditor::default();
        #[cfg(target_arch = "wasm32")]
        let browser_paste_wake = event_loop_proxy.clone();
        let event_handler = move |input: AppLoopEvent, active: Option<&winit::event_loop::ActiveEventLoop>| {
                #[cfg(target_arch = "wasm32")]
                let (event, browser_text) = match input {
                    AppLoopEvent::Native(event) => (event, None),
                    AppLoopEvent::BrowserNative { event, text } => (event, Some(text)),
                    AppLoopEvent::BrowserChoice(reply) => {
                        if !app_root.context.exit_requested() {
                            cancel_suspended_app_compile(&mut renderer, &mut suspended_app_compile);
                            deferred_scale_input.push_back(DeferredAppInput::HostChoice(reply));
                            pending_render = true;
                            ticker.notify_reason(FrameReason::Input);
                            window.request_redraw();
                        }
                        return;
                    }
                    AppLoopEvent::BrowserChoiceGeometry(origin) => {
                        if origin == window.id() {
                            event_router.retire_host_choice_window(WindowId::PRIMARY);
                            pending_render = true;
                            ticker.notify_reason(FrameReason::Input);
                            window.request_redraw();
                        }
                        return;
                    }
                    AppLoopEvent::BrowserText { text, event } => {
                        if app_root.context.exit_requested() || browser.session.is_none() {
                            return;
                        }
                        // A genuinely sourced browser callback can mutate the
                        // picture, just like other native ingress.
                        cancel_suspended_app_compile(&mut renderer, &mut suspended_app_compile);
                        let geometry_pending = scale_geometry_pending
                            || pending_resize.is_some()
                            || pending_platform_density.is_some();
                        let mut default_cursor_event = false;
                        AppInputDelivery {
                            app_root: &mut app_root,
                            widget_tree: &mut widget_tree,
                            event_router: &mut event_router,
                            interaction_domains: &mut interaction_domains,
                            input_fusion: &mut input_fusion,
                            text_engine: renderer.text_engine_mut(),
                            transition_layer: &mut transition_layer,
                            active_layout_transitions: &active_layout_transitions,
                            tooltip_manager: &mut tooltip_manager,
                            last_pointer_pos: &mut last_pointer_pos,
                            window: Some(window.as_ref()),
                            viewport: compositor.viewport(),
                            browser: &mut browser,
                            browser_text: Some(&text),
                        }.receive_browser_input(
                            event, &mut fused_events, &mut deferred_scale_input,
                            geometry_pending, &mut default_cursor_event,
                        );
                        apply_window_cursor(
                            window.as_ref(), &mut event_router, default_cursor_event,
                            EventRouter::relative_ingress_gate(interaction_domains.primary_state()).is_some(),
                        );
                        pending_render = true;
                        ticker.notify_reason(FrameReason::Input);
                        window.request_redraw();
                        return;
                    }
                    AppLoopEvent::ClipboardWrite { window: origin, text, from_proxy, event } => {
                        use crate::core::event_router::browser_text::BrowserTextTarget;
                        if origin != window.id() || app_root.context.exit_requested() {
                            return;
                        }
                        let Some(data) = event.clipboard_data() else { return; };
                        let proxy_source = if from_proxy {
                            let Some(session) = &browser.session else { return; };
                            Some(session.source())
                        } else {
                            None
                        };
                        // The trusted event's data expires on return. Settle any
                        // earlier keys (including selection) through the ordinary
                        // input drain before sampling the canonical editor.
                        let geometry_pending = scale_geometry_pending
                            || pending_resize.is_some()
                            || pending_platform_density.is_some();
                        cancel_suspended_app_compile(&mut renderer, &mut suspended_app_compile);
                        let mut default_cursor_event = false;
                        let mut delivery = AppInputDelivery {
                            app_root: &mut app_root,
                            widget_tree: &mut widget_tree,
                            event_router: &mut event_router,
                            interaction_domains: &mut interaction_domains,
                            input_fusion: &mut input_fusion,
                            text_engine: renderer.text_engine_mut(),
                            transition_layer: &mut transition_layer,
                            active_layout_transitions: &active_layout_transitions,
                            tooltip_manager: &mut tooltip_manager,
                            last_pointer_pos: &mut last_pointer_pos,
                            window: Some(window.as_ref()),
                            viewport: compositor.viewport(),
                            browser: &mut browser,
                            browser_text: Some(&text),
                        };
                        let drained = delivery.drain_browser_input(
                            &mut fused_events, &mut deferred_scale_input,
                            geometry_pending, &mut default_cursor_event,
                        );
                        let proxy_session = delivery.browser.session.as_ref().filter(|_| from_proxy);
                        if drained == AppInputDrain::Drained
                            && proxy_source.as_ref().is_none_or(|source| {
                                proxy_session.is_some_and(|session| session.owns_source(source))
                            })
                            && text.owns_clipboard_write_event(&event, from_proxy)
                            && let Some(root) = delivery.app_root.browser_paste_root()
                        {
                            let write_selection = |selection: &str| {
                                if data.set_data("text/plain", selection).is_err()
                                    || !data.get_data("text/plain").is_ok_and(|staged| staged == selection)
                                    || !text.owns_clipboard_write_event(&event, from_proxy)
                                {
                                    return false;
                                }
                                event.prevent_default();
                                event.default_prevented()
                            };
                            if event.type_() == "cut" {
                                if delivery.event_router.route_browser_cut(
                                    delivery.widget_tree,
                                    delivery.interaction_domains,
                                    WindowId::PRIMARY,
                                    root,
                                    proxy_session,
                                    delivery.text_engine,
                                    write_selection,
                                ) {
                                    delivery.refresh_browser_projection();
                                }
                            } else if let Some(selection) = BrowserTextTarget::copy_selection(
                                delivery.widget_tree,
                                delivery.interaction_domains,
                                WindowId::PRIMARY,
                                root,
                                proxy_session,
                            ) {
                                write_selection(selection);
                            }
                        }
                        apply_window_cursor(
                            window.as_ref(), &mut event_router, default_cursor_event,
                            EventRouter::relative_ingress_gate(interaction_domains.primary_state()).is_some(),
                        );
                        pending_render = true;
                        ticker.notify_reason(FrameReason::Input);
                        window.request_redraw();
                        return;
                    }
                    AppLoopEvent::Paste { window: origin, event } => {
                        use crate::core::event_router::browser_paste::{BrowserPasteTarget, PasteRefusal};
                        let intake = (|| {
                            if origin != window.id() { return Err(PasteRefusal::UntrustedSource); }
                            if browser.pending_paste.is_some()
                                || AppBrowserEditor::proxy_paste_pending(&deferred_scale_input)
                            {
                                return Err(PasteRefusal::Busy);
                            }
                            let root = app_root.browser_paste_root().ok_or(PasteRefusal::StaleOwner)?;
                            let target = BrowserPasteTarget::qualify(
                                &widget_tree, &interaction_domains, WindowId::PRIMARY, root,
                            )?;
                            // Still inside the genuine trusted ClipboardEvent; no widget borrow survives.
                            let text = crate::platform::web::read_paste_text(&event, target.text_policy())?;
                            let request = target.with_text(text)?;
                            if !request.is_current(&widget_tree, &interaction_domains, root) {
                                return Err(PasteRefusal::StaleOwner);
                            }
                            browser.pending_paste = Some(request);
                            if browser_paste_wake.send_event(()).is_err() {
                                browser.pending_paste = None;
                                return Err(PasteRefusal::Unavailable);
                            }
                            browser.record_proxy_paste(&mut deferred_scale_input);
                            event.prevent_default();
                            Ok(())
                        })();
                        if let Err(reason) = intake {
                            tracing::warn!(target: "halcyon::core::app", ?reason, "Browser Paste refused");
                        }
                        return;
                    }
                };
                #[cfg(not(target_arch = "wasm32"))]
                let AppLoopEvent::Native(event) = input;
                let Some(elwt) = active else { return; };
                #[cfg(target_arch = "wasm32")]
                let projected_touch_time = match &event {
                    WinitEvent::WindowEvent { window_id, event: WinitWindowEvent::Touch(touch) }
                        if *window_id == window.id() => browser_text.as_ref()
                            .and_then(|text| text.choice())
                            .and_then(|choice| choice.take_input_time(touch)),
                    _ => None,
                };
                #[cfg(not(target_arch = "wasm32"))]
                let projected_touch_time: Option<Instant> = None;
                #[cfg(target_arch = "wasm32")]
                if matches!(&event, WinitEvent::WindowEvent { window_id, event: WinitWindowEvent::Focused(false) } if *window_id == window.id()) {
                    if browser_text.as_ref().is_some_and(|text| text.has_proxy_focus())
                        && browser.session.is_some()
                    {
                        // A canvas-to-proxy transition remains inside this window.
                        return;
                    }
                    if browser_text.as_ref().and_then(|text| text.choice()).is_some_and(|choice| choice.has_focus()) {
                        return;
                    }
                    if let Some(text) = &browser_text { text.retire(); }
                    browser.session = None;
                    browser.geometry_pending |= browser.visible_size.take().is_some();
                    browser.restore_canvas_focus = false;
                    browser.pending_paste = None;
                    deferred_scale_input.retain(|input| matches!(input, DeferredAppInput::Raw(_)));
                }
                #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
                if matches!(&event, WinitEvent::WindowEvent {
                    window_id,
                    event: WinitWindowEvent::Focused(false) | WinitWindowEvent::Resized(_)
                        | WinitWindowEvent::ScaleFactorChanged { .. } | WinitWindowEvent::CloseRequested,
                } if *window_id == window.id()) {
                    event_router.retire_host_choice_window(WindowId::PRIMARY);
                    #[cfg(target_arch = "wasm32")]
                    if let Some(choice) = browser_text.as_ref().and_then(|text| text.choice()) {
                        choice.clear(&widget_tree);
                    }
                    #[cfg(target_os = "ios")]
                    ios_choice.retain(None);
                }
                // Winit may deliver queued callbacks after stop was requested.
                // A consumed (now empty) role must not admit another UI frame.
                #[cfg(target_arch = "wasm32")]
                if (elwt.exiting() || matches!(event, WinitEvent::LoopExiting))
                    && let Some((navigation, _, _)) = web_navigation.as_mut()
                {
                    navigation.retire();
                }
                #[cfg(not(target_arch = "wasm32"))]
                if matches!(event, WinitEvent::LoopExiting) {
                    cancel_suspended_app_compile(&mut renderer, &mut suspended_app_compile);
                    if let Some(mut staging) = retained_scene_cache.sdf_material_staging.take() {
                        // End retained scene custody while the staging and registrar are live.
                        drop(std::mem::take(&mut retained_scene_cache));
                        staging.retire_at_native_exit(&mut renderer);
                    }
                    return;
                }
                if elwt.exiting() { return; }
                // INV-333-QUIT-NO-RENDER-DEPENDENCY: a previously queued redraw
                // can precede the unit wake. Settle the published lifecycle role
                // at native ingress, including UserEvent, before any frame or
                // Screen flush. This is a bounded event drain, not a poll/clock.
                if app_root.take_standard_quit_request() {
                    tracing::info!(target: "halcyon::native_quit", "HAL333_QUIT native_ingress_exit");
                    exit_primary_native_app(
                        elwt,
                        window.as_ref(),
                        &mut event_router,
                        &mut widget_tree,
                        interaction_domains.primary_state_mut(),
                        &mut input_fusion,
                        #[cfg(all(target_os = "macos", debug_assertions))]
                        engine_conformance.as_ref(),
                        "application_shutdown",
                    );
                    return;
                }
                if let Some(waiting) = suspended_app_compile.as_ref() {
                    // Retire a pacing deadline while this frame waits for a JS
                    // Promise; only its one-shot event may resume the frame.
                    elwt.set_control_flow(ControlFlow::Wait);
                    let completed = matches!(&event, WinitEvent::UserEvent(()))
                        && waiting.identity.take_completion_notice();
                    if completed {
                        let mut waiting = suspended_app_compile.take().unwrap();
                        if window.inner_size() != primary_physical_size
                            || window.scale_factor() as f32 != platform_density
                        {
                            renderer.cancel_pending_preparation(&waiting.identity);
                            // The queued real resize/scale event owns new target
                            // geometry; an old completion cannot publish it.
                            return;
                        }
                        let resumed = renderer.resume_sdf_compile(&waiting.identity);
                        waiting.attempt.check_compile_outcome(
                            &resumed, primary_visible, base_surface_id,
                        );
                        match resumed {
                            Ok(crate::renderer::RendererCompileOutcome::Ready(frame)) => {
                                finish_compiled_app_attempt(
                                    &mut renderer,
                                    frame,
                                    &mut waiting.attempt,
                                    window.as_ref(),
                                    base_surface_id,
                                    &mut primary_visible,
                                    &mut required_text_faults,
                                    &mut retained_scene_cache,
                                    &mut app_root.context,
                                );
                                if waiting.attempt.should_present {
                                    ticker.mark_rendered(waiting.attempt.render_time);
                                    frame_number += 1;
                                    resize_profiler::inc_frame();
                                }
                            }
                            Ok(crate::renderer::RendererCompileOutcome::Pending(_)) => {
                                suspended_app_compile = Some(waiting);
                                return;
                            }
                            Ok(crate::renderer::RendererCompileOutcome::NoTargets) => {}
                            Err(error) => {
                                tracing::error!(target: "halcyon::core::app",
                                    ?error,
                                    "Suspended Compile failed; no frame encoded"
                                );
                            }
                        }
                        required_text_faults.emit_diagnostics();
                        if let Some(recorder) = waiting.attempt.recorder {
                            #[cfg(feature = "test-harness")]
                            let mut recorder = recorder;
                            #[cfg(feature = "test-harness")]
                            recorder.observe_submitted_hud_for_test_harness(&renderer);
                            if let Some(observer) = app_root.context.frame_realization_observer() {
                                observer.publish(recorder.finish());
                            }
                        }
                        return;
                    }
                    if matches!(
                        &event,
                        WinitEvent::AboutToWait
                            | WinitEvent::NewEvents(_)
                            | WinitEvent::WindowEvent {
                                event: WinitWindowEvent::RedrawRequested,
                                ..
                            }
                    ) {
                        // Current input is frozen while the GPU qualification is
                        // pending. A completion wake, not a pacing tick, resumes it.
                        return;
                    }
                    // Any real input/dependency/lifecycle event supersedes the
                    // frozen attempt before ordinary processing mutates state.
                    cancel_suspended_app_compile(&mut renderer, &mut suspended_app_compile);
                }
                // Default to Wait, but resize burst on Linux uses WaitUntil
                #[cfg(not(target_os = "linux"))]
                {
                    let now = Instant::now();
                    if let Some(target) = continuous_frame_pacer.deadline(now, animation_needed) {
                        // A stable deadline must survive through AboutToWait.
                        // Replacing it on each callback would bypass the wait.
                        elwt.set_control_flow(ControlFlow::WaitUntil(target));
                    } else {
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    if in_resize_burst {
                        if let Some(tick) = next_frame_tick {
                            elwt.set_control_flow(ControlFlow::WaitUntil(tick));
                        } else {
                            elwt.set_control_flow(ControlFlow::Wait);
                        }
                    } else {
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
                }

                #[cfg(target_os = "ios")]
                if ios_suspended {
                    elwt.set_control_flow(ControlFlow::Wait);
                    if matches!(
                        &event,
                        WinitEvent::AboutToWait
                            | WinitEvent::WindowEvent {
                                event: WinitWindowEvent::RedrawRequested,
                                ..
                            }
                    ) {
                        return;
                    }
                }

                let primary_inactive = {
                    let size = pending_resize.unwrap_or(primary_physical_size);
                    size.width == 0 || size.height == 0
                };
                let startup_frame = (!primary_visible || primary_inactive)
                    && matches!(&event, WinitEvent::AboutToWait)
                    && (pending_render || pending_resize.is_some());
                let primary_redraw = primary_visible
                    && matches!(
                        &event,
                        WinitEvent::WindowEvent {
                            window_id,
                            event: WinitWindowEvent::RedrawRequested,
                        } if *window_id == window.id()
                    );
                // Winit's Web request_redraw defers to a browser animation frame.
                // A trusted touch release instead enters this same App frame now,
                // so only its admitted editor Tap can open the host keyboard.
                #[cfg(target_arch = "wasm32")]
                let browser_touch_frame = if !primary_inactive
                    && let Some(text) = &browser_text
                    && let WinitEvent::WindowEvent { window_id, event: window_event @ WinitWindowEvent::Touch(touch) } = &event
                    && *window_id == window.id()
                    && touch.phase == winit::event::TouchPhase::Ended
                    && (text.trusted_touch_up(*window_id, touch.id)
                        || text.choice().is_some_and(|choice| choice.offer(*window_id, touch.id).is_some()))
                {
                    for converted in convert_window_event_at(
                        window_event, WindowId::PRIMARY, scale_factor, last_cursor_pos, current_modifiers,
                        projected_touch_time.unwrap_or_else(Instant::now),
                    ) {
                        if let HalcyonEvent::RawInput(raw) = converted {
                            if scale_geometry_pending || !deferred_scale_input.is_empty() {
                                deferred_scale_input.push_back(DeferredAppInput::Raw(raw));
                            } else {
                                let state = interaction_domains.primary_state_mut();
                                let gate = EventRouter::relative_ingress_gate(state);
                                observe_raw_geometry_allocation(
                                    &mut widget_tree, app_root.active_root_node(), &raw, scale_factor,
                                );
                                input_fusion.process_raw_with_gesture_sessions(
                                    raw, crate::core::event::InteractionDomainId::PRIMARY,
                                    &mut state.gesture_registry, &widget_tree, gate.as_ref(), &mut fused_events,
                                );
                            }
                        }
                    }
                    ticker.notify_reason(FrameReason::Input);
                    true
                } else { false };
                #[cfg(not(target_arch = "wasm32"))]
                let browser_touch_frame = false;
                match event {
                    #[cfg(target_os = "ios")]
                    WinitEvent::Suspended => {
                        event_router.retire_host_choice_window(WindowId::PRIMARY);
                        ios_choice.retain(None);
                        cancel_window_touch_input(
                            &mut app_root,
                            &mut widget_tree,
                            &mut event_router,
                            interaction_domains.primary_state_mut(),
                            &mut input_fusion,
                            WindowId::PRIMARY,
                            &mut fused_events,
                            &mut deferred_scale_input,
                        );
                        if !ios_suspended {
                            renderer
                                .unregister_render_target(base_surface_id)
                                .expect("iOS suspension must release the idle primary target");
                            ios_restore_target = true;
                            ios_suspended = true;
                            tracing::info!(target: "halcyon::core::app", frame_number, "iOS suspended; primary target released");
                        }
                        animation_needed = false;
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
                    #[cfg(target_os = "ios")]
                    WinitEvent::Resumed => {
                        ios_suspended = false;
                        pending_render = true;
                        ticker.notify_reason(FrameReason::Forced);
                        tracing::info!(target: "halcyon::core::app", frame_number, "iOS resumed; canonical frame requested");
                    }
                    _ if startup_frame || primary_redraw || browser_touch_frame => {
                        #[cfg(target_os = "ios")]
                        if ios_restore_target {
                            let size = window.inner_size();
                            if size.width == 0 || size.height == 0 {
                                pending_render = true;
                                return;
                            }
                            let surface = renderer.create_surface_for_window(window.clone());
                            renderer
                                .register_render_target(
                                    base_surface_id,
                                    surface,
                                    (size.width, size.height),
                                    scale_factor,
                                )
                                .expect("iOS resume must restore the primary render target");
                            ios_restore_target = false;
                            pending_resize = Some(size);
                            widget_tree.mark_paint_dirty(screen_host);
                            prev_scene_keys = None;
                            tracing::info!(target: "halcyon::core::app", frame_number, "iOS primary target restored");
                        }
                        pending_render = false;
                        if startup_frame {
                            ticker.notify_reason(FrameReason::Forced);
                            #[cfg(target_os = "linux")]
                            {
                                linux_redraw_pending = true;
                            }
                        }
                        #[cfg(all(target_os = "macos", debug_assertions))]
                        if let Some(recorder) = engine_conformance.as_ref() {
                            recorder.set_frame(frame_number + 1);
                        }
                        // INV-LINUX-REDRAW-2: Skip compositor-injected RedrawRequested during burst
                        #[cfg(target_os = "linux")]
                        {
                            if in_resize_burst && !linux_redraw_pending {
                                return;
                            }
                            linux_redraw_pending = false;
                        }

                        // Adaptive pacing: track frame start for duration measurement
                        #[cfg(target_os = "linux")]
                        let frame_start = Instant::now();

                        // Stage 42G: Reset continuous frames flag at start of each frame.
                        // INV-42G-3: Widgets must re-request if still needed.
                        event_router.reset_continuous_frames_request();

                        // INV-CURSOR-EPHEMERAL: Reset cursor request at start of each frame.
                        // Widgets must re-request cursor each frame if still hovering.
                        event_router.reset_cursor_request();

                        // Process coalesced resize events (once per frame)
                        // This reduces 100-200+ resize events/second to 1 per frame
                        if let Some(size) = pending_resize.take() {
                            primary_physical_size = size;
                            let resize_frame_start = Instant::now();

                            // SPEC-HAL-081B: Resize via render target registry
                            let resize_target_start = Instant::now();
                            if let Err(error) = renderer
                                .resize_render_target(base_surface_id, (size.width, size.height))
                            {
                                tracing::error!(target: "halcyon::core::app", %error, "primary target resize failed");
                                app_root.context.ui_scale.set_output(base_surface_id,
                                    UiScaleTargetOutput::Unavailable(UiScaleTargetFailure::InvalidMetrics));
                            }
                            let resize_target_us = resize_target_start.elapsed().as_micros();

                            // INV-RSZ-002: Use renderer.scale_factor() to ensure same scale
                            // factor is used for compositor bounds as compile_multi uses.
                            // INV-SCALE-001: Per-target scale factor from registry.
                            let sf = renderer.scale_factor(base_surface_id);
                            let logical_w = size.width as f32 / sf;
                            let logical_h = size.height as f32 / sf;
                            let new_bounds = RectF::new(0.0, 0.0, logical_w, logical_h);

                            let compositor_start = Instant::now();
                            compositor.set_viewport(new_bounds);
                            // INV-RSZ-002: Update compositor surface bounds so compile_multi()
                            // derives correct per-target dimensions from snapshot.
                            compositor.set_surface_bounds(base_surface_id, new_bounds);
                            let compositor_us = compositor_start.elapsed().as_micros();

                            let resize_total_us = resize_frame_start.elapsed().as_micros();
                            tracing::debug!(target: "halcyon::core::app",
                                w = size.width,
                                h = size.height,
                                resize_target_us,
                                compositor_us,
                                resize_total_us,
                                "frame: resize applied"
                            );

                        }

                        let physical_size = primary_physical_size;
                        // INV-SCALE-001: Per-target scale factor from registry.
                        let mut scale = renderer.scale_factor(base_surface_id);
                        #[cfg(target_arch = "wasm32")]
                        let mut constraints = browser_input_constraints(
                            physical_size, platform_density, scale, browser.visible_size,
                        );
                        #[cfg(not(target_arch = "wasm32"))]
                        let mut constraints = Constraints::loose(Size::new(
                            physical_size.width as f32 / scale, physical_size.height as f32 / scale,
                        ));
                        widget_tree.observe_geometry_allocation(
                            app_root.active_root_node(),
                            constraints,
                            scale_factor,
                        );

                        // SPEC-HAL-041/042: Canonical frame timing
                        let frame_time = ticker.begin_frame();

                        // SPEC-HAL-088 Stage 3: Update scroll animations BEFORE event routing
                        // SPEC-HAL-146: Must hold Update authority for visual state mutations.
                        let dt_secs = frame_time.dt.as_secs_f32();
                        {
                            let _update_authority = UpdateAuthorityGuard::enter();
                            widget_tree.update_scroll_animations(dt_secs);
                        }
                        geometry_effect_checkpoint(
                            &mut widget_tree,
                            &mut event_router,
                            interaction_domains.primary_state_mut(),
                            Some((&active_layout_transitions, &transition_layer)),
                            None,
                        );

                        // Get active root for this frame (used by geometry update, event routing, and scene building)
                        let active_root = app_root.active_root_node();

                        // SPEC-HAL-088 Stage 3: Recompute positions and viewports for hit-testing
                        // SPEC-HAL-141: Must hold Layout authority for geometry mutations.
                        {
                            let _layout_authority = LayoutAuthorityGuard::enter();
                            compute_absolute_positions(
                                &mut widget_tree,
                                active_root,
                                Point::zero(),
                            );
                            compute_absolute_paint_rects(
                                &mut widget_tree,
                                active_root,
                                Point::zero(),
                            );
                            widget_tree.compute_viewports(active_root);
                        }

                        // SPEC-HAL-086 Stage 12: Signal animation when scroll needs continuous frames
                        if widget_tree.any_scroll_needs_animation() {
                            ticker.notify_reason(FrameReason::Animation);
                        }

                        // SPEC-HAL-354: Signal update ticks for animation
                        // and update-only widgets.
                        if widget_tree.any_widget_needs_update() {
                            ticker.notify_reason(FrameReason::Animation);
                        }

                        // Latch: any widget needs continuous update ticks.
                        animation_needed = widget_tree.any_scroll_needs_animation()
                            || widget_tree.any_widget_needs_update()
                            || transition_layer.has_active_animations();

                        // SPEC-HAL-241: Update drag ghost smoothing
                        if let Some(session) = interaction_domains.primary_state_mut().drag_drop.session_mut() {
                            session.update(dt_secs);
                            ticker.notify_reason(FrameReason::Animation);
                        }
                        interaction_domains.primary_state_mut().drag_drop.cleanup_if_faded();

                        // Process input fusion
                        if !scale_geometry_pending {
                            fuse_deferred_raw_prefix(
                                &mut deferred_scale_input, &mut fused_events, &mut input_fusion,
                                &widget_tree, &mut interaction_domains,
                            );
                        }
                        input_fusion.process_frame_with_gesture_sessions(
                            &frame_time,
                            &mut ticker,
                            &mut fused_events,
                        );
                        // SPEC-HAL-101 Stage-4: Set frame time for caret blinking
                        event_router
                            .set_frame_time_ms(ticker.frame_time_ms(frame_time.now));

                        // SPEC-HAL-021 Stage 5: Advance timeline BEFORE events
                        // so that animations started during this frame's events
                        // get start_time = current now (not stale last_update_time).
                        // ANIM-INV-4: Still before layout (events → advance → layout).
                        let now_secs = ticker.timeline_time_secs(frame_time.now);
                        transition_layer.advance(now_secs);
                        trace_native_layout_transition_members(
                            "after_advance_previous_attempt", native_layout_trace_last_attempt,
                            frame_number, &widget_tree, &active_layout_transitions, &transition_layer,
                            None,
                        );
                        active_layout_transitions.cleanup_completed(&transition_layer);

                        #[cfg(target_arch = "wasm32")]
                        let previous_browser_visible_size = browser.visible_size;
                        #[cfg(target_arch = "wasm32")]
                        let had_deferred_browser_input =
                            !deferred_scale_input.is_empty() || browser.pending_paste.is_some();
                        let mut default_cursor_event = false;
                        {
                            let mut delivery = AppInputDelivery {
                                app_root: &mut app_root,
                                widget_tree: &mut widget_tree,
                                event_router: &mut event_router,
                                interaction_domains: &mut interaction_domains,
                                input_fusion: &mut input_fusion,
                                text_engine: renderer.text_engine_mut(),
                                transition_layer: &mut transition_layer,
                                active_layout_transitions: &active_layout_transitions,
                                tooltip_manager: &mut tooltip_manager,
                                last_pointer_pos: &mut last_pointer_pos,
                                window: Some(window.as_ref()),
                                viewport: compositor.viewport(),
                                #[cfg(any(target_arch = "wasm32", test))]
                                browser: &mut browser,
                                #[cfg(target_arch = "wasm32")]
                                browser_text: browser_text.as_ref(),
                                #[cfg(target_os = "ios")]
                                ios_choice: &mut ios_choice,
                                #[cfg(all(target_os = "macos", debug_assertions))]
                                engine_conformance: engine_conformance.as_ref(),
                            };
                            delivery.drain_frame_input(
                                &mut fused_events, &mut deferred_scale_input,
                                scale_geometry_pending || pending_platform_density.is_some(),
                                &mut default_cursor_event,
                            );
                        }

                        #[cfg(target_arch = "wasm32")]
                        if browser.visible_size != previous_browser_visible_size {
                            constraints = browser_input_constraints(
                                physical_size, platform_density, scale, browser.visible_size,
                            );
                            widget_tree.observe_geometry_allocation(
                                app_root.active_root_node(), constraints, scale,
                            );
                        }
                        #[cfg(target_arch = "wasm32")]
                        if had_deferred_browser_input || !deferred_scale_input.is_empty() {
                            pending_render = true;
                            ticker.notify_reason(FrameReason::Input);
                        }

                        {
                            let _update = UpdateAuthorityGuard::enter();
                            if widget_tree.advance_kinetic_scroll_for_frame(&frame_time) {
                                ticker.notify_reason(FrameReason::Animation);
                                animation_needed = true;
                            }
                        }
                        // SPEC-HAL-021: Keep engine active while timeline animations run.
                        // Must be BEFORE should_throttle — otherwise throttled frames
                        // never reach the animation_needed check, starving the animation.
                        if !fused_events.is_empty() {
                            pending_render = true;
                            ticker.notify_reason(FrameReason::Input);
                        }
                        if transition_layer.has_active_animations()
                            || retained_scene_cache.presentation.as_ref().is_some_and(|runtime| runtime.needs_reconciliation()) {
                            ticker.notify_reason(FrameReason::Animation);
                            animation_needed = true;
                        }

                        // Post-route flush: Apply pending actions from screens/widgets
                        // before build_scene. Enables same-frame application of discrete
                        // actions (e.g., column auto-size) without affecting continuous
                        // smoothing paths like drag-resize.
                        if !primary_visible {
                            app_root.flush_pending_startup_modal_actions_with_composite_editors(
                                &mut widget_tree, &mut event_router,
                                interaction_domains.primary_state_mut(), renderer.text_engine_mut(),
                            );
                        }
                        app_root.flush_pending_actions_with_composite_editors(
                            &mut widget_tree, &mut event_router,
                            interaction_domains.primary_state_mut(), renderer.text_engine_mut(),
                        );

                        let next_density = if scale_geometry_pending {
                            platform_density
                        } else {
                            pending_platform_density.take().unwrap_or(platform_density)
                        };
                        let mut scale_targets = Vec::new();
                        if app_root.context.ui_scale.pending().is_some() || next_density != platform_density {
                            scale_targets.push((base_surface_id, next_density,
                                (physical_size.width, physical_size.height)));
                            scale_targets.extend(runtime_child_windows.values().map(|child| {
                                let size = child.physical_size;
                                (child.surface_id, child.platform_density, (size.width, size.height))
                            }));
                        }
                        let current_shaping_scale = renderer.text_engine_mut().scale();
                        let prepared_scale = if scale_geometry_pending { Ok(None) } else { settle_ui_scale_for_frame(
                            &mut app_root.context,
                            &mut widget_tree,
                            &mut event_router,
                            interaction_domains.primary_state_mut(),
                            &mut input_fusion,
                            screen_host,
                            current_shaping_scale,
                            &scale_targets,
                            next_density != platform_density,
                        ) };
                        let prepared_scale = prepared_scale.and_then(|prepared| {
                            if prepared.is_none() && next_density != platform_density {
                                settle_ui_scale_for_frame(
                                    &mut app_root.context, &mut widget_tree, &mut event_router,
                                    interaction_domains.primary_state_mut(), &mut input_fusion,
                                    screen_host, current_shaping_scale, &scale_targets, true,
                                )
                            } else { Ok(prepared) }
                        });
                        let prepared_scale = match prepared_scale {
                            Ok(prepared) => prepared,
                            Err(reason) => {
                                app_root.context.ui_scale.set_output(base_surface_id,
                                    UiScaleTargetOutput::Unavailable(UiScaleTargetFailure::InvalidMetrics));
                                tracing::error!(target: "halcyon::core::app", ?reason, "App platform metrics cannot represent admitted UI scale");
                                elwt.exit();
                                return;
                            }
                        };
                        if let Some(prepared) = prepared_scale {
                            scale_geometry_pending = true;
                            platform_density = next_density;
                            for ((surface, _, physical), (_, effective, bounds)) in
                                scale_targets.iter().zip(prepared)
                            {
                                renderer.set_target_scale_factor(*surface, effective, *surface == base_surface_id);
                                compositor.set_surface_bounds(*surface, bounds);
                                let output = if physical.0 == 0 || physical.1 == 0 {
                                    UiScaleTargetOutput::Inactive
                                } else {
                                    UiScaleTargetOutput::Pending
                                };
                                app_root.context.ui_scale.set_output(*surface, output);
                                if *surface == base_surface_id {
                                    scale_factor = effective;
                                    scale = effective;
                                    let viewport = Size::new(bounds.width, bounds.height);
                                    #[cfg(target_arch = "wasm32")]
                                    let viewport = browser.visible_size.map_or(viewport, |visible| {
                                        Size::new(
                                            viewport.width.min(visible.width * platform_density / scale),
                                            viewport.height.min(visible.height * platform_density / scale),
                                        )
                                    });
                                    constraints = Constraints::loose(viewport);
                                    compositor.set_viewport(bounds);
                                } else if let Some(child) = runtime_child_windows.values_mut()
                                    .find(|child| child.surface_id == *surface)
                                {
                                    child.scale_factor = effective;
                                    if let Some((x, y)) = child.physical_cursor_pos {
                                        child.last_cursor_pos = crate::utils::coords::physical_xy_to_logical_point(
                                            x, y, effective.into(),
                                        );
                                    }
                                    child.action_regions.clear();
                                    child.hovered_action = None;
                                    child.armed_action = None;
                                    child.window.request_redraw();
                                }
                            }
                            if let Some((x, y)) = last_physical_cursor_pos {
                                last_pointer_pos = crate::utils::coords::physical_xy_to_logical_point(
                                    x, y, scale_factor.into(),
                                );
                                last_cursor_pos = (last_pointer_pos.x, last_pointer_pos.y);
                            }
                            if let Some(mut gpu) = app_root.context.gpu_resources().cloned() {
                                gpu.scale_factor = scale_factor;
                                app_root.context.set_gpu_resources(gpu);
                            }
                            ticker.notify_reason(FrameReason::Forced);
                        }

                        if physical_size.width == 0 || physical_size.height == 0 {
                            app_root.context.ui_scale.set_target(
                                base_surface_id, platform_density,
                                (physical_size.width, physical_size.height), scale_factor,
                                UiScaleTargetOutput::Inactive,
                            );
                        }

                        for request in app_root.context.take_child_window_requests() {
                            let child_logical_size = winit::dpi::LogicalSize::new(
                                request.width as f64,
                                request.height as f64,
                            );
                            let attrs = winit::window::Window::default_attributes()
                                .with_title(request.title.clone())
                                .with_inner_size(child_logical_size)
                                .with_min_inner_size(child_logical_size)
                                .with_max_inner_size(child_logical_size)
                                .with_resizable(false)
                                .with_enabled_buttons(
                                    winit::window::WindowButtons::MINIMIZE
                                        | winit::window::WindowButtons::CLOSE,
                                );
                            let child_window = match elwt.create_window(attrs) {
                                Ok(child_window) => Arc::new(child_window),
                                Err(err) => {
                                    tracing::error!(target: "halcyon::core::app",
                                        "runtime child window creation failed: {:?}",
                                        err
                                    );
                                    continue;
                                }
                            };

                            let child_winit_id = child_window.id();
                            let child_density = child_window.scale_factor() as f32;
                            let child_scale_factor = match crate::utils::coords::effective_scale_factor(
                                child_density, app_root.context.ui_scale.admitted().get(),
                            ) {
                                Ok(scale) => scale,
                                Err(reason) => {
                                    tracing::error!(target: "halcyon::core::app", ?reason, "child target cannot represent admitted App UI scale");
                                    continue;
                                }
                            };
                            let child_size = child_window.inner_size();
                            let child_viewport = RectF::new(
                                0.0,
                                0.0,
                                child_size.width as f32 / child_scale_factor,
                                child_size.height as f32 / child_scale_factor,
                            );
                            let child_surface =
                                renderer.create_surface_for_window(child_window.clone());
                            let surface_id = compositor.create_surface(
                                1,
                                child_viewport,
                                shared_scene_key_allocator.allocate_key(),
                            );
                            match renderer.register_render_target(
                                surface_id,
                                child_surface,
                                (child_size.width, child_size.height),
                                child_scale_factor,
                            ) {
                                Ok(()) => {
                                    app_root.context.ui_scale.set_target(
                                        surface_id, child_density,
                                        (child_size.width, child_size.height), child_scale_factor,
                                        UiScaleTargetOutput::Pending,
                                    );
                                    lifecycle.register_window(request.id);
                                    lifecycle.activate_window(request.id);
                                    runtime_child_windows.insert(
                                        child_winit_id,
                                        RuntimeChildWindow {
                                            window: child_window.clone(),
                                            window_id: request.id,
                                            parent_window_id: request.parent,
                                            surface_id,
                                            title: request.title,
                                            platform_density: child_density,
                                            scale_factor: child_scale_factor,
                                            physical_size: child_size,
                                            edit_buffer: request.initial_value,
                                            action_regions: Vec::new(),
                                            last_cursor_pos: Point::zero(),
                                            physical_cursor_pos: None,
                                            hovered_action: None,
                                            armed_action: None,
                                        },
                                    );
                                    app_root.context.signal_bus.enqueue(
                                        crate::core::signal_bus::SignalEnvelope::window_to_window(
                                            request.id,
                                            request.parent,
                                            crate::core::signal_bus::SignalKind::Notification,
                                            crate::core::signal_bus::SignalPayload::Custom {
                                                id: 0x4857_4F50,
                                                data: b"opened".to_vec(),
                                            },
                                        ),
                                    );
                                    child_window.request_redraw();
                                    window.request_redraw();
                                    ticker.notify_reason(FrameReason::Forced);
                                }
                                Err(err) => {
                                    tracing::error!(target: "halcyon::core::app",
                                        "runtime child render target registration failed: {:?}",
                                        err
                                    );
                                    compositor.destroy_surface(surface_id);
                                    required_text_faults.retire(surface_id);
                                }
                            }
                        }

                        // SPEC-HAL-269: Flush dismissed toasts in Phase 2
                        app_root.context.toast_manager.borrow_mut().flush_dismissed();
                        app_root.sync_toast_widget(&mut widget_tree);

                        // SPEC-HAL-161: Signal Delivery Phase.
                        // Must run after input routing and before layout/scene/paint.
                        let mut delivered_signals = Vec::new();
                        deliver_signals(
                            &mut app_root.context.signal_bus,
                            &lifecycle,
                            |envelope| delivered_signals.push(envelope),
                        );

                        let mut handled_signal = false;
                        for envelope in delivered_signals {
                            handled_signal |=
                                app_root.deliver_signal_modal_first(&envelope, &mut widget_tree);
                        }
                        if handled_signal {
                            ticker.notify_reason(FrameReason::Forced);
                        }

                        // Stage 42I: Immediate redraw after event routing when continuous frames needed.
                        // INV-42I-SMOOTH-1: During resize drag, request_redraw() immediately after
                        // routing to ensure smooth frame cadence. AboutToWait alone is too indirect
                        // during heavy event traffic.
                        // INV-LINUX-REDRAW-1: On Linux, latch intent; AboutToWait schedules.
                        if event_router.continuous_frames_requested() {
                            #[cfg(target_os = "linux")]
                            {
                                linux_continuous_redraw_latch = true;
                            }
                            #[cfg(not(target_os = "linux"))]
                            {
                                window.request_redraw();
                            }
                        }

                        // INV-CURSOR-EPHEMERAL: Apply cursor request from widgets.
                        // SPEC-HAL-268: Sticky cursor-hidden state — Hidden persists across
                        // events that don't explicitly request a cursor.
                        apply_window_cursor(
                            window.as_ref(),
                            &mut event_router,
                            default_cursor_event,
                            EventRouter::relative_ingress_gate(
                                interaction_domains.primary_state(),
                            )
                            .is_some(),
                        );

                        // Process navigation commands
                        app_root.process_nav_queue();

                        #[cfg(target_arch = "wasm32")]
                        if let Some((navigation, coordinator, participant)) = web_navigation.as_mut() {
                            if projection_bindings.guard_identity(screen_host_key) != *participant {
                                navigation.retire();
                            } else if !scale_geometry_pending
                                && !browser.geometry_pending
                                && !widget_tree.has_pending_widget_availability()
                                && fused_events.is_empty()
                                && deferred_scale_input.is_empty()
                                && navigation.process(
                                    &mut app_root.context, coordinator, &mut nav_orchestrator,
                                    &mut compositor,
                                )
                            {
                                event_router.clear_focus_with_text_engine(
                                    &mut widget_tree, interaction_domains.primary_state_mut(),
                                    Some(renderer.text_engine_mut()),
                                );
                                event_router.retire_host_choice_window(WindowId::PRIMARY);
                                if let Some(choice) = browser_text.as_ref().and_then(|text| text.choice()) {
                                    choice.clear(&widget_tree);
                                }
                                let previous_size = browser.visible_size;
                                browser.restore_canvas_focus = true;
                                browser.retire_session(browser_text.as_ref());
                                browser.pending_paste = None;
                                if previous_size.is_some() {
                                    constraints = browser_input_constraints(
                                        physical_size, platform_density, scale, None,
                                    );
                                }
                                app_root.close_all_modals();
                                app_root.screens.mark_top_rebuild();
                            }
                        }

                        // Check for exit request
                        if app_root.context.exit_requested() {
                            exit_primary_native_app(
                                elwt,
                                window.as_ref(),
                                &mut event_router,
                                &mut widget_tree,
                                interaction_domains.primary_state_mut(),
                                &mut input_fusion,
                                #[cfg(all(target_os = "macos", debug_assertions))]
                                engine_conformance.as_ref(),
                                "application_shutdown",
                            );
                            return;
                        }

                        // SPEC-HAL-248: Pull navigation projection into ScreenHost.
                        debug_assert_eq!(
                            nav_orchestrator.compositor_identity(),
                            compositor.identity()
                        );
                        let _ = pull_navigation_frame_projection(
                            FrameProjectionRuntimeAccess::new(
                                &mut projection_bindings,
                                FrameProjectionAuthorityAccess::new(
                                    &interaction_domains,
                                    &overlay_domains,
                                    &focus_resolver,
                                ),
                                &compositor,
                                FrameProjectionWindowAccess::new(
                                    WindowId::PRIMARY,
                                    crate::core::event::InteractionDomainId::PRIMARY,
                                    &compositor,
                                    &mut widget_tree,
                                ),
                            ),
                            screen_host_key,
                        );

                        // Rebuild active view
                        app_root.rebuild_active_view(&mut widget_tree);

                        // SPEC-HAL-128 Stage 5: Lifecycle cleanup for destroyed widgets
                        //
                        // SPEC-HAL-151: Single widget tree belongs to PRIMARY window.
                        // Multi-window requires per-window widget trees (not yet implemented).
                        debug_assert!(
                            interaction_domains.active_window() == Some(WindowId::PRIMARY)
                                || interaction_domains.active_window().is_none(),
                            "INV-151-PRIMARY-ONLY: Lifecycle cleanup uses primary domain; multi-window requires per-window trees"
                        );
                        let removed_widget_ids = widget_tree.drain_pending_removals();
                        if !removed_widget_ids.is_empty() {
                            for widget_id in &removed_widget_ids {
                                transition_layer.cancel_animation_owner(
                                    crate::core::animation::AnimationOwnerId::new(
                                        widget_id.0,
                                    ),
                                );
                            }
                            let domain_state = interaction_domains.primary_state_mut();
                            event_router.on_widgets_destroyed(&removed_widget_ids, domain_state);
                            #[cfg(all(target_os = "macos", debug_assertions))]
                            if let Some(recorder) = engine_conformance.as_ref() {
                                recorder.record_state("owner_unmount", domain_state);
                            }

                            // Focus recovery: if the focused widget was destroyed by rebuild,
                            // remember its position in the focus chain, rebuild the chain on the
                            // new tree, and re-focus the widget at the same index.
                            if let Some(focused_id) = domain_state.focus_manager.focused() {
                                if removed_widget_ids.contains(&focused_id) {
                                    let old_index =
                                        domain_state.focus_manager.focus_chain_index(focused_id);
                                    event_router.clear_focus(&mut widget_tree, domain_state);
                                    domain_state
                                        .focus_manager
                                        .rebuild_focus_chain(&widget_tree, active_root);
                                    let target = old_index
                                        .and_then(|idx| {
                                            domain_state.focus_manager.focus_chain_at(idx)
                                        })
                                        .or_else(|| domain_state.focus_manager.focus_chain_at(0));
                                    if let Some(target_id) = target {
                                        event_router.request_focus_for_widget(
                                            &mut widget_tree,
                                            target_id,
                                            domain_state,
                                        );
                                    }
                                }
                            }
                        }
                        let relative_facts = interaction_domains
                            .domain_for_window(WindowId::PRIMARY)
                            .map(|interaction_domain_id| RelativePointerWindowFacts {
                                window_id: WindowId::PRIMARY,
                                interaction_domain_id,
                                focused_window_id: interaction_domains.active_window(),
                                client_bounds: compositor.viewport(),
                            });
                        {
                            let domain_state = interaction_domains.primary_state_mut();
                            let _semantic_cancelled =
                                event_router.drain_relative_pointer_semantic_cancels(
                                &mut widget_tree,
                                domain_state,
                                relative_facts,
                            );
                            let gate = EventRouter::relative_ingress_gate(domain_state);
                            input_fusion.reconcile_relative_ingress(gate.as_ref());
                            #[cfg(all(target_os = "macos", debug_assertions))]
                            if _semantic_cancelled {
                                if let Some(recorder) = engine_conformance.as_ref() {
                                    recorder.record_state("semantic_cancel", domain_state);
                                }
                            }
                        }
                        // SPEC-HAL-031: Clean up signal subscriptions for removed nodes
                        let removed_node_ids = widget_tree.drain_pending_node_removals();
                        if !removed_node_ids.is_empty() {
                            app_root.context.signals.remove_subscribers(&removed_node_ids);
                        }

                        // Sync focus scopes with modal state
                        //
                        // SPEC-HAL-151: Focus scopes are primary-window-only until per-window trees exist.
                        debug_assert!(
                            interaction_domains.active_window() == Some(WindowId::PRIMARY)
                                || interaction_domains.active_window().is_none(),
                            "INV-151-PRIMARY-ONLY: Focus scope sync uses primary domain; multi-window requires per-window trees"
                        );
                        let modal_nodes = app_root.modal_mounted_nodes();
                        {
                            let domain_state = interaction_domains.primary_state_mut();
                            event_router.sync_focus_scopes(
                                &mut widget_tree,
                                &modal_nodes,
                                domain_state,
                            );
                            event_router.ensure_focus_within_active_scope(
                                &mut widget_tree,
                                domain_state,
                            );
                        }

                        // SPEC-HAL-356: Rebuild lifecycle can destroy the active cursor
                        // owner after the routed-event cursor pass. Apply that restoration
                        // immediately instead of waiting for another input event.
                        apply_window_cursor(
                            window.as_ref(),
                            &mut event_router,
                            false,
                            EventRouter::relative_ingress_gate(
                                interaction_domains.primary_state(),
                            )
                            .is_some(),
                        );

                        // SPEC-HAL-226A: Keep engine active while tooltip timer runs.
                        // notify_reason prevents throttle-skipping the frame.
                        // animation_needed drives AboutToWait → request_redraw → next frame.
                        // Both are required: one prevents skipping, the other causes delivery.
                        if tooltip_manager.needs_tick() {
                            ticker.notify_reason(FrameReason::Timer);
                            animation_needed = true;
                        }

                        // SPEC-HAL-021 / SPEC-HAL-354: Keep engine active during animations
                        // and update-only polling.
                        // Must run BEFORE should_throttle — mirrors run_ui_loop lines 2806-2814.
                        if widget_tree.any_widget_needs_update() {
                            ticker.notify_reason(FrameReason::Animation);
                            animation_needed = true;
                        }
                        if transition_layer.has_active_animations() {
                            ticker.notify_reason(FrameReason::Animation);
                            animation_needed = true;
                        }

                        // Check throttling
                        if app_root.context.ui_scale.pending().is_some() || scale_geometry_pending {
                            ticker.notify_reason(FrameReason::Forced);
                            animation_needed = true;
                            pending_render = true;
                        }
                        if should_throttle_native_frame(
                            &ticker,
                            frame_time.now,
                            &widget_tree,
                            app_root.active_root_node(),
                        ) {
                            return;
                        }

                        // Runtime theme switch: sync local theme from AppContext if changed
                        if app_root.context.take_theme_changed() {
                            theme = app_root.context.theme.clone();
                            widget_tree.mark_subtree_dirty(active_root);
                        }

                        // SPEC-HAL-031: Apply signal-driven dirty marks before layout
                        let dirty_nodes = app_root.context.signals.take_pending_dirty();
                        for node_id in dirty_nodes {
                            widget_tree.mark_layout_dirty(node_id);
                        }

                        // SPEC-HAL-190: Deferred Mutation Phase
                        // Process intents emitted during event routing before layout runs.
                        // This allows authority owners to mutate their state in response to
                        // child widget intents, with changes reflected in the upcoming layout.
                        run_deferred_mutation_phase(&mut widget_tree);

                        let composite_frame_time = event_router.frame_time_ms();
                        widget_tree.process_composite_choice_work(
                            &mut event_router, interaction_domains.primary_state_mut(),
                            renderer.text_engine_mut(), composite_frame_time,
                        );

                        if widget_tree.has_pending_widget_availability() {
                            debug_assert!(interaction_domains.active_window() == Some(WindowId::PRIMARY)
                                || interaction_domains.active_window().is_none());
                            crate::core::widget_availability::apply_pending_widget_availability(
                                &mut widget_tree,
                                &mut event_router,
                                &mut input_fusion,
                                interaction_domains.primary_state_mut(),
                                &mut app_root.context.registry.binding_registry,
                                renderer.text_engine_mut(),
                            );
                        }

                        // HAL383: all installation/deferred work is settled; Layout sees applied text.
                        widget_tree.process_text_field_handoffs(renderer.text_engine_mut(), &theme);

                        // Build and render scene
                        // SPEC-HAL-078 Stage 3: Pass active_layout_transitions for paint-time interpolation
                        // SPEC-HAL-078 FIX: transition_layer and active_layout_transitions now mutable
                        // SPEC-HAL-021 Stage 5: transition_layer.advance() ran BEFORE event routing.
                        let frame_time_ms = ticker.frame_time_ms(frame_time.now);
                        let frame_dt = frame_time.dt.as_secs_f32();
                        // Take pending focus reveal for post-layout scroll adjustment
                        let pending_focus_reveal = event_router.take_pending_focus_reveal();
                        geometry_effect_checkpoint(
                            &mut widget_tree,
                            &mut event_router,
                            interaction_domains.primary_state_mut(),
                            Some((&active_layout_transitions, &transition_layer)),
                            pending_focus_reveal,
                        );
                        // SPEC-HAL-078 Stage 0: Swap layout transition geometry buffers.
                        // begin_layout_transition_frame() clears the current-frame geometry
                        // map and moves it to previous. Without this, the map grows unboundedly
                        // (~69 entries per frame) and collect_rect_change_candidates() becomes
                        // O(total_frames × widgets), causing linear FPS decay.
                        widget_tree.begin_layout_transition_frame();

                        let build_scene_start = Instant::now();
                        let active_drag = None;
                        let active_drag_present = interaction_domains
                            .primary_state()
                            .drag_drop
                            .session()
                            .is_some();
                        let focused_widget = interaction_domains.primary_state().focus_manager.focused();
                        let keyboard_mode = interaction_domains.primary_state().keyboard_mode;
                        let any_pointer_captured = interaction_domains
                            .primary_state()
                            .pointer_capture_manager
                            .is_any_captured();
                        let tooltip_suppressed = should_suppress_tooltips(
                            &widget_tree,
                            keyboard_mode,
                            any_pointer_captured,
                            active_drag_present,
                        );

                        let visible_tooltip_target = advance_tooltip_manager(
                            &mut tooltip_manager,
                            last_pointer_pos,
                            frame_dt,
                            frame_time_ms,
                            tooltip_suppressed,
                        );
                        if tooltip_manager.needs_tick() {
                            ticker.notify_reason(FrameReason::Timer);
                            animation_needed = true;
                        }
                        let tooltip_render = TooltipSceneInput::Managed {
                            visible_target: visible_tooltip_target.as_ref(),
                            pointer: last_pointer_pos,
                            viewport: RectF::new(0.0, 0.0, constraints.max_width, constraints.max_height),
                        };

                        // SPEC-HAL-161: Production frame path must enforce the
                        // same no-signal-delivery critical section as run_single_frame().
                        let startup_ready = if !primary_visible && renderer.target_has_active_extent(base_surface_id) {
                            let ready = app_root.startup_ready().unwrap_or_else(|error| {
                                panic!("SPEC-HAL-018 §16: startup readiness failed: {error}")
                            });
                            if ready {
                                widget_tree.mark_paint_dirty(active_root);
                            }
                            ready
                        } else {
                            false
                        };
                        required_text_faults.retain_live_surfaces(|surface| {
                            compositor.surface_visibility(surface).is_some()
                        });
                        let attempt_id = frame_attempts.next();
                        let mut frame_recorder = app_root.context.frame_realization_observer()
                            .map(|_| AppFrameRecorder::new(
                                attempt_id,
                                compositor.snapshot_multi().iter().map(|surface| surface.id),
                            ));
                        let _frame_critical_section = FrameCriticalSectionGuard::enter();
                        let mut presentation_frame = custom_render_presentation_bindings.begin_frame();
                        let mut tooltip_triggers = Vec::new();
                        let custom_render_target = if renderer.target_has_active_extent(base_surface_id) {
                            Some(
                                renderer
                                    .custom_render_target_snapshot(base_surface_id)
                                    .unwrap_or_else(|error| {
                                        panic!(
                                            "SPEC-HAL-266A: base target snapshot failed: {error}"
                                        )
                                    })
                                    .with_app_frame_attempt(attempt_id),
                            )
                        } else {
                            widget_tree.withdraw_inactive_plot_surface(base_surface_id);
                            None
                        };
                        let scene_scale_factor = custom_render_target
                            .map_or(scale, CustomRenderTargetSnapshot::scale_factor);
                        let native_layout_trace_prebuild = tracing::enabled!(
                            target: "halcyon::native_layout_transition_pacer", tracing::Level::TRACE
                        ).then(|| (transition_layer.has_active_animations(), animation_needed));
                        let debug_outlines = app_root.context.debug_outlines_enabled();
                        let scene_outcome = build_canonical_scene_reusing(
                            &mut widget_tree,
                            active_root,
                            constraints,
                            &theme,
                            &mut renderer,
                            &icon_registry,
                            frame_time_ms,
                            frame_dt,
                            &mut transition_layer,
                            &mut active_layout_transitions,
                            pending_focus_reveal,
                            scene_scale_factor,
                            custom_render_target,
                            presentation_frame.preparation(),
                            Some(&mut app_root.context.signals),
                            active_drag,
                            any_pointer_captured,
                            &mut tooltip_triggers,
                            tooltip_render,
                            focused_widget,
                            keyboard_mode,
                            None,
                            !runtime_child_windows.is_empty(),
                            &mut recycled_primary_scene,
                            &mut retained_scene_cache,
                            Some(GestureSceneAccess {
                                domains: &mut interaction_domains,
                                window: WindowId::PRIMARY,
                                router: &mut event_router,
                                fusion: &mut input_fusion,
                            }),
                            Some(prepare_canonical_sdf_materials),
                            app_root.context.flat_viewport3d_transfers.as_deref_mut()
                                .map(|owner| (owner, Some(attempt_id))),
                            debug_outlines,
                        );
                        resize_profiler::add_build_scene_time_us(build_scene_start.elapsed().as_micros() as u64);
                        let primary_geometry_ready = !scene_outcome.needs_next_frame;
                        let plot_pending = scene_outcome.plot_evidence.iter()
                            .any(|plot| plot.pending_surface() == Some(base_surface_id));
                        let primary_output_ready = primary_geometry_ready
                            && !scene_outcome.material_pending && !plot_pending;
                        if plot_pending {
                            app_root.context.ui_scale.set_output(base_surface_id, UiScaleTargetOutput::Pending);
                        }
                        if !primary_geometry_ready {
                            pending_render = true;
                            if !primary_visible || primary_inactive {
                                if let Some(wake) = retained_scene_cache.image_wake.as_ref() {
                                    let _ = wake.send_event(());
                                }
                            }
                            ticker.notify_reason(FrameReason::Animation);
                            animation_needed = true;
                            if renderer.target_has_active_extent(base_surface_id) {
                                app_root.context.ui_scale.set_output(base_surface_id, UiScaleTargetOutput::Pending);
                            }
                            if runtime_child_windows.is_empty() {
                                drop(_frame_critical_section);
                                if let Some(mut recorder) = frame_recorder {
                                    recorder.unavailable(AppFrameCompileOutcome::LayoutDeferred);
                                    app_root.context.frame_realization_observer().unwrap()
                                        .publish(recorder.finish());
                                }
                                return;
                            }
                        }
                        if scene_outcome.material_pending {
                            pending_render = true;
                            window.request_redraw();
                            ticker.notify_reason(FrameReason::Forced);
                            app_root.context.ui_scale.set_output(base_surface_id, UiScaleTargetOutput::Pending);
                        }


                        // SPEC-HAL-260: Apply deferred focus + capture from layout phase.
                        if primary_geometry_ready {
                        scale_geometry_pending = false;
                        #[cfg(target_arch = "wasm32")]
                        { browser.geometry_pending = false; }
                        app_root.context.ui_scale.set_target(
                            base_surface_id, platform_density,
                            (physical_size.width, physical_size.height), scale_factor,
                            app_root.context.ui_scale.status(Some(base_surface_id)).target.unwrap().output,
                        );
                        apply_deferred_focus_requests(
                            &mut widget_tree, &mut event_router,
                            interaction_domains.primary_state_mut(), renderer.text_engine_mut(),
                        );
                        }
                        #[cfg(target_arch = "wasm32")]
                        if let Some(choice) = browser_text.as_ref().and_then(|text| text.choice()) {
                            let root = app_root.active_input_root_node();
                            let live = event_router.current_host_choice(
                                &widget_tree, root, interaction_domains.primary_state(), WindowId::PRIMARY,
                            );
                            choice.retain(live);
                            if primary_geometry_ready {
                                choice.synchronize(&widget_tree, root, compositor.viewport());
                            } else {
                                choice.clear(&widget_tree);
                            }
                        }
                        #[cfg(target_os = "ios")]
                        ios_choice.retain(event_router.current_host_choice(
                            &widget_tree, app_root.active_input_root_node(),
                            interaction_domains.primary_state(), WindowId::PRIMARY,
                        ));
                        for child in runtime_child_windows.values() {
                            let size = child.physical_size;
                            let output = if size.width == 0 || size.height == 0 {
                                UiScaleTargetOutput::Inactive
                            } else {
                                app_root.context.ui_scale.status(Some(child.surface_id)).target
                                    .map_or(UiScaleTargetOutput::Pending, |target| target.output)
                            };
                            app_root.context.ui_scale.set_target(
                                child.surface_id, child.platform_density,
                                (size.width, size.height), child.scale_factor, output,
                            );
                        }

                        if primary_output_ready {
                            let tooltip_hover_pending = publish_tooltip_triggers(
                                &mut tooltip_manager, tooltip_triggers, last_pointer_pos,
                            );
                            if !tooltip_suppressed && (tooltip_hover_pending || tooltip_manager.needs_tick()) {
                                ticker.notify_reason(FrameReason::Timer);
                                animation_needed = true;
                            }
                        }

                        // Recompute animation_needed AFTER event routing + build_scene.
                        // Widget state may have changed (e.g. capture entered, zone switched),
                        // and layout can set a new smooth-scroll target.
                        if widget_tree.any_widget_needs_update()
                            || widget_tree.any_scroll_needs_animation()
                            || transition_layer.has_active_animations()
                            || retained_scene_cache.presentation.as_ref().is_some_and(|runtime| runtime.needs_reconciliation())
                        {
                            animation_needed = true;
                        }

                        if let Some((prebuild_timeline, prebuild_demand)) = native_layout_trace_prebuild {
                            if native_layout_trace_last_attempt == 0 {
                                let adapter = renderer.gpu_resources().adapter.get_info();
                                tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                                    event = "adapter", attempt_sequence = attempt_id.sequence(),
                                    backend = ?adapter.backend, name = %adapter.name);
                            }
                            trace_native_layout_transition_members(
                                "postbuild", attempt_id.sequence(), frame_number,
                                &widget_tree, &active_layout_transitions, &transition_layer,
                                primary_output_ready.then_some(&retained_scene_cache.interpolated_rects),
                            );
                            tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                                event = "frame", attempt_sequence = attempt_id.sequence(), frame_number,
                                layout_epoch_known = cfg!(debug_assertions),
                                layout_epoch = {
                                    #[cfg(debug_assertions)] { current_layout_epoch() }
                                    #[cfg(not(debug_assertions))] { 0u64 }
                                },
                                prebuild_timeline, prebuild_demand,
                                timeline_active = transition_layer.has_active_animations(),
                                widget_active = widget_tree.any_widget_needs_update(),
                                scroll_active = widget_tree.any_scroll_needs_animation(),
                                presentation_active = retained_scene_cache.presentation.as_ref()
                                    .is_some_and(|runtime| runtime.needs_reconciliation()),
                                tooltip_active = tooltip_manager.needs_tick(),
                                capture_active = any_pointer_captured,
                                continuous_input = event_router.continuous_frames_requested(),
                                animation_needed, pending_render, primary_visible,
                                geometry_ready = primary_geometry_ready,
                                material_pending = scene_outcome.material_pending, plot_pending,
                                visual_changed = scene_outcome.visual_changed,
                                scene_built = scene_outcome.scene.is_some(),
                                physical_w = physical_size.width, physical_h = physical_size.height,
                                scale = f64::from(scale));
                        }

                        if primary_output_ready {
                            if retained_scene_cache.admit_sdf_materials_after_paint(
                                &mut renderer,
                                &widget_tree,
                            ) {
                                // Actual staged registration/retirement work needs
                                // another ordinary frame, not animation or dirty Paint.
                                pending_render = true;
                                window.request_redraw();
                            }
                        }

                        let should_present = (primary_output_ready && scene_outcome.visual_changed)
                            || !runtime_child_windows.is_empty();
                        let should_compile = should_present || scene_outcome.material_pending;
                        if !should_compile {
                            drain_requested_images_for_reused_scene(&mut renderer);
                        }
                        if should_compile {
                            if primary_output_ready && renderer.target_has_active_extent(base_surface_id) {
                                app_root.context.ui_scale.set_output(base_surface_id,
                                    UiScaleTargetOutput::Unavailable(UiScaleTargetFailure::OutputNotProduced));
                            }
                            for child in runtime_child_windows.values() {
                                let size = child.physical_size;
                                if size.width > 0 && size.height > 0
                                    && renderer.target_has_active_extent(child.surface_id)
                                {
                                    app_root.context.ui_scale.set_output(child.surface_id,
                                        UiScaleTargetOutput::Unavailable(UiScaleTargetFailure::OutputNotProduced));
                                }
                            }
                            let scene = if primary_output_ready { Some(scene_outcome
                                .scene
                                .expect("presented frame must produce a scene")) } else { None };
                        // ============================================================
                        // SPEC-HAL-083 Phase-4/16: Scene Diff + SceneCache at UI→Render Handoff
                        // Compute identity-based diff, then update the CPU-only scene cache.
                        // ============================================================
                        if let Some(scene) = scene.as_ref() {
                        let (current_keys, delta) =
                            compute_scene_diff_for_cache(prev_scene_keys.as_ref(), &scene, &scene_cache);

                        #[cfg(debug_assertions)]
                        {
                            // Log if env flag set (check every frame is fine for debug)
                            if std::env::var("HALCYON_TRACE_SCENE_DIFF").is_ok() && !delta.is_empty()
                            {
                                tracing::debug!(target: "halcyon::core::app",
                                    "SPEC-HAL-083 SceneDiff: +{} -{} ~{}",
                                    delta.inserts.len(),
                                    delta.removes.len(),
                                    delta.updates.len()
                                );
                            }

                            // SPEC-HAL-083 Phase-11: Identity consumption dry-run
                            // Logs detailed identity changes when HALCYON_TRACE_SCENE_IDENTITY=1
                            trace_identity_changes(&delta, &current_keys);

                            // SPEC-HAL-083 Phase-12: Identity stability assertions
                            // Verifies stable keys maintain widget affinity across frames
                            assert_identity_stability(&delta, scene.key_widget_map());
                        }

                        // SPEC-HAL-083 Phase-16: Dispatch cache operations after diff, before render.
                        dispatch_cache_operations(
                            SceneCacheExecutionMode::Enabled,
                            &mut cache_consumers,
                            &delta,
                            &scene,
                            &mut scene_cache,
                        );

                        prev_scene_keys = Some(current_keys);
                        }

                        // SPEC-HAL-081B + SPEC-HAL-163: Build phase — consume scene to produce DrawPlan
                        // Multi-surface path via snapshot_multi() and build_multi()
                        let surface_stack = compositor.snapshot_multi();

                        // ============================================================
                        // SPEC-HAL-163: Per-surface scene ownership (INV-SCENE-001)
                        // ============================================================
                        // INVARIANT: SceneGraphs must NEVER be cloned across surfaces.
                        // Any reintroduction of `scene.clone()` here is an invariant violation.
                        //
                        // Enforcement: Primary surface receives scene via Option::take() (move).
                        // All other surfaces receive SceneGraph::new() (empty, valid per INV-SCENE-005).
                        // ============================================================
                        let mut surface_scenes = std::collections::HashMap::new();
                        let mut scene_slot = scene;
                        for surface_desc in surface_stack.iter() {
                            if plot_pending && surface_desc.id == base_surface_id {
                                continue;
                            }
                            let surface_scene = if surface_desc.id == base_surface_id {
                                scene_slot
                                    .take()
                                    .unwrap_or_else(crate::core::scene::SceneGraph::new)
                            } else if let Some((
                                child_winit_id,
                                child_size,
                                child_window_id,
                                parent_window_id,
                                title,
                                scale_factor,
                                edit_buffer,
                                hovered_action,
                                armed_action,
                            )) = runtime_child_windows
                                .iter()
                                .find(|(_, child)| child.surface_id == surface_desc.id)
                                .map(|(child_winit_id, child_window)| {
                                    (
                                        *child_winit_id,
                                        child_window.physical_size,
                                        child_window.window_id,
                                        child_window.parent_window_id,
                                        child_window.title.clone(),
                                        child_window.scale_factor,
                                        child_window.edit_buffer.clone(),
                                        child_window.hovered_action,
                                        child_window.armed_action,
                                    )
                                })
                            {
                                let child_bounds = RectF::new(
                                    0.0,
                                    0.0,
                                    child_size.width as f32 / scale_factor,
                                    child_size.height as f32 / scale_factor,
                                );
                                let mut child_scene_builder =
                                    SceneBuilder::with_scale_factor(scale_factor);
                                child_scene_builder
                                    .set_clear_color(theme.colors.surfaces.base0.into());
                                let child_window_state = ChildWindowSceneState {
                                    window_id: child_window_id,
                                    parent_window_id,
                                    title,
                                    bounds: child_bounds,
                                    edit_buffer,
                                    hovered_action,
                                    armed_action,
                                };
                                let mut action_regions = Vec::new();
                                {
                                    let text_engine = renderer.text_engine_mut();
                                    let scene = &mut child_scene_builder
                                        as &mut dyn crate::core::paint::PaintSink;
                                    let _ = app_root.paint_child_window_scene_modal_first(
                                        &child_window_state,
                                        &theme,
                                        text_engine,
                                        scene,
                                        &mut action_regions,
                                    );
                                }
                                if let Some(child) =
                                    runtime_child_windows.get_mut(&child_winit_id)
                                {
                                    child.action_regions = action_regions;
                                }
                                child_scene_builder.build()
                            } else {
                                let mut child_scene = crate::core::scene::SceneGraph::new();
                                child_scene.clear_color =
                                    theme.colors.surfaces.level2.into();
                                child_scene
                            };
                            surface_scenes.insert(surface_desc.id, surface_scene);
                        }

                        // SPEC-HAL-295: Process deferred texture registrations.
                        // Admitted producers queue them through their declared publication
                        // lifecycle. Drain before compile so the renderer knows about new textures.
                        renderer.drain_pending_image_registrations();

                        let draw_plan_start = Instant::now();
                        let fc = renderer.frame_counter();
                        let mut image_occurrences = presentation_frame.begin_materialization(fc, Some(attempt_id));
                        let surface_scale_factors: std::collections::HashMap<_, _> =
                            surface_stack
                                .iter()
                                .map(|desc| (desc.id, renderer.scale_factor(desc.id)))
                                .collect();
                        let draw_plan = match draw_plan_builder
                            .build_multi_with_presentation_bindings(
                            &surface_scenes,
                            surface_stack,
                            renderer.mesh_cache(),
                            &mut image_occurrences,
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
                                NativeStartupOutput::new(
                                    attempt_id, base_surface_id, primary_visible,
                                    startup_ready, primary_output_ready,
                                ).reject(format_args!("{error}"));
                                tracing::error!(target: "halcyon::core::app", %error, "DrawPlan Build refused");
                                return;
                            }
                        };
                        if primary_output_ready {
                        recycled_primary_scene = surface_scenes
                            .remove(&base_surface_id)
                            .map(SceneGraph::into_recycled_storage);
                        }
                        let draw_plan_us = draw_plan_start.elapsed().as_micros();
                        resize_profiler::add_draw_plan_time_us(draw_plan_us as u64);

                        // SPEC-HAL-081B: Compile phase — GPU buffer uploads (multi-target)
                        let compile_start = Instant::now();
                        let guarded_plan = SdfFramePlan::new(
                            draw_plan,
                            retained_scene_cache.sdf_scene_custody.clone(),
                        );
                        let compiled = if primary_output_ready {
                            renderer.compile_sdf_frame_with_presentation_bindings(
                                guarded_plan, None, image_occurrences.freeze(),
                            )
                        } else {
                            let surfaces = compositor.snapshot_multi().iter()
                                .filter(|surface| surface.id != base_surface_id)
                                .map(|surface| surface.id).collect();
                            renderer.compile_sdf_frame_with_presentation_bindings(
                                guarded_plan, Some(&surfaces), image_occurrences.freeze(),
                            )
                        };
                        let mut compiled_frame = None;
                        let mut compile_attempt = CompiledAppAttempt {
                            id: attempt_id, plots: scene_outcome.plot_evidence,
                            recorder: frame_recorder.take(), primary_output_ready,
                            startup_ready, should_present, render_time: frame_time.now,
                            build_scene_started: build_scene_start, compile_started: compile_start, draw_plan_us,
                        };
                        compile_attempt.check_compile_outcome(
                            &compiled, primary_visible, base_surface_id,
                        );
                        match compiled {
                            Ok(crate::renderer::RendererCompileOutcome::Ready(frame)) => {
                                compiled_frame = Some(frame);
                            }
                            Ok(crate::renderer::RendererCompileOutcome::Pending(identity)) => {
                                if let Some(recorder) = &mut compile_attempt.recorder {
                                    recorder.unavailable(AppFrameCompileOutcome::Pending(identity.clone()));
                                }
                                suspended_app_compile = Some(SuspendedAppCompile {
                                    identity: identity.clone(), attempt: compile_attempt,
                                });
                                let wake = event_loop_proxy.clone();
                                identity.on_completed(move || { let _ = wake.send_event(()); });
                                drop(_frame_critical_section);
                                if let Some(waiting) = &suspended_app_compile {
                                    if let Some(recorder) = &waiting.attempt.recorder {
                                        if let Some(observer) = app_root.context.frame_realization_observer() {
                                            observer.publish(recorder.snapshot());
                                        }
                                    }
                                }
                                return;
                            }
                            Ok(crate::renderer::RendererCompileOutcome::NoTargets) => {}
                            Err(e) => {
                                tracing::error!(target: "halcyon::core::app", "Multi-surface compile error: {:?}", e);
                            }
                        }
                        drop(_frame_critical_section);
                        if let Some(frame) = compiled_frame {
                            finish_compiled_app_attempt(
                                &mut renderer, frame, &mut compile_attempt, window.as_ref(),
                                base_surface_id, &mut primary_visible, &mut required_text_faults,
                                &mut retained_scene_cache, &mut app_root.context,
                            );
                        }
                        frame_recorder = compile_attempt.recorder.take();
                        } else {
                            drop(_frame_critical_section);
                        }

                        required_text_faults.emit_diagnostics();
                        if should_present {
                            ticker.mark_rendered(frame_time.now);
                            frame_number += 1;

                            // HAL_PROFILE_RESIZE: Count completed frames
                            resize_profiler::inc_frame();
                        }
                        #[cfg(feature = "test-harness")]
                        if let Some(recorder) = frame_recorder.as_mut() {
                            recorder.observe_submitted_hud_for_test_harness(&renderer);
                        }
                        if let Some(recorder) = frame_recorder {
                            let receipt = recorder.finish();
                            if tracing::enabled!(target: "halcyon::native_layout_transition_pacer", tracing::Level::TRACE) {
                                let output = receipt.surface(base_surface_id);
                                tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                                    event = "output", attempt_sequence = attempt_id.sequence(), frame_number,
                                    compiled = should_compile, should_present,
                                    output_produced = output.is_some_and(|s| s.output_produced()),
                                    submitted = output.is_some_and(|s| s.submitted()),
                                    presented = output.is_some_and(|s| s.presented_called()));
                            }
                            #[cfg(target_os = "ios")]
                            tracing::debug!(target: "halcyon::core::app", ?receipt, "iOS canonical frame realization");
                            app_root.context.frame_realization_observer().unwrap()
                                .publish(receipt);
                        }
                        native_layout_trace_last_attempt = attempt_id.sequence();

                        // Adaptive pacing: update EMA of frame cost
                        #[cfg(target_os = "linux")]
                        {
                            let cost_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
                            linux_frame_cost_ema_ms = Some(match linux_frame_cost_ema_ms {
                                Some(ema) => ema * 0.8 + cost_ms * 0.2,
                                None => cost_ms,
                            });
                        }
                    }
                    WinitEvent::WindowEvent {
                        window_id,
                        event: win_event,
                    } => {
                        if runtime_child_windows.contains_key(&window_id) {
                            pending_render = true;
                            ticker.notify_reason(FrameReason::Input);
                            let mut response =
                                None::<crate::core::signal_bus::SignalPayload>;

                            match win_event {
                                WinitWindowEvent::CloseRequested => {
                                    response =
                                        Some(crate::core::signal_bus::SignalPayload::DialogCancel);
                                }
                                WinitWindowEvent::ScaleFactorChanged {
                                    scale_factor: new_scale_factor,
                                    ..
                                } => {
                                    if let Some(child) = runtime_child_windows.get_mut(&window_id) {
                                        child.platform_density = new_scale_factor as f32;
                                        child.scale_factor = crate::utils::coords::effective_scale_factor(
                                            child.platform_density,
                                            app_root.context.ui_scale.admitted().get(),
                                        ).expect("child DPI cannot represent admitted App UI scale");
                                        if let Some((x, y)) = child.physical_cursor_pos {
                                            child.last_cursor_pos = crate::utils::coords::physical_xy_to_logical_point(
                                                x, y, child.scale_factor.into(),
                                            );
                                        }
                                        child.action_regions.clear();
                                        child.hovered_action = None;
                                        child.armed_action = None;
                                        child.window.set_cursor(crate::core::event::cursor_to_winit(
                                            crate::core::event::CursorIcon::Default,
                                        ));
                                        renderer.set_target_scale_factor(child.surface_id, child.scale_factor, false);
                                        let size = child.window.inner_size();
                                        child.physical_size = size;
                                        if let Err(error) = renderer.resize_render_target(
                                            child.surface_id, (size.width, size.height),
                                        ) {
                                            tracing::error!(target: "halcyon::core::app", %error, "child target DPI resize failed");
                                            app_root.context.ui_scale.set_output(child.surface_id,
                                                UiScaleTargetOutput::Unavailable(UiScaleTargetFailure::InvalidMetrics));
                                        }
                                        let logical_w = size.width as f32 / child.scale_factor;
                                        let logical_h = size.height as f32 / child.scale_factor;
                                        compositor.set_surface_bounds(
                                            child.surface_id,
                                            RectF::new(0.0, 0.0, logical_w, logical_h),
                                        );
                                        child.window.request_redraw();
                                        window.request_redraw();
                                    }
                                }
                                WinitWindowEvent::Resized(size) => {
                                    if size.width > 0 && size.height > 0 {
                                        if let Some(child) =
                                            runtime_child_windows.get_mut(&window_id)
                                        {
                                            child.physical_size = size;
                                            if let Err(error) = renderer.resize_render_target(
                                                child.surface_id,
                                                (size.width, size.height),
                                            ) {
                                                tracing::error!(target: "halcyon::core::app", %error, "child target resize failed");
                                                app_root.context.ui_scale.set_output(child.surface_id,
                                                    UiScaleTargetOutput::Unavailable(UiScaleTargetFailure::InvalidMetrics));
                                            }
                                            let logical_w = size.width as f32 / child.scale_factor;
                                            let logical_h = size.height as f32 / child.scale_factor;
                                            child.action_regions.clear();
                                            child.hovered_action = None;
                                            child.armed_action = None;
                                            child.window.set_cursor(crate::core::event::cursor_to_winit(
                                                crate::core::event::CursorIcon::Default,
                                            ));
                                            compositor.set_surface_bounds(
                                                child.surface_id,
                                                RectF::new(0.0, 0.0, logical_w, logical_h),
                                            );
                                            child.window.request_redraw();
                                            window.request_redraw();
                                        }
                                    } else if let Some(child) = runtime_child_windows.get_mut(&window_id) {
                                        child.physical_size = size;
                                        if let Err(error) = renderer.resize_render_target(
                                            child.surface_id, (size.width, size.height),
                                        ) {
                                            tracing::error!(target: "halcyon::core::app", %error, "child target suspension failed");
                                        }
                                        child.action_regions.clear();
                                        child.hovered_action = None;
                                        child.armed_action = None;
                                        compositor.set_surface_bounds(child.surface_id, RectF::new(0.0, 0.0, 0.0, 0.0));
                                        app_root.context.ui_scale.set_target(
                                            child.surface_id, child.platform_density,
                                            (size.width, size.height), child.scale_factor,
                                            UiScaleTargetOutput::Inactive,
                                        );
                                        window.request_redraw();
                                    }
                                }
                                WinitWindowEvent::KeyboardInput { event: key_event, .. } => {
                                    if key_event.state == winit::event::ElementState::Pressed
                                        && !key_event.repeat
                                    {
                                        if let Some(child) =
                                            runtime_child_windows.get_mut(&window_id)
                                        {
                                            match &key_event.logical_key {
                                                Key::Named(NamedKey::Escape) => {
                                                    response = Some(
                                                        crate::core::signal_bus::SignalPayload::DialogCancel,
                                                    );
                                                }
                                                Key::Named(NamedKey::Enter) => {
                                                    response = Some(
                                                        crate::core::signal_bus::SignalPayload::DialogConfirm {
                                                            value: Some(child.edit_buffer.clone()),
                                                        },
                                                    );
                                                }
                                                Key::Named(NamedKey::Backspace) => {
                                                    child.edit_buffer.pop();
                                                    child.window.request_redraw();
                                                    window.request_redraw();
                                                }
                                                key => {
                                                    if let Some(ch) = child_text_key(key) {
                                                        child.edit_buffer.push(ch);
                                                        child.window.request_redraw();
                                                        window.request_redraw();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                WinitWindowEvent::CursorMoved { position, .. } => {
                                    if let Some(child) = runtime_child_windows.get_mut(&window_id) {
                                        child.physical_cursor_pos = Some((position.x, position.y));
                                        child.last_cursor_pos = crate::utils::coords::physical_xy_to_logical_point(
                                            position.x, position.y, child.scale_factor.into(),
                                        );
                                        let hovered_action = child
                                            .action_regions
                                            .iter()
                                            .rev()
                                            .find(|region| region.rect.contains(child.last_cursor_pos))
                                            .map(|region| region.action);
                                        if child.hovered_action != hovered_action {
                                            child.hovered_action = hovered_action;
                                            let cursor = if hovered_action.is_some() {
                                                crate::core::event::CursorIcon::Pointer
                                            } else {
                                                crate::core::event::CursorIcon::Default
                                            };
                                            child.window.set_cursor(crate::core::event::cursor_to_winit(
                                                cursor,
                                            ));
                                            child.window.request_redraw();
                                            window.request_redraw();
                                        }
                                    }
                                }
                                WinitWindowEvent::CursorLeft { .. } => {
                                    if let Some(child) = runtime_child_windows.get_mut(&window_id) {
                                        child.hovered_action = None;
                                        child.armed_action = None;
                                        child.window.set_cursor(crate::core::event::cursor_to_winit(
                                            crate::core::event::CursorIcon::Default,
                                        ));
                                        child.window.request_redraw();
                                        window.request_redraw();
                                    }
                                }
                                WinitWindowEvent::MouseInput { state, button, .. } => {
                                    if button == winit::event::MouseButton::Left {
                                        if let Some(child) =
                                            runtime_child_windows.get_mut(&window_id)
                                        {
                                            let current_action = child
                                                .action_regions
                                                .iter()
                                                .rev()
                                                .find(|region| {
                                                    region.rect.contains(child.last_cursor_pos)
                                                })
                                                .map(|region| region.action);
                                            match state {
                                                winit::event::ElementState::Pressed => {
                                                    child.armed_action = current_action;
                                                    if current_action.is_some() {
                                                        child.window.request_redraw();
                                                        window.request_redraw();
                                                    }
                                                }
                                                winit::event::ElementState::Released => {
                                                    let armed_action = child.armed_action.take();
                                                    if armed_action.is_some() {
                                                        child.window.request_redraw();
                                                        window.request_redraw();
                                                    }
                                                    match (armed_action, current_action) {
                                                        (
                                                            Some(ChildWindowAction::Cancel),
                                                            Some(ChildWindowAction::Cancel),
                                                        ) => {
                                                            response = Some(
                                                                crate::core::signal_bus::SignalPayload::DialogCancel,
                                                            );
                                                        }
                                                        (
                                                            Some(ChildWindowAction::Confirm),
                                                            Some(ChildWindowAction::Confirm),
                                                        ) => {
                                                            response = Some(
                                                                crate::core::signal_bus::SignalPayload::DialogConfirm {
                                                                    value: Some(child.edit_buffer.clone()),
                                                                },
                                                            );
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                WinitWindowEvent::RedrawRequested => {
                                    window.request_redraw();
                                }
                                _ => {}
                            }

                            if let Some(payload) = response {
                                if let Some(child) = runtime_child_windows.remove(&window_id) {
                                        app_root.context.ui_scale.remove_target(child.surface_id);
                                    app_root.context.signal_bus.enqueue(
                                        crate::core::signal_bus::SignalEnvelope::window_to_window(
                                            child.window_id,
                                            child.parent_window_id,
                                            crate::core::signal_bus::SignalKind::Response,
                                            payload,
                                        ),
                                    );
                                    lifecycle.begin_window_teardown(child.window_id);
                                    lifecycle.finalize_window_teardown(child.window_id);
                                    compositor.destroy_surface(child.surface_id);
                                    required_text_faults.retire(child.surface_id);
                                    let _ = renderer.unregister_render_target(child.surface_id);
                                    window.request_redraw();
                                }
                            }

                            return;
                        }

                        // Track focus state
                        if let WinitWindowEvent::Focused(focused) = &win_event {
                            renderer.set_focused(*focused);
                            #[cfg(target_os = "macos")]
                            {
                                primary_os_focused = *focused;
                            }
                            if *focused {
                                #[cfg(target_os = "macos")]
                                {
                                    let cursor_rects_refreshed = crate::platform::relative_pointer_macos::refresh_cursor_rects_after_focus(window.as_ref());
                                    #[cfg(debug_assertions)]
                                    if let Some(recorder) = engine_conformance.as_ref() {
                                        recorder.record(
                                            "engine_cursor_rects_refreshed",
                                            serde_json::json!({"refreshed": cursor_rects_refreshed}),
                                        );
                                    }
                                    #[cfg(not(debug_assertions))]
                                    let _ = cursor_rects_refreshed;
                                }
                                window.request_redraw();
                            }
                        }

                        #[cfg(not(target_os = "linux"))]
                        if matches!(
                            &win_event,
                            WinitWindowEvent::Moved(_)
                                | WinitWindowEvent::ScaleFactorChanged { .. }
                                | WinitWindowEvent::Focused(true)
                        ) {
                            continuous_frame_pacer.set_refresh_rate_millihertz(
                                window
                                    .current_monitor()
                                    .and_then(|monitor| monitor.refresh_rate_millihertz()),
                            );
                        }

                        if let WinitWindowEvent::ScaleFactorChanged {
                            scale_factor: new_scale_factor,
                            ..
                        } = &win_event
                        {
                            pending_platform_density = Some(*new_scale_factor as f32);
                            pending_resize = Some(window.inner_size());
                            #[cfg(target_arch = "wasm32")]
                            { browser.geometry_pending = true; }
                            ticker.notify_reason(FrameReason::Forced);
                            window.request_redraw();
                        }

                        // Track modifier key state from ModifiersChanged events
                        if let WinitWindowEvent::ModifiersChanged(mods) = &win_event {
                            current_modifiers = convert_modifiers(&mods.state());
                            modifier_keys.observe_snapshot(current_modifiers);
                        }

                        // Track modifier key state from KeyboardInput events.
                        // Winit sends KeyboardInput BEFORE ModifiersChanged for modifier
                        // key releases, so we must update modifiers here to avoid stale
                        // state when processing subsequent pointer events.
                        if let WinitWindowEvent::KeyboardInput { event, .. } = &win_event {
                            current_modifiers = modifier_keys.update_key(
                                crate::platform::window::convert_physical_key(event.physical_key),
                                event.state == winit::event::ElementState::Pressed,
                            );
                        }
                        if matches!(&win_event, WinitWindowEvent::Focused(false)) {
                            modifier_keys.clear();
                            current_modifiers = Modifiers::default();
                        }

                        // Handle cursor movement
                        if let WinitWindowEvent::CursorMoved { position, .. } = &win_event {
                            last_physical_cursor_pos = Some((position.x, position.y));
                            resize_profiler::inc_mouse_move_event();
                            let new_pos = (
                                position.x as f32 / scale_factor,
                                position.y as f32 / scale_factor,
                            );
                            for halcyon_event in convert_window_event_at(
                                &win_event,
                                WindowId::PRIMARY,
                                scale_factor,
                                last_cursor_pos,
                                current_modifiers,
                                projected_touch_time.unwrap_or_else(Instant::now),
                            ) {
                                if let HalcyonEvent::RawInput(raw) = halcyon_event {
                                    if scale_geometry_pending || !deferred_scale_input.is_empty() {
                                        deferred_scale_input.push_back(DeferredAppInput::Raw(raw));
                                        ticker.notify_reason(FrameReason::Input);
                                        pending_render = true;
                                        continue;
                                    }
                                    let gate = EventRouter::relative_ingress_gate(
                                        interaction_domains.primary_state(),
                                    );
                                    observe_raw_geometry_allocation(
                                        &mut widget_tree,
                                        app_root.active_root_node(),
                                        &raw,
                                        scale_factor,
                                    );
                                    input_fusion.process_raw_with_gesture_sessions(
                                        raw,
                                        crate::core::event::InteractionDomainId::PRIMARY,
                                        &mut interaction_domains.primary_state_mut().gesture_registry,
                                        &widget_tree,
                                        gate.as_ref(),
                                        &mut fused_events,
                                    );
                                }
                            }
                            last_cursor_pos = new_pos;
                            ticker.notify_reason(FrameReason::Input);
                            pending_render = true;

                            #[cfg(target_os = "linux")]
                            {
                                if in_resize_burst {
                                    linux_continuous_redraw_latch = true;
                                }
                            }
                        } else {
                            for halcyon_event in convert_window_event(
                                &win_event,
                                WindowId::PRIMARY,
                                scale_factor,
                                last_cursor_pos,
                                current_modifiers,
                            ) {
                                if let HalcyonEvent::RawInput(mut raw) = halcyon_event {
                                    if let RawInputEvent::Key { key_code, modifiers, state, repeat, reservation, .. } = &mut raw {
                                        if let Some(observed) = crate::core::event::KeyboardKeyReservation::from_observed_key(
                                            *key_code, *modifiers, *state == crate::core::event::KeyState::Down, *repeat,
                                        ) {
                                            *reservation = Some(observed);
                                            window.request_redraw();
                                        }
                                    }
                                    if scale_geometry_pending || !deferred_scale_input.is_empty() {
                                        deferred_scale_input.push_back(DeferredAppInput::Raw(raw));
                                        ticker.notify_reason(FrameReason::Input);
                                        pending_render = true;
                                        continue;
                                    }
                                    let gate = EventRouter::relative_ingress_gate(
                                        interaction_domains.primary_state(),
                                    );
                                    observe_raw_geometry_allocation(
                                        &mut widget_tree,
                                        app_root.active_root_node(),
                                        &raw,
                                        scale_factor,
                                    );
                                    input_fusion.process_raw_with_gesture_sessions(
                                        raw,
                                        crate::core::event::InteractionDomainId::PRIMARY,
                                        &mut interaction_domains.primary_state_mut().gesture_registry,
                                        &widget_tree,
                                        gate.as_ref(),
                                        &mut fused_events,
                                    );
                                    ticker.notify_reason(FrameReason::Input);
                                    pending_render = true;

                                    #[cfg(target_os = "linux")]
                                    {
                                        if in_resize_burst {
                                            linux_continuous_redraw_latch = true;
                                        }
                                    }
                                }
                            }
                        }

                        match win_event {
                            WinitWindowEvent::CloseRequested => {
                                #[cfg(all(target_os = "macos", debug_assertions))]
                                if let Some(recorder) = engine_conformance.as_ref() {
                                    recorder.record(
                                        "engine_trigger",
                                        serde_json::json!({"trigger": "window_close"}),
                                    );
                                }
                                exit_primary_native_app(
                                    elwt,
                                    window.as_ref(),
                                    &mut event_router,
                                    &mut widget_tree,
                                    interaction_domains.primary_state_mut(),
                                    &mut input_fusion,
                                    #[cfg(all(target_os = "macos", debug_assertions))]
                                    engine_conformance.as_ref(),
                                    "window_close",
                                );
                            }
                            WinitWindowEvent::Resized(size) => {
                                // Resize event coalescing: store pending size, process once per frame
                                resize_profiler::inc_resize_event();
                                pending_resize = Some(size);
                                #[cfg(target_arch = "wasm32")]
                                { browser.geometry_pending = true; }
                                ticker.notify_reason(FrameReason::Forced);

                                // Linux: detect resize burst and enable frame pacing
                                #[cfg(target_os = "linux")]
                                {
                                    let now = Instant::now();
                                    let entering_burst = if let Some(last) = last_resize_time {
                                        now.duration_since(last).as_millis() < RESIZE_BURST_THRESHOLD_MS as u128
                                    } else {
                                        false
                                    };
                                    last_resize_time = Some(now);

                                    if entering_burst && !in_resize_burst {
                                        // Entering burst: set tick ONCE, AboutToWait owns it after
                                        in_resize_burst = true;
                                        next_frame_tick = Some(now + Duration::from_millis(16));
                                    } else if in_resize_burst {
                                        // Already in burst: do NOT touch next_frame_tick
                                        // AboutToWait owns tick advancement
                                    } else {
                                        // First resize event, process immediately
                                        window.request_redraw();
                                    }
                                }
                                #[cfg(not(target_os = "linux"))]
                                {
                                    window.request_redraw();
                                }
                            }
                            WinitWindowEvent::RedrawRequested => {}
                            _ => {}
                        }
                    }
                    #[cfg(target_os = "macos")]
                    WinitEvent::DeviceEvent {
                        event: winit::event::DeviceEvent::MouseMotion { delta },
                        ..
                    } => {
                        let gate =
                            EventRouter::relative_ingress_gate(interaction_domains.primary_state());
                        #[cfg(debug_assertions)]
                        if let (Some(recorder), Some(gate)) =
                            (engine_conformance.as_ref(), gate)
                        {
                            recorder.record(
                                "engine_device",
                                serde_json::json!({
                                    "delta": [delta.0, delta.1],
                                    "lease_id": gate.lease_id().get(),
                                    "focused": primary_os_focused,
                                    "window_scale": window.scale_factor(),
                                }),
                            );
                        }
                        let focused_window_id = if primary_os_focused {
                            interaction_domains.active_window()
                        } else {
                            None
                        };
                        if let Some(raw) =
                            crate::platform::relative_pointer_macos::relative_motion_from_device_event(
                                gate,
                                focused_window_id,
                                WindowId::PRIMARY,
                                delta,
                                Instant::now(),
                            )
                        {
                            input_fusion.process_raw_with_gesture_sessions(
                                raw,
                                crate::core::event::InteractionDomainId::PRIMARY,
                                &mut interaction_domains.primary_state_mut().gesture_registry,
                                &widget_tree,
                                gate.as_ref(),
                                &mut fused_events,
                            );
                            ticker.notify_reason(FrameReason::Input);
                            pending_render = true;
                        }
                    }
                    WinitEvent::NewEvents(StartCause::Init) => {
                        pending_render = true;
                        if primary_visible {
                            window.request_redraw();
                        }
                    }
                    WinitEvent::UserEvent(()) => {
                        // Standard Quit has already been drained at native ingress.
                        // Wake-only ingress for resource dependency changes.
                        pending_render = true;
                        ticker.notify_reason(FrameReason::Forced);
                        #[cfg(target_os = "ios")]
                        if let Some(reply) = ios_choice.take_reply() {
                            deferred_scale_input.push_back(DeferredAppInput::HostChoice(reply));
                        }
                    }
                    WinitEvent::AboutToWait => {
                        if !primary_visible {
                            return;
                        }
                        // Coalesced input redraw: at most one request_redraw per vsync cadence.
                        if pending_render {
                            pending_render = false;
                            window.request_redraw();
                            tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                                event = "request", cause = "pending_render", frame_number,
                                last_attempt_sequence = native_layout_trace_last_attempt, animation_needed);
                        }

                        // Linux resize burst pacing: check for burst exit and manage frame timing
                        #[cfg(target_os = "linux")]
                        {
                            let now = Instant::now();

                            // Check if resize burst should exit (no resize event in 150ms)
                            if in_resize_burst {
                                if let Some(last) = last_resize_time {
                                    if now.duration_since(last).as_millis()
                                        >= RESIZE_BURST_EXIT_MS as u128
                                    {
                                        // Exit resize burst, return to normal mode
                                        in_resize_burst = false;
                                        next_frame_tick = None;
                                    }
                                }
                            }

                            // Compute needs_redraw including latched intent
                            let needs_redraw = pending_resize.is_some()
                                || linux_continuous_redraw_latch
                                || event_router.continuous_frames_requested()
                                || animation_needed;

                            // During resize burst, pace frames adaptively based on EMA frame cost
                            if in_resize_burst {
                                if needs_redraw {
                                    if let Some(tick) = next_frame_tick {
                                        if now >= tick {
                                            linux_redraw_pending = true;
                                            window.request_redraw();
                                            linux_continuous_redraw_latch = false;

                                            // Compute adaptive interval using EMA, ceil, clamp, quantize
                                            let interval_ms = if let Some(ema) = linux_frame_cost_ema_ms {
                                                let ideal = ema / RESIZE_TARGET_UTILIZATION;
                                                let ceiled = ideal.ceil() as u64;
                                                let clamped =
                                                    ceiled.clamp(RESIZE_MIN_INTERVAL_MS, RESIZE_MAX_INTERVAL_MS);
                                                // Quantize to stable band
                                                *RESIZE_INTERVAL_BANDS
                                                    .iter()
                                                    .find(|&&b| b >= clamped)
                                                    .unwrap_or(&RESIZE_MAX_INTERVAL_MS)
                                            } else {
                                                16 // Conservative default
                                            };
                                            // Advance from tick, not now (prevents drift)
                                            next_frame_tick = Some(tick + Duration::from_millis(interval_ms));
                                        }
                                    }
                                }
                                return;
                            }

                            // Not in burst: normal redraw scheduling
                            if needs_redraw {
                                linux_redraw_pending = true;
                                window.request_redraw();
                                linux_continuous_redraw_latch = false;
                            }
                            return;
                        }

                        // Non-Linux: continuous update/animation work is admitted only
                        // when its stable display-rate deadline is due. Input-driven
                        // continuous frames remain immediate and coalesce through winit.
                        #[cfg(not(target_os = "linux"))]
                        {
                            let continuous_input = event_router.continuous_frames_requested();
                            let pacer_due = !continuous_input
                                && continuous_frame_pacer.take_due(Instant::now(), animation_needed);
                            if continuous_input || pacer_due {
                                window.request_redraw();
                                tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                                    event = "request", cause = if continuous_input { "continuous_input" } else { "pacer" },
                                    frame_number, last_attempt_sequence = native_layout_trace_last_attempt, animation_needed);
                            }
                        }
                    }
                    _ => {}
                }
            };
        event_handler
    }
}
