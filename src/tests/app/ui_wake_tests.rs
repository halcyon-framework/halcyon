    use super::*;

    #[test]
    fn stale_projection_withdrawal_repaints_only_its_retained_local_fragment() {
        use crate::core::layout::{Alignment, EdgeInsets, FlexParams};
        use crate::widgets::{button::Button, fixed_spacer::FixedSpacer, flex::Flex};

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()
            .with_align_items(Alignment::Stretch).with_padding(EdgeInsets::all(8.0))), None, 0);
        let spacer = tree.add_child(root, Box::new(FixedSpacer::vertical(1.0)), None, 0);
        tree.set_flex(spacer, FlexParams::fixed(144.0));
        let button = tree.add_child(root, Box::new(Button::with_text("Action").with_enabled(false)), None, 0);
        tree.set_flex(button, FlexParams::fixed(32.0));
        let owner = tree.node(button).unwrap().id;
        let root_owner = tree.node(root).unwrap().id;
        let theme = Theme::default();
        let icons = crate::core::icons::builtin_registry();
        let mut text = crate::platform::font_manager_text_engine::FontManagerTextEngine::new();
        let mut h = RetainedSceneTestHarness::new();
        h.advance_transition_time(0.0);
        tree.begin_layout_transition_frame();
        h.frame(&mut tree, root, Constraints::tight(Size::new(640.0, 480.0)), &theme, &mut text, &icons);
        tree.set_flex(spacer, FlexParams::fixed(216.0));
        h.advance_transition_time(0.001);
        tree.begin_layout_transition_frame();
        h.frame(&mut tree, root, Constraints::tight(Size::new(640.0, 480.0)), &theme, &mut text, &icons);
        let original_handle = h.active_layout_transitions.get(owner).unwrap().handle;
        assert!(h.cache.interpolated_rects.contains_key(&owner));
        let before_button = h.local_fragment(owner).unwrap();
        let before_root = h.cache.entries[&root_owner].composed.clone();

        h.advance_transition_time(0.02);
        tree.begin_layout_transition_frame();
        let changed = h.frame(&mut tree, root, Constraints::tight(Size::new(480.0, 400.0)), &theme, &mut text, &icons);
        assert!(changed.scene_built && changed.visual_changed);
        assert_eq!(h.active_layout_transitions.get(owner).unwrap().handle, original_handle);
        assert!(!h.cache.interpolated_rects.contains_key(&owner));
        assert_eq!(tree.get_absolute_paint_rect(button), Some(RectF::new(8.0, 224.0, 464.0, 32.0)));
        let current_button = h.local_fragment(owner).unwrap();
        assert!(!Arc::ptr_eq(&before_button, &current_button));
        assert!(!Arc::ptr_eq(&before_root, &h.cache.entries[&root_owner].composed));

        h.advance_transition_time(0.04);
        tree.begin_layout_transition_frame();
        h.frame(&mut tree, root, Constraints::tight(Size::new(480.0, 400.0)), &theme, &mut text, &icons);
        assert!(!h.cache.interpolated_rects.contains_key(&owner));
        assert!(Arc::ptr_eq(&current_button, &h.local_fragment(owner).unwrap()));
        assert!(h.active_layout_transitions.get(owner).is_none());
        assert_eq!(h.transition_layer.timeline().sample::<f32>(original_handle),
            crate::core::animation::AnimationSample::Cancelled);

        h.advance_transition_time(0.06);
        tree.begin_layout_transition_frame();
        h.frame(&mut tree, root, Constraints::tight(Size::new(640.0, 480.0)), &theme, &mut text, &icons);
        assert_eq!(h.transition_layer.timeline().sample::<f32>(original_handle),
            crate::core::animation::AnimationSample::Missing);
        assert!(h.active_layout_transitions.get(owner).is_none());
        assert!(!h.cache.interpolated_rects.contains_key(&owner));
        assert_eq!(tree.get_absolute_paint_rect(button), Some(RectF::new(8.0, 224.0, 624.0, 32.0)));
    }

    #[test]
    fn postbuild_timeline_activity_drives_layout_progress_and_settles_retained_output() {
        use crate::widgets::{fixed_spacer::FixedSpacer, flex::Flex, label::Label};

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::row()), None, 0);
        let child = tree.add_child(root, Box::new(FixedSpacer::new(40.0, 40.0)), None, 0);
        tree.add_child(root, Box::new(Label::new("follows layout")), None, 0);
        let owner = tree.node(child).unwrap().id;
        let theme = Theme::default();
        let icons = crate::core::icons::builtin_registry();
        let mut text = crate::platform::font_manager_text_engine::FontManagerTextEngine::new();
        let mut retained = RetainedSceneTestHarness::new();
        let constraints = Constraints::tight(Size::new(320.0, 100.0));
        retained.advance_transition_time(0.0);
        tree.begin_layout_transition_frame();
        retained.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let before = tree.get_absolute_paint_rect(child).unwrap();
        assert!(!retained.transition_layer.has_active_animations());

        let start = Instant::now();
        let mut pacer = ContinuousFramePacer::new(Some(60_000));
        assert_eq!(pacer.deadline(start, false), None);

        // The early activity query above precedes this real Layout/Stage2 bind.
        // No update-only receiver, scroll, timer or widget animation drives it.
        tree.replace_widget_only(child, Box::new(FixedSpacer::new(120.0, 40.0)));
        tree.begin_layout_transition_frame();
        retained.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let target = tree.get_absolute_paint_rect(child).unwrap();
        let state = *retained.active_layout_transitions.get(owner).unwrap();
        assert_eq!(state.from, before);
        assert_eq!(state.to, target);
        assert_ne!(before, target);
        assert!(!tree.any_widget_needs_update());
        assert!(!tree.any_scroll_needs_animation());
        assert!(retained.transition_layer.has_active_animations());
        assert_eq!(retained.interpolated_rect(owner), Some(before));

        let next = pacer.deadline(start, retained.transition_layer.has_active_animations()).unwrap();
        assert!(pacer.take_due(next, retained.transition_layer.has_active_animations()));
        retained.advance_transition_time(next.duration_since(start).as_secs_f64());
        retained.active_layout_transitions.cleanup_completed(&retained.transition_layer);
        tree.begin_layout_transition_frame();
        retained.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let progressing = retained.interpolated_rect(owner).unwrap();
        assert!(progressing.width > before.width && progressing.width < target.width);
        assert_eq!(retained.active_layout_transitions.get(owner).unwrap().handle, state.handle);

        // Completion may leave one already-latched CPU frame. It must not
        // manufacture another scene/output when the projection is unchanged.
        retained.advance_transition_time(0.5);
        retained.active_layout_transitions.cleanup_completed(&retained.transition_layer);
        tree.begin_layout_transition_frame();
        retained.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!retained.transition_layer.has_active_animations());
        assert_eq!(retained.interpolated_rect(owner), None);
        retained.advance_transition_time(0.6);
        tree.begin_layout_transition_frame();
        let quiet = retained.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!quiet.scene_built && !quiet.visual_changed);
        assert_eq!(pacer.deadline(start + Duration::from_millis(600), false), None);
        assert!(!pacer.take_due(start + Duration::from_millis(600), false));
    }

    #[test]
    fn native_paint_settlement_is_not_lost_at_the_idle_gate() {
        use crate::widgets::{flex::Flex, label::Label};

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let child = tree.add_child(root, Box::new(Label::new("retained")), None, 0);
        let theme = Theme::default();
        let icons = crate::core::icons::builtin_registry();
        let mut text = crate::platform::font_manager_text_engine::FontManagerTextEngine::new();
        let mut retained = RetainedSceneTestHarness::new();
        let constraints = Constraints::tight(Size::new(240.0, 100.0));
        retained.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!tree.node(root).unwrap().dirty_composition);
        let geometry = tree.get_absolute_paint_rect(child).unwrap();

        let start = Instant::now();
        let mut ticker = EngineTicker::new(start);
        for elapsed in [0, 34, 68] {
            let frame = ticker.begin_frame_at(start + Duration::from_millis(elapsed));
            ticker.mark_rendered(frame.now);
        }
        let frame = ticker.begin_frame_at(start + Duration::from_millis(69));
        assert_eq!(frame.reason, FrameReason::Tick);
        assert!(ticker.is_idle() && ticker.should_throttle(frame.now));
        assert!(should_throttle_native_frame(
            &ticker, frame.now, &tree, root
        ));

        // Real paint-only invalidation as settled by Screen flush, after the
        // current event's frame time was captured. There is no second wake.
        for _ in 0..3 {
            tree.mark_paint_dirty(child);
        }
        assert!(!tree.any_widget_needs_update());
        assert!(!tree.node(root).unwrap().dirty_layout);
        assert!(!tree.node(child).unwrap().dirty_layout);
        assert!(tree.node(child).unwrap().dirty_paint);
        assert!(tree.node(root).unwrap().dirty_composition);
        assert!(ticker.should_throttle(frame.now));
        assert!(
            !should_throttle_native_frame(&ticker, frame.now, &tree, root),
            "settled retained paint must reach the current AppFrame without another UserEvent"
        );

        let painted = retained.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(painted.scene_built);
        assert!(!tree.node(root).unwrap().dirty_composition);
        assert!(!tree.node(child).unwrap().dirty_paint);
        assert_eq!(tree.get_absolute_paint_rect(child).unwrap(), geometry);
        // The admission predicate queues nothing and does not change ticker
        // activity/reasons. A clean unprompted Tick keeps the original policy.
        assert!(ticker.is_idle());
        assert_eq!(ticker.pending_reason, FrameReason::Tick);
        assert!(should_throttle_native_frame(
            &ticker, frame.now, &tree, root
        ));
        assert!(!ticker.should_throttle(start + Duration::from_millis(101)));
    }

    #[test]
    fn native_paint_settlement_gate_follows_screen_flush() {
        let source = include_str!("../../core/app/run.rs");
        let gate = source.rfind("if should_throttle_native_frame(").unwrap();
        let flush = source[..gate]
            .rfind("app_root.flush_pending_actions_with_composite_editors(")
            .unwrap();
        assert!(flush < gate);
        assert!(source[gate..].starts_with(
            "if should_throttle_native_frame(\n                            &ticker,\n                            frame_time.now,\n                            &widget_tree,\n                            app_root.active_root_node(),\n                        )"
        ));
    }

    #[test]
    fn native_ui_wake_actual_user_event_owner_notifies_before_about_to_wait() {
        // Wiring proof for the actual native arm, alongside the exercised
        // ticker and real App::run regression. This grants no scheduler policy.
        let source = include_str!("../../core/app/run.rs");
        let arm = source
            .rsplit_once("\n                    WinitEvent::UserEvent(()) => {")
            .unwrap()
            .1
            .split("WinitEvent::AboutToWait => {")
            .next()
            .unwrap();
        assert!(arm.contains("pending_render = true;"));
        assert!(
            arm.contains("ticker.notify_reason(FrameReason::Forced);"),
            "native wake must retain its explicit ticker reason before the one-shot redraw can be throttled"
        );
        assert!(
            !arm.contains("request_redraw"),
            "existing AboutToWait owns coalescing"
        );
    }

    #[test]
    fn hal373_ui_waker_manual_contexts_have_no_native_sender() {
        assert!(AppContext::new(Theme::default()).ui_waker().is_none());
        let context = AppContext::new_with_flat_viewport3d_transfer_capacity(
            Theme::default(),
            std::num::NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        assert!(context.ui_waker().is_none());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let app = AppRoot::new(context, root);
        assert!(app.context.ui_waker().is_none());
    }

    #[test]
    fn hal373_ui_waker_is_send_sync_clone_without_ui_context() {
        fn send_sync_clone<T: Send + Sync + Clone>() {}
        send_sync_clone::<UiWakeHandle>();
        send_sync_clone::<UiWakeError>();
    }
