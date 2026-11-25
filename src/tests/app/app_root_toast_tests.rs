    use super::*;
    use crate::core::event::{
        Event, EventPhase, FusedInputEvent, InteractionDomainState, KeyCode, Modifiers,
        PointerButton, PointerDevice, WindowId,
    };
    use crate::core::layout::{Constraints, LayoutResult};
    use crate::core::math::{Point, RectF, Size};
    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::core::widget::{
        EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
    };
    use crate::model::form_binder::FormBinder;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::three_d::{
        CameraNavigationController3, CameraNavigationMode3, CameraNavigationPolicy3, CameraState3,
        PerspectiveProjection, WorldPoint3,
    };
    use crate::utils::id::WidgetInternalId;
    use crate::widgets::switch::Switch;

    struct CapturedFlyNavigationProbe {
        internal_id: WidgetInternalId,
        navigation: CameraNavigationController3,
        capture_entries: u8,
        motion_updates: u8,
    }

    impl CapturedFlyNavigationProbe {
        fn new(mode: CameraNavigationMode3) -> Self {
            let camera = CameraState3::try_orbit(
                WorldPoint3::try_new(0.0, 0.0, -3.0).unwrap(),
                0.0,
                0.0,
                3.8,
                PerspectiveProjection::try_finite(55.0_f32.to_radians(), 0.1, 100.0)
                    .unwrap()
                    .into(),
            )
            .unwrap();
            Self {
                internal_id: WidgetInternalId::new(),
                navigation: CameraNavigationController3::try_new(
                    camera,
                    CameraNavigationPolicy3::default(),
                    mode,
                )
                .unwrap(),
                capture_entries: 0,
                motion_updates: 0,
            }
        }

        fn set_fly(&mut self, fly: bool) {
            self.navigation.set_mode(if fly {
                CameraNavigationMode3::Fly
            } else {
                CameraNavigationMode3::Orbit
            });
        }
    }

    impl WidgetDowncast for CapturedFlyNavigationProbe {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for CapturedFlyNavigationProbe {
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(240.0, 120.0))
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

        fn update(&mut self, dt: f32, _cx: &mut UpdateContext) {
            if self.navigation.advance(dt).unwrap() {
                self.motion_updates += 1;
            }
        }

        fn id(&self) -> WidgetInternalId {
            self.internal_id
        }

        fn focusable(&self) -> bool {
            true
        }

        fn capture_eligible(&self) -> bool {
            true
        }

        fn needs_update(&self) -> bool {
            self.navigation.needs_update()
        }

        fn on_capture_enter(&mut self, _cx: &mut EventContext) {
            self.capture_entries += 1;
        }

        fn handle_event(
            &mut self,
            cx: &mut EventContext,
            event: &Event,
            phase: EventPhase,
        ) -> bool {
            if phase != EventPhase::Target {
                return false;
            }
            let Event::Key(key) = event else {
                return false;
            };
            let handled = self.navigation.handle_key_event(key, cx.keyboard_mode);
            if handled {
                cx.request_continuous_frames();
            }
            handled
        }
    }

    struct BoundControlScreen {
        host: ScreenHost,
        binder: FormBinder,
        applied: Rc<Cell<bool>>,
    }

    impl BoundControlScreen {
        fn with_pending_fly_binding(applied: Rc<Cell<bool>>) -> Self {
            let mut binder = FormBinder::new();
            binder.set_bool("viewport_3d_fly", true);
            Self {
                host: ScreenHost::new(),
                binder,
                applied,
            }
        }
    }

    impl Screen for BoundControlScreen {
        fn build<'a>(
            &'a mut self,
            _cx: &'a mut ScreenFrameContext<'a>,
        ) -> &'a mut dyn crate::core::widget::Widget {
            &mut self.host
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            let mut fly = self.applied.get();
            self.binder.bind(cx, |binding| {
                binding.bool("viewport_3d_fly").sync(&mut fly);
            });
            self.applied.set(fly);
        }
    }

    #[test]
    fn sync_toast_widget_marks_existing_node_dirty_after_push() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);

        app_root.context.push_toast("first", 1000, None);
        app_root.sync_toast_widget(&mut tree);
        let toast_node = app_root
            .toast_widget_node
            .expect("toast widget should mount after first push");

        tree.node_mut(toast_node)
            .expect("mounted toast node should exist")
            .dirty_layout = false;

        app_root.context.push_toast("second", 1000, None);
        app_root.sync_toast_widget(&mut tree);

        assert!(
            tree.node(toast_node)
                .expect("mounted toast node should still exist")
                .dirty_layout,
            "existing toast widget must be re-laid out after manager queue changes"
        );
    }

    #[test]
    fn post_route_flush_applies_a_bound_showcase_control_value() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);
        let applied = Rc::new(Cell::new(false));
        app_root.push_screen(Box::new(BoundControlScreen::with_pending_fly_binding(
            Rc::clone(&applied),
        )));

        app_root.flush_pending_actions_all(&mut tree);

        assert!(applied.get());
    }

    #[test]
    fn mounted_showcase_fly_routes_key_tick_and_release_without_free_look() {
        let binder = FormBinder::new();
        let mut fly_toggle = Switch::on(true).with_label("Fly camera");
        fly_toggle.set_binding("viewport_3d_fly", binder.writer());

        let theme = Theme::dark();
        let mut layout_tree = WidgetTree::new();
        let mut layout_text_engine = FontManagerTextEngine::new();
        let toggle_layout = {
            let mut cx = LayoutContext::new(&mut layout_tree, &mut layout_text_engine, &theme);
            fly_toggle.layout(&mut cx, Constraints::unbounded(), &[])
        };

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let toggle_node = tree.add_child(root, Box::new(fly_toggle), None, 0);
        let viewport_node = tree.add_child(
            root,
            Box::new(CapturedFlyNavigationProbe::new(CameraNavigationMode3::Fly)),
            None,
            1,
        );
        {
            let _layout = LayoutAuthorityGuard::enter();
            tree.set_rect(root, RectF::new(0.0, 0.0, 320.0, 220.0));
            tree.set_rect(
                toggle_node,
                RectF::new(
                    0.0,
                    0.0,
                    toggle_layout.size.width,
                    toggle_layout.size.height,
                ),
            );
            tree.set_rect(viewport_node, RectF::new(0.0, 80.0, 240.0, 120.0));
            compute_absolute_positions(&mut tree, root, Point::zero());
            compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }

        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        let mut text_engine = FontManagerTextEngine::new();
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        tree.defer_focus_sequence([toggle_node, viewport_node]);
        apply_deferred_focus_requests(&mut tree, &mut router, &mut domain_state, &mut text_engine);

        let viewport_id = tree.widget_id_for_node(viewport_node).unwrap();
        assert_eq!(domain_state.focus_manager.focused(), Some(viewport_id));
        assert_eq!(
            domain_state.keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        assert_eq!(
            tree.node(viewport_node)
                .and_then(|node| node
                    .widget
                    .as_any()
                    .downcast_ref::<CapturedFlyNavigationProbe>())
                .unwrap()
                .capture_entries,
            1,
            "the mounted viewport must acknowledge router-owned capture entry"
        );
        router.reset_continuous_frames_request();

        for key_code in [
            KeyCode::Character('w'),
            KeyCode::Character('a'),
            KeyCode::Character('s'),
            KeyCode::Character('d'),
        ] {
            router.route_input_event(
                &mut tree,
                FusedInputEvent::KeyDown {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code,
                    physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                    reservation: None,
                    modifiers: Modifiers::default(),
                    repeat: false,
                },
                Some(root),
                &mut text_engine,
                None,
                &mut domain_state,
                &mut bindings,
            );
            assert!(
                router.continuous_frames_requested(),
                "routed Fly key-down must request the normal update cadence"
            );
            let mut transition_layer = TransitionLayer::new();
            {
                let _update = UpdateAuthorityGuard::enter();
                pre_layout_update_widgets(
                    &mut tree,
                    root,
                    1.0 / 60.0,
                    &mut transition_layer,
                    &theme,
                    None,
                    None,
                );
            }
            let moved = tree
                .node(viewport_node)
                .and_then(|node| {
                    node.widget
                        .as_any()
                        .downcast_ref::<CapturedFlyNavigationProbe>()
                })
                .unwrap()
                .motion_updates;
            assert!(
                moved > 0,
                "routed Fly key {key_code:?} must move on the normal update tick before any free-look gesture"
            );
            router.route_input_event(
                &mut tree,
                FusedInputEvent::KeyUp {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code,
                    physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                    reservation: None,
                    modifiers: Modifiers::default(),
                },
                Some(root),
                &mut text_engine,
                None,
                &mut domain_state,
                &mut bindings,
            );
            for _ in 0..30 {
                let _update = UpdateAuthorityGuard::enter();
                pre_layout_update_widgets(
                    &mut tree,
                    root,
                    1.0 / 60.0,
                    &mut transition_layer,
                    &theme,
                    None,
                    None,
                );
            }
            assert!(
                !tree
                    .node(viewport_node)
                    .and_then(|node| node
                        .widget
                        .as_any()
                        .downcast_ref::<CapturedFlyNavigationProbe>())
                    .unwrap()
                    .navigation
                    .needs_update(),
                "routed Fly key-up must decelerate to a finite stopped state"
            );
        }
    }

    #[test]
    fn routed_showcase_fly_toggle_routes_key_and_tick_without_free_look() {
        let mut binder = FormBinder::new();
        let mut fly_toggle = Switch::on(false).with_label("Fly camera");
        fly_toggle.set_binding("viewport_3d_fly", binder.writer());

        let theme = Theme::dark();
        let mut layout_tree = WidgetTree::new();
        let mut layout_text_engine = FontManagerTextEngine::new();
        let toggle_layout = {
            let mut cx = LayoutContext::new(&mut layout_tree, &mut layout_text_engine, &theme);
            fly_toggle.layout(&mut cx, Constraints::unbounded(), &[])
        };

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let toggle_node = tree.add_child(root, Box::new(fly_toggle), None, 0);
        let viewport_node = tree.add_child(
            root,
            Box::new(CapturedFlyNavigationProbe::new(
                CameraNavigationMode3::Orbit,
            )),
            None,
            1,
        );
        {
            let _layout = LayoutAuthorityGuard::enter();
            tree.set_rect(root, RectF::new(0.0, 0.0, 320.0, 220.0));
            tree.set_rect(
                toggle_node,
                RectF::new(
                    0.0,
                    0.0,
                    toggle_layout.size.width,
                    toggle_layout.size.height,
                ),
            );
            tree.set_rect(viewport_node, RectF::new(0.0, 80.0, 240.0, 120.0));
            compute_absolute_positions(&mut tree, root, Point::zero());
            compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }

        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        let mut text_engine = FontManagerTextEngine::new();
        let mut bindings = crate::runtime::binding_registry::BindingRegistry::new();
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(1.0, 1.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            Some(root),
            &mut text_engine,
            None,
            &mut domain_state,
            &mut bindings,
        );

        let mut app = AppContext::new(Theme::dark());
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        let binding_cx = ScreenContext {
            composite_editors: None,
            app: &mut app,
            screens: &mut screens,
            modals: &mut modals,
            tree: None,
        };
        let mut fly = false;
        binder.bind(&binding_cx, |binding| {
            binding.bool("viewport_3d_fly").sync(&mut fly);
        });
        assert!(
            fly,
            "the routed Fly toggle must update its bound control state"
        );

        tree.node_mut(viewport_node)
            .and_then(|node| {
                node.widget
                    .as_any_mut()
                    .downcast_mut::<CapturedFlyNavigationProbe>()
            })
            .unwrap()
            .set_fly(fly);
        tree.defer_focus_sequence([toggle_node, viewport_node]);
        apply_deferred_focus_requests(&mut tree, &mut router, &mut domain_state, &mut text_engine);
        router.reset_continuous_frames_request();

        router.route_input_event(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Character('w'),
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            Some(root),
            &mut text_engine,
            None,
            &mut domain_state,
            &mut bindings,
        );
        assert!(
            router.continuous_frames_requested(),
            "the routed Fly toggle key-down must request the normal update cadence"
        );
        let mut transition_layer = TransitionLayer::new();
        {
            let _update = UpdateAuthorityGuard::enter();
            pre_layout_update_widgets(
                &mut tree,
                root,
                1.0 / 60.0,
                &mut transition_layer,
                &theme,
                None,
                None,
            );
        }
        let probe = tree
            .node(viewport_node)
            .and_then(|node| {
                node.widget
                    .as_any()
                    .downcast_ref::<CapturedFlyNavigationProbe>()
            })
            .unwrap();
        assert_eq!(probe.capture_entries, 1);
        assert!(
            probe.motion_updates > 0,
            "a routed Fly-toggle must permit normal key routing and update motion before any free-look gesture"
        );
    }
