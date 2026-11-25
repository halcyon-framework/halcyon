use super::*;

pub fn run_ui_loop<F>(
    config: UiThreadConfig,
    builder: F,
    input_rx: Receiver<RawInputEvent>,
    scene_swapchain: Arc<SceneSwapchain>,
) where
    F: FnOnce() -> (WidgetTree, NodeId),
{
    run_ui_loop_with_cursor_output(config, builder, input_rx, scene_swapchain, None);
}

pub(super) fn run_ui_loop_with_cursor_output<F>(
    config: UiThreadConfig,
    builder: F,
    input_rx: Receiver<RawInputEvent>,
    scene_swapchain: Arc<SceneSwapchain>,
    cursor_output: Option<cursor_output::CursorOutputSender>,
) where
    F: FnOnce() -> (WidgetTree, NodeId),
{
    // Build widget tree inside the UI thread - widgets don't need to be Send
    let (mut widget_tree, root) = builder();
    // SPEC-HAL-151: Interaction domains own per-window interaction state.
    let mut interaction_domains = config.interaction_domains();

    let mut theme = config.theme.clone();

    // SPEC-HAL-018 Stage 3: AppRoot now owns the active render root.
    let mut app_root = AppRoot::new(AppContext::new(theme.clone()), root);

    // SPEC-HAL-018 Stage 14: Push initial screen if provided via config.
    // The initial_screen builder runs in the UI thread, so Screen doesn't need Send.
    if let Some(screen_builder) = config.initial_screen {
        let initial_screen = screen_builder();
        app_root.push_screen(initial_screen);
    }

    let mut text_engine = config.text_engine;
    let mut constraints = config.initial_constraints;

    let mut ticker = EngineTicker::new(Instant::now());
    let mut input_fusion = InputFusion::new();
    let mut event_router = EventRouter::new();
    // SPEC-HAL-046: High-frequency scheduler for animations/charts
    let mut scheduler = HighFreqScheduler::default_120hz();
    // SPEC-HAL-133: Scroll link coordinator for synchronized scrolling
    let mut scroll_link = ScrollLinkCoordinator::new();
    // SPEC-HAL-021 Stage 3: TransitionLayer owns animation timelines and policy
    // INV-TL-4: TransitionLayer is the sole animation policy owner
    let mut transition_layer = TransitionLayer::new();
    let mut frame_id: u64 = 0;
    let mut fused_events = Vec::new();
    // SPEC-HAL-078 Stage 2: Engine-owned storage for active layout transitions
    // SPEC-HAL-078 FIX: Candidate collection buffer now local to build_scene
    let mut active_layout_transitions = ActiveLayoutTransitions::new();
    // SPEC-HAL-265 M8: Icon registry for paint-time icon lookup
    let icon_registry = crate::core::icons::builtin_registry();
    // SPEC-HAL-266: Track current scale factor for scene build
    let mut scale_factor = config.initial_scale_factor;
    // SPEC-HAL-081A: Single-window compositor for surface ordering
    let viewport_rect = RectF::new(0.0, 0.0, constraints.max_width, constraints.max_height);
    let mut compositor = Compositor::new(viewport_rect);
    // Create base surface covering the viewport (scene_key 1 = main scene content)
    let base_surface = compositor.create_surface(0, viewport_rect, SceneKey::BASE);

    // SPEC-HAL-248: Navigation projection pull is live for the primary root.
    let nav_orchestrator = NavigationSurfaceOrchestrator::new_with_existing_root_surface(
        NavigationHost::new(NavigationNodeId(1)),
        viewport_rect,
        base_surface,
        &mut compositor,
        Box::new(IncrementalSceneKeyAllocator::new()),
    );
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
    let screen_host_key = ScreenHostProjectionKey::new(WindowId::PRIMARY, root);
    let root_is_screen_host = widget_tree
        .node(root)
        .is_some_and(|node| node.widget.as_any().downcast_ref::<ScreenHost>().is_some());
    if root_is_screen_host {
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

    // SPEC-HAL-226A: Framework-level tooltip manager
    let mut tooltip_manager =
        crate::model::tooltip::TooltipManager::new(crate::model::tooltip::TooltipTiming {
            show_delay_ms: theme.tooltip.show_delay_ms,
            hide_delay_ms: theme.tooltip.hide_delay_ms,
            reshow_window_ms: theme.tooltip.reshow_window_ms,
        });
    let mut last_pointer_pos = crate::core::math::Point::zero();

    loop {
        // 1. Drain all available RawInputEvents, handle resize
        let mut had_input = false;
        loop {
            match input_rx.try_recv() {
                Ok(raw_event) => {
                    had_input = true;
                    // SPEC-HAL-044: Handle resize events to update constraints
                    if let RawInputEvent::Resize { width, height, .. } = &raw_event {
                        constraints = Constraints::loose(Size::new(*width, *height));
                        // SPEC-HAL-081A: Update compositor viewport only.
                        // INV-RSZ-002: Do NOT call set_surface_bounds here - the render thread
                        // path handles it atomically with resize_render_target to prevent race
                        // conditions where an older resize event overwrites correct bounds.
                        let new_viewport = RectF::new(0.0, 0.0, *width, *height);
                        compositor.set_viewport(new_viewport);
                    }
                    // SPEC-HAL-266: Track scale factor changes for scene build
                    if let RawInputEvent::ScaleFactorChanged {
                        scale_factor: sf, ..
                    } = &raw_event
                    {
                        if EventRouter::relative_ingress_gate(interaction_domains.primary_state())
                            .is_some()
                        {
                            event_router.cancel_active_relative_with_tree(
                                &mut widget_tree,
                                interaction_domains.primary_state_mut(),
                                RelativePointerCancellationReason::NormalizationChanged,
                                None,
                            );
                            input_fusion.discard_pending_relative_pointer();
                        }
                        scale_factor = *sf;
                    }
                    let gate =
                        EventRouter::relative_ingress_gate(interaction_domains.primary_state());
                    widget_tree.observe_geometry_allocation(root, constraints, scale_factor);
                    input_fusion.process_raw_with_gesture_sessions(
                        raw_event,
                        crate::core::event::InteractionDomainId::PRIMARY,
                        &mut interaction_domains.primary_state_mut().gesture_registry,
                        &widget_tree,
                        gate.as_ref(),
                        &mut fused_events,
                    );
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    event_router.cancel_active_relative_with_tree(
                        &mut widget_tree,
                        interaction_domains.primary_state_mut(),
                        RelativePointerCancellationReason::ApplicationShutdown,
                        None,
                    );
                    input_fusion.discard_pending_relative_pointer();
                    event_router.cancel_all_pointer_sessions_with_tree(
                        &mut widget_tree,
                        crate::core::event::CancellationReason::SystemCancel,
                        interaction_domains.primary_state_mut(),
                    );
                    tracing::info!(target: "halcyon::core::app", "UI thread: input channel disconnected, exiting");
                    return;
                }
            }
        }

        // 2. Determine frame reason
        if had_input {
            ticker.notify_reason(FrameReason::Input);
        }
        // 3. Begin frame timing
        // SPEC-HAL-041: begin_frame() is the ONLY place that calls Instant::now()
        // All timing below uses frame_time.now exclusively
        let frame_time = ticker.begin_frame();

        // SPEC-HAL-133: Begin linked scroll frame (clears seen tokens)
        scroll_link.begin_frame();

        // SPEC-HAL-078 Stage 0: Begin layout transition frame (swap geometry buffers)
        // Must occur before any compute_absolute_paint_rects calls.
        widget_tree.begin_layout_transition_frame();

        // 4. SPEC-HAL-046: Run high-frequency scheduler only when jobs exist.
        if scheduler.has_jobs() {
            scheduler.run(&frame_time);
        }

        // 4.5 SPEC-HAL-088 Stage 3: Update scroll animations BEFORE event routing
        //     so that hit-testing uses viewports matching the displayed content.
        //     This advances visual offsets toward targets set in PREVIOUS frame.
        //     SPEC-HAL-146: Visual state mutations require UpdateAuthorityGuard.
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

        // 4.55 SPEC-HAL-021 Stage 3: Advance TransitionLayer BEFORE layout.
        //      INV-TL-3: Timeline does not request frames; caller decides when to call.
        //      ANIM-INV-4: Must complete before layout begins.
        let now_secs = ticker.timeline_time_secs(frame_time.now);
        transition_layer.advance(now_secs);

        // SPEC-HAL-078 Stage 2: Clean up completed layout transitions
        // LT-INV-22: Timeline is the only time authority.
        active_layout_transitions.cleanup_completed(&transition_layer);

        // 4.6 SPEC-HAL-088 Stage 3: Recompute positions and viewports for hit-testing
        //     This ensures hit-test regions match what's currently displayed.
        //     SPEC-HAL-141: Must hold Layout authority for geometry mutations.
        {
            let _layout_authority = LayoutAuthorityGuard::enter();
            let active_root = app_root.active_root_node();
            compute_absolute_positions(&mut widget_tree, active_root, Point::zero());
            compute_absolute_paint_rects(&mut widget_tree, active_root, Point::zero());
            widget_tree.compute_viewports(active_root);
        }

        // SPEC-HAL-078 FIX: Candidate collection/binding moved to build_scene (AFTER layout)
        // This ensures to_rect equals post-layout authoritative geometry.

        // SPEC-HAL-086 Stage 12: Signal animation needed if any scroll is active
        // This ensures drag mode gets continuous frames (not just input-driven).
        if widget_tree.any_scroll_needs_animation() {
            ticker.notify_reason(FrameReason::Animation);
        }

        // SPEC-HAL-354: Keep update ticks alive for animation and update-only
        // widgets. Update-only polling does not imply paint invalidation.
        if widget_tree.any_widget_needs_update() {
            ticker.notify_reason(FrameReason::Animation);
        }

        // SPEC-HAL-021 / SPEC-HAL-078: keep frames alive while transitions are active
        if transition_layer.has_active_animations() {
            ticker.notify_reason(FrameReason::Animation);
        }

        // SPEC-HAL-241: Update drag ghost smoothing
        if let Some(session) = interaction_domains
            .primary_state_mut()
            .drag_drop
            .session_mut()
        {
            session.update(dt_secs);
            ticker.notify_reason(FrameReason::Animation);
        }
        interaction_domains
            .primary_state_mut()
            .drag_drop
            .cleanup_if_faded();

        // 5. Process input fusion (flush coalesced events)
        input_fusion.process_frame_with_gesture_sessions(
            &frame_time,
            &mut ticker,
            &mut fused_events,
        );

        // 6. Route fused events through AppRoot then widgets.
        //    Active overlays bypass base-screen input before widget-tree routing.
        // SPEC-HAL-038 Stage 2: Get active root for Tab navigation
        let active_root = app_root.active_root_node();

        // SPEC-HAL-101 Stage-4: Set frame time for caret blinking
        event_router.set_frame_time_ms(ticker.frame_time_ms(frame_time.now));

        let mut default_cursor_event = false;
        let mut availability_intake = std::mem::take(&mut fused_events).into_iter();
        while let Some(qualified) =
            next_qualified_widget_input(&widget_tree, &mut availability_intake, &mut fused_events)
        {
            publish_ui_cursor(
                cursor_output.as_ref(),
                &mut event_router,
                default_cursor_event,
                EventRouter::relative_ingress_gate(interaction_domains.primary_state()).is_some(),
            );
            default_cursor_event = false;
            let _delivery = SessionInputDelivery(&qualified);
            let event = qualified.event().clone();
            // SPEC-HAL-226A: Track pointer position for tooltip hover detection
            match &event {
                FusedInputEvent::PointerMove { pos, .. }
                | FusedInputEvent::PointerDown { pos, .. }
                | FusedInputEvent::PointerUp { pos, .. } => {
                    last_pointer_pos = *pos;
                }
                _ => {}
            }
            // SPEC-HAL-226A: Dismiss tooltip on pointer-down or Escape
            if matches!(&event, FusedInputEvent::PointerDown { .. }) {
                tooltip_manager.pointer_down();
            }
            if let FusedInputEvent::KeyDown { key_code, .. } = &event {
                if *key_code == crate::core::event::KeyCode::Escape {
                    tooltip_manager.escape_pressed();
                }
            }

            if let Some(snapshot) = app_root.capture_blueprint_shortcut(
                &event,
                &mut widget_tree,
                &mut interaction_domains,
                &mut event_router,
                text_engine.as_mut(),
            ) {
                write_blueprint_shortcut_snapshot(&snapshot);
                qualified.finish_delivery(true);
                continue;
            }

            // SPEC-HAL-151: Active-window gate - drop events from non-active windows
            // except FocusChanged, Resize, and Tick which are always routed
            if !interaction_domains.should_route_event(&event) {
                tracing::debug!(target: "halcyon::core::app",
                    "SPEC-HAL-151: Dropped event from non-active window {:?}",
                    event.window_id().0
                );
                continue;
            }

            let event_window_id = event.window_id();
            let relative_facts = interaction_domains.domain_for_window(event_window_id).map(
                |interaction_domain_id| RelativePointerWindowFacts {
                    window_id: event_window_id,
                    interaction_domain_id,
                    focused_window_id: interaction_domains.active_window(),
                    client_bounds: compositor.viewport(),
                },
            );
            if let FusedInputEvent::FocusChanged {
                window_id,
                focused: false,
                ..
            } = &event
            {
                if let Some(state) = interaction_domains.resolve_window_mut(*window_id) {
                    event_router.cancel_active_relative_with_tree(
                        &mut widget_tree,
                        state,
                        RelativePointerCancellationReason::FocusLost,
                        None,
                    );
                    input_fusion.discard_pending_relative_pointer();
                }
            }

            // SPEC-HAL-151: Handle FocusChanged specially for domain teardown
            let domain_state = if let FusedInputEvent::FocusChanged {
                window_id, focused, ..
            } = &event
            {
                match handle_window_focus_changed(
                    &mut interaction_domains,
                    &mut event_router,
                    &mut widget_tree,
                    *window_id,
                    *focused,
                    text_engine.as_mut(),
                ) {
                    Some(state) => state,
                    None => {
                        tracing::debug!(target: "halcyon::core::app",
                            "SPEC-HAL-151: No domain for window {:?}, dropping FocusChanged",
                            window_id.0
                        );
                        continue;
                    }
                }
            } else {
                // SPEC-HAL-151: Resolve domain from window_id
                let window_id = event.window_id();
                match interaction_domains.resolve_window_mut(window_id) {
                    Some(state) => state,
                    None => {
                        tracing::debug!(target: "halcyon::core::app",
                            "SPEC-HAL-151: No domain for window {:?}, dropping event",
                            window_id.0
                        );
                        continue;
                    }
                }
            };

            if !prepare_session_input_for_app(
                &mut widget_tree,
                &mut event_router,
                domain_state,
                &qualified,
            ) {
                continue;
            }
            default_cursor_event = cursor_default_event(
                &event,
                domain_state.pointer_capture_manager.is_any_captured(),
            );
            if matches!(
                &event,
                FusedInputEvent::KeyDown {
                    key_code: crate::core::event::KeyCode::Escape,
                    ..
                }
            ) {
                if event_router.cancel_active_relative_with_tree(
                    &mut widget_tree,
                    domain_state,
                    RelativePointerCancellationReason::ExplicitSemanticCancel,
                    relative_facts,
                ) {
                    input_fusion.discard_pending_relative_pointer();
                }
            }
            cancel_relative_overlay_takeover(
                &app_root,
                &mut widget_tree,
                &mut event_router,
                domain_state,
            );
            if EventRouter::claims_relative_event(&event, domain_state) {
                let input_root = app_root.active_input_root_node();
                event_router.route_session_input_event_with_relative_facts(
                    &mut widget_tree,
                    qualified.clone(),
                    Some(input_root),
                    text_engine.as_mut(),
                    Some(transition_layer.animation_context()),
                    domain_state,
                    &mut app_root.context.registry.binding_registry,
                    relative_facts.expect("active relative lease has a registered domain"),
                );
                let gate = EventRouter::relative_ingress_gate(domain_state);
                input_fusion.reconcile_relative_ingress(gate.as_ref());
                continue;
            }

            let app_dispatch = dispatch_session_input_to_app_root_with_captured_routing(
                &mut app_root,
                &mut widget_tree,
                &mut event_router,
                domain_state,
                &qualified,
                text_engine.as_mut(),
                None,
                Some(transition_layer.animation_context()),
                relative_facts,
            );
            cancel_relative_overlay_takeover(
                &app_root,
                &mut widget_tree,
                &mut event_router,
                domain_state,
            );
            if app_dispatch.handled {
                if app_dispatch.captured_key
                    != crate::core::event_router::CapturedAppKeyOutcome::NotEligible
                {
                    let pending_focus = event_router.geometry_pending_focus_reveal();
                    geometry_effect_checkpoint(
                        &mut widget_tree,
                        &mut event_router,
                        domain_state,
                        Some((&active_layout_transitions, &transition_layer)),
                        pending_focus,
                    );
                    let gate = EventRouter::relative_ingress_gate(domain_state);
                    input_fusion.reconcile_relative_ingress(gate.as_ref());
                }
                // Event fully handled at app/screen level – do not send to widgets
                continue;
            }

            let input_root = app_root.active_input_root_node();
            if let Some(modal_root) = app_root.active_modal_input_root_node() {
                event_router.enforce_modal_input_boundary_with_text_engine(
                    &mut widget_tree,
                    modal_root,
                    domain_state,
                    Some(text_engine.as_mut()),
                );
            }

            // SPEC-HAL-021 Stage 5 / SPEC-HAL-038 Stage 6: Construct animation
            // capability BEFORE dispatch_engine_action so keyboard activation
            // (Enter/Space/i via binding registry → handle_engine_action →
            // activate_focused_widget → on_keyboard_activate) can start
            // animations like Button's press-pulse feedback. If the engine
            // action consumes the event, the animation context is moved in
            // and dropped with the call; otherwise a fresh context is built
            // for the fallback route_input_event path below.
            // INV-TL-2: Router accepts animation capability but does not own or create it.
            let animation = Some(transition_layer.animation_context());

            // SPEC-HAL-176: Engine action dispatch (shared with App::run)
            if dispatch_engine_action(
                &event,
                &mut app_root.context.registry.binding_registry,
                &mut event_router,
                &mut widget_tree,
                input_root,
                domain_state,
                text_engine.as_mut(),
                animation,
            ) {
                qualified.finish_delivery(true);
                geometry_effect_checkpoint(
                    &mut widget_tree,
                    &mut event_router,
                    domain_state,
                    None,
                    None,
                );
                continue;
            }

            // Fallback: normal widget routing
            // SPEC-HAL-038 Stage 2: Pass root for Tab navigation
            // SPEC-HAL-021 Stage 5: Fresh animation context for fallback path —
            // the one passed to dispatch_engine_action was moved into that call.
            // Hover, toggle, activation — any widget interaction may start animations.
            // INV-TL-2: Router accepts animation capability but does not own or create it.
            // Phase-safe: event routing (Phase 1) is before timeline.advance (pre-layout).
            let animation = Some(transition_layer.animation_context());
            if domain_state.relative_pointer_authority.is_some() {
                event_router.route_session_input_event_with_relative_facts(
                    &mut widget_tree,
                    qualified.clone(),
                    Some(input_root),
                    text_engine.as_mut(),
                    animation,
                    domain_state,
                    &mut app_root.context.registry.binding_registry,
                    relative_facts.expect("relative authority has a registered domain"),
                );
            } else {
                event_router.route_session_input_event(
                    &mut widget_tree,
                    qualified.clone(),
                    Some(input_root),
                    text_engine.as_mut(),
                    animation,
                    domain_state,
                    &mut app_root.context.registry.binding_registry,
                );
            }
            let pending_focus = event_router.geometry_pending_focus_reveal();
            geometry_effect_checkpoint(
                &mut widget_tree,
                &mut event_router,
                domain_state,
                Some((&active_layout_transitions, &transition_layer)),
                pending_focus,
            );
            let gate = EventRouter::relative_ingress_gate(domain_state);
            input_fusion.reconcile_relative_ingress(gate.as_ref());
        }
        publish_ui_cursor(
            cursor_output.as_ref(),
            &mut event_router,
            default_cursor_event,
            EventRouter::relative_ingress_gate(interaction_domains.primary_state()).is_some(),
        );

        // Apply bound control writes in the same post-route phase as the native
        // application loop before retained screen state is updated or rebuilt.
        app_root.flush_pending_actions_with_composite_editors(
            &mut widget_tree,
            &mut event_router,
            interaction_domains.primary_state_mut(),
            text_engine.as_mut(),
        );

        // 7. SPEC-HAL-018 Stage 9: Process queued navigation commands
        //    Screens/modals can queue navigation via ScreenContext helpers (nav_push_screen, etc.)
        //    Commands are processed here, after input but before rebuild.
        app_root.process_nav_queue();

        // 7.5 SPEC-HAL-248: Pull navigation projection into ScreenHost.
        //     Pull projection and apply to compositor.
        //     ScreenHost is the sole runtime consumer of projection data.
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

        // 8. SPEC-HAL-018 Stage 4: Rebuild app view under ScreenHost root (screens + modals)
        //    This calls Screen::build() for active screen and all modals,
        //    populating the ScreenHost's children before layout/paint.
        app_root.rebuild_active_view(&mut widget_tree);

        // 8.1 SPEC-HAL-128 Stage 5: Lifecycle cleanup for destroyed widgets
        //     After rebuild, widgets may have been removed. Drain pending removals
        //     and notify EventRouter to clean up gesture/capture state.
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
                    crate::core::animation::AnimationOwnerId::new(widget_id.0),
                );
            }
            let domain_state = interaction_domains.primary_state_mut();
            event_router.on_widgets_destroyed(&removed_widget_ids, domain_state);

            // Focus recovery: if the focused widget was destroyed by rebuild,
            // remember its position in the focus chain, rebuild the chain on the
            // new tree, and re-focus the widget at the same index.
            if let Some(focused_id) = domain_state.focus_manager.focused() {
                if removed_widget_ids.contains(&focused_id) {
                    let old_index = domain_state.focus_manager.focus_chain_index(focused_id);
                    event_router.clear_focus(&mut widget_tree, domain_state);
                    domain_state
                        .focus_manager
                        .rebuild_focus_chain(&widget_tree, active_root);
                    let target = old_index
                        .and_then(|idx| domain_state.focus_manager.focus_chain_at(idx))
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
            event_router.drain_relative_pointer_semantic_cancels(
                &mut widget_tree,
                domain_state,
                relative_facts,
            );
            let gate = EventRouter::relative_ingress_gate(domain_state);
            input_fusion.reconcile_relative_ingress(gate.as_ref());
        }
        publish_ui_cursor(
            cursor_output.as_ref(),
            &mut event_router,
            false,
            EventRouter::relative_ingress_gate(interaction_domains.primary_state()).is_some(),
        );
        // SPEC-HAL-031: Clean up signal subscriptions for removed nodes
        let removed_node_ids = widget_tree.drain_pending_node_removals();
        if !removed_node_ids.is_empty() {
            app_root
                .context
                .signals
                .remove_subscribers(&removed_node_ids);
        }

        // 8.5 SPEC-HAL-038 Stage 3: Sync focus scopes with modal state
        //     After rebuild, modals have their mounted_nodes set.
        //     Sync the scope stack to match current modal stack.
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
            event_router.sync_focus_scopes(&mut widget_tree, &modal_nodes, domain_state);
            event_router.ensure_focus_within_active_scope(&mut widget_tree, domain_state);
        }

        let composite_frame_time = event_router.frame_time_ms();
        widget_tree.process_composite_choice_work(
            &mut event_router,
            interaction_domains.primary_state_mut(),
            text_engine.as_mut(),
            composite_frame_time,
        );

        if widget_tree.has_pending_widget_availability() {
            crate::core::widget_availability::apply_pending_widget_availability(
                &mut widget_tree,
                &mut event_router,
                &mut input_fusion,
                interaction_domains.primary_state_mut(),
                &mut app_root.context.registry.binding_registry,
                text_engine.as_mut(),
            );
        }

        // 9. Check throttling - sleep to avoid busy-spinning when idle
        // SPEC-HAL-041: Use frame_time.now, not a separate Instant::now() call
        {
            let _update = UpdateAuthorityGuard::enter();
            if widget_tree.advance_kinetic_scroll_for_frame(&frame_time) {
                ticker.notify_reason(FrameReason::Animation);
            }
        }
        if !fused_events.is_empty() {
            ticker.notify_reason(FrameReason::Input);
        }
        if ticker.should_throttle(frame_time.now) {
            // Sleep briefly instead of busy-spinning. 4ms keeps input latency low
            // while reducing idle CPU from 100% to ~1%.
            std::thread::sleep(std::time::Duration::from_millis(4));
            continue;
        }

        // 9.1 Runtime theme switch: sync local theme from AppContext if changed
        if app_root.context.take_theme_changed() {
            theme = app_root.context.theme.clone();
            widget_tree.mark_subtree_dirty(root);
        }

        // 9.2 SPEC-HAL-031: Apply signal-driven dirty marks before layout
        let dirty_nodes = app_root.context.signals.take_pending_dirty();
        for node_id in dirty_nodes {
            widget_tree.mark_layout_dirty(node_id);
        }

        // 9.5 SPEC-HAL-133: Process linked scroll intents
        //     INV-LINK-ORDER-1: Occurs before layout_tree()
        //     INV-LINK-ORDER-2: Pure scroll-state mutation only
        //     INV-LINK-ORDER-3: No anchor resolution (that happens after layout)
        //     Drain intents and apply anchors to linked ScrollAreas
        process_linked_scroll_intents(&mut scroll_link, &mut widget_tree);

        // HAL383: settle staged editors at the existing deferred cut before first Layout.
        widget_tree.process_text_field_handoffs(text_engine.as_mut(), &theme);

        // 10. Build scene
        // Note: active_root already computed for Tab navigation at step 6
        // SPEC-HAL-101 Stage-4C: Pass frame_time_ms for caret blinking
        // SPEC-HAL-021 Stage 3: Pass timeline from TransitionLayer for animation value consumption
        // SPEC-HAL-078 Stage 3: Pass active_layout_transitions for paint-time interpolation
        // SPEC-HAL-078 FIX: transition_layer and active_layout_transitions now mutable for binding
        // INV-TL-1: Widgets query values via handles, not references
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
        let tooltip_render = TooltipSceneInput::Managed {
            visible_target: visible_tooltip_target.as_ref(),
            pointer: last_pointer_pos,
            viewport: RectF::new(0.0, 0.0, constraints.max_width, constraints.max_height),
        };

        let mut tooltip_triggers = Vec::new();
        let mut frame_retained_cache = RetainedSceneCache::default();
        let mut frame_recycled_scene = None;
        let mut custom_render_presentation_bindings = CustomRenderPresentationBindings::default();
        let debug_outlines = app_root.context.debug_outlines_enabled();
        let scene_outcome = build_scene_reusing(
            &mut widget_tree,
            active_root,
            constraints,
            &theme,
            text_engine.as_mut(),
            &icon_registry,
            frame_time_ms,
            frame_dt,
            &mut transition_layer,
            &mut active_layout_transitions,
            pending_focus_reveal,
            scale_factor,
            None,
            &mut custom_render_presentation_bindings,
            Some(&mut app_root.context.signals),
            active_drag,
            any_pointer_captured,
            &mut tooltip_triggers,
            tooltip_render,
            focused_widget,
            keyboard_mode,
            None,
            true,
            &mut frame_recycled_scene,
            &mut frame_retained_cache,
            Some(GestureSceneAccess {
                domains: &mut interaction_domains,
                window: WindowId::PRIMARY,
                router: &mut event_router,
                fusion: &mut input_fusion,
            }),
            debug_outlines,
        );
        publish_ui_cursor(
            cursor_output.as_ref(),
            &mut event_router,
            false,
            EventRouter::relative_ingress_gate(interaction_domains.primary_state()).is_some(),
        );
        if scene_outcome.needs_next_frame {
            ticker.notify_reason(FrameReason::Animation);
            continue;
        }
        let scene = scene_outcome
            .scene
            .expect("fresh build_scene must produce a scene");

        // SPEC-HAL-260: Apply deferred focus + capture from layout phase.
        apply_deferred_focus_requests(
            &mut widget_tree,
            &mut event_router,
            interaction_domains.primary_state_mut(),
            text_engine.as_mut(),
        );
        publish_ui_cursor(
            cursor_output.as_ref(),
            &mut event_router,
            false,
            EventRouter::relative_ingress_gate(interaction_domains.primary_state()).is_some(),
        );
        // Deferred focus or layout-owned scroll changes may need another frame.
        if widget_tree.any_widget_needs_update() || widget_tree.any_scroll_needs_animation() {
            ticker.notify_reason(FrameReason::Animation);
        }

        let tooltip_hover_pending =
            publish_tooltip_triggers(&mut tooltip_manager, tooltip_triggers, last_pointer_pos);
        if !tooltip_suppressed && (tooltip_hover_pending || tooltip_manager.needs_tick()) {
            ticker.notify_reason(FrameReason::Timer);
        }

        // 11. Create and publish SceneFrame to swapchain (SPEC-HAL-045)
        // SPEC-HAL-041: Use frame_time.now for scene timestamp
        // SPEC-HAL-081B: surface_stack is MANDATORY (MultiSurfaceContract)
        let surface_snapshot = compositor.snapshot_multi();
        let frame = SceneFrame::new(frame_time.now, frame_id, scene, surface_snapshot);

        frame_id += 1;

        // SPEC-HAL-045: Publish to triple-buffer swapchain
        // Non-blocking - always succeeds, overwrites oldest slot
        scene_swapchain.publish(frame);

        // 12. Mark rendered
        // SPEC-HAL-041: Use frame_time.now consistently
        ticker.mark_rendered(frame_time.now);

        // Brief sleep to avoid busy-spinning on frames that completed quickly.
        // 1ms is short enough for smooth animation while preventing 100% CPU.
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
