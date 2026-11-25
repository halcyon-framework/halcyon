    use super::*;
    use crate::core::event::{
        CursorIcon, InteractionDomainState, KeyCode, Modifiers, PointerButton, PointerDevice,
        PointerId,
    };
    use crate::core::layout::LayoutResult;
    use crate::core::widget::{EventContext, Widget, WidgetDowncast};
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::utils::id::WidgetInternalId;
    use web_time::Instant;

    struct BrowserInputOrderScreen {
        host: ScreenHost,
        nodes: Rc<Cell<Option<[NodeId; 3]>>>,
    }

    impl Screen for BrowserInputOrderScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            use crate::widgets::{button::Button, flex::Flex, text_field::TextField};
            let column = cx.mount_child(Box::new(Flex::column().with_spacing(8.0)));
            self.nodes.set(Some([
                cx.tree
                    .add_child(column, Box::new(TextField::new().with_text("ab")), None, 0),
                cx.tree.add_child(
                    column,
                    Box::new(TextField::new().with_text("next")),
                    None,
                    0,
                ),
                cx.tree
                    .add_child(column, Box::new(Button::with_text("Availability")), None, 0),
            ]));
            &mut self.host
        }

        fn handle_input(&mut self, _: &FusedInputEvent, _: &mut ScreenContext<'_>) -> bool {
            false
        }
    }

    struct BrowserInputOrderFixture {
        app: AppRoot,
        tree: WidgetTree,
        root: NodeId,
        nodes: [NodeId; 3],
        router: EventRouter,
        domains: InteractionDomains,
        fusion: InputFusion,
        text: FontManagerTextEngine,
        timeline: TransitionLayer,
        transitions: ActiveLayoutTransitions,
        tooltip: crate::model::tooltip::TooltipManager,
        pointer: Point,
        ticker: EngineTicker,
        browser: AppBrowserEditor,
        fused: Vec<SessionQualifiedInput>,
        deferred: VecDeque<DeferredAppInput>,
    }

    impl BrowserInputOrderFixture {
        fn new() -> Self {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
            let theme = Theme::dark();
            let nodes = Rc::new(Cell::new(None));
            let mut app = AppRoot::new(AppContext::new(theme.clone()), root);
            app.push_screen(Box::new(BrowserInputOrderScreen {
                host: ScreenHost::new(),
                nodes: nodes.clone(),
            }));
            app.rebuild_active_view(&mut tree);
            let mut text = FontManagerTextEngine::new_with_system_ui();
            let mut ticker = EngineTicker::new(Instant::now());
            ticker.begin_frame();
            {
                let _layout = LayoutAuthorityGuard::enter();
                layout_tree(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(320.0, 160.0)),
                    &theme,
                    &mut text,
                    None,
                );
            }
            let mut fixture = Self {
                app,
                tree,
                root,
                nodes: nodes.get().unwrap(),
                router: EventRouter::new(),
                domains: InteractionDomains::new(),
                fusion: InputFusion::new(),
                text,
                timeline: TransitionLayer::new(),
                transitions: ActiveLayoutTransitions::new(),
                tooltip: crate::model::tooltip::TooltipManager::new(Default::default()),
                pointer: Point::zero(),
                ticker,
                browser: AppBrowserEditor::default(),
                fused: Vec::new(),
                deferred: VecDeque::new(),
            };
            fixture.capture(0);
            fixture
        }

        fn capture(&mut self, index: usize) {
            let state = self.domains.primary_state_mut();
            let owner = self.tree.widget_id_for_node(self.nodes[index]).unwrap();
            self.router
                .request_focus_for_widget(&mut self.tree, owner, state);
            state
                .focus_manager
                .rebuild_focus_chain(&self.tree, self.root);
            if state.keyboard_mode != KeyboardAuthorityMode::Capture {
                assert!(self.router.handle_engine_action(
                    &mut self.tree,
                    crate::runtime::actions::ActionId::ENTER_WIDGET,
                    Some(self.root),
                    state,
                    &mut self.text,
                    None,
                ));
            }
            let (target, projection) =
                crate::core::event_router::browser_text::BrowserTextTarget::qualify(
                    &self.tree,
                    &self.domains,
                    WindowId::PRIMARY,
                    self.app.browser_paste_root().unwrap(),
                )
                .unwrap();
            assert_eq!(target.owner(), owner);
            self.browser.session = Some(BrowserTextSession::new(target, projection));
        }

        fn field(&self, index: usize) -> &crate::widgets::text_field::TextField {
            self.tree
                .node(self.nodes[index])
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }

        fn delivery<T>(
            &mut self,
            run: impl FnOnce(
                &mut AppInputDelivery<'_>,
                &mut Vec<SessionQualifiedInput>,
                &mut VecDeque<DeferredAppInput>,
            ) -> T,
        ) -> T {
            run(
                &mut AppInputDelivery {
                    app_root: &mut self.app,
                    widget_tree: &mut self.tree,
                    event_router: &mut self.router,
                    interaction_domains: &mut self.domains,
                    input_fusion: &mut self.fusion,
                    text_engine: &mut self.text,
                    transition_layer: &mut self.timeline,
                    active_layout_transitions: &self.transitions,
                    tooltip_manager: &mut self.tooltip,
                    last_pointer_pos: &mut self.pointer,
                    window: None,
                    viewport: RectF::new(0.0, 0.0, 320.0, 160.0),
                    browser: &mut self.browser,
                    #[cfg(all(target_os = "macos", debug_assertions))]
                    engine_conformance: None,
                },
                &mut self.fused,
                &mut self.deferred,
            )
        }

        fn receive(&mut self, report: BrowserTextReport) -> AppInputDrain {
            self.delivery(|delivery, fused, deferred| {
                delivery.receive_browser_input(report, fused, deferred, false, &mut false)
            })
        }

        fn frame(&mut self) -> AppInputDrain {
            let frame = self.ticker.begin_frame();
            fuse_deferred_raw_prefix(
                &mut self.deferred,
                &mut self.fused,
                &mut self.fusion,
                &self.tree,
                &mut self.domains,
            );
            self.fusion.process_frame_with_gesture_sessions(
                &frame,
                &mut self.ticker,
                &mut self.fused,
            );
            self.delivery(|delivery, fused, deferred| {
                delivery.drain_frame_input(fused, deferred, false, &mut false)
            })
        }

        fn assert_editor(&self, value: &str, anchor: usize, head: usize) {
            let field = self.field(0);
            assert_eq!(field.text(), value);
            assert_eq!(
                (field.selection().anchor, field.selection().head),
                (anchor, head)
            );
            assert_eq!(field.cursor_pos(), head);
            let projection = &self.browser.session.as_ref().unwrap().projection;
            assert_eq!(projection.value, value);
            assert_eq!(projection.selection, (anchor as u32, head as u32));
        }
    }

    fn browser_physical_key(
        logical: KeyCode,
        physical: crate::core::event::PhysicalKeyCode,
        modifiers: Modifiers,
    ) -> BrowserTextReport {
        BrowserTextReport::Key {
            key: physical_raw_key(logical, physical, modifiers, false, None),
            shortcut_text: None,
        }
    }

    fn browser_insert_report(value: &str) -> BrowserTextReport {
        BrowserTextReport::Input {
            value: value.into(),
            kind: "insertText".into(),
            composing: false,
            selection: None,
        }
    }

    #[test]
    fn hal398_app_settles_caret_before_dom_insert_and_does_not_replay() {
        use crate::core::event::PhysicalKeyCode;
        let mut f = BrowserInputOrderFixture::new();
        f.assert_editor("ab", 2, 2);
        let position = Point::new(270.0, 145.0);
        f.fusion.process_raw_with_gesture_sessions(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                device: PointerDevice::Mouse,
                position,
                delta: (12.0, 4.0),
            },
            crate::core::event::InteractionDomainId::PRIMARY,
            &mut f.domains.primary_state_mut().gesture_registry,
            &f.tree,
            None,
            &mut f.fused,
        );
        assert!(f.fused.is_empty(), "the earlier move is still coalesced");
        assert_eq!(
            f.receive(browser_physical_key(
                KeyCode::Left,
                PhysicalKeyCode::ArrowLeft,
                Modifiers::default()
            )),
            AppInputDrain::Drained
        );
        assert_eq!(f.pointer, position);
        f.assert_editor("ab", 1, 1);
        assert_eq!(
            f.receive(BrowserTextReport::BeforeInput),
            AppInputDrain::Drained
        );
        assert_eq!(
            f.receive(browser_insert_report("axb")),
            AppInputDrain::Drained
        );
        f.assert_editor("axb", 2, 2);
        assert!(f.fused.is_empty() && f.deferred.is_empty());
        assert_eq!(f.frame(), AppInputDrain::Drained);
        f.assert_editor("axb", 2, 2);
    }

    #[test]
    fn hal398_app_projects_shift_selection_before_dom_replacement() {
        use crate::core::event::PhysicalKeyCode;
        let mut f = BrowserInputOrderFixture::new();
        assert_eq!(
            f.receive(browser_physical_key(
                KeyCode::Left,
                PhysicalKeyCode::ArrowLeft,
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                }
            )),
            AppInputDrain::Drained
        );
        f.assert_editor("ab", 2, 1);
        assert_eq!(
            f.receive(BrowserTextReport::BeforeInput),
            AppInputDrain::Drained
        );
        assert_eq!(
            f.receive(browser_insert_report("ax")),
            AppInputDrain::Drained
        );
        f.assert_editor("ax", 2, 2);
        assert_eq!(f.frame(), AppInputDrain::Drained);
        f.assert_editor("ax", 2, 2);
    }

    #[test]
    fn hal398_app_escape_and_tab_retire_the_original_editor_lease() {
        use crate::core::event::PhysicalKeyCode;
        for (logical, physical) in [
            (KeyCode::Escape, PhysicalKeyCode::Escape),
            (KeyCode::Tab, PhysicalKeyCode::Tab),
        ] {
            let mut f = BrowserInputOrderFixture::new();
            let source = f.browser.session.as_ref().unwrap().source();
            assert_eq!(
                f.receive(browser_physical_key(
                    logical,
                    physical,
                    Modifiers::default()
                )),
                AppInputDrain::Drained
            );
            assert!(f.browser.session.is_none());
            f.capture(1);
            f.deferred.push_back(DeferredAppInput::Browser {
                source,
                report: browser_insert_report("abx"),
            });
            assert_eq!(f.frame(), AppInputDrain::Drained);
            assert_eq!(f.field(0).text(), "ab");
            assert_eq!(f.field(1).text(), "next");
            assert_eq!(f.browser.session.as_ref().unwrap().projection.value, "next");
        }
    }

    #[test]
    fn hal398_app_availability_cutoff_keeps_later_reports_in_order() {
        use crate::core::event::PhysicalKeyCode;
        let mut f = BrowserInputOrderFixture::new();
        f.tree.request_widget_enabled(f.nodes[2], false).unwrap();
        assert_eq!(
            f.receive(BrowserTextReport::BeforeInput),
            AppInputDrain::AvailabilityPending
        );
        assert_eq!(
            f.receive(browser_insert_report("abx")),
            AppInputDrain::AvailabilityPending
        );
        assert_eq!(
            f.receive(browser_physical_key(
                KeyCode::Left,
                PhysicalKeyCode::ArrowLeft,
                Modifiers::default()
            )),
            AppInputDrain::AvailabilityPending
        );
        f.assert_editor("ab", 2, 2);
        assert_eq!(f.deferred.len(), 3);
        crate::core::widget_availability::apply_pending_widget_availability(
            &mut f.tree,
            &mut f.router,
            &mut f.fusion,
            f.domains.primary_state_mut(),
            &mut f.app.context.registry.binding_registry,
            &mut f.text,
        );
        assert_eq!(f.frame(), AppInputDrain::Drained);
        f.assert_editor("abx", 2, 2);
        assert!(f.fused.is_empty() && f.deferred.is_empty());
    }

    #[test]
    fn hal398_app_proxy_paste_respects_deferred_key_order() {
        use crate::core::event::PhysicalKeyCode;
        use crate::core::event_router::browser_paste::BrowserPasteTarget;

        let queue_paste = |f: &mut BrowserInputOrderFixture| {
            let target = BrowserPasteTarget::qualify(
                &f.tree,
                &f.domains,
                WindowId::PRIMARY,
                f.app.browser_paste_root().unwrap(),
            )
            .unwrap();
            f.browser.pending_paste = Some(target.with_text("x".into()).unwrap());
            f.browser.record_proxy_paste(&mut f.deferred);
        };
        let left = || {
            browser_physical_key(
                KeyCode::Left,
                PhysicalKeyCode::ArrowLeft,
                Modifiers::default(),
            )
        };

        let mut f = BrowserInputOrderFixture::new();
        f.browser.geometry_pending = true;
        assert_eq!(f.receive(left()), AppInputDrain::GeometryPending);
        queue_paste(&mut f);
        f.assert_editor("ab", 2, 2);

        f.browser.geometry_pending = false;
        assert_eq!(f.frame(), AppInputDrain::Drained);
        // The earlier key changes the paste's snapshotted caret. The existing
        // exact-source check must refuse it instead of applying it at either caret.
        f.assert_editor("ab", 1, 1);
        assert!(f.browser.pending_paste.is_none());

        queue_paste(&mut f);
        assert_eq!(f.receive(left()), AppInputDrain::Drained);
        f.assert_editor("axb", 1, 1);
        assert_eq!(f.frame(), AppInputDrain::Drained);
        f.assert_editor("axb", 1, 1);
    }

    struct ActivatingWidget {
        id: WidgetInternalId,
        activations: Rc<Cell<u32>>,
    }

    impl ActivatingWidget {
        fn new(activations: Rc<Cell<u32>>) -> Self {
            Self {
                id: WidgetInternalId::new(),
                activations,
            }
        }
    }

    impl WidgetDowncast for ActivatingWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for ActivatingWidget {
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(80.0, 24.0))
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

        fn id(&self) -> WidgetInternalId {
            self.id
        }

        fn focusable(&self) -> bool {
            true
        }

        fn accepts_keyboard_activation(&self) -> bool {
            true
        }

        fn on_keyboard_activate(&mut self, _cx: &mut EventContext) {
            self.activations.set(self.activations.get() + 1);
        }
    }

    #[derive(Default)]
    struct LifecycleProbe {
        capture_enters: Cell<u32>,
        capture_leaves: Cell<u32>,
        blurs: Cell<u32>,
    }

    struct LifecycleProbeWidget {
        id: WidgetInternalId,
        probe: Rc<LifecycleProbe>,
        capture_eligible: bool,
        releases_pointer_on_capture_leave: bool,
    }

    impl LifecycleProbeWidget {
        fn new(
            probe: Rc<LifecycleProbe>,
            capture_eligible: bool,
            releases_pointer_on_capture_leave: bool,
        ) -> Self {
            Self {
                id: WidgetInternalId::new(),
                probe,
                capture_eligible,
                releases_pointer_on_capture_leave,
            }
        }
    }

    impl WidgetDowncast for LifecycleProbeWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for LifecycleProbeWidget {
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(80.0, 24.0))
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

        fn id(&self) -> WidgetInternalId {
            self.id
        }

        fn focusable(&self) -> bool {
            true
        }

        fn capture_eligible(&self) -> bool {
            self.capture_eligible
        }

        fn on_capture_enter(&mut self, _cx: &mut EventContext) {
            self.probe
                .capture_enters
                .set(self.probe.capture_enters.get() + 1);
        }

        fn on_capture_leave(&mut self, cx: &mut EventContext) {
            self.probe
                .capture_leaves
                .set(self.probe.capture_leaves.get() + 1);
            if self.releases_pointer_on_capture_leave {
                cx.release_pointer();
                cx.request_cursor(CursorIcon::Default);
            }
        }

        fn on_blur(&mut self, _cx: &mut EventContext) {
            self.probe.blurs.set(self.probe.blurs.get() + 1);
        }
    }

    struct CountingScreen {
        handled_count: Rc<Cell<u32>>,
        host: ScreenHost,
    }

    impl CountingScreen {
        fn new(handled_count: Rc<Cell<u32>>) -> Self {
            Self {
                handled_count,
                host: ScreenHost::new(),
            }
        }
    }

    impl Screen for CountingScreen {
        fn build<'a>(
            &'a mut self,
            _cx: &'a mut ScreenFrameContext<'a>,
        ) -> &'a mut dyn crate::core::widget::Widget {
            &mut self.host
        }

        fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            self.handled_count.set(self.handled_count.get() + 1);
            true
        }
    }

    #[test]
    fn browser_paste_app_root_refuses_pending_screen_retirement_and_exit() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app = AppRoot::new(AppContext::new(Theme::default()), root);
        assert_eq!(app.browser_paste_root(), Some(root));
        app.push_screen(Box::new(CountingScreen::new(Rc::new(Cell::new(0)))));
        assert_eq!(
            app.browser_paste_root(),
            None,
            "unbuilt Screen is not the old mounted owner"
        );
        let mut app = AppRoot::new(AppContext::new(Theme::default()), root);
        app.pending_widget_teardowns.push(root);
        assert_eq!(app.browser_paste_root(), None);
        let mut app = AppRoot::new(AppContext::new(Theme::default()), root);
        app.context.request_exit();
        assert_eq!(app.browser_paste_root(), None);
    }

    struct PopupBoundaryScreen {
        host: ScreenHost,
    }

    impl PopupBoundaryScreen {
        fn new() -> Self {
            Self {
                host: ScreenHost::new(),
            }
        }
    }

    impl Screen for PopupBoundaryScreen {
        fn build<'a>(
            &'a mut self,
            _cx: &'a mut ScreenFrameContext<'a>,
        ) -> &'a mut dyn crate::core::widget::Widget {
            &mut self.host
        }

        fn dismiss_policy(&self) -> DismissPolicy {
            DismissPolicy::popup_default()
        }

        fn covers_point(&self, logical_pos: Point) -> bool {
            RectF::new(0.0, 0.0, 50.0, 50.0).contains(logical_pos)
        }
    }

    struct MountedModalScreen {
        placeholder: ScreenHost,
    }

    impl MountedModalScreen {
        fn new() -> Self {
            Self {
                placeholder: ScreenHost::new(),
            }
        }
    }

    struct ScopeModalRoot {
        id: WidgetInternalId,
    }

    impl ScopeModalRoot {
        fn new() -> Self {
            Self {
                id: WidgetInternalId::new(),
            }
        }
    }

    impl WidgetDowncast for ScopeModalRoot {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for ScopeModalRoot {
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(180.0, 64.0))
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

        fn id(&self) -> WidgetInternalId {
            self.id
        }

        fn is_focus_scope(&self) -> bool {
            true
        }
    }

    struct FocusableModalScreen {
        placeholder: ScreenHost,
        focus_target_count: Rc<Cell<u32>>,
    }

    impl FocusableModalScreen {
        fn new(focus_target_count: Rc<Cell<u32>>) -> Self {
            Self {
                placeholder: ScreenHost::new(),
                focus_target_count,
            }
        }
    }

    impl Screen for FocusableModalScreen {
        fn build<'a>(
            &'a mut self,
            cx: &'a mut ScreenFrameContext<'a>,
        ) -> &'a mut dyn crate::core::widget::Widget {
            let scope_root = cx.mount_child(Box::new(ScopeModalRoot::new()));
            cx.tree.add_child(
                scope_root,
                Box::new(ActivatingWidget::new(Rc::clone(&self.focus_target_count))),
                None,
                0,
            );
            &mut self.placeholder
        }

        fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            false
        }

        fn covers_point(&self, _logical_pos: Point) -> bool {
            true
        }

        fn dismiss_policy(&self) -> DismissPolicy {
            DismissPolicy::modal_default()
        }
    }

    impl Screen for MountedModalScreen {
        fn build<'a>(
            &'a mut self,
            cx: &'a mut ScreenFrameContext<'a>,
        ) -> &'a mut dyn crate::core::widget::Widget {
            cx.mount_child(Box::new(ScreenHost::new()));
            &mut self.placeholder
        }

        fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            false
        }

        fn covers_point(&self, _logical_pos: Point) -> bool {
            true
        }

        fn dismiss_policy(&self) -> DismissPolicy {
            DismissPolicy::modal_default()
        }
    }

    fn empty_modifiers() -> Modifiers {
        Modifiers {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }

    #[derive(Default)]
    struct PhysicalResponseTrace {
        keys: RefCell<Vec<crate::core::event::KeyEvent>>,
        responses: RefCell<Vec<crate::core::event::KeyEvent>>,
        pending: RefCell<Vec<crate::core::event::KeyEvent>>,
        committed: Cell<usize>,
        interruptions: RefCell<Vec<crate::core::event::KeyboardCaptureInterruption>>,
        leaves: Cell<u32>,
        unmounts: Cell<u32>,
        screen_keys: Cell<u32>,
        screen_shortcuts: Cell<u32>,
        pending_at_shortcut: Cell<usize>,
        text: RefCell<Vec<String>>,
        redirect_on_tab: Cell<Option<crate::utils::id::TreeWidgetId>>,
        release_on_tab: Cell<bool>,
        app_consumes: Cell<bool>,
        screen_widget_key_counts: RefCell<Vec<usize>>,
        screen_events: RefCell<Vec<FusedInputEvent>>,
        notifications: RefCell<Option<std::sync::mpsc::Sender<&'static str>>>,
        capture_gate: RefCell<Option<std::sync::mpsc::Receiver<()>>>,
        notify_pending: Cell<bool>,
    }

    struct PhysicalResponseWidget {
        id: WidgetInternalId,
        trace: Rc<PhysicalResponseTrace>,
        active: bool,
        physical: bool,
    }

    impl WidgetDowncast for PhysicalResponseWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for PhysicalResponseWidget {
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(80.0, 24.0))
        }
        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn focusable(&self) -> bool {
            true
        }
        fn capture_eligible(&self) -> bool {
            true
        }
        fn physical_keyboard_capture(&self) -> bool {
            self.physical
        }
        fn on_capture_enter(&mut self, _cx: &mut EventContext) {
            self.active = true;
            if let Some(sender) = self.trace.notifications.borrow().as_ref() {
                sender.send("capture").unwrap();
            }
            if let Some(gate) = self.trace.capture_gate.borrow_mut().take() {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        }
        fn on_capture_leave(&mut self, _cx: &mut EventContext) {
            self.active = false;
            self.trace.pending.borrow_mut().clear();
            self.trace.leaves.set(self.trace.leaves.get() + 1);
        }
        fn on_unmount(&mut self) {
            self.active = false;
            self.trace.pending.borrow_mut().clear();
            self.trace.unmounts.set(self.trace.unmounts.get() + 1);
        }
        fn on_keyboard_capture_interrupted(
            &mut self,
            interruption: &crate::core::event::KeyboardCaptureInterruption,
            _cx: &mut EventContext,
        ) {
            self.trace
                .interruptions
                .borrow_mut()
                .push(interruption.clone());
            self.trace.pending.borrow_mut().clear();
        }
        fn handle_event(
            &mut self,
            cx: &mut EventContext,
            event: &crate::core::event::Event,
            phase: crate::core::event::EventPhase,
        ) -> bool {
            use crate::core::event::{Event, EventPhase};
            if phase != EventPhase::Target {
                return false;
            }
            match event {
                Event::Key(key) if self.active => {
                    self.trace.keys.borrow_mut().push(key.clone());
                    if key.is_down {
                        self.trace.notify_pending.set(true);
                        if let Some(sender) = self.trace.notifications.borrow().as_ref() {
                            sender.send("widget").unwrap();
                        }
                    }
                    if key.key_code == KeyCode::Tab {
                        if let Some(owner) = self.trace.redirect_on_tab.get() {
                            cx.redirect_focus(owner);
                        }
                        if self.trace.release_on_tab.get() {
                            cx.release_keyboard_capture();
                        }
                        return false;
                    }
                    if key.is_down && !key.is_repeat && !key.physical_key.is_modifier() {
                        self.trace.responses.borrow_mut().push(key.clone());
                        self.trace.pending.borrow_mut().push(key.clone());
                    }
                    true
                }
                Event::Text(text) => {
                    self.trace.text.borrow_mut().push(text.text.clone());
                    true
                }
                _ => false,
            }
        }
    }

    struct PhysicalResponseScreen {
        host: ScreenHost,
        trace: Rc<PhysicalResponseTrace>,
    }

    impl Screen for PhysicalResponseScreen {
        fn build<'a>(&'a mut self, _cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            &mut self.host
        }
        fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            if event.key_event().is_some() {
                self.trace.screen_keys.set(self.trace.screen_keys.get() + 1);
            }
            if matches!(event, FusedInputEvent::Shortcut { .. }) {
                self.trace
                    .screen_shortcuts
                    .set(self.trace.screen_shortcuts.get() + 1);
                self.trace
                    .pending_at_shortcut
                    .set(self.trace.pending.borrow().len());
                return true;
            }
            false
        }
        fn flush_pending_actions(&mut self, _cx: &mut ScreenContext<'_>) {
            let count = self.trace.pending.borrow_mut().drain(..).count();
            self.trace.committed.set(self.trace.committed.get() + count);
        }
    }

    fn physical_response_fixture() -> (
        AppRoot,
        WidgetTree,
        EventRouter,
        InteractionDomainState,
        NodeId,
        crate::utils::id::TreeWidgetId,
        Rc<PhysicalResponseTrace>,
    ) {
        physical_response_fixture_with_policy(true, AppContext::new(Theme::dark()))
    }

    fn physical_response_fixture_with_policy(
        physical: bool,
        context: AppContext,
    ) -> (
        AppRoot,
        WidgetTree,
        EventRouter,
        InteractionDomainState,
        NodeId,
        crate::utils::id::TreeWidgetId,
        Rc<PhysicalResponseTrace>,
    ) {
        let trace = Rc::new(PhysicalResponseTrace::default());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let owner = tree.add_child(
            root,
            Box::new(PhysicalResponseWidget {
                id: WidgetInternalId::new(),
                trace: Rc::clone(&trace),
                active: false,
                physical,
            }),
            None,
            0,
        );
        let owner_id = tree.widget_id_for_node(owner).unwrap();
        let mut app = AppRoot::new(context, root);
        app.push_screen(Box::new(PhysicalResponseScreen {
            host: ScreenHost::new(),
            trace: Rc::clone(&trace),
        }));
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        router.request_focus_for_widget(&mut tree, owner_id, &mut state);
        (app, tree, router, state, root, owner_id, trace)
    }

    pub(crate) fn dispatch_physical_raw(
        app: &mut AppRoot,
        tree: &mut WidgetTree,
        router: &mut EventRouter,
        state: &mut InteractionDomainState,
        fusion: &mut InputFusion,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        raw: RawInputEvent,
    ) {
        let mut events = Vec::new();
        fusion.process_raw_with_gesture_sessions(
            raw,
            crate::core::event::InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            tree,
            None,
            &mut events,
        );
        fusion.flush_with_gesture_sessions(&mut events);
        for input in events {
            if dispatch_session_input_to_app_root(app, tree, router, state, &input, text_engine) {
                continue;
            }
            let root = app.active_input_root_node();
            if dispatch_engine_action(
                input.event(),
                &mut app.context.registry.binding_registry,
                router,
                tree,
                root,
                state,
                text_engine,
                None,
            ) {
                continue;
            }
            router.route_session_input_event(
                tree,
                input,
                Some(root),
                text_engine,
                None,
                state,
                &mut app.context.registry.binding_registry,
            );
        }
    }

    #[test]
    fn captured_modifier_consumed_widget_does_not_reach_screen() {
        use crate::core::event::{KeyboardAuthorityMode, PhysicalKeyCode};
        let (mut app, mut tree, mut router, mut state, root, owner, trace) =
            physical_response_fixture_with_policy(false, AppContext::new(Theme::dark()));
        let mut text = FontManagerTextEngine::new();
        let mut fusion = InputFusion::new();
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(state.focus_manager.focused(), Some(owner));
        let modifiers = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        let raw = physical_raw_key(
            KeyCode::Character('k'),
            PhysicalKeyCode::KeyK,
            modifiers,
            false,
            None,
        );
        let time = match &raw {
            RawInputEvent::Key { time, .. } => *time,
            _ => unreachable!(),
        };
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            raw,
        );
        assert_eq!(
            trace.keys.borrow().len(),
            1,
            "actual captured widget sees the original key once"
        );
        assert_eq!(
            trace.responses.borrow().len(),
            1,
            "the real target handled it"
        );
        assert_eq!(
            trace.screen_keys.get(),
            0,
            "INV-MODIFIER-WIDGET-FIRST: consumed modified key must never reach Screen/App"
        );
        assert_eq!(state.swallow_text_at, Some(time));
        assert_eq!(state.focus_manager.focused(), Some(owner));
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(trace.leaves.get(), 0);
    }

    struct CapturedModifierScreen {
        host: ScreenHost,
        trace: Rc<PhysicalResponseTrace>,
        nodes: Rc<Cell<Option<(NodeId, NodeId)>>>,
        initial_focus: bool,
    }

    impl Screen for CapturedModifierScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let column = cx.mount_child(Box::new(
                crate::widgets::flex::Flex::column().with_spacing(8.0),
            ));
            let probe = cx.tree.add_child(
                column,
                Box::new(PhysicalResponseWidget {
                    id: WidgetInternalId::new(),
                    trace: self.trace.clone(),
                    active: false,
                    physical: false,
                }),
                None,
                0,
            );
            let field = cx.tree.add_child(
                column,
                Box::new(crate::widgets::text_field::TextField::new().with_text("editor")),
                None,
                0,
            );
            self.nodes.set(Some((probe, field)));
            if self.initial_focus {
                cx.tree.defer_focus_sequence([probe]);
            }
            &mut self.host
        }

        fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            if matches!(event, FusedInputEvent::KeyDown { .. }) {
                self.trace.screen_keys.set(self.trace.screen_keys.get() + 1);
                self.trace
                    .screen_widget_key_counts
                    .borrow_mut()
                    .push(self.trace.keys.borrow().len());
                self.trace.screen_events.borrow_mut().push(event.clone());
                self.trace.notify_pending.set(true);
                if let Some(sender) = self.trace.notifications.borrow().as_ref() {
                    sender.send("screen").unwrap();
                }
                return self.trace.app_consumes.get();
            }
            false
        }

        fn flush_pending_actions(&mut self, _cx: &mut ScreenContext<'_>) {
            let count = self.trace.pending.borrow_mut().drain(..).count();
            self.trace.committed.set(self.trace.committed.get() + count);
            if self.trace.notify_pending.replace(false) {
                assert!(
                    self.trace.text.borrow().is_empty(),
                    "consumed paired Text cannot reach the UI-loop target"
                );
                if let Some(sender) = self.trace.notifications.borrow().as_ref() {
                    sender.send("flush").unwrap();
                }
            }
        }
    }

    struct CapturedModifierFixture {
        app: AppRoot,
        tree: WidgetTree,
        router: EventRouter,
        state: InteractionDomainState,
        fusion: InputFusion,
        text: FontManagerTextEngine,
        root: NodeId,
        probe: NodeId,
        field: NodeId,
        trace: Rc<PhysicalResponseTrace>,
    }

    impl CapturedModifierFixture {
        fn new() -> Self {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
            let trace = Rc::new(PhysicalResponseTrace::default());
            trace.app_consumes.set(true);
            let nodes = Rc::new(Cell::new(None));
            let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
            app.push_screen(Box::new(CapturedModifierScreen {
                host: ScreenHost::new(),
                trace: trace.clone(),
                nodes: nodes.clone(),
                initial_focus: false,
            }));
            app.rebuild_active_view(&mut tree);
            let (probe, field) = nodes.get().unwrap();
            let mut text = FontManagerTextEngine::new_with_system_ui();
            test_begin_frame_layout_tracking();
            {
                let _layout = LayoutAuthorityGuard::enter();
                layout_tree(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(320.0, 160.0)),
                    &app.context.theme,
                    &mut text,
                    None,
                );
            }
            let mut fixture = Self {
                app,
                tree,
                router: EventRouter::new(),
                state: InteractionDomainState::new(),
                fusion: InputFusion::new(),
                text,
                root,
                probe,
                field,
                trace,
            };
            fixture.enter(probe);
            fixture
        }

        fn enter(&mut self, node: NodeId) {
            if self.state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture {
                assert!(self.router.handle_engine_action(
                    &mut self.tree,
                    crate::runtime::actions::ActionId::RELEASE_CAPTURE,
                    Some(self.root),
                    &mut self.state,
                    &mut self.text,
                    None
                ));
            }
            let owner = self.tree.widget_id_for_node(node).unwrap();
            self.router
                .request_focus_for_widget(&mut self.tree, owner, &mut self.state);
            assert!(self.router.handle_engine_action(
                &mut self.tree,
                crate::runtime::actions::ActionId::ENTER_WIDGET,
                Some(self.root),
                &mut self.state,
                &mut self.text,
                None
            ));
            assert_eq!(self.state.focus_manager.focused(), Some(owner));
            assert_eq!(
                self.state.keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture
            );
        }

        fn raw(&mut self, raw: RawInputEvent) {
            dispatch_physical_raw(
                &mut self.app,
                &mut self.tree,
                &mut self.router,
                &mut self.state,
                &mut self.fusion,
                &mut self.text,
                raw,
            );
        }

        fn key(&mut self, key: KeyCode, modifiers: Modifiers) -> Instant {
            let raw = physical_raw_key(
                key,
                crate::core::event::PhysicalKeyCode::Unknown,
                modifiers,
                false,
                None,
            );
            let time = match &raw {
                RawInputEvent::Key { time, .. } => *time,
                _ => unreachable!(),
            };
            self.raw(raw);
            time
        }

        fn qualified_tab(&mut self) -> crate::core::event::SessionQualifiedInput {
            let mut events = Vec::new();
            self.fusion.process_raw_with_gesture_sessions(
                physical_raw_key(
                    KeyCode::Tab,
                    crate::core::event::PhysicalKeyCode::Tab,
                    Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    },
                    false,
                    None,
                ),
                self.state.focus_manager.interaction_domain_id(),
                &mut self.state.gesture_registry,
                &self.tree,
                None,
                &mut events,
            );
            self.fusion.flush_with_gesture_sessions(&mut events);
            assert_eq!(events.len(), 1);
            events.pop().unwrap()
        }
    }

    #[test]
    fn captured_modifier_refusal_reaches_screen_once_after_widget() {
        for modifiers in [
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            Modifiers {
                alt: true,
                ..Modifiers::default()
            },
            Modifiers {
                meta: true,
                ..Modifiers::default()
            },
        ] {
            let mut f = CapturedModifierFixture::new();
            let owner = f.state.focus_manager.focused();
            let time = f.key(KeyCode::Tab, modifiers);
            assert_eq!(f.trace.keys.borrow().len(), 1);
            assert_eq!(f.trace.screen_keys.get(), 1);
            assert_eq!(&*f.trace.screen_widget_key_counts.borrow(), &[1]);
            assert_eq!(f.state.focus_manager.focused(), owner);
            assert_eq!(
                f.state.keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture
            );
            assert_eq!(f.state.swallow_text_at, Some(time));
            assert!(
                matches!(&f.trace.screen_events.borrow()[0], FusedInputEvent::KeyDown { window_id: WindowId::PRIMARY, time: delivered, key_code: KeyCode::Tab, physical_key: crate::core::event::PhysicalKeyCode::Unknown, reservation: None, modifiers: delivered_modifiers, repeat: false } if *delivered == time && *delivered_modifiers == modifiers)
            );
        }
    }

    #[test]
    fn browser_paste_original_app_intake_distinguishes_clipboard_seal_and_app_consumption() {
        use crate::core::event::{KeyboardKeyReservation, PhysicalKeyCode};
        for (clipboard, consumes) in [(true, true), (false, false), (false, true)] {
            let mut f = CapturedModifierFixture::new();
            f.enter(f.field);
            f.trace.app_consumes.set(consumes);
            let raw = physical_raw_key(
                if clipboard {
                    KeyCode::Character('v')
                } else {
                    KeyCode::Tab
                },
                PhysicalKeyCode::Unknown,
                Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
                false,
                clipboard.then_some(KeyboardKeyReservation::Clipboard),
            );
            let mut events = Vec::new();
            f.fusion.process_raw_with_gesture_sessions(
                raw,
                f.state.focus_manager.interaction_domain_id(),
                &mut f.state.gesture_registry,
                &f.tree,
                None,
                &mut events,
            );
            f.fusion.flush_with_gesture_sessions(&mut events);
            assert_eq!(events.len(), 1);
            let input = events.pop().unwrap();
            let result = dispatch_session_input_to_app_root_with_captured_routing(
                &mut f.app,
                &mut f.tree,
                &mut f.router,
                &mut f.state,
                &input,
                &mut f.text,
                None,
                None,
                None,
            );
            assert!(
                result.handled,
                "original intake seals this key against replay"
            );
            assert_eq!(
                result.blocks_browser_paste(&input, &f.state),
                !clipboard && consumes
            );
            assert_eq!(f.trace.screen_keys.get(), u32::from(!clipboard));
            assert_eq!(
                f.state.focus_manager.focused(),
                f.tree.widget_id_for_node(f.field)
            );
            assert_eq!(f.state.keyboard_mode, KeyboardAuthorityMode::Capture);
        }
    }

    #[test]
    fn captured_modifier_both_decline_keeps_capture_and_does_not_swallow_text() {
        let mut f = CapturedModifierFixture::new();
        f.trace.app_consumes.set(false);
        let owner = f.state.focus_manager.focused();
        let time = f.key(
            KeyCode::Tab,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(&*f.trace.screen_widget_key_counts.borrow(), &[1]);
        assert_eq!(f.state.focus_manager.focused(), owner);
        assert_eq!(
            f.state.keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        assert_eq!(f.state.swallow_text_at, None);
        assert_eq!(f.trace.leaves.get(), 0);
        f.raw(RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time,
            text: "same".into(),
            modifiers: Modifiers::default(),
        });
        assert_eq!(&*f.trace.text.borrow(), &["same"]);
    }

    #[test]
    fn captured_modifier_widget_and_app_consumption_suppress_only_exact_paired_text() {
        for key in [KeyCode::Character('k'), KeyCode::Tab] {
            let mut f = CapturedModifierFixture::new();
            let time = f.key(
                key,
                Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            );
            f.raw(RawInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time,
                text: "paired".into(),
                modifiers: Modifiers::default(),
            });
            assert!(f.trace.text.borrow().is_empty());
            assert_eq!(f.state.swallow_text_at, None);
            f.raw(RawInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: time + Duration::from_millis(1),
                text: "fresh".into(),
                modifiers: Modifiers::default(),
            });
            assert_eq!(&*f.trace.text.borrow(), &["fresh"]);
            assert_eq!(f.trace.screen_keys.get(), u32::from(key == KeyCode::Tab));
            assert_eq!(f.trace.keys.borrow().len(), 1);
        }
    }

    #[test]
    fn captured_modifier_real_text_field_modified_home_precedes_app() {
        let mut f = CapturedModifierFixture::new();
        f.enter(f.field);
        let modifiers = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        };
        let time = f.key(KeyCode::Home, modifiers);
        let snapshot = f
            .tree
            .node(f.field)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::text_field::TextField>()
            .unwrap()
            .capture_edit_handoff()
            .unwrap();
        assert_eq!(snapshot.text(), "editor");
        assert_eq!(
            (
                snapshot.selection_anchor_byte(),
                snapshot.selection_head_byte()
            ),
            (6, 0)
        );
        assert_eq!(
            f.trace.screen_keys.get(),
            0,
            "the Screen would consume this shortcut if incorrectly called first"
        );
        assert_eq!(f.state.swallow_text_at, Some(time));
        f.raw(RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time,
            text: "paired".into(),
            modifiers: Modifiers::default(),
        });
        let current = f
            .tree
            .node(f.field)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::text_field::TextField>()
            .unwrap()
            .capture_edit_handoff()
            .unwrap();
        assert_eq!(current.text(), "editor");
    }

    #[test]
    fn captured_modifier_actual_focus_redirect_and_release_drop_fallback() {
        for redirect in [false, true] {
            let mut f = CapturedModifierFixture::new();
            let next = f.tree.widget_id_for_node(f.field).unwrap();
            if redirect {
                f.trace.redirect_on_tab.set(Some(next));
            } else {
                f.trace.release_on_tab.set(true);
            }
            f.key(
                KeyCode::Tab,
                Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            );
            assert_eq!(f.trace.keys.borrow().len(), 1);
            assert_eq!(
                f.trace.screen_keys.get(),
                0,
                "an actual changed owner/mode cannot redirect the old key to App"
            );
            assert_eq!(f.state.swallow_text_at, None);
            if redirect {
                assert_eq!(f.state.focus_manager.focused(), Some(next));
            } else {
                assert_eq!(
                    f.state.keyboard_mode,
                    crate::core::event::KeyboardAuthorityMode::Navigation
                );
            }
        }
    }

    #[test]
    fn captured_modifier_retired_replaced_and_modal_targets_cannot_fall_through() {
        use crate::core::event_router::{CapturedAppKeyOutcome, CapturedAppKeyTarget};
        for change in [0, 1, 2] {
            let mut f = CapturedModifierFixture::new();
            let input = f.qualified_tab();
            assert!(prepare_session_input_for_app(
                &mut f.tree,
                &mut f.router,
                &mut f.state,
                &input
            ));
            let target = CapturedAppKeyTarget::qualify(&f.tree, &f.state, &input, f.root).unwrap();
            let actual = f.router.route_session_input_event_for_app(
                &mut f.tree,
                input.clone(),
                f.root,
                &mut f.text,
                None,
                &mut f.state,
                &mut f.app.context.registry.binding_registry,
                &target,
                None,
            );
            assert_eq!(actual, CapturedAppKeyOutcome::AppFallback);
            assert_eq!(f.trace.keys.borrow().len(), 1);
            match change {
                0 => {
                    f.tree.remove_node(f.probe);
                }
                1 => {
                    f.tree.replace_widget_only(
                        f.probe,
                        Box::new(PhysicalResponseWidget {
                            id: WidgetInternalId::new(),
                            trace: f.trace.clone(),
                            active: false,
                            physical: false,
                        }),
                    );
                }
                _ => {
                    f.app.modals.open(Box::new(MountedModalScreen::new()));
                }
            }
            finish_captured_app_key(
                &mut f.app,
                &mut f.tree,
                &mut f.router,
                &mut f.state,
                &input,
                &target,
                actual,
            );
            assert_eq!(f.trace.screen_keys.get(), 0);
            assert_eq!(
                f.trace.keys.borrow().len(),
                1,
                "no replay to the old or successor widget"
            );
        }
    }

    #[test]
    fn captured_modifier_plain_and_shift_tab_keep_auto_capture_chain() {
        let mut f = CapturedModifierFixture::new();
        f.trace.app_consumes.set(false);
        f.key(KeyCode::Tab, Modifiers::default());
        assert_eq!(
            f.state.focus_manager.focused(),
            f.tree.widget_id_for_node(f.field)
        );
        assert_eq!(
            f.state.keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        f.key(
            KeyCode::Tab,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(
            f.state.focus_manager.focused(),
            f.tree.widget_id_for_node(f.probe)
        );
        assert_eq!(
            f.state.keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        assert_eq!(f.trace.screen_keys.get(), 2);
        assert_eq!(
            f.trace.screen_widget_key_counts.borrow()[0],
            0,
            "plain Tab keeps its original pre-widget App path"
        );
    }

    #[test]
    fn captured_modifier_reservation_escape_and_noneligible_keys_keep_precedence() {
        let mut f = CapturedModifierFixture::new();
        f.raw(physical_raw_key(
            KeyCode::Character('k'),
            crate::core::event::PhysicalKeyCode::Unknown,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            false,
            Some(crate::core::event::KeyboardKeyReservation::Fullscreen),
        ));
        assert!(f.trace.keys.borrow().is_empty());
        assert_eq!(f.trace.screen_keys.get(), 0);
        f.key(
            KeyCode::Control,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        );
        assert!(
            f.trace.keys.borrow().is_empty(),
            "bare modifier retains Screen/App precedence"
        );
        assert_eq!(f.trace.screen_keys.get(), 1);
        f.key(KeyCode::F1, Modifiers::default());
        assert!(
            f.trace.keys.borrow().is_empty(),
            "unmodified function keys retain Screen/App precedence"
        );
        assert_eq!(f.trace.screen_keys.get(), 2);
        f.key(
            KeyCode::Escape,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(
            f.state.keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Navigation
        );
        assert_eq!(
            f.trace.screen_keys.get(),
            2,
            "Escape remains structural before widgets/App"
        );
        assert!(f.trace.keys.borrow().is_empty());
    }

    #[test]
    fn captured_modifier_wrong_domain_window_root_and_relative_facts_are_rejected() {
        use crate::core::event::{InteractionDomainId, KeyboardAuthorityMode};
        use crate::core::event_router::{
            CapturedAppKeyOutcome, CapturedAppKeyTarget, RelativePointerWindowFacts,
        };
        let mut f = CapturedModifierFixture::new();
        let input = f.qualified_tab();
        let target = CapturedAppKeyTarget::qualify(&f.tree, &f.state, &input, f.root).unwrap();
        assert!(!target.is_current(&f.tree, &f.state, input.event(), f.field));
        let foreign_state = InteractionDomainState::new_for_domain(
            InteractionDomainId(71),
            f.state.ui_interaction_policy(),
        );
        assert!(!target.is_current(&f.tree, &foreign_state, input.event(), f.root));
        let facts = RelativePointerWindowFacts {
            window_id: WindowId::PRIMARY,
            interaction_domain_id: InteractionDomainId::PRIMARY,
            focused_window_id: Some(WindowId::PRIMARY),
            client_bounds: RectF::new(0.0, 0.0, 320.0, 160.0),
        };
        for wrong in [
            RelativePointerWindowFacts {
                window_id: WindowId(7),
                ..facts
            },
            RelativePointerWindowFacts {
                interaction_domain_id: InteractionDomainId(7),
                ..facts
            },
            RelativePointerWindowFacts {
                focused_window_id: None,
                ..facts
            },
        ] {
            assert_eq!(
                f.router.route_session_input_event_for_app(
                    &mut f.tree,
                    input.clone(),
                    f.root,
                    &mut f.text,
                    None,
                    &mut f.state,
                    &mut f.app.context.registry.binding_registry,
                    &target,
                    Some(wrong)
                ),
                CapturedAppKeyOutcome::NotEligible
            );
        }
        assert!(f.trace.keys.borrow().is_empty());
        assert_eq!(f.trace.screen_keys.get(), 0);
        assert_eq!(f.state.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(
            f.router.route_session_input_event_for_app(
                &mut f.tree,
                input.clone(),
                f.root,
                &mut f.text,
                None,
                &mut f.state,
                &mut f.app.context.registry.binding_registry,
                &target,
                Some(facts)
            ),
            CapturedAppKeyOutcome::AppFallback
        );
        let mut raw = physical_raw_key(
            KeyCode::Tab,
            crate::core::event::PhysicalKeyCode::Tab,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            false,
            None,
        );
        if let RawInputEvent::Key { window_id, .. } = &mut raw {
            *window_id = WindowId(7);
        }
        let mut foreign_input = Vec::new();
        f.fusion.process_raw_with_gesture_sessions(
            raw,
            InteractionDomainId::PRIMARY,
            &mut f.state.gesture_registry,
            &f.tree,
            None,
            &mut foreign_input,
        );
        f.fusion.flush_with_gesture_sessions(&mut foreign_input);
        assert_eq!(foreign_input.len(), 1);
        finish_captured_app_key(
            &mut f.app,
            &mut f.tree,
            &mut f.router,
            &mut f.state,
            &foreign_input[0],
            &target,
            CapturedAppKeyOutcome::AppFallback,
        );
        assert_eq!(
            f.trace.screen_keys.get(),
            0,
            "original qualification cannot authenticate another window's key"
        );
        assert_eq!(f.trace.keys.borrow().len(), 1);
    }

    #[test]
    fn captured_modifier_public_router_without_app_recipient_does_not_navigate() {
        for qualified in [false, true] {
            let mut f = CapturedModifierFixture::new();
            let owner = f.state.focus_manager.focused();
            let input = f.qualified_tab();
            if qualified {
                f.router.route_session_input_event(
                    &mut f.tree,
                    input,
                    Some(f.root),
                    &mut f.text,
                    None,
                    &mut f.state,
                    &mut f.app.context.registry.binding_registry,
                );
            } else {
                f.router.route_input_event(
                    &mut f.tree,
                    input.event().clone(),
                    Some(f.root),
                    &mut f.text,
                    None,
                    &mut f.state,
                    &mut f.app.context.registry.binding_registry,
                );
            }
            assert_eq!(f.trace.keys.borrow().len(), 1);
            assert_eq!(f.trace.screen_keys.get(), 0);
            assert_eq!(f.state.focus_manager.focused(), owner);
            assert_eq!(
                f.state.keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture
            );
        }
    }

    #[test]
    fn captured_modifier_real_ui_loop_orders_widget_screen_and_flush_once() {
        use std::sync::mpsc;
        let (input_tx, input_rx) = mpsc::channel();
        let (witness_tx, witness_rx) = mpsc::channel();
        let (start_tx, start_rx) = mpsc::channel();
        let mut config = UiThreadConfig::new(
            Theme::dark(),
            Box::new(FontManagerTextEngine::new_with_system_ui()),
            Constraints::tight(Size::new(320.0, 160.0)),
        );
        config.initial_screen = Some(Box::new(move || {
            let trace = Rc::new(PhysicalResponseTrace::default());
            trace.app_consumes.set(true);
            *trace.notifications.borrow_mut() = Some(witness_tx);
            *trace.capture_gate.borrow_mut() = Some(start_rx);
            Box::new(CapturedModifierScreen {
                host: ScreenHost::new(),
                trace,
                nodes: Rc::new(Cell::new(None)),
                initial_focus: true,
            })
        }));
        let worker = spawn_ui_thread(
            config,
            || {
                let mut tree = WidgetTree::new();
                let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
                (tree, root)
            },
            input_rx,
            SceneSwapchain::new(),
        );
        assert_eq!(
            witness_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            "capture",
            "original deferred focus must genuinely enter Capture"
        );
        let mut raw = Vec::new();
        for code in [KeyCode::Character('k'), KeyCode::Tab] {
            let key = physical_raw_key(
                code,
                crate::core::event::PhysicalKeyCode::Unknown,
                Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
                false,
                None,
            );
            let time = match &key {
                RawInputEvent::Key { time, .. } => *time,
                _ => unreachable!(),
            };
            raw.push(key);
            raw.push(RawInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time,
                text: "paired".into(),
                modifiers: Modifiers::default(),
            });
        }
        // One atomic input batch; the existing loop's post-route flush is the witness.
        for event in raw {
            input_tx.send(event).unwrap();
        }
        start_tx.send(()).unwrap();
        let mut order = Vec::new();
        loop {
            let item = witness_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            order.push(item);
            if item == "flush" {
                break;
            }
        }
        drop(input_tx);
        worker.join().unwrap();
        assert_eq!(order, ["widget", "widget", "screen", "flush"]);
        assert!(
            witness_rx
                .try_iter()
                .all(|event| event != "widget" && event != "screen"),
            "no second routing/Screen pass"
        );
    }

    struct AvailabilityCutoffScreen {
        host: ScreenHost,
        nodes: Rc<Cell<Option<(NodeId, NodeId)>>>,
        seen: Rc<RefCell<Vec<FusedInputEvent>>>,
        clicks: Rc<Cell<u32>>,
        requested: bool,
    }

    impl Screen for AvailabilityCutoffScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let column = cx.mount_child(Box::new(
                crate::widgets::flex::Flex::column().with_spacing(8.0),
            ));
            let clicks = Rc::clone(&self.clicks);
            let button = cx.tree.add_child(
                column,
                Box::new(
                    crate::widgets::button::Button::with_text("Apply")
                        .on_click(move || clicks.set(clicks.get() + 1)),
                ),
                None,
                0,
            );
            let field = cx.tree.add_child(
                column,
                Box::new(crate::widgets::text_field::TextField::new().with_text("field")),
                None,
                0,
            );
            self.nodes.set(Some((button, field)));
            &mut self.host
        }

        fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
            self.seen.borrow_mut().push(event.clone());
            if matches!(
                event,
                FusedInputEvent::KeyDown {
                    key_code: KeyCode::Character('i'),
                    repeat: false,
                    ..
                }
            ) && !self.requested
            {
                self.requested = true;
                cx.request_widget_enabled(self.nodes.get().unwrap().0, false)
                    .unwrap();
            }
            false
        }
    }

    #[test]
    fn atomic_availability_preserves_paired_text_across_deferred_cutoff() {
        use crate::core::event::{KeyState, PhysicalKeyCode, RawButtonState, UiInteractionPolicy};
        use crate::core::widget_availability::apply_pending_widget_availability;
        use crate::runtime::actions::ActionId;
        use crate::widgets::{button::Button, text_field::TextField};

        for policy in [
            UiInteractionPolicy::NavigationAndCapture,
            UiInteractionPolicy::PointerDriven,
        ] {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
            let nodes = Rc::new(Cell::new(None));
            let seen = Rc::new(RefCell::new(Vec::new()));
            let clicks = Rc::new(Cell::new(0));
            let theme = Theme::light();
            let mut app = AppRoot::new(AppContext::new(theme.clone()), root);
            app.push_screen(Box::new(AvailabilityCutoffScreen {
                host: ScreenHost::new(),
                nodes: Rc::clone(&nodes),
                seen: Rc::clone(&seen),
                clicks: Rc::clone(&clicks),
                requested: false,
            }));
            app.rebuild_active_view(&mut tree);
            let (button, field) = nodes.get().unwrap();
            let button_owner = tree.widget_id_for_node(button).unwrap();
            let field_owner = tree.widget_id_for_node(field).unwrap();
            let mut text = FontManagerTextEngine::new();
            test_begin_frame_layout_tracking();
            {
                let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
                layout_tree(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(320.0, 160.0)),
                    &theme,
                    &mut text,
                    None,
                );
            }
            let mut router = EventRouter::new();
            let mut state = InteractionDomainState::with_ui_interaction_policy(policy);
            let mut fusion = InputFusion::new();
            router.request_focus_for_widget(&mut tree, button_owner, &mut state);
            assert_eq!(
                app.context.registry.binding_registry.lookup_key(
                    KeyCode::Character('i'),
                    PhysicalKeyCode::KeyI,
                    Modifiers::default()
                ),
                Some(ActionId::ENTER_WIDGET)
            );

            let time = Instant::now();
            let fresh_time = time + Duration::from_millis(1);
            let mut events = Vec::new();
            for raw in [
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code: KeyCode::Character('i'),
                    physical_key: PhysicalKeyCode::KeyI,
                    reservation: None,
                    state: KeyState::Down,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time,
                    text: "i".into(),
                    modifiers: Modifiers::default(),
                },
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: fresh_time,
                    text: "fresh".into(),
                    modifiers: Modifiers::default(),
                },
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time: time + Duration::from_millis(2),
                    key_code: KeyCode::Character('i'),
                    physical_key: PhysicalKeyCode::KeyI,
                    reservation: None,
                    state: KeyState::Up,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ] {
                fusion.process_raw_with_gesture_sessions(
                    raw,
                    state.focus_manager.interaction_domain_id(),
                    &mut state.gesture_registry,
                    &tree,
                    None,
                    &mut events,
                );
            }
            fusion.flush_with_gesture_sessions(&mut events);
            assert_eq!(events.len(), 4);
            let originals: Vec<_> = events.iter().skip(1).cloned().collect();
            let mut intake = events.into_iter();
            let mut deferred = Vec::new();
            let key = next_qualified_widget_input(&tree, &mut intake, &mut deferred).unwrap();
            assert!(!dispatch_session_input_to_app_root(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &key,
                &mut text
            ));
            assert!(tree.has_pending_widget_availability());
            let input_root = app.active_input_root_node();
            let engine_consumed = dispatch_engine_action(
                key.event(),
                &mut app.context.registry.binding_registry,
                &mut router,
                &mut tree,
                input_root,
                &mut state,
                &mut text,
                None,
            );
            assert_eq!(
                engine_consumed,
                policy == UiInteractionPolicy::NavigationAndCapture
            );
            if !engine_consumed {
                router.route_session_input_event(
                    &mut tree,
                    key,
                    Some(input_root),
                    &mut text,
                    None,
                    &mut state,
                    &mut app.context.registry.binding_registry,
                );
            }
            assert_eq!(
                state.swallow_text_at,
                Some(time),
                "real engine/router consumption arms evidence"
            );
            assert_eq!(
                tree.node(button)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<Button>()
                    .unwrap()
                    .keyboard_pulse_active
                    .get(),
                engine_consumed
            );
            assert!(next_qualified_widget_input(&tree, &mut intake, &mut deferred).is_none());
            assert_eq!(deferred.len(), originals.len());
            for (parked, original) in deferred.iter().zip(&originals) {
                assert!(
                    std::ptr::eq(parked.event(), original.event()),
                    "original envelope retained in order"
                );
            }
            let settlement = apply_pending_widget_availability(
                &mut tree,
                &mut router,
                &mut fusion,
                &mut state,
                &mut app.context.registry.binding_registry,
                &mut text,
            );
            assert!(settlement.rejected.is_empty());
            assert_eq!(settlement.changed, vec![button_owner]);
            assert_eq!(state.focus_manager.focused(), Some(field_owner));
            assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
            assert!(
                !tree
                    .node(button)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<Button>()
                    .unwrap()
                    .keyboard_pulse_active
                    .get()
            );
            assert_eq!(
                state.swallow_text_at,
                Some(time),
                "availability does not own paired consumption"
            );
            assert_eq!(clicks.get(), 0);

            let mut intake = deferred.into_iter();
            let mut deferred = Vec::new();
            let residue = next_qualified_widget_input(&tree, &mut intake, &mut deferred).unwrap();
            assert!(std::ptr::eq(residue.event(), originals[0].event()));
            assert!(
                matches!(residue.event(), FusedInputEvent::TextInput { time: paired, text, .. }
                if *paired == time && text == "i")
            );
            assert!(
                dispatch_session_input_to_app_root(
                    &mut app,
                    &mut tree,
                    &mut router,
                    &mut state,
                    &residue,
                    &mut text
                ),
                "residue consumed before Screen/engine/widget"
            );
            assert_eq!(
                seen.borrow().len(),
                1,
                "Screen saw only the original KeyDown"
            );
            assert_eq!(
                state.swallow_text_at, None,
                "the authoritative Text consumer retires evidence"
            );
            while let Some(input) = next_qualified_widget_input(&tree, &mut intake, &mut deferred) {
                if dispatch_session_input_to_app_root(
                    &mut app,
                    &mut tree,
                    &mut router,
                    &mut state,
                    &input,
                    &mut text,
                ) {
                    continue;
                }
                if dispatch_engine_action(
                    input.event(),
                    &mut app.context.registry.binding_registry,
                    &mut router,
                    &mut tree,
                    input_root,
                    &mut state,
                    &mut text,
                    None,
                ) {
                    continue;
                }
                router.route_session_input_event(
                    &mut tree,
                    input,
                    Some(input_root),
                    &mut text,
                    None,
                    &mut state,
                    &mut app.context.registry.binding_registry,
                );
            }
            assert!(deferred.is_empty());
            assert_eq!(seen.borrow().len(), 3);
            assert!(
                matches!(&seen.borrow()[1], FusedInputEvent::TextInput { time, text, .. }
                if *time == fresh_time && text == "fresh")
            );
            assert!(matches!(
                &seen.borrow()[2],
                FusedInputEvent::KeyUp {
                    key_code: KeyCode::Character('i'),
                    ..
                }
            ));
            assert_eq!(
                tree.node(field)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<TextField>()
                    .unwrap()
                    .text(),
                "field"
            );
            assert_eq!(
                clicks.get(),
                0,
                "late old KeyUp cannot activate the disabled owner"
            );

            let field_rect = tree.get_absolute_paint_rect(field).unwrap();
            let position = Point::new(
                field_rect.x + field_rect.width * 0.5,
                field_rect.y + field_rect.height * 0.5,
            );
            let fresh_press = time + Duration::from_secs(1);
            for (offset, button_state) in
                [(0, RawButtonState::Pressed), (1, RawButtonState::Released)]
            {
                dispatch_physical_raw(
                    &mut app,
                    &mut tree,
                    &mut router,
                    &mut state,
                    &mut fusion,
                    &mut text,
                    RawInputEvent::PointerButton {
                        window_id: WindowId::PRIMARY,
                        time: fresh_press + Duration::from_millis(offset),
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        state: button_state,
                        position,
                    },
                );
            }
            assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: fresh_press + Duration::from_millis(2),
                    text: "Z".into(),
                    modifiers: Modifiers::default(),
                },
            );
            assert_eq!(
                tree.node(field)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<TextField>()
                    .unwrap()
                    .text(),
                "fieldZ"
            );
            println!(
                "ATOMIC PAIRED TEXT {policy:?}: original pair survives cutoff; residue suppressed before Screen; fresh text/pointer/keyboard PASS"
            );
        }
    }

    struct HeldKeyButtonScreen {
        host: ScreenHost,
        nodes: Rc<Cell<Option<[NodeId; 3]>>>,
        clicks: [Rc<Cell<u32>>; 2],
        seen: Rc<RefCell<Vec<FusedInputEvent>>>,
    }

    impl Screen for HeldKeyButtonScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let column = cx.mount_child(Box::new(
                crate::widgets::flex::Flex::column().with_spacing(8.0),
            ));
            let buttons = [0, 1].map(|index| {
                let clicks = Rc::clone(&self.clicks[index]);
                cx.tree.add_child(
                    column,
                    Box::new(
                        crate::widgets::button::Button::with_text(if index == 0 {
                            "Original"
                        } else {
                            "Successor"
                        })
                        .on_click(move || clicks.set(clicks.get() + 1)),
                    ),
                    None,
                    0,
                )
            });
            let peer = cx.tree.add_child(
                column,
                Box::new(crate::widgets::label::Label::new("retained peer")),
                None,
                0,
            );
            self.nodes.set(Some([buttons[0], buttons[1], peer]));
            &mut self.host
        }

        fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            self.seen.borrow_mut().push(event.clone());
            false
        }
    }

    #[test]
    fn atomic_availability_held_key_cannot_arm_focus_successor() {
        use crate::core::event::{
            InteractionDomains, KeyState, PhysicalKeyCode, RawButtonState, UiInteractionPolicy,
        };
        use crate::core::widget_availability::apply_pending_widget_availability;
        use crate::runtime::actions::ActionId;
        use crate::widgets::button::Button;

        let pulse = |tree: &WidgetTree, node| {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<Button>()
                .unwrap()
                .keyboard_pulse_active
                .get()
        };
        for policy in [
            UiInteractionPolicy::NavigationAndCapture,
            UiInteractionPolicy::PointerDriven,
        ] {
            for (key_code, physical_key, paired_text, action) in [
                (
                    KeyCode::Space,
                    PhysicalKeyCode::Space,
                    Some(" "),
                    ActionId::ACTIVATE,
                ),
                (
                    KeyCode::Enter,
                    PhysicalKeyCode::Enter,
                    None,
                    ActionId::ENTER_WIDGET,
                ),
                (
                    KeyCode::Character('i'),
                    PhysicalKeyCode::KeyI,
                    Some("i"),
                    ActionId::ENTER_WIDGET,
                ),
            ] {
                let mut tree = WidgetTree::new();
                let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
                let nodes = Rc::new(Cell::new(None));
                let clicks = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
                let seen = Rc::new(RefCell::new(Vec::new()));
                let theme = Theme::light();
                let mut app = AppRoot::new(AppContext::new(theme.clone()), root);
                app.push_screen(Box::new(HeldKeyButtonScreen {
                    host: ScreenHost::new(),
                    nodes: Rc::clone(&nodes),
                    clicks: clicks.clone(),
                    seen: Rc::clone(&seen),
                }));
                app.rebuild_active_view(&mut tree);
                let [original, successor, peer] = nodes.get().unwrap();
                let original_owner = tree.widget_id_for_node(original).unwrap();
                let successor_owner = tree.widget_id_for_node(successor).unwrap();
                let peer_owner = tree.widget_id_for_node(peer).unwrap();
                let root_owner = tree.widget_id_for_node(root).unwrap();
                let mut text = FontManagerTextEngine::new();
                let mut router = EventRouter::new();
                let mut domains = InteractionDomains::with_ui_interaction_policy(policy);
                let mut fusion = InputFusion::new();
                let mut retained = RetainedSceneTestHarness::new();
                let mut frame_index = 0;
                let time = Instant::now();
                macro_rules! frame {
                    () => {{
                        test_begin_frame_layout_tracking();
                        frame_index += 1;
                        retained.frame_with_gesture_sessions(
                            &mut tree,
                            root,
                            Constraints::tight(Size::new(320.0, 160.0)),
                            &theme,
                            &mut text,
                            &crate::core::icons::builtin_registry(),
                            &FrameTime {
                                now: time + Duration::from_millis(frame_index * 16),
                                dt: Duration::from_millis(16),
                                frame_index,
                                reason: FrameReason::Input,
                            },
                            None,
                            &mut domains,
                            WindowId::PRIMARY,
                            &mut router,
                            &mut fusion,
                        )
                    }};
                }
                macro_rules! raw {
                    ($event:expr) => {
                        dispatch_physical_raw(
                            &mut app,
                            &mut tree,
                            &mut router,
                            domains.primary_state_mut(),
                            &mut fusion,
                            &mut text,
                            $event,
                        )
                    };
                }
                let key = |time, state, repeat| RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code,
                    physical_key,
                    reservation: None,
                    state,
                    repeat,
                    modifiers: Modifiers::default(),
                };
                frame!();
                router.request_focus_for_widget(
                    &mut tree,
                    original_owner,
                    domains.primary_state_mut(),
                );
                frame!();
                assert_eq!(
                    app.context.registry.binding_registry.lookup_key(
                        key_code,
                        physical_key,
                        Modifiers::default()
                    ),
                    Some(action)
                );
                let keyboard_allowed = policy == UiInteractionPolicy::NavigationAndCapture;

                raw!(key(time, KeyState::Down, false));
                assert_eq!(pulse(&tree, original), keyboard_allowed);
                for offset in [1, 2] {
                    raw!(key(
                        time + Duration::from_millis(offset),
                        KeyState::Down,
                        true
                    ));
                    assert_eq!(pulse(&tree, original), keyboard_allowed);
                    assert_eq!(clicks[0].get(), 0);
                }
                raw!(key(time + Duration::from_millis(3), KeyState::Up, false));
                assert_eq!(clicks[0].get(), u32::from(keyboard_allowed));
                assert!(!pulse(&tree, original));

                raw!(key(time + Duration::from_secs(1), KeyState::Down, false));
                assert_eq!(pulse(&tree, original), keyboard_allowed);
                tree.request_widget_enabled(original, false).unwrap();
                let repeat_time = time + Duration::from_secs(2);
                let mut pending_raw = vec![key(repeat_time, KeyState::Down, true)];
                if let Some(value) = paired_text {
                    pending_raw.push(RawInputEvent::TextInput {
                        window_id: WindowId::PRIMARY,
                        time: repeat_time,
                        text: value.into(),
                        modifiers: Modifiers::default(),
                    });
                }
                pending_raw.push(key(
                    repeat_time + Duration::from_millis(1),
                    KeyState::Up,
                    false,
                ));
                pending_raw.push(RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: repeat_time + Duration::from_millis(2),
                    text: "fresh".into(),
                    modifiers: Modifiers::default(),
                });
                let mut events = Vec::new();
                for raw in pending_raw {
                    let state = domains.primary_state_mut();
                    fusion.process_raw_with_gesture_sessions(
                        raw,
                        state.focus_manager.interaction_domain_id(),
                        &mut state.gesture_registry,
                        &tree,
                        None,
                        &mut events,
                    );
                }
                fusion.flush_with_gesture_sessions(&mut events);
                let originals = events.clone();
                let mut intake = events.into_iter();
                let mut deferred = Vec::new();
                assert!(next_qualified_widget_input(&tree, &mut intake, &mut deferred).is_none());
                assert_eq!(deferred.len(), originals.len());
                for (parked, original) in deferred.iter().zip(&originals) {
                    assert!(std::ptr::eq(parked.event(), original.event()));
                }
                let settlement = apply_pending_widget_availability(
                    &mut tree,
                    &mut router,
                    &mut fusion,
                    domains.primary_state_mut(),
                    &mut app.context.registry.binding_registry,
                    &mut text,
                );
                assert!(settlement.rejected.is_empty());
                assert_eq!(settlement.changed, vec![original_owner]);
                assert_eq!(
                    domains.primary_state().focus_manager.focused(),
                    Some(successor_owner)
                );
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Navigation
                );
                assert!(!pulse(&tree, original) && !pulse(&tree, successor));
                assert_eq!(clicks[1].get(), 0);
                frame!();
                frame!();
                let owners = [root_owner, original_owner, successor_owner, peer_owner];
                let locals = owners.map(|owner| retained.local_fragment(owner).unwrap());
                let geometry = [root, original, successor, peer].map(|node| {
                    (
                        tree.get_absolute_paint_rect(node).unwrap(),
                        tree.node(node).unwrap().layout_epoch,
                    )
                });
                assert!([root, original, successor, peer].iter().all(|node| {
                    let node = tree.node(*node).unwrap();
                    !node.dirty_layout && !node.dirty_paint && !node.dirty_composition
                }));
                let trace_start = seen.borrow().len();
                let mut intake = deferred.into_iter();
                let mut deferred = Vec::new();
                let mut delivered = 0;
                while let Some(input) =
                    next_qualified_widget_input(&tree, &mut intake, &mut deferred)
                {
                    assert!(std::ptr::eq(input.event(), originals[delivered].event()));
                    delivered += 1;
                    if dispatch_session_input_to_app_root(
                        &mut app,
                        &mut tree,
                        &mut router,
                        domains.primary_state_mut(),
                        &input,
                        &mut text,
                    ) {
                        continue;
                    }
                    let input_root = app.active_input_root_node();
                    let consumed = dispatch_engine_action(
                        input.event(),
                        &mut app.context.registry.binding_registry,
                        &mut router,
                        &mut tree,
                        input_root,
                        domains.primary_state_mut(),
                        &mut text,
                        None,
                    );
                    if matches!(input.event(), FusedInputEvent::KeyDown { repeat: true, .. }) {
                        assert_eq!(consumed, keyboard_allowed);
                        assert!(
                            !pulse(&tree, successor),
                            "repeat must never arm focus successor"
                        );
                        if keyboard_allowed {
                            assert_eq!(domains.primary_state().swallow_text_at, Some(repeat_time));
                        }
                    }
                    if !consumed {
                        router.route_session_input_event(
                            &mut tree,
                            input,
                            Some(input_root),
                            &mut text,
                            None,
                            domains.primary_state_mut(),
                            &mut app.context.registry.binding_registry,
                        );
                    }
                }
                assert_eq!(delivered, originals.len());
                assert!(deferred.is_empty());
                assert!(!pulse(&tree, successor));
                assert_eq!(clicks[0].get(), u32::from(keyboard_allowed));
                assert_eq!(clicks[1].get(), 0);
                let trace = seen.borrow();
                assert!(!trace[trace_start..].iter().any(|event| {
                    matches!(event, FusedInputEvent::TextInput { time, .. } if *time == repeat_time)
                }));
                assert!(trace[trace_start..].iter().any(|event| {
                    matches!(event, FusedInputEvent::TextInput { text, .. } if text == "fresh")
                }));
                drop(trace);
                assert!(
                    [root, original, successor, peer].iter().all(|node| {
                        let node = tree.node(*node).unwrap();
                        !node.dirty_layout && !node.dirty_paint && !node.dirty_composition
                    }),
                    "skipped repeat adds no owner or composition dirtiness"
                );
                let repeated_frame = frame!();
                assert!(!repeated_frame.scene_built);
                for (owner, local) in owners.into_iter().zip(locals) {
                    assert!(Arc::ptr_eq(
                        &local,
                        &retained.local_fragment(owner).unwrap()
                    ));
                }
                assert_eq!(
                    geometry,
                    [root, original, successor, peer].map(|node| (
                        tree.get_absolute_paint_rect(node).unwrap(),
                        tree.node(node).unwrap().layout_epoch,
                    ))
                );

                let fresh_time = time + Duration::from_secs(4);
                raw!(key(fresh_time, KeyState::Down, false));
                assert_eq!(pulse(&tree, successor), keyboard_allowed);
                raw!(key(
                    fresh_time + Duration::from_millis(1),
                    KeyState::Up,
                    false
                ));
                assert_eq!(clicks[1].get(), u32::from(keyboard_allowed));
                if !keyboard_allowed {
                    let rect = tree.get_absolute_paint_rect(successor).unwrap();
                    let position =
                        Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
                    for (offset, state) in
                        [(0, RawButtonState::Pressed), (1, RawButtonState::Released)]
                    {
                        raw!(RawInputEvent::PointerButton {
                            window_id: WindowId::PRIMARY,
                            time: fresh_time
                                + Duration::from_secs(1)
                                + Duration::from_millis(offset),
                            device: PointerDevice::Mouse,
                            button: PointerButton::Primary,
                            state,
                            position,
                        });
                    }
                    assert_eq!(clicks[1].get(), 1);
                }
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Navigation
                );
                println!(
                    "ATOMIC HELD KEY {policy:?} {key_code:?}: same-owner hold, successor repeat/pair cutoff, retained reuse and fresh activation PASS"
                );
            }
        }
    }

    struct OutlineAppFixture {
        app: AppRoot,
        tree: WidgetTree,
        root: NodeId,
        children: [NodeId; 2],
        retained: RetainedSceneTestHarness,
        router: EventRouter,
        state: InteractionDomainState,
        fusion: InputFusion,
        text: FontManagerTextEngine,
        icons: crate::core::icon::IconRegistry,
    }

    impl OutlineAppFixture {
        fn new() -> Self {
            Self::with_config(&AppConfig::default())
        }

        fn with_config(config: &AppConfig) -> Self {
            use crate::widgets::{fixed_spacer::FixedSpacer, flex::Flex};
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::row()), None, 0);
            let children = [
                tree.add_child(root, Box::new(FixedSpacer::new(40.0, 40.0)), None, 0),
                tree.add_child(root, Box::new(FixedSpacer::new(40.0, 40.0)), None, 0),
            ];
            Self {
                app: AppRoot::new(AppContext::from_config(config), root),
                tree,
                root,
                children,
                retained: RetainedSceneTestHarness::new(),
                router: EventRouter::new(),
                state: InteractionDomainState::new(),
                fusion: InputFusion::new(),
                text: FontManagerTextEngine::new(),
                icons: crate::core::icons::builtin_registry(),
            }
        }

        fn frame(&mut self) -> RetainedSceneTestFrame {
            self.tree.begin_layout_transition_frame();
            self.retained.frame_with_app_context(
                &mut self.tree,
                self.root,
                Constraints::tight(Size::new(160.0, 80.0)),
                &self.app.context,
                &mut self.text,
                &self.icons,
            )
        }

        fn f12(&mut self) {
            use crate::core::event::{KeyState, PhysicalKeyCode};
            let time = Instant::now();
            for state in [KeyState::Down, KeyState::Up] {
                dispatch_physical_raw(
                    &mut self.app,
                    &mut self.tree,
                    &mut self.router,
                    &mut self.state,
                    &mut self.fusion,
                    &mut self.text,
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY,
                        time,
                        key_code: KeyCode::F12,
                        physical_key: PhysicalKeyCode::F12,
                        reservation: None,
                        state,
                        repeat: false,
                        modifiers: Modifiers::default(),
                    },
                );
            }
        }

        fn locals(&self) -> [Arc<[SceneNode]>; 3] {
            [self.root, self.children[0], self.children[1]].map(|node| {
                self.retained
                    .local_fragment(self.tree.node(node).unwrap().id)
                    .unwrap()
            })
        }

        fn assert_quiet(&mut self) {
            let before = self.locals();
            let frame = self.frame();
            assert!(!frame.scene_built && !frame.visual_changed);
            for (before, after) in before.iter().zip(self.locals()) {
                assert!(Arc::ptr_eq(before, &after));
            }
        }
    }

    fn outline_tokens(
        nodes: &[SceneNode],
    ) -> Vec<(RectF, crate::core::scene::Color, crate::core::scene::ZIndex)> {
        fn collect(
            nodes: &[SceneNode],
            tokens: &mut Vec<(RectF, crate::core::scene::Color, crate::core::scene::ZIndex)>,
        ) {
            for node in nodes {
                match node {
                    SceneNode::RetainedFragment(fragment) => collect(&fragment.children, tokens),
                    SceneNode::Rect(rect) => {
                        assert_eq!(rect.z_index, 99);
                        tokens.push((rect.rect, rect.color, rect.z_index));
                    }
                    _ => panic!("outline fixture emitted unexpected primitive"),
                }
            }
        }
        let mut tokens = Vec::new();
        collect(nodes, &mut tokens);
        tokens
    }

    #[test]
    fn app_owned_outlines_programmatic_changes_reuse_independent_apps() {
        let mut a = OutlineAppFixture::new();
        let mut b = OutlineAppFixture::new();
        assert!(!a.app.context.debug_outlines_enabled());
        assert!(!b.app.context.debug_outlines_enabled());
        assert!(outline_tokens(&a.frame().scene_root).is_empty());
        assert!(outline_tokens(&b.frame().scene_root).is_empty());
        a.assert_quiet();
        b.assert_quiet();
        let b_locals = b.locals();
        let structure = a.tree.structure_generation();
        let geometry =
            [a.root, a.children[0], a.children[1]].map(|node| a.tree.get_absolute_paint_rect(node));
        a.app.context.set_debug_outlines_enabled(true);
        assert!(a.app.context.debug_outlines_enabled());
        assert!(!b.app.context.debug_outlines_enabled());
        assert_eq!(a.tree.structure_generation(), structure);
        assert!(
            [a.root, a.children[0], a.children[1]].iter().all(|node| {
                let node = a.tree.node(*node).unwrap();
                !node.dirty_layout && !node.dirty_paint && !node.dirty_composition
            }),
            "setting outlines changes only the App value, not WidgetTree state"
        );
        let on = a.frame();
        assert!(on.scene_built && on.visual_changed);
        assert_eq!(outline_tokens(&on.scene_root).len(), 12);
        assert_eq!(
            geometry,
            [a.root, a.children[0], a.children[1]].map(|node| a.tree.get_absolute_paint_rect(node))
        );
        a.assert_quiet();
        b.assert_quiet();
        let locals = a.locals();
        {
            let _update = UpdateAuthorityGuard::enter();
            a.tree.mark_paint_dirty(a.children[0]);
        }
        assert!(a.frame().visual_changed);
        let narrowed = a.locals();
        assert!(Arc::ptr_eq(&locals[0], &narrowed[0]));
        assert!(!Arc::ptr_eq(&locals[1], &narrowed[1]));
        assert!(
            Arc::ptr_eq(&locals[2], &narrowed[2]),
            "unchanged ON never repaints the sibling"
        );
        a.assert_quiet();
        a.app.context.set_debug_outlines_enabled(false);
        assert!(!a.app.context.debug_outlines_enabled());
        let off = a.frame();
        assert!(off.scene_built && off.visual_changed);
        assert!(outline_tokens(&off.scene_root).is_empty());
        a.assert_quiet();
        a.app.context.set_debug_outlines_enabled(false);
        a.assert_quiet();
        a.app.context.set_debug_outlines_enabled(true);
        assert_eq!(outline_tokens(&a.frame().scene_root).len(), 12);
        a.app.context.set_debug_outlines_enabled(true);
        a.assert_quiet();
        a.app.context.set_debug_outlines_enabled(false);
        assert!(outline_tokens(&a.frame().scene_root).is_empty());
        a.assert_quiet();
        b.assert_quiet();
        for (before, after) in b_locals.iter().zip(b.locals()) {
            assert!(Arc::ptr_eq(before, &after));
        }
        assert!(!AppContext::new(Theme::default()).debug_outlines_enabled());
    }

    #[test]
    fn app_owned_outlines_f12_uses_ordinary_app_routing() {
        for developer_shortcuts in [true, false] {
            let config =
                AppConfig::new("Public app").with_developer_shortcuts_enabled(developer_shortcuts);
            let mut fixture = OutlineAppFixture::with_config(&config);
            assert!(outline_tokens(&fixture.frame().scene_root).is_empty());

            fixture.f12();
            assert!(!fixture.app.context.debug_outlines_enabled());
            fixture.assert_quiet();

            fixture.app.context.set_debug_outlines_enabled(true);
            assert_eq!(outline_tokens(&fixture.frame().scene_root).len(), 12);
            fixture.f12();
            assert!(fixture.app.context.debug_outlines_enabled());
            fixture.assert_quiet();

            let binding_calls = Rc::new(Cell::new(0));
            let calls = Rc::clone(&binding_calls);
            fixture.app.bind_shortcut(Shortcut::key(KeyCode::F12), move |_| {
                calls.set(calls.get() + 1);
                true
            });
            fixture.f12();
            assert_eq!(binding_calls.get(), 1);
            assert!(fixture.app.context.debug_outlines_enabled());
            fixture.assert_quiet();
        }
    }

    #[test]
    fn app_owned_outlines_developer_policy_guards_f9_before_capture_and_export() {
        use crate::core::event::{
            KeyboardAuthorityMode, KeyboardCaptureInterruptionReason, PhysicalKeyCode,
        };

        for (config, enabled) in [
            (
                AppConfig::default().with_developer_shortcuts_enabled(false),
                false,
            ),
            (AppConfig::default(), true),
        ] {
            let (mut app, mut tree, mut router, state, root, owner, trace) =
                physical_response_fixture_with_policy(true, AppContext::from_config(&config));
            let mut domains = config.interaction_domains();
            *domains.primary_state_mut() = state;
            let mut text = FontManagerTextEngine::new();
            assert!(router.handle_engine_action(
                &mut tree,
                crate::runtime::actions::ActionId::ENTER_WIDGET,
                Some(root),
                domains.primary_state_mut(),
                &mut text,
                None,
            ));
            let mut fusion = InputFusion::new();
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                domains.primary_state_mut(),
                &mut fusion,
                &mut text,
                physical_raw_key(
                    KeyCode::F12,
                    PhysicalKeyCode::F12,
                    Modifiers::default(),
                    false,
                    None,
                ),
            );
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            assert_eq!(trace.keys.borrow()[0].key_code, KeyCode::F12);
            assert!(trace.interruptions.borrow().is_empty());
            assert!(!app.context.debug_outlines_enabled());

            let event = FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::F9,
                physical_key: PhysicalKeyCode::F9,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            };
            let snapshot = app.capture_blueprint_shortcut(
                &event,
                &mut tree,
                &mut domains,
                &mut router,
                &mut text,
            );
            if enabled {
                let snapshot = snapshot.expect("default F9 captures the mounted tree for export");
                assert!(snapshot.widgets.contains_key(&owner.0));
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Navigation
                );
                let interruptions = trace.interruptions.borrow();
                assert_eq!(interruptions.len(), 1);
                assert_eq!(
                    interruptions[0].reason,
                    KeyboardCaptureInterruptionReason::AppConsumed
                );
            } else {
                assert!(snapshot.is_none());
                dispatch_physical_raw(
                    &mut app,
                    &mut tree,
                    &mut router,
                    domains.primary_state_mut(),
                    &mut fusion,
                    &mut text,
                    physical_raw_key(
                        KeyCode::F9,
                        PhysicalKeyCode::F9,
                        Modifiers::default(),
                        false,
                        None,
                    ),
                );
                let keys = trace.keys.borrow();
                assert_eq!(
                    keys.iter().map(|key| key.key_code).collect::<Vec<_>>(),
                    [KeyCode::F12, KeyCode::F9]
                );
                assert!(trace.interruptions.borrow().is_empty());
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
                assert!(!app.context.debug_outlines_enabled());
            }
            assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        }
    }

    #[test]
    fn app_owned_outlines_immutable_snapshot_matches_fresh_visual_tokens() {
        let mut f = OutlineAppFixture::new();
        f.frame();
        f.app.context.set_debug_outlines_enabled(true);
        f.tree.begin_layout_transition_frame();
        test_begin_frame_layout_tracking();
        let continuation = prepare_scene_paint(
            &mut f.tree,
            f.root,
            Constraints::tight(Size::new(160.0, 80.0)),
            &f.app.context.theme,
            &mut f.text,
            0,
            0.0,
            &mut f.retained.transition_layer,
            &mut f.retained.active_layout_transitions,
            None,
            1.0,
            None,
            &mut f.retained.custom_render_presentation_bindings,
            None,
            None,
            false,
            None,
            KeyboardAuthorityMode::Navigation,
            None,
            false,
            &mut f.retained.cache,
            None,
            None,
            f.app.context.debug_outlines_enabled(),
        );
        let SceneBuildContinuation::Paint(continuation) = continuation else {
            panic!("current outline snapshot must reach paint");
        };
        assert!(continuation.debug_outlines);
        f.app.context.set_debug_outlines_enabled(false);
        let outcome = resume_scene_paint(
            continuation,
            &mut f.tree,
            &f.app.context.theme,
            &mut f.text,
            &f.icons,
            &f.retained.transition_layer,
            &mut Vec::new(),
            TooltipSceneInput::Explicit(None),
            &mut f.retained.recycled_scene,
            &mut f.retained.cache,
        );
        let scene = outcome.scene.unwrap();
        let retained = outline_tokens(&scene.root);
        assert_eq!(retained.len(), 12);
        assert!(f.retained.cache.context_key.unwrap().debug_outlines);
        let rects = InterpolatedVisualRects::default();
        for scale in [1.0, 2.0] {
            let (fresh, _) = {
                let _paint = PaintAuthorityGuard::enter();
                paint_tree_fresh(
                    &f.tree,
                    f.root,
                    &f.app.context.theme,
                    &mut f.text,
                    &f.icons,
                    0,
                    f.retained.transition_layer.timeline(),
                    &rects,
                    scale,
                    None,
                    &mut Vec::new(),
                    None,
                    None,
                    KeyboardAuthorityMode::Navigation,
                    true,
                )
            };
            let tokens = outline_tokens(&fresh.root);
            if scale == 1.0 {
                assert_eq!(tokens, retained);
            }
            let mut expected = Vec::new();
            for (node, color) in [
                (
                    f.root,
                    crate::core::paint::Color::from_srgb(1.0, 0.0, 0.0, 0.8),
                ),
                (
                    f.children[0],
                    crate::core::paint::Color::from_srgb(0.0, 0.8, 0.0, 0.8),
                ),
                (
                    f.children[1],
                    crate::core::paint::Color::from_srgb(0.0, 0.8, 0.0, 0.8),
                ),
            ] {
                let r = f.tree.get_absolute_paint_rect(node).unwrap();
                let thickness = 3.0 / scale;
                for edge in [
                    RectF::new(r.x, r.y, r.width, thickness),
                    RectF::new(r.x, r.y + r.height - thickness, r.width, thickness),
                    RectF::new(r.x, r.y, thickness, r.height),
                    RectF::new(r.x + r.width - thickness, r.y, thickness, r.height),
                ] {
                    expected.push((edge, color.into(), 99));
                }
            }
            assert_eq!(
                tokens, expected,
                "original depth colors, Z and three physical pixels"
            );
        }
        assert!(
            outline_tokens(&f.frame().scene_root).is_empty(),
            "next frame consumes the owner's newer OFF value"
        );
        f.assert_quiet();
    }

    fn physical_raw_key(
        logical: KeyCode,
        physical: crate::core::event::PhysicalKeyCode,
        modifiers: Modifiers,
        repeat: bool,
        reservation: Option<crate::core::event::KeyboardKeyReservation>,
    ) -> RawInputEvent {
        RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code: logical,
            physical_key: physical,
            reservation,
            state: crate::core::event::KeyState::Down,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn physical_keyboard_capture_interrupts_before_fusion_shortcut_consumption() {
        use crate::core::event::{KeyboardAuthorityMode, PhysicalKeyCode, ShortcutScope};
        use crate::core::input_fusion::KeyChord;
        let (mut app, mut tree, mut router, mut state, root, owner, trace) =
            physical_response_fixture();
        let mut text = FontManagerTextEngine::new();
        let mut fusion = InputFusion::new();
        let modifiers = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        fusion.register_shortcut(
            ShortcutScope::Global,
            KeyChord::physical(PhysicalKeyCode::KeyA, modifiers).unwrap(),
        );
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('ф'),
                PhysicalKeyCode::KeyA,
                modifiers,
                false,
                None,
            ),
        );
        assert_eq!(trace.screen_shortcuts.get(), 1);
        assert_eq!(trace.pending_at_shortcut.get(), 0);
        assert_eq!(trace.interruptions.borrow().len(), 1);
        assert_eq!(trace.leaves.get(), 1);
        assert!(trace.pending.borrow().is_empty());
        assert_eq!(trace.committed.get(), 0);
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(state.focus_manager.focused(), Some(owner));
        assert!(trace.keys.borrow().is_empty());
        assert!(trace.responses.borrow().is_empty());
    }

    #[test]
    fn physical_keyboard_registered_global_reservation_preserves_other_modes() {
        use crate::core::event::{KeyboardAuthorityMode, PhysicalKeyCode, ShortcutScope};
        use crate::core::input_fusion::KeyChord;
        use crate::runtime::binding_registry::{
            Binding, BindingLayer, BindingPriority, InputPattern,
        };
        let modifiers = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        for (captured, physical) in [(false, true), (true, false), (true, true)] {
            let (mut app, mut tree, mut router, mut state, root, owner, trace) =
                physical_response_fixture_with_policy(physical, AppContext::new(Theme::dark()));
            let mut text = FontManagerTextEngine::new();
            let mut fusion = InputFusion::new();
            let normal = crate::runtime::actions::ActionId(900);
            app.context.registry.binding_registry.add(Binding {
                pattern: InputPattern::from_key_modifiers(KeyCode::Character('ф'), modifiers),
                action_id: normal,
                layer: BindingLayer::App,
                priority: BindingPriority::Normal,
                tag: None,
            });
            app.context.registry.binding_registry.add(Binding {
                pattern: InputPattern::from_physical_key_modifiers(
                    PhysicalKeyCode::KeyA,
                    modifiers,
                )
                .unwrap(),
                action_id: crate::runtime::actions::ActionId(901),
                layer: BindingLayer::Runtime,
                priority: BindingPriority::Soft,
                tag: None,
            });
            fusion.register_shortcut(
                ShortcutScope::Global,
                KeyChord::physical(PhysicalKeyCode::KeyA, modifiers).unwrap(),
            );
            if captured {
                assert!(router.handle_engine_action(
                    &mut tree,
                    crate::runtime::actions::ActionId::ENTER_WIDGET,
                    Some(root),
                    &mut state,
                    &mut text,
                    None,
                ));
            }
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                physical_raw_key(
                    KeyCode::Character('ф'),
                    PhysicalKeyCode::KeyA,
                    modifiers,
                    false,
                    None,
                ),
            );
            assert_eq!(
                app.context.registry.binding_registry.lookup_key(
                    KeyCode::Character('ф'),
                    PhysicalKeyCode::KeyA,
                    modifiers,
                ),
                Some(normal),
            );
            assert_eq!(trace.screen_shortcuts.get(), 1);
            assert_eq!(state.focus_manager.focused(), Some(owner));
            if captured && physical {
                assert_eq!(trace.screen_keys.get(), 0);
                assert_eq!(trace.pending_at_shortcut.get(), 0);
                assert!(trace.keys.borrow().is_empty());
                assert!(trace.responses.borrow().is_empty());
                assert_eq!(trace.interruptions.borrow().len(), 1);
                assert_eq!(trace.leaves.get(), 1);
                assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
            } else {
                assert_eq!(trace.screen_keys.get(), 1);
                assert!(trace.interruptions.borrow().is_empty());
                assert_eq!(trace.leaves.get(), 0);
                if captured {
                    assert_eq!(trace.keys.borrow().len(), 1);
                    let key = trace.keys.borrow()[0].clone();
                    assert_eq!(key.key_code, KeyCode::Character('ф'));
                    assert_eq!(key.physical_key, PhysicalKeyCode::KeyA);
                    assert_eq!(key.modifiers, modifiers);
                    assert_eq!(trace.pending_at_shortcut.get(), 1);
                    assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
                } else {
                    assert!(trace.keys.borrow().is_empty());
                    assert_eq!(trace.pending_at_shortcut.get(), 0);
                    assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
                }
            }
        }
    }

    #[test]
    fn physical_keyboard_keybind_editor_registered_global_does_not_commit() {
        use crate::core::event::{
            KeyboardAuthorityMode, KeyboardCaptureInterruptionReason, PhysicalBindingUnavailable,
            PhysicalKeyCode, ShortcutScope,
        };
        use crate::core::input_fusion::KeyChord;
        use crate::runtime::actions::Shortcut as RecordedShortcut;
        use crate::widgets::keybind_editor::{KeyRecordingUnavailable, KeybindEditor};
        let trace = Rc::new(PhysicalResponseTrace::default());
        let committed = Rc::clone(&trace);
        let unavailable = Rc::new(RefCell::new(Vec::new()));
        let unavailable_log = Rc::clone(&unavailable);
        let cancelled = Rc::new(Cell::new(0));
        let cancellation_log = Rc::clone(&cancelled);
        let original = RecordedShortcut::key_only(KeyCode::Character('p'));
        let editor = KeybindEditor::new()
            .with_value(Some(original))
            .with_physical_keys(true)
            .on_change(move |_| committed.committed.set(committed.committed.get() + 1))
            .on_unavailable(move |reason| unavailable_log.borrow_mut().push(reason))
            .on_cancel(move || cancellation_log.set(cancellation_log.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(editor), None, 0);
        let owner = tree.widget_id_for_node(root).unwrap();
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        app.push_screen(Box::new(PhysicalResponseScreen {
            host: ScreenHost::new(),
            trace: Rc::clone(&trace),
        }));
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        let mut fusion = InputFusion::new();
        let modifiers = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        fusion.register_shortcut(
            ShortcutScope::Global,
            KeyChord::physical(PhysicalKeyCode::KeyA, modifiers).unwrap(),
        );
        router.request_focus_for_widget(&mut tree, owner, &mut state);
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('ф'),
                PhysicalKeyCode::KeyA,
                modifiers,
                false,
                None,
            ),
        );
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(state.focus_manager.focused(), Some(owner));
        assert_eq!(trace.committed.get(), 0);
        assert_eq!(trace.screen_keys.get(), 0);
        assert_eq!(trace.screen_shortcuts.get(), 1);
        assert_eq!(cancelled.get(), 1);
        assert_eq!(
            unavailable.borrow().as_slice(),
            &[KeyRecordingUnavailable::PhysicalBinding(
                PhysicalBindingUnavailable::Reserved(
                    KeyboardCaptureInterruptionReason::AppConsumed
                )
            )],
        );
        assert_eq!(
            tree.node(root)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<KeybindEditor>()
                .unwrap()
                .value(),
            Some(original),
        );
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('й'),
                PhysicalKeyCode::KeyQ,
                Modifiers::default(),
                false,
                None,
            ),
        );
        assert_eq!(trace.committed.get(), 1);
        assert_eq!(cancelled.get(), 1);
        assert_eq!(trace.screen_shortcuts.get(), 1);
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(
            tree.node(root)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<KeybindEditor>()
                .unwrap()
                .value(),
            Some(
                RecordedShortcut::from_physical_key_and_modifiers(
                    PhysicalKeyCode::KeyQ,
                    Modifiers::default(),
                )
                .unwrap()
            ),
        );
    }

    #[test]
    fn physical_keyboard_capture_settles_before_reserved_actions() {
        use crate::core::event::{
            KeyboardAuthorityMode, KeyboardCaptureInterruptionReason as Reason,
            KeyboardKeyReservation, PhysicalKeyCode,
        };
        let (mut app, mut tree, mut router, mut state, root, owner, trace) =
            physical_response_fixture();
        let mut text = FontManagerTextEngine::new();
        let mut fusion = InputFusion::new();
        let action_count = Rc::new(Cell::new(0));
        let count = Rc::clone(&action_count);
        let observed = Rc::clone(&trace);
        app.bind_shortcut(Shortcut::key(KeyCode::F9), move |_cx| {
            assert!(observed.pending.borrow().is_empty());
            assert_eq!(observed.leaves.get(), 1);
            assert_eq!(
                observed.interruptions.borrow().last().unwrap().reason,
                Reason::AppConsumed
            );
            count.set(count.get() + 1);
            true
        });
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('a'),
                PhysicalKeyCode::KeyA,
                Modifiers::default(),
                false,
                None,
            ),
        );
        assert_eq!(trace.pending.borrow().len(), 1);
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::F9,
                PhysicalKeyCode::F9,
                Modifiers::default(),
                false,
                None,
            ),
        );
        assert_eq!(action_count.get(), 1);
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert!(EventRouter::physical_keyboard_capture_owner(&tree, &state).is_none());
        assert_eq!(state.focus_manager.focused(), Some(owner));
        app.flush_pending_actions_all(&mut tree);
        assert_eq!(trace.committed.get(), 0);

        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('x'),
                PhysicalKeyCode::KeyX,
                Modifiers::default(),
                false,
                None,
            ),
        );
        let modifiers = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        let reservation = KeyboardKeyReservation::from_observed_key(
            KeyCode::Character('F'),
            modifiers,
            true,
            false,
        );
        let raw = physical_raw_key(
            KeyCode::Character('F'),
            PhysicalKeyCode::KeyJ,
            modifiers,
            false,
            reservation,
        );
        let mut events = Vec::new();
        fusion.process_raw_with_gesture_sessions(
            raw,
            crate::core::event::InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            &tree,
            None,
            &mut events,
        );
        fusion.flush_with_gesture_sessions(&mut events);
        assert_eq!(events.len(), 1);
        let mut actuator_calls = 0;
        let mut actuator = |reservation| {
            assert_eq!(reservation, KeyboardKeyReservation::Fullscreen);
            assert!(trace.pending.borrow().is_empty());
            assert_eq!(trace.leaves.get(), 2);
            let interruptions = trace.interruptions.borrow();
            let interruption = interruptions.last().unwrap();
            assert_eq!(
                interruption.reason,
                Reason::Reserved(KeyboardKeyReservation::Fullscreen)
            );
            assert_eq!(
                interruption.key.as_ref().unwrap().physical_key,
                PhysicalKeyCode::KeyJ
            );
            actuator_calls += 1;
        };
        assert!(dispatch_session_input_to_app_root_with_reserved_action(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &events[0],
            &mut text,
            Some(&mut actuator),
        ));
        assert_eq!(actuator_calls, 1);
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert!(EventRouter::physical_keyboard_capture_owner(&tree, &state).is_none());
        assert_eq!(state.focus_manager.focused(), Some(owner));
        app.flush_pending_actions_all(&mut tree);
        assert_eq!(trace.committed.get(), 0);
    }

    #[test]
    fn physical_keyboard_capture_keeps_wrong_responses_and_settles_interruptions() {
        use crate::core::event::{
            KeyboardAuthorityMode, KeyboardCaptureInterruptionReason as Reason,
            KeyboardKeyReservation, PhysicalKeyCode,
        };
        let (mut app, mut tree, mut router, mut state, root, owner, trace) =
            physical_response_fixture();
        let mut fusion = InputFusion::new();
        let mut text = FontManagerTextEngine::new();
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None
        ));
        let mods = Modifiers {
            ctrl: true,
            meta: false,
            alt: false,
            shift: true,
        };
        for (logical, physical, repeat) in [
            (KeyCode::Character('ф'), PhysicalKeyCode::KeyA, false),
            (KeyCode::Character('w'), PhysicalKeyCode::KeyW, false),
            (KeyCode::Character('w'), PhysicalKeyCode::KeyW, true),
            (KeyCode::Shift, PhysicalKeyCode::ShiftRight, false),
        ] {
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                physical_raw_key(logical, physical, mods, repeat, None),
            );
        }
        assert_eq!(trace.responses.borrow().len(), 2);
        assert_eq!(
            trace.responses.borrow()[0].physical_key,
            PhysicalKeyCode::KeyA
        );
        assert_eq!(
            trace.responses.borrow()[0].key_code,
            KeyCode::Character('ф')
        );
        assert_eq!(trace.responses.borrow()[1].modifiers, mods);
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(state.focus_manager.focused(), Some(owner));
        assert_eq!(trace.screen_keys.get(), 0);
        app.flush_pending_actions_all(&mut tree);
        assert_eq!(trace.committed.get(), 2);

        app.bind_shortcut(Shortcut::new(KeyCode::F12, mods), |_| true);
        for (logical, physical, reservation, reason) in [
            (
                KeyCode::Character('F'),
                PhysicalKeyCode::KeyF,
                Some(KeyboardKeyReservation::Fullscreen),
                Reason::Reserved(KeyboardKeyReservation::Fullscreen),
            ),
            (
                KeyCode::F12,
                PhysicalKeyCode::F12,
                None,
                Reason::AppConsumed,
            ),
            (
                KeyCode::Escape,
                PhysicalKeyCode::Escape,
                None,
                Reason::Escape,
            ),
        ] {
            if state.keyboard_mode != KeyboardAuthorityMode::Capture {
                assert!(router.handle_engine_action(
                    &mut tree,
                    crate::runtime::actions::ActionId::ENTER_WIDGET,
                    Some(root),
                    &mut state,
                    &mut text,
                    None
                ));
            }
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                physical_raw_key(
                    KeyCode::Character('x'),
                    PhysicalKeyCode::KeyX,
                    Modifiers::default(),
                    false,
                    None,
                ),
            );
            assert_eq!(trace.pending.borrow().len(), 1);
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                physical_raw_key(logical, physical, mods, false, reservation),
            );
            assert_eq!(trace.interruptions.borrow().last().unwrap().reason, reason);
            assert_eq!(
                trace
                    .interruptions
                    .borrow()
                    .last()
                    .unwrap()
                    .key
                    .as_ref()
                    .unwrap()
                    .physical_key,
                physical
            );
            assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
            assert_eq!(state.focus_manager.focused(), Some(owner));
            app.flush_pending_actions_all(&mut tree);
            assert_eq!(
                trace.committed.get(),
                2,
                "no interrupted pending response may commit"
            );
        }
        assert_eq!(trace.leaves.get(), 3);
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('a'),
                PhysicalKeyCode::KeyA,
                Modifiers::default(),
                false,
                None,
            ),
        );
        let mut domains = InteractionDomains::new();
        domains.register_window(WindowId::PRIMARY);
        *domains.resolve_window_mut(WindowId::PRIMARY).unwrap() = state;
        handle_window_focus_changed(
            &mut domains,
            &mut router,
            &mut tree,
            WindowId::PRIMARY,
            false,
            &mut text,
        );
        assert_eq!(
            trace.interruptions.borrow().last().unwrap().reason,
            Reason::FocusLost
        );
        app.flush_pending_actions_all(&mut tree);
        assert_eq!(trace.committed.get(), 2);
        assert_eq!(trace.leaves.get(), 4);
        let responses_before_return = trace.responses.borrow().len();
        handle_window_focus_changed(
            &mut domains,
            &mut router,
            &mut tree,
            WindowId::PRIMARY,
            true,
            &mut text,
        );
        let returned = domains.resolve_window_mut(WindowId::PRIMARY).unwrap();
        assert_eq!(returned.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert!(EventRouter::physical_keyboard_capture_owner(&tree, returned).is_none());
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            returned,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('ж'),
                PhysicalKeyCode::KeyZ,
                Modifiers::default(),
                false,
                None,
            ),
        );
        assert_eq!(returned.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert!(EventRouter::physical_keyboard_capture_owner(&tree, returned).is_none());
        assert_eq!(trace.responses.borrow().len(), responses_before_return);
        assert!(trace.pending.borrow().is_empty());
        assert_eq!(trace.leaves.get(), 4);
    }

    #[test]
    fn physical_keyboard_capture_preserves_tab_and_logical_screen_defaults() {
        use crate::core::event::{KeyboardAuthorityMode, PhysicalKeyCode};
        let (mut app, mut tree, mut router, mut state, root, owner, trace) =
            physical_response_fixture();
        let activations = Rc::new(Cell::new(0));
        let next = tree.add_child(root, Box::new(ActivatingWidget::new(activations)), None, 1);
        let next_id = tree.widget_id_for_node(next).unwrap();
        let mut text = FontManagerTextEngine::new();
        let mut fusion = InputFusion::new();
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Tab,
                PhysicalKeyCode::Tab,
                Modifiers::default(),
                false,
                None,
            ),
        );
        assert_eq!(state.focus_manager.focused(), Some(next_id));
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(trace.leaves.get(), 1);
        assert!(trace.responses.borrow().is_empty());
        assert_eq!(trace.interruptions.borrow().len(), 1);
        assert_eq!(
            trace.interruptions.borrow()[0].reason,
            crate::core::event::KeyboardCaptureInterruptionReason::Tab
        );
        assert_eq!(
            trace.interruptions.borrow()[0]
                .key
                .as_ref()
                .unwrap()
                .physical_key,
            PhysicalKeyCode::Tab
        );
        let count = Rc::new(Cell::new(0));
        app.push_screen(Box::new(CountingScreen::new(Rc::clone(&count))));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('a'),
                PhysicalKeyCode::KeyA,
                Modifiers::default(),
                false,
                None,
            ),
        );
        assert_eq!(count.get(), 1);
        assert_ne!(state.focus_manager.focused(), Some(owner));
    }

    #[test]
    fn physical_keyboard_capture_selects_clipboard_key_or_text_once() {
        use crate::core::event::{
            KeyState, KeyboardAuthorityMode, KeyboardKeyReservation, PhysicalKeyCode,
        };
        let modifiers = Modifiers {
            ctrl: !cfg!(target_os = "macos"),
            meta: cfg!(target_os = "macos"),
            alt: false,
            shift: false,
        };
        for physical in [false, true] {
            let (mut app, mut tree, mut router, mut state, root, owner, trace) =
                physical_response_fixture_with_policy(physical, AppContext::new(Theme::dark()));
            let mut fusion = InputFusion::new();
            let mut text = FontManagerTextEngine::new();
            assert!(router.handle_engine_action(
                &mut tree,
                crate::runtime::actions::ActionId::ENTER_WIDGET,
                Some(root),
                &mut state,
                &mut text,
                None
            ));
            let time = Instant::now();
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code: KeyCode::Character('v'),
                    physical_key: PhysicalKeyCode::KeyV,
                    reservation: Some(KeyboardKeyReservation::Clipboard),
                    state: KeyState::Down,
                    repeat: false,
                    modifiers,
                },
            );
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time,
                    text: "v".into(),
                    modifiers,
                },
            );
            assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
            assert_eq!(state.focus_manager.focused(), Some(owner));
            assert!(trace.interruptions.borrow().is_empty());
            assert_eq!(trace.screen_keys.get(), 0);
            if !physical {
                assert!(trace.keys.borrow().is_empty());
                assert!(trace.responses.borrow().is_empty());
                assert_eq!(&*trace.text.borrow(), &[String::from("v")]);
                app.flush_pending_actions_all(&mut tree);
                assert_eq!(trace.committed.get(), 0);
                continue;
            }
            assert_eq!(trace.responses.borrow().len(), 1);
            assert_eq!(
                trace.responses.borrow()[0].physical_key,
                PhysicalKeyCode::KeyV
            );
            assert_eq!(trace.responses.borrow()[0].modifiers, modifiers);
            assert!(trace.text.borrow().is_empty());
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                physical_raw_key(
                    KeyCode::Character('v'),
                    PhysicalKeyCode::KeyV,
                    modifiers,
                    true,
                    Some(KeyboardKeyReservation::Clipboard),
                ),
            );
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code: KeyCode::Character('v'),
                    physical_key: PhysicalKeyCode::KeyV,
                    reservation: Some(KeyboardKeyReservation::Clipboard),
                    state: KeyState::Up,
                    repeat: false,
                    modifiers,
                },
            );
            assert_eq!(trace.keys.borrow().len(), 3);
            assert!(trace.keys.borrow()[1].is_repeat);
            assert!(!trace.keys.borrow()[2].is_down);
            assert_eq!(trace.responses.borrow().len(), 1);
            app.flush_pending_actions_all(&mut tree);
            assert_eq!(trace.committed.get(), 1);
            assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);

            let time = Instant::now();
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code: KeyCode::Character('v'),
                    physical_key: PhysicalKeyCode::KeyV,
                    reservation: Some(KeyboardKeyReservation::Clipboard),
                    state: KeyState::Down,
                    repeat: false,
                    modifiers,
                },
            );
            assert_eq!(trace.pending.borrow().len(), 1);
            assert!(router.handle_engine_action(
                &mut tree,
                crate::runtime::actions::ActionId::RELEASE_CAPTURE,
                Some(root),
                &mut state,
                &mut text,
                None
            ));
            dispatch_physical_raw(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &mut fusion,
                &mut text,
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time,
                    text: "v".into(),
                    modifiers,
                },
            );
            assert!(trace.text.borrow().is_empty());
            assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
            app.flush_pending_actions_all(&mut tree);
            assert_eq!(trace.committed.get(), 1);
            assert_eq!(trace.leaves.get(), 1);
        }
    }

    #[test]
    fn physical_keyboard_capture_unmount_discards_pending_and_fresh_owner_works() {
        use crate::core::event::{KeyboardAuthorityMode, PhysicalKeyCode};
        let (mut app, mut tree, mut router, mut state, root, owner, trace) =
            physical_response_fixture();
        let mut fusion = InputFusion::new();
        let mut text = FontManagerTextEngine::new();
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('a'),
                PhysicalKeyCode::KeyA,
                Modifiers::default(),
                false,
                None,
            ),
        );
        assert_eq!(trace.pending.borrow().len(), 1);
        let old_node = tree.find_node_by_widget_id(owner).unwrap();
        tree.remove_node(old_node);
        router.on_widgets_destroyed(&[owner], &mut state);
        router.clear_focus(&mut tree, &mut state);
        assert!(tree.find_node_by_widget_id(owner).is_none());
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(
            EventRouter::physical_keyboard_capture_owner(&tree, &state),
            None
        );
        assert_eq!(trace.unmounts.get(), 1);
        assert_eq!(trace.leaves.get(), 0, "no callback to a dead capture mount");
        assert!(trace.interruptions.borrow().is_empty());
        app.flush_pending_actions_all(&mut tree);
        assert_eq!(trace.committed.get(), 0);

        let new_node = tree.add_child(
            root,
            Box::new(PhysicalResponseWidget {
                id: WidgetInternalId::new(),
                trace: Rc::clone(&trace),
                active: false,
                physical: true,
            }),
            None,
            0,
        );
        let new_owner = tree.widget_id_for_node(new_node).unwrap();
        assert_eq!(new_node, old_node);
        assert_ne!(new_owner, owner);
        router.request_focus_for_widget(&mut tree, new_owner, &mut state);
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None
        ));
        dispatch_physical_raw(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            &mut text,
            physical_raw_key(
                KeyCode::Character('b'),
                PhysicalKeyCode::KeyB,
                Modifiers::default(),
                false,
                None,
            ),
        );
        app.flush_pending_actions_all(&mut tree);
        assert_eq!(trace.committed.get(), 1);
        assert_eq!(state.focus_manager.focused(), Some(new_owner));
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(trace.unmounts.get(), 1);
        assert_eq!(trace.leaves.get(), 0);
    }

    fn test_app_root(base_count: Rc<Cell<u32>>) -> (AppRoot, WidgetTree) {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);
        app_root.push_screen(Box::new(CountingScreen::new(base_count)));
        app_root.open_modal(Box::new(PopupBoundaryScreen::new()));
        (app_root, tree)
    }

    fn make_enter_event() -> FusedInputEvent {
        FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code: KeyCode::Enter,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: empty_modifiers(),
            repeat: false,
        }
    }

    fn app_with_focused_base_activation(
        activations: Rc<Cell<u32>>,
    ) -> (AppRoot, WidgetTree, EventRouter, InteractionDomainState) {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let base_node = tree.add_child(root, Box::new(ActivatingWidget::new(activations)), None, 0);
        let base_widget_id = tree.widget_id_for_node(base_node).unwrap();
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);
        let mut event_router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();

        event_router.request_focus_for_widget(&mut tree, base_widget_id, &mut domain_state);
        app_root.open_modal(Box::new(MountedModalScreen::new()));
        app_root.rebuild_active_view(&mut tree);

        (app_root, tree, event_router, domain_state)
    }

    pub(crate) struct RecordingInputScreen {
        pub(crate) host: ScreenHost,
        pub(crate) events: Rc<RefCell<Vec<FusedInputEvent>>>,
    }

    impl Screen for RecordingInputScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            cx.mount_child(Box::new(ScreenHost::new()));
            &mut self.host
        }

        fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            self.events.borrow_mut().push(event.clone());
            false
        }
    }

    fn qualified_escape_input(
        tree: &WidgetTree,
        state: &mut InteractionDomainState,
        modifiers: Modifiers,
        repeat: bool,
    ) -> crate::core::event::SessionQualifiedInput {
        let mut fusion = InputFusion::new();
        let mut events = Vec::new();
        fusion.process_raw_with_gesture_sessions(
            physical_raw_key(
                KeyCode::Escape,
                crate::core::event::PhysicalKeyCode::Escape,
                modifiers,
                repeat,
                None,
            ),
            crate::core::event::InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            tree,
            None,
            &mut events,
        );
        fusion.flush_with_gesture_sessions(&mut events);
        assert_eq!(events.len(), 1);
        events.pop().unwrap()
    }

    #[test]
    fn root_escape_delivers_original_screen_input_once() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        app.push_screen(Box::new(RecordingInputScreen {
            host: ScreenHost::new(),
            events: Rc::clone(&events),
        }));
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        let modifiers = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        };

        for (index, repeat) in [false, true].into_iter().enumerate() {
            let input = qualified_escape_input(&tree, &mut state, modifiers, repeat);
            let original_time = match input.event() {
                FusedInputEvent::KeyDown { time, .. } => *time,
                _ => panic!("Escape must normalize to KeyDown"),
            };
            assert!(dispatch_session_input_to_app_root(
                &mut app,
                &mut tree,
                &mut router,
                &mut state,
                &input,
                &mut text,
            ));
            let recorded = events.borrow();
            assert_eq!(recorded.len(), index + 1);
            match &recorded[index] {
                FusedInputEvent::KeyDown {
                    window_id,
                    time,
                    key_code,
                    physical_key,
                    reservation,
                    modifiers: delivered_modifiers,
                    repeat: delivered_repeat,
                } => {
                    assert_eq!(*window_id, WindowId::PRIMARY);
                    assert_eq!(*time, original_time);
                    assert_eq!(*key_code, KeyCode::Escape);
                    assert_eq!(*physical_key, crate::core::event::PhysicalKeyCode::Escape);
                    assert_eq!(*reservation, None);
                    assert_eq!(*delivered_modifiers, modifiers);
                    assert_eq!(*delivered_repeat, repeat);
                }
                _ => panic!("Screen must receive the original Escape KeyDown"),
            }
            assert_eq!(state.swallow_text_at, Some(original_time));
            assert!(state.focus_manager.focused().is_none());
            assert!(!app.context.exit_requested());
        }
    }

    #[test]
    fn logical_capture_escape_releases_before_screen_and_global_shortcut() {
        use crate::core::event::KeyboardAuthorityMode;
        use crate::prelude::TextField;

        let events = Rc::new(RefCell::new(Vec::new()));
        let global_count = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let field_node = tree.add_child(
            root,
            Box::new(TextField::new().with_text("current edited text")),
            None,
            0,
        );
        let field_id = tree.widget_id_for_node(field_node).unwrap();
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        app.push_screen(Box::new(RecordingInputScreen {
            host: ScreenHost::new(),
            events: Rc::clone(&events),
        }));
        let handled = Rc::clone(&global_count);
        app.bind_shortcut(Shortcut::key(KeyCode::Escape), move |_cx| {
            handled.set(handled.get() + 1);
            true
        });
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        router.request_focus_for_widget(&mut tree, field_id, &mut state);
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);

        let input = qualified_escape_input(&tree, &mut state, Modifiers::default(), false);
        assert!(dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &input,
            &mut text,
        ));
        assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(state.focus_manager.focused(), Some(field_id));
        assert!(events.borrow().is_empty());
        assert_eq!(global_count.get(), 0);
        assert_eq!(
            tree.node(field_node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextField>()
                .unwrap()
                .text(),
            "current edited text",
        );

        let input = qualified_escape_input(&tree, &mut state, Modifiers::default(), false);
        assert!(dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &input,
            &mut text,
        ));
        assert_eq!(events.borrow().len(), 1);
        assert_eq!(global_count.get(), 1);
        assert_eq!(state.focus_manager.focused(), Some(field_id));
    }

    #[test]
    fn structural_scope_escape_is_consumed_before_screen_input() {
        let count = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let scope = tree.add_child(root, Box::new(ScopeModalRoot::new()), None, 0);
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        app.push_screen(Box::new(CountingScreen::new(Rc::clone(&count))));
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        router.push_focus_scope(scope, &mut state);
        assert_eq!(state.focus_manager.active_scope(), Some(scope));

        let input = qualified_escape_input(&tree, &mut state, Modifiers::default(), false);
        assert!(dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &input,
            &mut text,
        ));
        assert_eq!(state.focus_manager.scope_depth(), 0);
        assert_eq!(count.get(), 0);
    }

    #[test]
    fn synchronized_modal_escape_dismisses_without_base_screen_delivery() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        app.push_screen(Box::new(RecordingInputScreen {
            host: ScreenHost::new(),
            events: Rc::clone(&events),
        }));
        app.open_modal(Box::new(FocusableModalScreen::new(Rc::new(Cell::new(0)))));
        app.rebuild_active_view(&mut tree);
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        router.sync_focus_scopes(&mut tree, &app.modal_mounted_nodes(), &mut state);
        router.ensure_focus_within_active_scope(&mut tree, &mut state);
        let modal_root = app.active_modal_input_root_node().unwrap();
        assert_eq!(state.focus_manager.active_scope(), Some(modal_root));
        assert!(tree.is_descendant_of_node(state.focus_manager.focused().unwrap(), modal_root));

        let input = qualified_escape_input(&tree, &mut state, Modifiers::default(), false);
        assert!(dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &input,
            &mut text,
        ));
        assert!(app.modals.is_empty());
        assert!(app.overlay_root.is_empty());
        assert!(events.borrow().is_empty());
        app.rebuild_active_view(&mut tree);
        router.sync_focus_scopes(&mut tree, &app.modal_mounted_nodes(), &mut state);
        assert_eq!(state.focus_manager.scope_depth(), 0);
    }

    #[test]
    fn modal_escape_preserves_parent_focus_before_dismissal() {
        let activations = Rc::new(Cell::new(0));
        let (mut app, mut tree, mut router, mut state) =
            app_with_focused_base_activation(Rc::clone(&activations));
        let parent_focus = state.focus_manager.focused();
        assert!(parent_focus.is_some());
        let mut text = FontManagerTextEngine::new();
        let input = qualified_escape_input(&tree, &mut state, Modifiers::default(), false);

        assert!(dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &input,
            &mut text,
        ));
        assert!(app.modals.is_empty());
        assert!(app.overlay_root.is_empty());
        assert_eq!(state.focus_manager.focused(), parent_focus);
        assert_eq!(activations.get(), 0);
    }

    #[test]
    fn overlay_escape_dismisses_before_base_screen_input() {
        let base_count = Rc::new(Cell::new(0));
        let (mut app_root, mut tree) = test_app_root(Rc::clone(&base_count));
        let event = FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code: KeyCode::Escape,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: empty_modifiers(),
            repeat: false,
        };

        assert!(app_root.handle_fused_input_modal_first(&event, &mut tree));
        assert_eq!(base_count.get(), 0);
        assert!(app_root.modals.is_empty());
        assert!(app_root.overlay_root.is_empty());
    }

    #[test]
    fn overlay_outside_pointer_down_dismisses_without_clickthrough() {
        let base_count = Rc::new(Cell::new(0));
        let (mut app_root, mut tree) = test_app_root(Rc::clone(&base_count));
        let event = FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(100.0, 100.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        };

        assert!(app_root.handle_fused_input_modal_first(&event, &mut tree));
        assert_eq!(base_count.get(), 0);
        assert!(app_root.modals.is_empty());
        assert!(app_root.overlay_root.is_empty());
    }

    #[test]
    fn overlay_inside_pointer_down_bypasses_base_screen() {
        let base_count = Rc::new(Cell::new(0));
        let (mut app_root, mut tree) = test_app_root(Rc::clone(&base_count));
        let event = FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(10.0, 10.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        };

        assert!(!app_root.handle_fused_input_modal_first(&event, &mut tree));
        assert_eq!(base_count.get(), 0);
        assert_eq!(app_root.modals.len(), 1);
        assert_eq!(app_root.overlay_root.len(), 1);
    }

    #[test]
    fn modal_boundary_clears_base_focus_before_keyboard_dispatch() {
        let activations = Rc::new(Cell::new(0));
        let (mut app_root, mut tree, mut event_router, mut domain_state) =
            app_with_focused_base_activation(Rc::clone(&activations));
        let event = make_enter_event();

        assert!(!app_root.handle_fused_input_modal_first(&event, &mut tree));
        let input_root = app_root.active_input_root_node();
        assert_ne!(input_root, app_root.active_root_node());
        event_router.enforce_modal_input_boundary(&mut tree, input_root, &mut domain_state);

        let mut text_engine = FontManagerTextEngine::new();
        let handled = dispatch_engine_action(
            &event,
            &mut app_root.context.registry.binding_registry,
            &mut event_router,
            &mut tree,
            input_root,
            &mut domain_state,
            &mut text_engine,
            None,
        );

        assert!(!handled);
        assert_eq!(activations.get(), 0);
        assert!(domain_state.focus_manager.focused().is_none());
    }

    #[test]
    fn modal_tab_entry_does_not_restore_base_last_focus() {
        let activations = Rc::new(Cell::new(0));
        let (app_root, mut tree, mut event_router, mut domain_state) =
            app_with_focused_base_activation(activations);
        let input_root = app_root.active_input_root_node();

        event_router.enforce_modal_input_boundary(&mut tree, input_root, &mut domain_state);
        assert!(domain_state.focus_manager.focused().is_none());

        let handled =
            event_router.handle_tab_navigation(&mut tree, input_root, false, &mut domain_state);

        assert!(!handled);
        assert!(domain_state.focus_manager.focused().is_none());
    }

    #[test]
    fn modal_scope_entry_moves_focus_inside_modal_scope() {
        let base_activations = Rc::new(Cell::new(0));
        let modal_activations = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let base_node = tree.add_child(
            root,
            Box::new(ActivatingWidget::new(Rc::clone(&base_activations))),
            None,
            0,
        );
        let base_widget_id = tree.widget_id_for_node(base_node).unwrap();
        let mut app_root = AppRoot::new(AppContext::new(Theme::dark()), root);
        let mut event_router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();

        event_router.request_focus_for_widget(&mut tree, base_widget_id, &mut domain_state);
        app_root.open_modal(Box::new(FocusableModalScreen::new(Rc::clone(
            &modal_activations,
        ))));
        app_root.rebuild_active_view(&mut tree);

        let modal_nodes = app_root.modal_mounted_nodes();
        event_router.sync_focus_scopes(&mut tree, &modal_nodes, &mut domain_state);
        event_router.ensure_focus_within_active_scope(&mut tree, &mut domain_state);

        let modal_root = app_root.active_modal_input_root_node().unwrap();
        let focused = domain_state.focus_manager.focused().unwrap();
        assert!(tree.is_descendant_of_node(focused, modal_root));
        assert_ne!(focused, base_widget_id);
    }

    #[test]
    fn showcase_ordered_deferred_focus_cancels_old_capture_before_reentering_new_capture() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let viewport_probe = Rc::new(LifecycleProbe::default());
        let control_probe = Rc::new(LifecycleProbe::default());
        let viewport_node = tree.add_child(
            root,
            Box::new(LifecycleProbeWidget::new(
                Rc::clone(&viewport_probe),
                true,
                true,
            )),
            None,
            0,
        );
        let control_node = tree.add_child(
            root,
            Box::new(LifecycleProbeWidget::new(
                Rc::clone(&control_probe),
                false,
                false,
            )),
            None,
            1,
        );
        let viewport_id = tree.widget_id_for_node(viewport_node).unwrap();
        let mut event_router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        let mut text_engine = FontManagerTextEngine::new();

        event_router.request_focus_for_widget(&mut tree, viewport_id, &mut domain_state);
        event_router.enter_capture_for_widget(
            &mut tree,
            viewport_id,
            viewport_node,
            &mut domain_state,
        );
        domain_state
            .pointer_capture_manager
            .request_capture(PointerId::MOUSE, viewport_id)
            .unwrap();
        domain_state
            .pointer_capture
            .capture(viewport_id, PointerButton::Primary);

        tree.defer_focus_sequence([control_node, viewport_node]);
        apply_deferred_focus_requests(
            &mut tree,
            &mut event_router,
            &mut domain_state,
            &mut text_engine,
        );

        assert_eq!(viewport_probe.capture_leaves.get(), 1);
        assert_eq!(viewport_probe.blurs.get(), 1);
        assert_eq!(viewport_probe.capture_enters.get(), 2);
        assert_eq!(control_probe.blurs.get(), 1);
        assert_eq!(domain_state.focus_manager.focused(), Some(viewport_id));
        assert_eq!(
            domain_state.keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        assert!(
            domain_state
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none(),
            "the old interaction must release pointer capture before the new mode enters capture"
        );
        assert_eq!(
            event_router.take_requested_cursor(),
            Some(CursorIcon::Default)
        );
    }

    fn touch_contact_button_fixture() -> (
        AppRoot,
        WidgetTree,
        NodeId,
        FontManagerTextEngine,
        Rc<Cell<u32>>,
    ) {
        let activations = Rc::new(Cell::new(0));
        let callback = Rc::clone(&activations);
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                crate::widgets::button::Button::with_text("Tap").on_click(move || {
                    callback.set(callback.get() + 1);
                }),
            ),
            None,
            0,
        );
        let theme = Theme::dark();
        let mut text = FontManagerTextEngine::new();
        RetainedSceneTestHarness::new().frame(
            &mut tree,
            root,
            Constraints::loose(Size::new(200.0, 100.0)),
            &theme,
            &mut text,
            &crate::core::icon::IconRegistry::new(&[]),
        );
        (
            AppRoot::new(AppContext::new(theme), root),
            tree,
            root,
            text,
            activations,
        )
    }

    fn queue_touch_contact(
        fusion: &mut InputFusion,
        state: &mut InteractionDomainState,
        tree: &WidgetTree,
        pending: &mut Vec<crate::core::event::SessionQualifiedInput>,
        contact_id: u64,
        phase: crate::core::event::TouchPhase,
    ) {
        fusion.process_raw_with_gesture_sessions(
            RawInputEvent::Touch {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                contact_id,
                phase,
                pos: Point::new(10.0, 10.0),
            },
            crate::core::event::InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            tree,
            None,
            pending,
        );
    }

    #[test]
    fn touch_contact_cancel_bypasses_screen_and_modal_consumption() {
        use crate::core::event::TouchPhase;
        let (mut app, mut tree, root, mut text, activations) = touch_contact_button_fixture();
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut fusion = InputFusion::new();
        let mut pending = Vec::new();
        queue_touch_contact(
            &mut fusion,
            &mut state,
            &tree,
            &mut pending,
            1,
            TouchPhase::Started,
        );
        let down = pending.pop().unwrap();
        let identity = touch_contact_identity(down.event()).unwrap();
        router.route_session_input_event(
            &mut tree,
            down,
            Some(root),
            &mut text,
            None,
            &mut state,
            &mut app.context.registry.binding_registry,
        );
        assert!(state.pointer_capture_manager.is_captured(identity.1));
        let base_count = Rc::new(Cell::new(0));
        let modal_count = Rc::new(Cell::new(0));
        app.push_screen(Box::new(CountingScreen::new(Rc::clone(&base_count))));
        app.open_modal(Box::new(CountingScreen::new(Rc::clone(&modal_count))));
        app.modal_dismiss_touch_sequences.insert(identity);
        queue_touch_contact(
            &mut fusion,
            &mut state,
            &tree,
            &mut pending,
            1,
            TouchPhase::Cancelled,
        );
        let cancel = pending.pop().unwrap();
        assert!(dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &cancel,
            &mut text,
        ));
        assert!(!state.pointer_capture_manager.is_captured(identity.1));
        assert!(!state.gesture_registry.has_session(identity.1));
        assert!(!app.modal_dismiss_touch_sequences.contains(&identity));
        assert!(cancel.is_replayed());
        assert_eq!(base_count.get(), 0);
        assert_eq!(modal_count.get(), 0);
        assert_eq!(activations.get(), 0);
    }

    #[test]
    fn touch_contact_outside_dismiss_shields_only_its_contact_until_terminal() {
        for cancelled in [false, true] {
            let base_count = Rc::new(Cell::new(0));
            let (mut app, mut tree) = test_app_root(Rc::clone(&base_count));
            let down = FusedInputEvent::TouchDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pointer_id: PointerId(7),
                pos: Point::new(100.0, 100.0),
                button: PointerButton::Primary,
                click_count: 1,
            };
            assert!(app.handle_fused_input_modal_first(&down, &mut tree));
            assert!(app.modals.is_empty());
            let other = FusedInputEvent::TouchDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pointer_id: PointerId(8),
                pos: Point::new(10.0, 10.0),
                button: PointerButton::Primary,
                click_count: 1,
            };
            assert!(app.handle_fused_input_modal_first(&other, &mut tree));
            assert_eq!(base_count.get(), 1);
            let moved = FusedInputEvent::TouchMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pointer_id: PointerId(7),
                pos: Point::new(10.0, 10.0),
                delta: (-90.0, -90.0),
                buttons: crate::core::event::PointerButtonsState::new(),
            };
            assert!(app.handle_fused_input_modal_first(&moved, &mut tree));
            let terminal = if cancelled {
                FusedInputEvent::TouchCancel {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pointer_id: PointerId(7),
                    pos: Point::new(10.0, 10.0),
                }
            } else {
                FusedInputEvent::TouchUp {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pointer_id: PointerId(7),
                    pos: Point::new(10.0, 10.0),
                    button: PointerButton::Primary,
                }
            };
            assert!(app.handle_fused_input_modal_first(&terminal, &mut tree));
            assert_eq!(base_count.get(), 1);
            assert!(app.modal_dismiss_touch_sequences.is_empty());
            assert!(!app.modal_dismiss_pointer_sequence_active);
        }
    }

    #[test]
    fn touch_contact_suspend_settles_live_queued_and_deferred_input() {
        use crate::core::event::{SessionQualifiedInput, TouchPhase};
        let (mut app, mut tree, root, mut text, activations) = touch_contact_button_fixture();
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut fusion = InputFusion::new();
        let mut pending = Vec::new();
        queue_touch_contact(
            &mut fusion,
            &mut state,
            &tree,
            &mut pending,
            1,
            TouchPhase::Started,
        );
        let down = pending.pop().unwrap();
        let identity = touch_contact_identity(down.event()).unwrap();
        router.route_session_input_event(
            &mut tree,
            down,
            Some(root),
            &mut text,
            None,
            &mut state,
            &mut app.context.registry.binding_registry,
        );
        assert!(state.pointer_capture_manager.is_captured(identity.1));
        queue_touch_contact(
            &mut fusion,
            &mut state,
            &tree,
            &mut pending,
            2,
            TouchPhase::Started,
        );
        let queued_down = pending.last().unwrap().clone();
        queue_touch_contact(
            &mut fusion,
            &mut state,
            &tree,
            &mut pending,
            2,
            TouchPhase::Ended,
        );
        queue_touch_contact(
            &mut fusion,
            &mut state,
            &tree,
            &mut pending,
            3,
            TouchPhase::Started,
        );
        let other_window = WindowId(2);
        pending.push(SessionQualifiedInput::unqualified(
            FusedInputEvent::TouchDown {
                window_id: other_window,
                time: Instant::now(),
                pointer_id: PointerId(99),
                pos: Point::new(10.0, 10.0),
                button: PointerButton::Primary,
                click_count: 1,
            },
        ));
        fusion.process_raw_with_gesture_sessions(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                device: PointerDevice::Mouse,
                position: Point::new(7.0, 9.0),
                delta: (7.0, 9.0),
            },
            crate::core::event::InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            &tree,
            None,
            &mut pending,
        );
        let raw_touch = |window_id| RawInputEvent::Touch {
            window_id,
            time: Instant::now(),
            contact_id: 88,
            phase: TouchPhase::Started,
            pos: Point::new(10.0, 10.0),
        };
        let mut deferred = VecDeque::from([
            DeferredAppInput::Raw(raw_touch(WindowId::PRIMARY)),
            DeferredAppInput::Raw(raw_touch(other_window)),
        ]);
        app.modal_dismiss_touch_sequences.insert(identity);
        app.modal_dismiss_touch_sequences
            .insert((other_window, PointerId(99)));
        cancel_window_touch_input(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &mut fusion,
            WindowId::PRIMARY,
            &mut pending,
            &mut deferred,
        );
        assert!(!state.pointer_capture_manager.is_captured(identity.1));
        assert!(state.gesture_registry.active_pointers().is_empty());
        assert!(queued_down.is_replayed());
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|input| matches!(input.event(), FusedInputEvent::PointerMove { pos, .. } if *pos == Point::new(7.0, 9.0))));
        assert!(
            pending
                .iter()
                .any(|input| touch_contact_identity(input.event())
                    == Some((other_window, PointerId(99))))
        );
        assert_eq!(deferred.len(), 1);
        assert!(matches!(&deferred[0],
            DeferredAppInput::Raw(raw) if raw.window_id() == other_window));
        assert_eq!(app.modal_dismiss_touch_sequences.len(), 1);
        assert!(
            app.modal_dismiss_touch_sequences
                .contains(&(other_window, PointerId(99)))
        );
        let mut late = Vec::new();
        queue_touch_contact(
            &mut fusion,
            &mut state,
            &tree,
            &mut late,
            1,
            TouchPhase::Ended,
        );
        assert!(late.is_empty());
        assert_eq!(activations.get(), 0);
    }

    #[test]
    fn outside_dismiss_consumes_followup_pointer_up_after_modal_closes() {
        let base_count = Rc::new(Cell::new(0));
        let (mut app_root, mut tree) = test_app_root(Rc::clone(&base_count));
        let down = FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(100.0, 100.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        };
        let up = FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(100.0, 100.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        };

        assert!(app_root.handle_fused_input_modal_first(&down, &mut tree));
        assert!(app_root.modals.is_empty());
        assert!(app_root.overlay_root.is_empty());
        assert!(app_root.handle_fused_input_modal_first(&up, &mut tree));
        assert_eq!(base_count.get(), 0);
    }
