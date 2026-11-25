    use super::*;
    use crate::core::event::{
        CursorIcon, Event, EventPhase, InteractionDomainId, KeyCode, KeyState, Modifiers,
        PhysicalKeyCode, PointerButton, PointerDevice, PointerEventKind, PointerId, RawButtonState,
        RawInputEvent, UiInteractionPolicy,
    };
    use crate::core::paint::TrackDisplayStyle;
    use crate::core::widget::{EventContext, Widget, WidgetDowncast};
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::utils::id::WidgetInternalId;
    use crate::widgets::combo_box::ComboBox;
    use crate::widgets::screen_host::ScreenHost;
    use crate::widgets::slider::{Slider, ValueFieldSide};
    use crate::widgets::spin_box::SpinBox;
    use crate::widgets::tag_input::{
        TagCreationMode, TagDisplayMode, TagInput, TagInputPolicy, TagItem,
    };
    use crate::widgets::text_area::{TextArea, TextAreaConfig};
    use crate::widgets::text_edit::TextEdit;
    use crate::widgets::text_field::{TextField, TextFieldConfig};
    use web_time::Instant;

    struct Fixture {
        tree: WidgetTree,
        root: NodeId,
        app: AppRoot,
        router: EventRouter,
        domains: InteractionDomains,
        fusion: InputFusion,
        text: FontManagerTextEngine,
        retained: RetainedSceneTestHarness,
        icon: CursorIcon,
        visible: bool,
        output: Vec<(Option<bool>, Option<CursorIcon>)>,
        cursor_sender: Option<cursor_output::CursorOutputSender>,
    }
    impl Fixture {
        fn new(widget: Box<dyn Widget>, policy: UiInteractionPolicy) -> Self {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(widget, None, 0);
            Self::from_tree(tree, root, policy)
        }
        fn from_tree(tree: WidgetTree, root: NodeId, policy: UiInteractionPolicy) -> Self {
            let mut this = Self {
                tree,
                root,
                app: AppRoot::new(AppContext::new(Theme::dark()), root),
                router: EventRouter::new(),
                domains: AppConfig::default()
                    .with_ui_interaction_policy(policy)
                    .interaction_domains(),
                fusion: InputFusion::new(),
                text: FontManagerTextEngine::new(),
                retained: RetainedSceneTestHarness::new(),
                icon: CursorIcon::Default,
                visible: true,
                output: Vec::new(),
                cursor_sender: None,
            };
            this.frame();
            this
        }
        fn frame(&mut self) {
            self.retained.frame_with_appearance(
                &mut self.tree,
                self.root,
                Constraints::loose(Size::new(300.0, 120.0)),
                &Theme::dark(),
                &mut self.text,
                &crate::core::icon::IconRegistry::new(&[]),
                &self.domains,
                WindowId::PRIMARY,
            );
        }
        fn settle(&mut self, default: bool, relative: bool) {
            if let Some(sender) = &self.cursor_sender {
                publish_ui_cursor(Some(sender), &mut self.router, default, relative);
                return;
            }
            apply_event_router_cursor(&mut self.router, default, relative, |visible, icon| {
                self.output.push((visible, icon));
                if let Some(value) = visible {
                    self.visible = value;
                }
                if let Some(value) = icon {
                    self.icon = value;
                }
            });
        }
        fn batch(&mut self, raw: Vec<RawInputEvent>) -> usize {
            let mut inputs = Vec::new();
            for raw in raw {
                self.fusion.process_raw_with_gesture_sessions(
                    raw,
                    InteractionDomainId::PRIMARY,
                    &mut self.domains.primary_state_mut().gesture_registry,
                    &self.tree,
                    None,
                    &mut inputs,
                );
            }
            self.fusion.flush_with_gesture_sessions(&mut inputs);
            let count = inputs.len();
            let mut default = false;
            self.router.reset_cursor_request();
            for input in inputs {
                self.settle(
                    default,
                    EventRouter::relative_ingress_gate(self.domains.primary_state()).is_some(),
                );
                default = false;
                if !self.domains.should_route_event(input.event()) {
                    continue;
                }
                let Some(state) = self.domains.resolve_window_mut(input.event().window_id()) else {
                    continue;
                };
                if !prepare_session_input_for_app(&mut self.tree, &mut self.router, state, &input) {
                    continue;
                }
                default = cursor_default_event(
                    input.event(),
                    state.pointer_capture_manager.is_captured(PointerId::MOUSE),
                );
                if dispatch_session_input_to_app_root(
                    &mut self.app,
                    &mut self.tree,
                    &mut self.router,
                    state,
                    &input,
                    &mut self.text,
                ) {
                    continue;
                }
                if dispatch_engine_action(
                    input.event(),
                    &mut self.app.context.registry.binding_registry,
                    &mut self.router,
                    &mut self.tree,
                    self.root,
                    state,
                    &mut self.text,
                    None,
                ) {
                    continue;
                }
                self.router.route_session_input_event(
                    &mut self.tree,
                    input,
                    Some(self.root),
                    &mut self.text,
                    None,
                    state,
                    &mut self.app.context.registry.binding_registry,
                );
            }
            self.settle(
                default,
                EventRouter::relative_ingress_gate(self.domains.primary_state()).is_some(),
            );
            count
        }
        fn hover(&mut self, point: Point, expected: CursorIcon) {
            self.batch(vec![moved(point)]);
            assert_eq!(self.icon, expected, "pointer at {point:?}");
        }
        fn rect(&self) -> RectF {
            self.tree.get_absolute_paint_rect(self.root).unwrap()
        }
        fn assert_no_capture(&self) {
            let state = self.domains.primary_state();
            assert_eq!(state.focus_manager.focused(), None);
            assert_eq!(
                state.keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Navigation
            );
            assert!(!state.pointer_capture_manager.is_captured(PointerId::MOUSE));
        }
    }
    fn moved(position: Point) -> RawInputEvent {
        RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            position,
            delta: (1.0, 0.0),
        }
    }
    fn key(state: KeyState) -> RawInputEvent {
        RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code: KeyCode::Shift,
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            state,
            repeat: false,
            modifiers: Modifiers::default(),
        }
    }
    fn button(position: Point, button: PointerButton, state: RawButtonState) -> RawInputEvent {
        RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            button,
            state,
            position,
        }
    }
    struct ConsumeOutside {
        host: ScreenHost,
        consumed: Rc<Cell<usize>>,
    }
    impl Screen for ConsumeOutside {
        fn build<'a>(&'a mut self, _cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            &mut self.host
        }
        fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
            if matches!(event, FusedInputEvent::PointerMove { pos, .. } if pos.x > 300.0) {
                self.consumed.set(self.consumed.get() + 1);
                true
            } else {
                false
            }
        }
    }
    #[test]
    fn cursor_host_settles_real_fusion_batches_and_app_consumed_pointer_exits() {
        for policy in [
            UiInteractionPolicy::NavigationAndCapture,
            UiInteractionPolicy::PointerDriven,
        ] {
            for consume in [false, true] {
                let mut f = Fixture::new(Box::new(TextField::new().with_text("kept")), policy);
                let consumed = Rc::new(Cell::new(0));
                if consume {
                    f.app.push_screen(Box::new(ConsumeOutside {
                        host: ScreenHost::new(),
                        consumed: Rc::clone(&consumed),
                    }));
                }
                assert!(
                    f.batch(vec![
                        moved(Point::new(20.0, 10.0)),
                        key(KeyState::Down),
                        moved(Point::new(400.0, 10.0)),
                        key(KeyState::Up)
                    ]) >= 4
                );
                assert!(
                    f.output
                        .iter()
                        .any(|(_, icon)| *icon == Some(CursorIcon::Text))
                );
                assert_eq!(f.icon, CursorIcon::Default);
                assert_eq!(consumed.get(), usize::from(consume));
                f.output.clear();
                f.batch(vec![
                    moved(Point::new(20.0, 10.0)),
                    key(KeyState::Down),
                    key(KeyState::Up),
                ]);
                assert_eq!(f.icon, CursorIcon::Text);
                assert!(
                    !f.output
                        .iter()
                        .any(|(_, icon)| *icon == Some(CursorIcon::Default))
                );
                f.output.clear();
                f.batch(vec![
                    RawInputEvent::PointerWheel {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        device: PointerDevice::Mouse,
                        delta: crate::core::event::WheelDelta::Pixels(0.0, -1.0),
                        position: Point::new(20.0, 10.0),
                    },
                    key(KeyState::Down),
                    key(KeyState::Up),
                ]);
                assert_eq!(f.icon, CursorIcon::Text);
                assert!(f.output.is_empty());
                f.output.clear();
                f.batch(vec![key(KeyState::Down), key(KeyState::Up)]);
                assert!(f.output.is_empty());
                f.assert_no_capture();
                assert_eq!(
                    f.tree
                        .node(f.root)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<TextField>()
                        .unwrap()
                        .text(),
                    "kept"
                );
            }
        }
    }
    #[test]
    fn cursor_editable_text_owners_exclude_disabled_read_only_and_scrollbar_gutters() {
        for policy in [
            UiInteractionPolicy::NavigationAndCapture,
            UiInteractionPolicy::PointerDriven,
        ] {
            for (enabled, read_only) in [(true, false), (true, true), (false, false)] {
                let mut field = TextField::new().with_text("kept");
                field.set_config_for_test(TextFieldConfig {
                    is_enabled: enabled,
                    is_read_only: read_only,
                    ..Default::default()
                });
                let mut f = Fixture::new(Box::new(field), policy);
                let expected = if enabled && !read_only {
                    CursorIcon::Text
                } else {
                    CursorIcon::Default
                };
                f.hover(Point::new(20.0, 10.0), expected);
                f.hover(Point::new(400.0, 10.0), CursorIcon::Default);
                f.assert_no_capture();
                let mut area = TextArea::new()
                    .with_text("long line of text ".repeat(100))
                    .with_fixed_height(120.0);
                area.set_config_for_test(TextAreaConfig {
                    is_enabled: enabled,
                    is_read_only: read_only,
                    fixed_height: Some(120.0),
                    ..Default::default()
                });
                let mut f = Fixture::new(Box::new(area), policy);
                let viewport = f
                    .tree
                    .node(f.root)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<TextArea>()
                    .unwrap()
                    .debug_text_viewport_for_test();
                f.hover(Point::new(viewport.x + 2.0, viewport.y + 2.0), expected);
                f.hover(Point::new(299.0, 110.0), CursorIcon::Default);
                f.hover(Point::new(400.0, 10.0), CursorIcon::Default);
                f.assert_no_capture();
            }
            let mut f = Fixture::new(Box::new(TextEdit::with_text("editor")), policy);
            f.hover(Point::new(20.0, 10.0), CursorIcon::Text);
            f.hover(Point::new(400.0, 10.0), CursorIcon::Default);
            f.assert_no_capture();
        }
    }
    #[test]
    fn slider_value_field_first_click_selects_all_and_later_click_places_caret() {
        use crate::core::composite_text_edit::CompositeEditorId;
        use crate::core::event::KeyboardAuthorityMode;
        use crate::core::math::snap_to_physical;
        use crate::core::scene::{SceneNode, TextNode};
        use std::cell::Cell;
        use std::rc::Rc;

        fn collect(
            nodes: &[SceneNode],
            field: RectF,
            fields: &mut Vec<RectF>,
            texts: &mut Vec<TextNode>,
        ) {
            for node in nodes {
                match node {
                    SceneNode::RetainedFragment(fragment) => {
                        collect(&fragment.children, field, fields, texts)
                    }
                    SceneNode::Clip(clip) => collect(&clip.children, field, fields, texts),
                    SceneNode::Scroll(scroll) => collect(&scroll.children, field, fields, texts),
                    SceneNode::Layer(layer) => collect(&layer.children, field, fields, texts),
                    SceneNode::RoundedRect(rect)
                        if rect.z_index == crate::core::scene::LAYER_CONTENT && rect.rect == field =>
                    {
                        fields.push(rect.rect)
                    }
                    SceneNode::Text(text) => texts.push(text.clone()),
                    _ => {}
                }
            }
        }

        fn contains_local(nodes: &[SceneNode], local: &Arc<[SceneNode]>) -> bool {
            nodes.iter().any(|node| match node {
                SceneNode::RetainedFragment(fragment) => {
                    Arc::ptr_eq(&fragment.children, local) || contains_local(&fragment.children, local)
                }
                SceneNode::Clip(clip) => contains_local(&clip.children, local),
                SceneNode::Scroll(scroll) => contains_local(&scroll.children, local),
                SceneNode::Layer(layer) => contains_local(&layer.children, local),
                _ => false,
            })
        }

        fn editor_texts(f: &Fixture, id: &CompositeEditorId, field: RectF) -> Vec<TextNode> {
            let child = f
                .tree
                .composite_child(id)
                .expect("current sealed editor child");
            assert_eq!(f.tree.widget_id_for_node(child), Some(id.child()));
            assert!(f.tree.composite_child_presentation_visible(id.child()));
            let local = f
                .retained
                .local_fragment(id.child())
                .expect("active child paint");
            assert!(
                contains_local(&f.retained.cache.entries[&id.host()].composed, &local),
                "the real child's local Arc must participate in the host's composed scene"
            );
            let mut fields = Vec::new();
            let mut texts = Vec::new();
            collect(&local, field, &mut fields, &mut texts);
            assert!(
                fields.is_empty(),
                "the child does not duplicate the host chrome"
            );
            texts
        }

        fn width(f: &mut Fixture, text: &TextNode, source: &str) -> f32 {
            f.text
                .shape_bound_text(
                    source,
                    text.font_size,
                    text.binding.as_ref().expect("actual editor face binding"),
                )
                .expect("measure the actual displayed editor face")
                .width
        }

        let mut completed = 0;
        for policy in [
            UiInteractionPolicy::NavigationAndCapture,
            UiInteractionPolicy::PointerDriven,
        ] {
            for style in [
                TrackDisplayStyle::Track,
                TrackDisplayStyle::Bar,
                TrackDisplayStyle::Pill,
                TrackDisplayStyle::Radial,
            ] {
                for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
                    for insertion_prefix in [None, Some(""), Some("42")] {
                        let changes = Rc::new(Cell::new(0));
                        let changed = Rc::clone(&changes);
                        let slider = Slider::new(0.0, 1000.0)
                            .with_value(42.0)
                            .with_value_display()
                            .with_label("Level")
                            .with_value_field_side(side)
                            .with_display_style(style)
                            .on_value_changed(move |_| changed.set(changed.get() + 1));
                        let mut f = Fixture::new(Box::new(slider), policy);
                        let owner = f.tree.widget_id_for_node(f.root).unwrap();
                        let composite = f
                            .tree
                            .node(f.root)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<Slider>()
                            .unwrap()
                            .composite_id()
                            .cloned()
                            .expect("Slider installs its sealed TextField during tree construction");
                        let child = f
                            .tree
                            .composite_child(&composite)
                            .expect("installed editor child");
                        assert_eq!(composite.host(), owner);
                        assert_eq!(f.tree.widget_id_for_node(child), Some(composite.child()));
                        assert_eq!(f.tree.node(child).unwrap().parent, Some(f.root));
                        let child_rect = f
                            .tree
                            .get_absolute_paint_rect(child)
                            .expect("editor allocation");
                        let field_bounds = RectF::new(
                            snap_to_physical(child_rect.x, 1.0),
                            snap_to_physical(child_rect.y, 1.0),
                            snap_to_physical(child_rect.right(), 1.0)
                                - snap_to_physical(child_rect.x, 1.0),
                            snap_to_physical(child_rect.bottom(), 1.0)
                                - snap_to_physical(child_rect.y, 1.0),
                        );
                        let mut fields = Vec::new();
                        let mut texts = Vec::new();
                        collect(
                            &f.retained.local_fragment(owner).unwrap(),
                            field_bounds,
                            &mut fields,
                            &mut texts,
                        );
                        assert_eq!(
                            fields.len(),
                            1,
                            "actual host chrome for the sealed child allocation"
                        );
                        let field = fields[0];
                        assert!(
                            !f.tree
                                .composite_child_presentation_visible(composite.child())
                        );
                        assert_eq!(
                            f.tree.composite_editor_snapshot(&composite).unwrap().text,
                            "42.0"
                        );
                        let inactive = texts
                            .iter()
                            .find(|text| text.text == "42.0")
                            .expect("inactive host value");
                        assert!(inactive.backgrounds.is_empty());
                        eprintln!(
                            "SLIDER_TREE_EDIT_PROOF policy={policy:?} style={style:?} side={side:?} prefix={insertion_prefix:?} phase=inactive host={} child={} field={field:?}",
                            owner.0,
                            composite.child().0
                        );
                        let point = Point::new(field.x + 1.0, field.y + field.height * 0.5);
                        f.batch(vec![
                            button(point, PointerButton::Primary, RawButtonState::Pressed),
                            button(point, PointerButton::Primary, RawButtonState::Released),
                        ]);
                        assert_eq!(
                            f.domains.primary_state().focus_manager.focused(),
                            Some(owner)
                        );
                        assert_eq!(
                            f.domains.primary_state().keyboard_mode,
                            KeyboardAuthorityMode::Capture
                        );
                        assert!(
                            !f.domains
                                .primary_state()
                                .pointer_capture_manager
                                .is_captured(PointerId::MOUSE)
                        );
                        let slider = f
                            .tree
                            .node(f.root)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<Slider>()
                            .unwrap();
                        assert_eq!(slider.value(), 42.0);
                        assert!(!slider.state().active);
                        assert_eq!(changes.get(), 0);
                        f.frame();
                        let entered = f.tree.composite_editor_snapshot(&composite).unwrap();
                        eprintln!(
                            "SLIDER_TREE_EDIT_PROOF phase=entered session={} cursor={} selection={}..{} bytes={}",
                            entered.session,
                            entered.cursor,
                            entered.selection_anchor,
                            entered.selection_focus,
                            entered.text.len()
                        );
                        assert_eq!(entered.text, "42.0");
                        assert_eq!(entered.selection_anchor.min(entered.selection_focus), 0);
                        assert_eq!(
                            entered.selection_anchor.max(entered.selection_focus),
                            entered.text.len()
                        );
                        texts = editor_texts(&f, &composite, field);
                        let text = texts
                            .iter()
                            .find(|text| text.text == "42.0")
                            .expect("editable value glyphs");
                        let text_width = width(&mut f, text, "42.0");
                        assert_eq!(
                            text.backgrounds.len(),
                            2,
                            "first click must retain selection and caret"
                        );
                        let selection = &text.backgrounds[0];
                        assert!(selection.x_offset.abs() < 0.001);
                        assert!((selection.width - text_width).abs() < 0.5);
                        assert!(selection.height > 0.0);
                        let caret = &text.backgrounds[1];
                        assert_eq!(caret.width, Theme::dark().typography.caret_thickness);
                        assert!(caret.height > 0.0);
                        if let Some(prefix) = insertion_prefix {
                            let caret_x = text.position.x + width(&mut f, text, prefix);
                            let point = Point::new(caret_x + 0.1, point.y);
                            f.batch(vec![
                                button(point, PointerButton::Primary, RawButtonState::Pressed),
                                button(point, PointerButton::Primary, RawButtonState::Released),
                            ]);
                            f.frame();
                            let positioned = f.tree.composite_editor_snapshot(&composite).unwrap();
                            assert_eq!(positioned.cursor, prefix.len());
                            assert_eq!(positioned.selection_anchor, positioned.selection_focus);
                            texts = editor_texts(&f, &composite, field);
                            let text = texts.iter().find(|text| text.text == "42.0").unwrap();
                            assert_eq!(text.backgrounds.len(), 1);
                            let caret = &text.backgrounds[0];
                            assert_eq!(caret.width, Theme::dark().typography.caret_thickness);
                            assert!(
                                (text.position.x + caret.x_offset - caret_x).abs() < 0.5,
                                "{style:?}/{side:?}/{policy:?}: wrong subsequent-click caret"
                            );
                        }
                        f.batch(vec![RawInputEvent::TextInput {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            text: "7".into(),
                            modifiers: Modifiers::default(),
                        }]);
                        assert_eq!(changes.get(), 0, "typing remains pending until commit");
                        assert_eq!(
                            f.tree
                                .node(f.root)
                                .unwrap()
                                .widget
                                .as_any()
                                .downcast_ref::<Slider>()
                                .unwrap()
                                .value(),
                            42.0
                        );
                        f.batch(vec![RawInputEvent::Key {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            key_code: KeyCode::Enter,
                            physical_key: PhysicalKeyCode::Unknown,
                            reservation: None,
                            state: KeyState::Down,
                            repeat: false,
                            modifiers: Modifiers::default(),
                        }]);
                        let slider = f
                            .tree
                            .node(f.root)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<Slider>()
                            .unwrap();
                        let expected = match insertion_prefix {
                            None => 7.0,
                            Some("") => 742.0,
                            Some("42") => 427.0,
                            _ => unreachable!(),
                        };
                        assert_eq!(slider.value(), expected);
                        assert_eq!(changes.get(), 1);
                        assert_eq!(f.tree.widget_id_for_node(f.root), Some(owner));
                        assert_eq!(f.tree.composite_child(&composite), Some(child));
                        completed += 1;
                    }
                }
            }
        }
        assert_eq!(
            completed, 48,
            "the original policy/style/side/insertion matrix"
        );
    }

    #[test]
    fn cursor_compound_inputs_exclude_affordances_buttons_track_labels_and_disabled_fields() {
        let policy = UiInteractionPolicy::PointerDriven;
        for enabled in [true, false] {
            let mut combo = ComboBox::simple(vec!["First".into(), "Second".into()]);
            combo.set_text_input_enabled(enabled);
            combo.set_loading(true);
            let mut f = Fixture::new(Box::new(combo), policy);
            f.hover(
                Point::new(20.0, 10.0),
                if enabled {
                    CursorIcon::Text
                } else {
                    CursorIcon::Default
                },
            );
            f.hover(Point::new(299.0, 10.0), CursorIcon::Default);
            f.hover(
                Point::new(
                    300.0 - Theme::dark().geometry.combo_box.affordance_width,
                    10.0,
                ),
                CursorIcon::Default,
            );
            f.assert_no_capture();
            let mut spin = SpinBox::new().with_label("label");
            spin.set_disabled(!enabled);
            let mut f = Fixture::new(Box::new(spin), policy);
            let geom = Theme::dark().geometry.spin_box;
            f.hover(
                Point::new(geom.button_width + 5.0, 10.0),
                if enabled {
                    CursorIcon::Text
                } else {
                    CursorIcon::Default
                },
            );
            f.hover(Point::new(1.0, 10.0), CursorIcon::Default);
            f.hover(Point::new(f.rect().width - 1.0, 10.0), CursorIcon::Default);
            f.assert_no_capture();
            for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
                for style in [TrackDisplayStyle::Track, TrackDisplayStyle::Radial] {
                    let mut slider = Slider::new(0.0, 100.0)
                        .with_value(42.0)
                        .with_value_display()
                        .with_value_field_side(side)
                        .with_display_style(style);
                    slider.set_disabled(!enabled);
                    let mut f = Fixture::new(Box::new(slider), policy);
                    let x = if side == ValueFieldSide::Left {
                        10.0
                    } else {
                        f.rect().width - 10.0
                    };
                    f.hover(
                        Point::new(x, f.rect().height * 0.5),
                        if enabled {
                            CursorIcon::Text
                        } else {
                            CursorIcon::Default
                        },
                    );
                    let track_x = if side == ValueFieldSide::Left {
                        f.rect().width - 5.0
                    } else {
                        5.0
                    };
                    f.hover(
                        Point::new(track_x, f.rect().height * 0.5),
                        CursorIcon::Default,
                    );
                    f.assert_no_capture();
                    assert_eq!(
                        f.tree
                            .node(f.root)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<Slider>()
                            .unwrap()
                            .value(),
                        42.0
                    );
                }
            }
        }
    }
    enum CompositeTestAction {
        ReplaceText(String),
        SetTagItems(Vec<TagItem>),
    }

    struct CompositeActionScreen {
        host: ScreenHost,
        target: crate::utils::id::TreeWidgetId,
        action: Option<CompositeTestAction>,
        receipt: Rc<
            std::cell::RefCell<
                Option<
                    Result<
                        crate::core::composite_text_edit::CompositeEditReply,
                        crate::core::composite_text_edit::CompositeEditOutcome,
                    >,
                >,
            >,
        >,
    }

    impl Screen for CompositeActionScreen {
        fn build<'a>(&'a mut self, _cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            &mut self.host
        }

        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            if self.action.is_none() {
                return;
            }
            let result = cx
                .with_composite_editors(|owners| {
                    let id = owners.resolve(self.target)?;
                    Ok(match self.action.take().expect("one requested operation") {
                        CompositeTestAction::ReplaceText(text) => owners.replace_text(&id, text),
                        CompositeTestAction::SetTagItems(items) => owners.set_tag_items(&id, items),
                    })
                })
                .and_then(|result| result);
            *self.receipt.borrow_mut() = Some(result);
        }
    }

    impl Fixture {
        fn queue_composite_action(
            &mut self,
            action: CompositeTestAction,
        ) -> Rc<
            std::cell::RefCell<
                Option<
                    Result<
                        crate::core::composite_text_edit::CompositeEditReply,
                        crate::core::composite_text_edit::CompositeEditOutcome,
                    >,
                >,
            >,
        > {
            let receipt = Rc::new(std::cell::RefCell::new(None));
            self.app.push_screen(Box::new(CompositeActionScreen {
                host: ScreenHost::new(),
                target: self.tree.widget_id_for_node(self.root).unwrap(),
                action: Some(action),
                receipt: Rc::clone(&receipt),
            }));
            receipt
        }

        fn flush_composite_actions(&mut self) {
            self.app.flush_pending_actions_with_composite_editors(
                &mut self.tree,
                &mut self.router,
                self.domains.primary_state_mut(),
                &mut self.text,
            );
        }
    }

    #[test]
    fn composite_editor_public_owner_scope_is_postroute_and_needs_no_user_capture() {
        use crate::core::composite_text_edit::CompositeEditOutcome;
        let mut f = Fixture::new(
            Box::new(TagInput::new()),
            UiInteractionPolicy::PointerDriven,
        );
        let receipt =
            f.queue_composite_action(CompositeTestAction::ReplaceText("owned draft".into()));
        f.app.flush_pending_actions_all(&mut f.tree);
        assert!(matches!(
            receipt.borrow().as_ref().unwrap(),
            Err(CompositeEditOutcome::WrongPhase)
        ));
        f.assert_no_capture();
        f.flush_composite_actions();
        let receipt = receipt.borrow();
        let reply = receipt.as_ref().unwrap().as_ref().unwrap();
        assert!(reply.applied());
        assert_eq!(reply.snapshot.as_ref().unwrap().text, "owned draft");
        assert_eq!(reply.stage, 1);
        let id = reply.id.as_ref().unwrap();
        assert_eq!(
            f.tree.composite_editor_snapshot(id).unwrap().text,
            "owned draft"
        );
        f.assert_no_capture();
    }

    #[test]
    fn numeric_editor_surface_uses_canonical_textfield_chrome() {
        use crate::core::paint::{TrackDisplayStyle, ZIndex};
        use crate::core::scene::{Color, SceneNode};
        use crate::widgets::{ControlAlignmentGroup, groupbox::GroupBox};

        fn render(f: &mut Fixture, theme: &Theme, constraints: Constraints) {
            // This fixture mounts with the dark theme. Explicit theme replacement
            // must invalidate every local paint fragment, including nested hosts.
            let nodes: Vec<_> = f.tree.iter_nodes().map(|(id, _)| id).collect();
            for node in nodes {
                f.tree.mark_paint_dirty(node);
            }
            f.tree.mark_layout_dirty(f.root);
            f.retained.frame_with_appearance(
                &mut f.tree, f.root, constraints, theme, &mut f.text,
                &crate::core::icon::IconRegistry::new(&[]), &f.domains, WindowId::PRIMARY,
            );
        }
        fn snapped(rect: RectF) -> RectF {
            RectF::new(rect.x.round(), rect.y.round(),
                rect.right().round() - rect.x.round(), rect.bottom().round() - rect.y.round())
        }
        fn surface(f: &Fixture, node: NodeId, rect: RectF) -> Vec<(RectF, Color, f32, ZIndex)> {
            let rect = snapped(rect);
            let owner = f.tree.widget_id_for_node(node).unwrap();
            let local = f.retained.local_fragment(owner).unwrap();
            let result: Vec<_> = local.iter().filter_map(|node| {
                let (bounds, color, radius, z) = match node {
                    SceneNode::RoundedRect(node) => (node.rect, node.color, node.radius, node.z_index),
                    SceneNode::Rect(node) => (node.rect, node.color, 0.0, node.z_index),
                    _ => return None,
                };
                (z >= 2 && z <= 4 && bounds.left() >= rect.left() && bounds.right() <= rect.right()
                    && bounds.top() >= rect.top() && bounds.bottom() <= rect.bottom())
                    .then_some((RectF::new(bounds.x - rect.x, bounds.y - rect.y,
                        bounds.width, bounds.height), color, radius, z))
            }).collect();
            assert!(!result.is_empty(), "region={rect:?}, host={:?}, kinds={:?}, shapes={:?}",
                f.tree.get_absolute_paint_rect(node),
                local.iter().take(16).map(SceneNode::kind).collect::<Vec<_>>(),
                local.iter().take(16).filter_map(|node| match node {
                    SceneNode::Rect(node) => Some((node.rect, node.z_index)),
                    SceneNode::RoundedRect(node) => Some((node.rect, node.z_index)),
                    _ => None,
                }).collect::<Vec<_>>());
            result
        }
        fn click(f: &mut Fixture, rect: RectF) {
            let point = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
            f.batch(vec![
                button(point, PointerButton::Primary, RawButtonState::Pressed),
                button(point, PointerButton::Primary, RawButtonState::Released),
            ]);
        }
        for dark in [false, true] {
            let theme = if dark { Theme::dark() } else { Theme::light() };
            // Both the Calibration alignment path and direct GroupBox stretch
            // must give the real editor and the host painter the same region.
            let cases: Vec<(Box<dyn Widget>, bool, bool)> = vec![
                (Box::new(SpinBox::new().with_value(7)), false, true),
                (Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display()), true, true),
                (Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display()), false, false),
                (Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display()
                    .with_label("Level")), false, false),
                (Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display()
                    .with_display_style(TrackDisplayStyle::Bar)), false, false),
                (Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display()
                    .with_value_field_side(ValueFieldSide::Left)), false, false),
                (Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display()
                    .with_display_style(TrackDisplayStyle::Radial)), false, false),
            ];
            for (widget, aligned, grouped) in cases {
                let mut tree = WidgetTree::new();
                let root = tree.create_root(Box::new(GroupBox::new(NodeId(0), "Numeric")), None, 0);
                let group = if grouped {
                    tree.add_child(root, Box::new(ControlAlignmentGroup::column()), None, 0)
                } else { root };
                let control = tree.add_child(group, widget, None, 0);
                tree.set_control_alignment_member(control, aligned);
                tree.replace_widget_only(root, Box::new(GroupBox::new(
                    if grouped { group } else { control }, "Numeric",
                )));
                let mut f = Fixture::from_tree(tree, root, UiInteractionPolicy::PointerDriven);
                let host = f.tree.widget_id_for_node(control).unwrap();
                let id = f.tree.composite_editor_id(host).unwrap();
                let child = f.tree.find_by_id(id.child()).unwrap();
                let mut constraints = Constraints::loose(Size::new(300.0, 120.0));
                render(&mut f, &theme, constraints);
                let mut field = f.tree.get_absolute_paint_rect(child).unwrap();
                let mut bounds = f.tree.get_absolute_paint_rect(control).unwrap();
                if !grouped {
                    assert_eq!(bounds.width, 300.0 - 2.0 * bounds.x,
                        "GroupBox must pass its final stretched width through child Layout");
                }
                let mut plain = Fixture::new(
                    Box::new(TextField::new().with_text("5")), UiInteractionPolicy::PointerDriven,
                );
                let physical_field = snapped(field);
                let field_constraints = Constraints::tight(Size::new(physical_field.width, physical_field.height));
                render(&mut plain, &theme, field_constraints);
                let reference = plain.rect();
                let baseline = surface(&plain, plain.root, reference);
                assert_eq!(baseline.len(), 3);
                assert!(baseline.first().unwrap().3 > 1, "input stays above GroupBox panel fill");
                assert_eq!(surface(&f, control, field), baseline,
                    "idle numeric region must use the actual ordinary TextField recipe");
                f.retained.frame_with_appearance(
                    &mut f.tree, f.root, constraints, &theme, &mut f.text,
                    &crate::core::icon::IconRegistry::new(&[]), &f.domains, WindowId::PRIMARY,
                );
                assert_eq!(f.tree.get_absolute_paint_rect(child), Some(field));
                assert_eq!(surface(&f, control, field), baseline,
                    "retained reuse cannot move the numeric surface away from its child");

                click(&mut f, field);
                assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(host));
                assert_eq!(f.domains.primary_state().keyboard_mode,
                    crate::core::event::KeyboardAuthorityMode::Capture);
                assert_ne!(id.child(), host);
                render(&mut f, &theme, constraints);
                click(&mut plain, reference);
                render(&mut plain, &theme, field_constraints);
                assert_eq!(surface(&f, control, field), surface(&plain, plain.root, reference),
                    "captured numeric region must keep the canonical surface");
                assert_eq!(f.tree.get_absolute_paint_rect(child), Some(field));
                assert_eq!(f.tree.get_absolute_paint_rect(control), Some(bounds));

                // The shared palette is presentation only: real paired input
                // edits the selected child draft without committing its numeric value.
                let time = Instant::now();
                f.batch(vec![
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY, time,
                        key_code: KeyCode::Character('5'), physical_key: PhysicalKeyCode::Digit5,
                        reservation: None, state: KeyState::Down, repeat: false,
                        modifiers: Modifiers::default(),
                    },
                    RawInputEvent::TextInput {
                        window_id: WindowId::PRIMARY, time, text: "5".into(),
                        modifiers: Modifiers::default(),
                    },
                ]);
                assert_eq!(f.tree.composite_editor_snapshot(&id).unwrap().text, "5");
                let widget = f.tree.node(control).unwrap().widget();
                if let Some(spin) = widget.as_any().downcast_ref::<SpinBox>() {
                    assert_eq!(spin.value(), 7);
                } else {
                    assert_eq!(widget.as_any().downcast_ref::<Slider>().unwrap().value(), 7.0);
                }
                constraints = Constraints::loose(Size::new(420.0, 120.0));
                render(&mut f, &theme, constraints);
                let resized = f.tree.get_absolute_paint_rect(child).unwrap();
                assert_eq!(resized.width, field.width);
                assert_eq!(resized.height, field.height);
                field = resized;
                bounds = f.tree.get_absolute_paint_rect(control).unwrap();
                assert!(field.x >= bounds.x && field.right() <= bounds.right());
                assert_eq!(surface(&f, control, field), surface(&plain, plain.root, reference),
                    "resize must update child Layout and host surface together");
                assert_eq!(f.domains.primary_state().keyboard_mode,
                    crate::core::event::KeyboardAuthorityMode::Capture);
                let child_scene = f.retained.local_fragment(id.child()).unwrap();
                let geom = theme.geometry.text_field;
                assert!(child_scene.iter().any(|node| match node {
                    SceneNode::Clip(clip) => clip.rect == RectF::new(
                        field.x + geom.padding_x, field.y + geom.padding_y,
                        field.width - 2.0 * geom.padding_x, field.height - 2.0 * geom.padding_y,
                    ),
                    _ => false,
                }), "the real child's text clip must follow the same current region");
                let text = child_scene.iter().find_map(|node| {
                    let SceneNode::Clip(clip) = node else { return None };
                    clip.children.iter().find_map(|node| match node {
                        SceneNode::Text(text) if text.text == "5" => Some(text),
                        _ => None,
                    })
                }).unwrap();
                assert!(surface(&f, control, field).last().unwrap().3 < text.z_index,
                    "input surface must be strictly below glyphs, including a separate child");
                let caret = text.backgrounds.first().unwrap();
                let expected_caret: Color = theme.input_chrome(
                    crate::core::theme::ControlChromeState::Captured).caret.into();
                assert_eq!(caret.color, expected_caret);
            }
        }
    }

    #[test]
    fn composite_editor_slider_value_has_one_retained_painter() {
        use crate::core::layout::{Alignment, ControlTrailingOrder};
        use crate::core::scene::SceneNode;
        use crate::widgets::slider::SliderLabelSide;
        use crate::widgets::{ControlAlignmentGroup, flex::Flex, range_slider::RangeSlider};
        use crate::widgets::tooltip_host::TooltipHost;

        fn count_text(nodes: &[SceneNode], expected: &str) -> usize {
            nodes.iter().map(|node| match node {
                SceneNode::Text(text) => usize::from(text.text == expected),
                SceneNode::RetainedFragment(fragment) => count_text(&fragment.children, expected),
                SceneNode::Clip(clip) => count_text(&clip.children, expected),
                SceneNode::Layer(layer) => count_text(&layer.children, expected),
                SceneNode::Scroll(scroll) => count_text(&scroll.children, expected),
                _ => 0,
            }).sum()
        }
        fn render(f: &mut Fixture) -> Vec<SceneNode> {
            f.retained.frame_with_appearance(
                &mut f.tree,
                f.root,
                Constraints::loose(Size::new(420.0, 300.0)),
                &Theme::dark(),
                &mut f.text,
                &crate::core::icon::IconRegistry::new(&[]),
                &f.domains,
                WindowId::PRIMARY,
            );
            // A clean frame correctly returns no newly built scene. Inspect
            // its authoritative retained composition, including cache hits.
            let owner = f.tree.widget_id_for_node(f.root).unwrap();
            f.retained.cache.entries.get(&owner).unwrap().composed.to_vec()
        }
        fn assert_values(nodes: &[SceneNode], changed: bool) {
            assert_eq!(count_text(nodes, "42.0"), if changed { 1 } else { 2 });
            assert_eq!(count_text(nodes, "75.0"), 2);
            assert_eq!(count_text(nodes, "13.0"), usize::from(changed));
            assert_eq!(count_text(nodes, "Range"), 1);
        }

        // Match Calibration's two aligned orders and TooltipHost wrappers.
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_align_items(Alignment::Stretch)), None, 0,
        );
        let mut sliders = Vec::new();
        for (order, side) in [
            (ControlTrailingOrder::ValueThenLabel, SliderLabelSide::Right),
            (ControlTrailingOrder::LabelThenValue, SliderLabelSide::Left),
        ] {
            let group = tree.add_child(
                root,
                Box::new(ControlAlignmentGroup::column().with_trailing_order(order)),
                None, 0,
            );
            for (label, value) in [("Overlay alpha", 42.0), ("Thermal cap", 75.0)] {
                let tip = tree.add_child(group, Box::new(TooltipHost::new(label)), None, 0);
                let slider = tree.add_child(
                    tip,
                    Box::new(Slider::new(0.0, 100.0)
                        .with_value(value)
                        .with_label(label)
                        .with_label_side(side)
                        .with_value_display()
                        .with_value_field_side(ValueFieldSide::Right)),
                    None, 0,
                );
                tree.set_control_alignment_member(tip, true);
                sliders.push(slider);
            }
        }
        let range = tree.add_child(
            root,
            Box::new(RangeSlider::new(0.0, 100.0, 25.0, 75.0).with_label("Range")),
            None, 0,
        );
        assert!(tree.children(range).is_empty());
        let mut f = Fixture::from_tree(tree, root, UiInteractionPolicy::PointerDriven);
        assert_values(&render(&mut f), false);
        assert_values(&render(&mut f), false);

        // The inactive child's construction-time 42.0 must never overprint a
        // later host value, even while retained fragments can be reused.
        let slider = sliders[0];
        f.tree.node_mut(slider).unwrap().widget_mut().as_any_mut()
            .downcast_mut::<Slider>().unwrap().set_value(13.0);
        f.tree.mark_paint_dirty(slider);
        assert_values(&render(&mut f), true);

        let host = f.tree.widget_id_for_node(slider).unwrap();
        let id = f.tree.composite_editor_id(host).unwrap();
        let child = f.tree.find_by_id(id.child()).unwrap();
        let region = f.tree.get_absolute_paint_rect(child).unwrap();
        let point = Point::new(region.x + region.width * 0.5, region.y + region.height * 0.5);
        f.batch(vec![
            button(point, PointerButton::Primary, RawButtonState::Pressed),
            button(point, PointerButton::Primary, RawButtonState::Released),
        ]);
        assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(host));
        assert_eq!(f.tree.composite_editor_snapshot(&id).unwrap().text, "13.0");
        assert_values(&render(&mut f), true);
        assert!(f.retained.local_fragment(id.child()).is_some());

        f.batch(vec![RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code: KeyCode::Enter,
            physical_key: PhysicalKeyCode::Enter,
            reservation: None,
            state: KeyState::Down,
            repeat: false,
            modifiers: Modifiers::default(),
        }]);
        assert!(!f.tree.composite_child_presentation_visible(id.child()));
        assert_values(&render(&mut f), true);
        assert_values(&render(&mut f), true);
    }

    #[test]
    fn composite_editor_long_input_keeps_caret_in_retained_text_viewport() {
        use crate::core::layout::Alignment;
        use crate::core::scene::{SceneNode, TextNode};
        use crate::widgets::flex::Flex;

        fn render(f: &mut Fixture, width: f32) {
            f.retained.frame_with_appearance(
                &mut f.tree,
                f.root,
                Constraints::loose(Size::new(width, 120.0)),
                &Theme::dark(),
                &mut f.text,
                &crate::core::icon::IconRegistry::new(&[]),
                &f.domains,
                WindowId::PRIMARY,
            );
        }
        fn matching_text(
            nodes: &[SceneNode],
            expected: &str,
            clips: &mut Vec<RectF>,
            found: &mut Vec<(TextNode, Vec<RectF>)>,
        ) {
            for node in nodes {
                match node {
                    SceneNode::Text(text) if text.text == expected => {
                        found.push((text.clone(), clips.clone()));
                    }
                    SceneNode::RetainedFragment(fragment) => {
                        matching_text(&fragment.children, expected, clips, found);
                    }
                    SceneNode::Clip(clip) => {
                        clips.push(clip.rect);
                        matching_text(&clip.children, expected, clips, found);
                        clips.pop();
                    }
                    SceneNode::Layer(layer) => {
                        matching_text(&layer.children, expected, clips, found);
                    }
                    _ => {}
                }
            }
        }
        fn assert_viewport(f: &Fixture, child: NodeId, expected: &str, at_end: bool, captured: bool) {
            let theme = Theme::dark();
            let rect = f.tree.get_absolute_paint_rect(child).unwrap();
            let geom = theme.geometry.text_field;
            let viewport = RectF::new(
                rect.x + geom.padding_x,
                rect.y + geom.padding_y,
                rect.width - 2.0 * geom.padding_x,
                rect.height - 2.0 * geom.padding_y,
            );
            let field = f.tree.node(child).unwrap().widget().as_any()
                .downcast_ref::<TextField>().unwrap();
            assert_eq!(field.text(), expected);
            assert_eq!(field.cursor_pos(), if at_end { expected.len() } else { 0 });
            assert!(field.scroll_x_for_test().is_finite());
            assert!(field.scroll_x_for_test() >= 0.0);
            if at_end {
                assert!(field.scroll_x_for_test() > 0.0, "long text must scroll");
            } else {
                assert_eq!(field.scroll_x_for_test(), 0.0);
            }
            let owner = f.tree.widget_id_for_node(f.root).unwrap();
            let scene = &f.retained.cache.entries.get(&owner).unwrap().composed;
            let mut found = Vec::new();
            matching_text(scene, expected, &mut Vec::new(), &mut found);
            assert_eq!(found.len(), 1, "one current whole-string painter");
            let (text, clips) = &found[0];
            assert!(clips.iter().any(|clip| *clip == viewport),
                "current text viewport must bound text and caret: {clips:?} / {viewport:?}");
            assert_eq!(text.backgrounds.len(), usize::from(captured),
                "Capture owns caret: source={expected:?}, at_end={at_end}, viewport={viewport:?}, focus={:?}, mode={:?}, selection={}..{}, animation={}",
                f.domains.primary_state().focus_manager.focused(),
                f.domains.primary_state().keyboard_mode,
                field.selection().anchor, field.selection().head, field.needs_animation());
            if captured {
                let caret = &text.backgrounds[0];
                let x = text.position.x + caret.x_offset;
                assert!(x >= viewport.x - 0.5, "caret left edge {x} / {viewport:?}");
                assert!(x + caret.width <= viewport.x + viewport.width + 0.5,
                    "complete caret must remain visible: {x} / {viewport:?}");
            }
            if at_end {
                assert!(text.position.x < viewport.x, "full source is scrolled under clip");
            }
        }
        fn navigation(f: &mut Fixture, code: KeyCode, physical: PhysicalKeyCode, repeat: bool) {
            f.batch(vec![RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: code,
                physical_key: physical,
                reservation: None,
                state: KeyState::Down,
                repeat,
                modifiers: Modifiers::default(),
            }]);
        }

        // Actual Connection Profile seeds and Runtime Endpoint options. Input
        // crosses the real physical-key/TextInput seam, never direct child dispatch.
        let cases: Vec<(&str, Box<dyn Widget>, &str)> = vec![
            ("alias", Box::new(TextField::new()
                .with_text("edge-router-07.ops.local")
                .with_placeholder("host or service alias")), "edge-router-07.ops.local"),
            ("endpoint", Box::new(TextField::new()
                .with_text("10.42.18.77:9443")
                .with_placeholder("control endpoint")), "10.42.18.77:9443"),
            ("ComboBox", Box::new(ComboBox::simple(vec![
                "ops-gpu-17".into(), "ops-gpu-18".into(),
                "staging-gpu-03".into(), "local-sim".into(),
            ])), ""),
        ];
        for (name, widget, seed) in cases {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Flex::column().with_align_items(Alignment::Stretch)), None, 0,
            );
            let control = tree.add_child(root, widget, None, 0);
            let sibling = tree.add_child(root,
                Box::new(TextField::new().with_text("independent")), None, 1);
            let mut f = Fixture::from_tree(tree, root, UiInteractionPolicy::PointerDriven);
            let host = f.tree.widget_id_for_node(control).unwrap();
            let relation = f.tree.composite_editor_id(host).ok();
            let child = relation.as_ref().map_or(control, |id| {
                f.tree.find_by_id(id.child()).unwrap()
            });
            let rect = f.tree.get_absolute_paint_rect(child).unwrap();
            let point = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
            f.batch(vec![
                button(point, PointerButton::Primary, RawButtonState::Pressed),
                button(point, PointerButton::Primary, RawButtonState::Released),
            ]);
            // Open-popup Home/End belong to ComboBox option navigation, not
            // delegated text editing. Its actual initial draft is already empty.
            if relation.is_none() {
                navigation(&mut f, KeyCode::End, PhysicalKeyCode::End, false);
            }
            render(&mut f, 300.0);
            let mut expected = seed.to_owned();
            for _ in 0..80 {
                let time = Instant::now();
                f.batch(vec![
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY, time,
                        key_code: KeyCode::Character('5'),
                        physical_key: PhysicalKeyCode::Digit5,
                        reservation: None, state: KeyState::Down, repeat: false,
                        modifiers: Modifiers::default(),
                    },
                    RawInputEvent::TextInput {
                        window_id: WindowId::PRIMARY, time,
                        text: "5".into(), modifiers: Modifiers::default(),
                    },
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY, time: Instant::now(),
                        key_code: KeyCode::Character('5'),
                        physical_key: PhysicalKeyCode::Digit5,
                        reservation: None, state: KeyState::Up, repeat: false,
                        modifiers: Modifiers::default(),
                    },
                ]);
                expected.push('5');
                render(&mut f, 300.0);
                let field = f.tree.node(child).unwrap().widget().as_any()
                    .downcast_ref::<TextField>().unwrap();
                assert_eq!(field.text(), expected, "{name}: each paired input inserts once");
            }
            assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(host));
            assert_eq!(f.domains.primary_state().keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture);
            if let Some(id) = &relation {
                assert_ne!(id.child(), host);
                assert_eq!(f.tree.composite_editor_snapshot(id).unwrap().text, expected);
            }
            assert_viewport(&f, child, &expected, true, true);
            for width in [112.0, 300.0] {
                f.tree.mark_layout_dirty(f.root);
                render(&mut f, width);
                assert_viewport(&f, child, &expected, true, true);
            }
            if relation.is_none() {
                navigation(&mut f, KeyCode::Home, PhysicalKeyCode::Home, false);
                render(&mut f, 300.0);
                assert_viewport(&f, child, &expected, false, true);
                navigation(&mut f, KeyCode::End, PhysicalKeyCode::End, false);
                render(&mut f, 300.0);
                assert_viewport(&f, child, &expected, true, true);
            }
            navigation(&mut f, KeyCode::Backspace, PhysicalKeyCode::Backspace, true);
            expected.pop();
            render(&mut f, 300.0);
            assert_viewport(&f, child, &expected, true, true);

            // Captured blink intentionally repaints under the existing owner.
            // A real sibling click retires this Capture; its clipped long text
            // must then survive actual local-fragment reuse without an old caret.
            if relation.is_some() {
                // Retire the popup through its existing engine Escape owner;
                // its outside-dismiss gesture must not be mistaken for a click-through.
                navigation(&mut f, KeyCode::Escape, PhysicalKeyCode::Escape, false);
                assert_eq!(f.domains.primary_state().keyboard_mode,
                    crate::core::event::KeyboardAuthorityMode::Navigation);
                render(&mut f, 300.0);
            }
            let rect = f.tree.get_absolute_paint_rect(sibling).unwrap();
            let point = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
            f.batch(vec![
                button(point, PointerButton::Primary, RawButtonState::Pressed),
                button(point, PointerButton::Primary, RawButtonState::Released),
            ]);
            assert_eq!(f.domains.primary_state().focus_manager.focused(),
                f.tree.widget_id_for_node(sibling));
            render(&mut f, 300.0);
            render(&mut f, 300.0);
            let child_owner = f.tree.widget_id_for_node(child).unwrap();
            let local = f.retained.cache.entries.get(&child_owner).unwrap().local.clone();
            render(&mut f, 300.0);
            assert!(Arc::ptr_eq(&local,
                &f.retained.cache.entries.get(&child_owner).unwrap().local));
            assert_viewport(&f, child, &expected, true, false);
        }
    }

    #[test]
    fn composite_editor_default_navigation_reaches_retained_paint() {
        use crate::core::scene::SceneNode;

        fn tick(f: &mut Fixture, time: f64) {
            f.router.set_frame_time_ms((time * 1000.0) as u64);
            f.retained.advance_transition_time(time);
            f.frame();
        }
        fn navigation(f: &mut Fixture, code: KeyCode, physical: PhysicalKeyCode, repeat: bool) {
            let time = Instant::now();
            f.batch(vec![
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY, time, key_code: code,
                    physical_key: physical, reservation: None, state: KeyState::Down,
                    repeat, modifiers: Modifiers::default(),
                },
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY, time, key_code: code,
                    physical_key: physical, reservation: None, state: KeyState::Up,
                    repeat: false, modifiers: Modifiers::default(),
                },
            ]);
        }
        fn caret(nodes: &[SceneNode]) -> Option<f32> {
            nodes.iter().find_map(|node| match node {
                SceneNode::Text(text) if text.text == "45" => {
                    assert_eq!(text.backgrounds.len(), 1, "one collapsed, captured caret");
                    Some(text.position.x + text.backgrounds[0].x_offset)
                }
                SceneNode::Clip(clip) => caret(&clip.children),
                SceneNode::RetainedFragment(fragment) => caret(&fragment.children),
                SceneNode::Layer(layer) => caret(&layer.children),
                _ => None,
            })
        }
        let cases: Vec<(&str, Box<dyn Widget>)> = vec![
            ("SpinBox", Box::new(SpinBox::new().with_value(7))),
            ("Slider", Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display())),
            ("ComboBox", Box::new(ComboBox::simple(Vec::new()))),
            ("TagInput", Box::new(TagInput::new().with_tag_creation(TagCreationMode::FreeFormOnly))),
        ];
        for (name, widget) in cases {
            // No opt-in builder or fabricated child Focus/Capture is involved.
            let mut f = Fixture::new(widget, UiInteractionPolicy::PointerDriven);
            let host = f.tree.widget_id_for_node(f.root).unwrap();
            let id = f.tree.composite_editor_id(host).unwrap();
            let child = f.tree.find_by_id(id.child()).unwrap();
            assert!(f.tree.node(child).unwrap().widget().as_any()
                .downcast_ref::<TextField>().unwrap().smooth_caret_prototype_enabled());
            let region = f.tree.get_absolute_paint_rect(child).unwrap();
            let point = Point::new(region.x + region.width * 0.5, region.y + region.height * 0.5);
            f.batch(vec![
                button(point, PointerButton::Primary, RawButtonState::Pressed),
                button(point, PointerButton::Primary, RawButtonState::Released),
            ]);
            tick(&mut f, 0.0);
            for (ch, physical) in [('4', PhysicalKeyCode::Digit4), ('5', PhysicalKeyCode::Digit5)] {
                let time = Instant::now();
                f.batch(vec![
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY, time, key_code: KeyCode::Character(ch),
                        physical_key: physical, reservation: None, state: KeyState::Down,
                        repeat: false, modifiers: Modifiers::default(),
                    },
                    RawInputEvent::TextInput {
                        window_id: WindowId::PRIMARY, time, text: ch.to_string(),
                        modifiers: Modifiers::default(),
                    },
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY, time, key_code: KeyCode::Character(ch),
                        physical_key: physical, reservation: None, state: KeyState::Up,
                        repeat: false, modifiers: Modifiers::default(),
                    },
                ]);
            }
            tick(&mut f, 0.100); // reconcile content/selection boundaries before navigation
            tick(&mut f, 0.200); // an admitted reveal starts at Update, then settles after 60 ms
            let before = f.tree.composite_editor_snapshot(&id).unwrap();
            assert_eq!(before.text, "45", "{name}");
            assert_eq!(before.selection_anchor, before.selection_focus);
            let x0 = caret(&f.retained.local_fragment(id.child()).unwrap()).unwrap();
            navigation(&mut f, KeyCode::Left, PhysicalKeyCode::ArrowLeft, false);
            let moved = f.tree.composite_editor_snapshot(&id).unwrap();
            assert_eq!(moved.text, before.text);
            assert_eq!(moved.revision, before.revision);
            assert_eq!(moved.selection_focus, 1, "logical cursor moves immediately: {name}");
            tick(&mut f, 0.200);
            let start = caret(&f.retained.local_fragment(id.child()).unwrap()).unwrap();
            tick(&mut f, 0.2325);
            let middle = caret(&f.retained.local_fragment(id.child()).unwrap()).unwrap();
            tick(&mut f, 0.266);
            let end = caret(&f.retained.local_fragment(id.child()).unwrap()).unwrap();
            assert_eq!(start, x0, "start frame preserves current qualified display: {name}");
            assert!(end < middle && middle < start,
                "default navigation must reach Paint through the real child: {name}: {start}/{middle}/{end}");
            assert!((middle - (start + (end - start) * 0.75)).abs() <= 1.0,
                "65 ms EaseOut midpoint: {name}");
            assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(host));
            assert_ne!(host, id.child());
            assert_eq!(f.domains.primary_state().keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture);

            // Verified repeat remains a canonical static boundary, with no old leg replay.
            navigation(&mut f, KeyCode::Right, PhysicalKeyCode::ArrowRight, true);
            tick(&mut f, 0.267);
            assert_eq!(caret(&f.retained.local_fragment(id.child()).unwrap()), Some(x0), "{name}");
            let widget = f.tree.node(f.root).unwrap().widget();
            if let Some(spin) = widget.as_any().downcast_ref::<SpinBox>() {
                assert_eq!(spin.value(), 7);
            } else if let Some(slider) = widget.as_any().downcast_ref::<Slider>() {
                assert_eq!(slider.value(), 7.0);
            }
            if name == "Slider" {
                let previous = f.tree.composite_editor_snapshot(&id).unwrap();
                navigation(&mut f, KeyCode::Escape, PhysicalKeyCode::Escape, false);
                assert_eq!(f.domains.primary_state().keyboard_mode,
                    crate::core::event::KeyboardAuthorityMode::Navigation);
                assert_eq!(f.tree.composite_editor_snapshot(&id).unwrap().text, "45",
                    "engine Escape preserves the uncommitted draft");
                f.batch(vec![
                    button(point, PointerButton::Secondary, RawButtonState::Pressed),
                    button(point, PointerButton::Secondary, RawButtonState::Released),
                ]);
                assert_eq!(f.domains.primary_state().keyboard_mode,
                    crate::core::event::KeyboardAuthorityMode::Navigation,
                    "a secondary press cannot acquire text-entry authority");
                tick(&mut f, 0.280);
                f.batch(vec![
                    button(point, PointerButton::Primary, RawButtonState::Pressed),
                    button(point, PointerButton::Primary, RawButtonState::Released),
                ]);
                assert_eq!(f.domains.primary_state().keyboard_mode,
                    crate::core::event::KeyboardAuthorityMode::Capture);
                let entered = f.tree.composite_editor_snapshot(&id).unwrap();
                assert_eq!(entered.text, "45");
                assert!(entered.session > previous.session,
                    "re-entry is a new qualified session, not an old motion leg");
                let a = entered.selection_anchor.min(entered.selection_focus);
                let b = entered.selection_anchor.max(entered.selection_focus);
                let expected = format!("{}5{}", &entered.text[..a], &entered.text[b..]);
                let time = Instant::now();
                f.batch(vec![
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY, time,
                        key_code: KeyCode::Character('5'), physical_key: PhysicalKeyCode::Digit5,
                        reservation: None, state: KeyState::Down, repeat: false,
                        modifiers: Modifiers::default(),
                    },
                    RawInputEvent::TextInput {
                        window_id: WindowId::PRIMARY, time, text: "5".into(),
                        modifiers: Modifiers::default(),
                    },
                    RawInputEvent::Key {
                        window_id: WindowId::PRIMARY, time,
                        key_code: KeyCode::Character('5'), physical_key: PhysicalKeyCode::Digit5,
                        reservation: None, state: KeyState::Up, repeat: false,
                        modifiers: Modifiers::default(),
                    },
                ]);
                assert_eq!(f.tree.composite_editor_snapshot(&id).unwrap().text, expected,
                    "the original re-entry pointer and later paired typing execute once");
                assert_eq!(f.tree.node(f.root).unwrap().widget().as_any()
                    .downcast_ref::<Slider>().unwrap().value(), 7.0);
            }
        }
    }

    #[test]
    fn composite_editor_platform_key_text_pair_preserves_printable_typing() {
        let cases: Vec<(&str, Box<dyn Widget>)> = vec![
            ("ComboBox", Box::new(ComboBox::simple(Vec::new()))),
            ("SpinBox", Box::new(SpinBox::new().with_value(7))),
            (
                "Slider",
                Box::new(Slider::new(0.0, 100.0).with_value(7.0).with_value_display()),
            ),
            (
                "TagInput",
                Box::new(TagInput::new().with_tag_creation(TagCreationMode::FreeFormOnly)),
            ),
            ("TextField", Box::new(TextField::new())),
        ];
        let mut failures = Vec::new();
        for (name, widget) in cases {
            let mut f = Fixture::new(widget, UiInteractionPolicy::PointerDriven);
            let host = f.tree.widget_id_for_node(f.root).unwrap();
            let relation = f.tree.composite_editor_id(host).ok();
            let text_node = relation
                .as_ref()
                .map_or(f.root, |id| f.tree.find_by_id(id.child()).unwrap());
            let region = f.tree.get_absolute_paint_rect(text_node).unwrap();
            let point = Point::new(
                region.x + region.width * 0.5,
                region.y + region.height * 0.5,
            );
            f.batch(vec![
                button(point, PointerButton::Primary, RawButtonState::Pressed),
                button(point, PointerButton::Primary, RawButtonState::Released),
            ]);
            assert_eq!(
                f.domains.primary_state().focus_manager.focused(),
                Some(host),
                "{name}"
            );
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture,
                "{name}"
            );

            // The native adapter gives the physical KeyDown and its TextInput
            // the same timestamp. A child request is not key consumption.
            let time = Instant::now();
            f.batch(vec![
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code: KeyCode::Character('5'),
                    physical_key: PhysicalKeyCode::Digit5,
                    reservation: None,
                    state: KeyState::Down,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time,
                    text: "5".into(),
                    modifiers: Modifiers::default(),
                },
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code: KeyCode::Character('5'),
                    physical_key: PhysicalKeyCode::Digit5,
                    reservation: None,
                    state: KeyState::Up,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ]);
            let text = if let Some(id) = &relation {
                f.tree.composite_editor_snapshot(id).unwrap().text
            } else {
                f.tree
                    .node(text_node)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<TextField>()
                    .unwrap()
                    .text()
                    .to_owned()
            };
            if text != "5" {
                failures.push(format!("{name}: expected 5, got {text:?}"));
            }
            assert_eq!(
                f.domains.primary_state().focus_manager.focused(),
                Some(host),
                "{name}"
            );
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture,
                "{name}"
            );
            if let Some(id) = relation {
                // Space is actually consumed by the child KeyDown. Its paired
                // text must still be suppressed, so it is inserted only once.
                if matches!(name, "ComboBox" | "TagInput") {
                    let time = Instant::now();
                    f.batch(vec![
                        RawInputEvent::Key {
                            window_id: WindowId::PRIMARY,
                            time,
                            key_code: KeyCode::Space,
                            physical_key: PhysicalKeyCode::Space,
                            reservation: None,
                            state: KeyState::Down,
                            repeat: false,
                            modifiers: Modifiers::default(),
                        },
                        RawInputEvent::TextInput {
                            window_id: WindowId::PRIMARY,
                            time,
                            text: " ".into(),
                            modifiers: Modifiers::default(),
                        },
                        RawInputEvent::Key {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            key_code: KeyCode::Space,
                            physical_key: PhysicalKeyCode::Space,
                            reservation: None,
                            state: KeyState::Up,
                            repeat: false,
                            modifiers: Modifiers::default(),
                        },
                    ]);
                    assert_eq!(
                        f.tree.composite_editor_snapshot(&id).unwrap().text,
                        "5 ",
                        "{name}"
                    );
                }
                let appearance = FrameworkAppearanceSnapshot::capture(
                    &f.domains,
                    WindowId::PRIMARY,
                    &f.tree,
                )
                .unwrap();
                assert!(!appearance.for_owner(id.child()).focused(), "{name}");
                assert!(!appearance.for_owner(id.child()).keyboard_captured(), "{name}");
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("; "));
    }

    #[test]
    fn composite_editor_first_combo_pointer_activates_host_before_original_input() {
        let mut f = Fixture::new(
            Box::new(ComboBox::simple(Vec::new())),
            UiInteractionPolicy::PointerDriven,
        );
        let host = f.tree.widget_id_for_node(f.root).unwrap();
        let id = f.tree.composite_editor_id(host).unwrap();
        let before = f.tree.composite_editor_snapshot(&id).unwrap();
        let point = Point::new(30.0, 10.0);
        f.batch(vec![
            button(point, PointerButton::Primary, RawButtonState::Pressed),
            button(point, PointerButton::Primary, RawButtonState::Released),
            RawInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                text: "Z".into(),
                modifiers: Modifiers::default(),
            },
        ]);
        let after = f.tree.composite_editor_snapshot(&id).unwrap();
        assert_eq!(after.text, "Z");
        assert_eq!(after.session, before.session + 1);
        let child = f.tree.find_by_id(id.child()).unwrap();
        assert!(!f.tree.node(child).unwrap().widget.needs_animation());
        f.frame();
        let appearance =
            FrameworkAppearanceSnapshot::capture(&f.domains, WindowId::PRIMARY, &f.tree).unwrap();
        let actual_child_facts = appearance.for_owner(id.child());
        assert!(!actual_child_facts.focused());
        assert!(!actual_child_facts.keyboard_captured());
        let delegation = f
            .tree
            .composite_delegation(id.child(), &appearance)
            .unwrap();
        assert_eq!(delegation.host, host);
        assert!(delegation.host_facts.keyboard_captured());
        // The canonical frame's real child Update enabled its blink demand from
        // delegation while the child's actual ownership facts remain false.
        assert!(f.tree.node(child).unwrap().widget.needs_animation());
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(host)
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        assert_ne!(
            f.domains.primary_state().focus_manager.focused(),
            Some(id.child())
        );
    }

    #[test]
    fn composite_editor_numeric_router_commits_preserve_callbacks_and_capture_policy() {
        for is_spin_box in [true, false] {
            let changes = Rc::new(Cell::new(0));
            let changed_value = Rc::new(Cell::new(0.0_f64));
            let commits = Rc::new(Cell::new(0));
            let on_change_count = Rc::clone(&changes);
            let on_change_value = Rc::clone(&changed_value);
            let widget: Box<dyn Widget> = if is_spin_box {
                let mut spin = SpinBox::new().with_value(7).on_value_changed(move |value| {
                    on_change_count.set(on_change_count.get() + 1);
                    on_change_value.set(value as f64);
                });
                let on_commit_count = Rc::clone(&commits);
                spin.set_on_commit(move |value| {
                    assert_eq!(value, 42);
                    on_commit_count.set(on_commit_count.get() + 1);
                });
                Box::new(spin)
            } else {
                Box::new(
                    Slider::new(0.0, 100.0)
                        .with_value(7.0)
                        .with_value_display()
                        .on_value_changed(move |value| {
                            on_change_count.set(on_change_count.get() + 1);
                            on_change_value.set(value as f64);
                        }),
                )
            };
            let mut f = Fixture::new(widget, UiInteractionPolicy::PointerDriven);
            let host = f.tree.widget_id_for_node(f.root).unwrap();
            let id = f.tree.composite_editor_id(host).unwrap();
            let child = f.tree.find_by_id(id.child()).unwrap();
            let region = f.tree.get_absolute_paint_rect(child).unwrap();
            let point = Point::new(
                region.x + region.width * 0.5,
                region.y + region.height * 0.5,
            );
            f.batch(vec![
                button(point, PointerButton::Primary, RawButtonState::Pressed),
                button(point, PointerButton::Primary, RawButtonState::Released),
            ]);
            assert_eq!(
                f.domains.primary_state().focus_manager.focused(),
                Some(host)
            );
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture
            );
            let entry = f.tree.composite_editor_snapshot(&id).unwrap();
            assert_eq!(entry.text, if is_spin_box { "7" } else { "7.0" });
            assert_eq!(
                (
                    entry.selection_anchor.min(entry.selection_focus),
                    entry.selection_anchor.max(entry.selection_focus)
                ),
                (0, entry.text.len())
            );
            assert_eq!(changes.get(), 0);
            f.batch(vec![
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    text: "42".into(),
                    modifiers: Modifiers::default(),
                },
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    key_code: KeyCode::Enter,
                    physical_key: PhysicalKeyCode::Unknown,
                    reservation: None,
                    state: KeyState::Down,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ]);
            let final_value = if is_spin_box {
                f.tree
                    .node(f.root)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<SpinBox>()
                    .unwrap()
                    .value() as f64
            } else {
                f.tree
                    .node(f.root)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<Slider>()
                    .unwrap()
                    .value() as f64
            };
            assert_eq!(final_value, 42.0);
            assert_eq!(changed_value.get(), 42.0);
            assert_eq!(changes.get(), 1);
            assert_eq!(commits.get(), if is_spin_box { 1 } else { 0 });
            let applied = f.tree.composite_editor_snapshot(&id).unwrap();
            assert_eq!(applied.text, "42");
            assert!(applied.revision > entry.revision);
            assert_eq!(
                f.domains.primary_state().focus_manager.focused(),
                Some(host)
            );
            assert_ne!(
                f.domains.primary_state().focus_manager.focused(),
                Some(id.child())
            );
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                if is_spin_box {
                    crate::core::event::KeyboardAuthorityMode::Navigation
                } else {
                    crate::core::event::KeyboardAuthorityMode::Capture
                }
            );
            f.frame();
            assert!(!f.tree.node(child).unwrap().widget.needs_animation());
        }
    }

    #[test]
    fn composite_editor_routed_text_then_enter_has_current_result_and_one_callback() {
        let callbacks = Rc::new(Cell::new(0));
        let observed = Rc::clone(&callbacks);
        let tag = TagInput::new()
            .with_tag_creation(TagCreationMode::FreeFormOnly)
            .on_change(move |entries| {
                assert_eq!(entries.len(), 1);
                observed.set(observed.get() + 1);
            });
        let mut f = Fixture::new(Box::new(tag), UiInteractionPolicy::PointerDriven);
        let host = f.tree.widget_id_for_node(f.root).unwrap();
        let id = f.tree.composite_editor_id(host).unwrap();
        let text = |value: &str| RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: value.into(),
            modifiers: Modifiers::default(),
        };
        f.batch(vec![text("uncaptured")]);
        assert_eq!(f.tree.composite_editor_snapshot(&id).unwrap().text, "");
        let point = Point::new(100.0, 10.0);
        f.batch(vec![
            button(point, PointerButton::Primary, RawButtonState::Pressed),
            button(point, PointerButton::Primary, RawButtonState::Released),
        ]);
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(host)
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        let before = f.tree.composite_editor_snapshot(&id).unwrap();
        f.batch(vec![
            text("newtag"),
            RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Enter,
                physical_key: PhysicalKeyCode::Unknown,
                reservation: None,
                state: KeyState::Down,
                repeat: false,
                modifiers: Modifiers::default(),
            },
        ]);
        let after = f.tree.composite_editor_snapshot(&id).unwrap();
        assert_eq!(callbacks.get(), 1);
        assert_eq!(after.text, "");
        assert!(after.revision > before.revision);
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(host)
        );
        assert_ne!(
            f.domains.primary_state().focus_manager.focused(),
            Some(id.child())
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
    }

    #[test]
    fn cursor_tag_chip_overflow_preserves_bounded_typing() {
        for first in ["Rust", "A selected tag too wide to fit in the available control"] {
            let tag = TagInput::new()
                .with_items(vec![
                    TagItem { id: 1, label: first.into(), group: None, enabled: true },
                    TagItem { id: 2, label: "Graphics".into(), group: None, enabled: true },
                    TagItem { id: 3, label: "Systems".into(), group: None, enabled: true },
                ])
                .with_selection(&[1, 2, 3])
                .with_tag_creation(TagCreationMode::Hybrid);
            let mut f = Fixture::new(Box::new(tag), UiInteractionPolicy::PointerDriven);
            let host = f.tree.widget_id_for_node(f.root).unwrap();
            let id = f.tree.composite_editor_id(host).unwrap();
            let child = f.tree.find_by_id(id.child()).unwrap();
            let bounds = f.tree.get_absolute_paint_rect(f.root).unwrap();
            let input = f.tree.get_absolute_paint_rect(child).unwrap();
            let text_right = bounds.right() - Theme::dark().geometry.combo_box.affordance_width;
            assert!(input.x >= bounds.x && input.width >= 60.0, "{input:?}");
            assert!(input.right() <= text_right, "child must not overrun its host: {input:?}");
            let point = Point::new(input.x + input.width * 0.5, input.y + input.height * 0.5);
            f.hover(point, CursorIcon::Text);
            let time = Instant::now();
            f.batch(vec![
                button(point, PointerButton::Primary, RawButtonState::Pressed),
                button(point, PointerButton::Primary, RawButtonState::Released),
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY, time, key_code: KeyCode::Character('q'),
                    physical_key: PhysicalKeyCode::KeyQ, reservation: None, state: KeyState::Down,
                    repeat: false, modifiers: Modifiers::default(),
                },
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY, time, text: "q".into(),
                    modifiers: Modifiers::default(),
                },
                RawInputEvent::Key {
                    window_id: WindowId::PRIMARY, time, key_code: KeyCode::Character('q'),
                    physical_key: PhysicalKeyCode::KeyQ, reservation: None, state: KeyState::Up,
                    repeat: false, modifiers: Modifiers::default(),
                },
            ]);
            assert_eq!(f.tree.composite_editor_snapshot(&id).unwrap().text, "q");
            assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(host));
            assert_eq!(f.domains.primary_state().keyboard_mode,
                crate::core::event::KeyboardAuthorityMode::Capture);
            assert_eq!(f.tree.node(f.root).unwrap().widget().as_any()
                .downcast_ref::<TagInput>().unwrap().committed_entries().len(), 3);
            f.frame();
            let painted_input = f.tree.get_absolute_paint_rect(child).unwrap();
            assert!(painted_input.right() <= text_right && painted_input.width >= 60.0);
            f.hover(Point::new(painted_input.x + painted_input.width * 0.5, point.y), CursorIcon::Text);
        }
    }

    #[test]
    fn cursor_tag_inline_geometry_refreshes_through_layout_and_reuses_settled_retained_output() {
        let item = |label: &str| TagItem {
            id: 1,
            label: label.into(),
            group: None,
            enabled: true,
        };
        for policy in [
            TagInputPolicy::Full,
            TagInputPolicy::ToggleOnly,
            TagInputPolicy::ReadOnly,
            TagInputPolicy::Locked,
        ] {
            let tag = TagInput::new()
                .with_items(vec![item("A")])
                .with_selection(&[1])
                .with_policy(policy)
                .with_tag_creation(TagCreationMode::Hybrid);
            let mut f = Fixture::new(Box::new(tag), UiInteractionPolicy::PointerDriven);
            f.hover(
                Point::new(200.0, 10.0),
                if policy == TagInputPolicy::Full {
                    CursorIcon::Text
                } else {
                    CursorIcon::Default
                },
            );
            f.hover(Point::new(10.0, 10.0), CursorIcon::Default);
            f.hover(Point::new(299.0, 10.0), CursorIcon::Default);
            f.hover(
                Point::new(
                    300.0 - Theme::dark().geometry.combo_box.affordance_width,
                    10.0,
                ),
                CursorIcon::Default,
            );
            f.assert_no_capture();
            f.frame();
            let owner = f.tree.widget_id_for_node(f.root).unwrap();
            let before = f.retained.local_fragment(owner).unwrap();
            f.hover(Point::new(298.0, 10.0), CursorIcon::Default);
            f.frame();
            assert!(Arc::ptr_eq(
                &before,
                &f.retained.local_fragment(owner).unwrap()
            ));
            if policy == TagInputPolicy::Full {
                let receipt =
                    f.queue_composite_action(CompositeTestAction::SetTagItems(vec![item(
                        "A considerably longer chip",
                    )]));
                f.flush_composite_actions();
                assert!(
                    receipt
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .applied()
                );
                f.frame();
                assert!(!Arc::ptr_eq(
                    &before,
                    &f.retained.local_fragment(owner).unwrap()
                ));
                let id = f.tree.composite_editor_id(owner).unwrap();
                let child = f.tree.find_by_id(id.child()).unwrap();
                let input = f.tree.get_absolute_paint_rect(child).unwrap();
                f.hover(Point::new(input.x - 1.0, 10.0), CursorIcon::Default);
                f.hover(Point::new(input.x + input.width * 0.5, 10.0), CursorIcon::Text);
                f.frame();
                let settled = f.retained.local_fragment(owner).unwrap();
                f.frame();
                assert!(Arc::ptr_eq(
                    &settled,
                    &f.retained.local_fragment(owner).unwrap()
                ));
            }
        }
        for display in [TagDisplayMode::CountBadge, TagDisplayMode::Summary] {
            let tag = TagInput::new()
                .with_tag_creation(TagCreationMode::Hybrid)
                .with_display_mode(display);
            let mut f = Fixture::new(Box::new(tag), UiInteractionPolicy::PointerDriven);
            f.hover(Point::new(200.0, 10.0), CursorIcon::Default);
        }
    }
    #[test]
    fn cursor_tag_stretched_inline_preserves_allocated_text_baseline() {
        use crate::core::layout::Alignment;
        use crate::core::scene::{SceneNode, TextNode};
        use crate::widgets::flex::Flex;
        fn collect(nodes: &[SceneNode], glyphs: &mut Vec<TextNode>) {
            for node in nodes {
                match node {
                    SceneNode::Text(text) if text.text == "A" || text.text == "query" => {
                        glyphs.push(text.clone())
                    }
                    SceneNode::RetainedFragment(fragment) => collect(&fragment.children, glyphs),
                    SceneNode::Clip(clip) => collect(&clip.children, glyphs),
                    _ => {}
                }
            }
        }
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::row().with_align_items(Alignment::Stretch)),
            None,
            0,
        );
        let tag = tree.add_child(
            root,
            Box::new(
                TagInput::new()
                    .with_items(vec![TagItem {
                        id: 1,
                        label: "A".into(),
                        group: None,
                        enabled: true,
                    }])
                    .with_selection(&[1])
                    .with_tag_creation(TagCreationMode::Hybrid),
            ),
            None,
            0,
        );
        tree.add_child(
            root,
            Box::new(TextArea::new().with_fixed_height(80.0)),
            None,
            1,
        );
        tree.set_flex(
            tag,
            crate::core::layout::FlexParams {
                grow: 0.0,
                shrink: 0.0,
                basis: 200.0,
            },
        );
        let mut f = Fixture::from_tree(tree, root, UiInteractionPolicy::PointerDriven);
        let rect = f.tree.get_absolute_paint_rect(tag).unwrap();
        assert_eq!(rect.height, f.rect().height);
        assert!(rect.height > Theme::dark().geometry.text_field.height);
        assert!(
            rect.width.is_finite() && rect.width > 80.0,
            "Tag allocation: {rect:?}"
        );
        let point = Point::new(rect.x + 70.0, rect.y + 10.0);
        f.batch(vec![
            button(point, PointerButton::Primary, RawButtonState::Pressed),
            button(point, PointerButton::Primary, RawButtonState::Released),
            RawInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                text: "query".into(),
                modifiers: Modifiers::default(),
            },
        ]);
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            f.tree.widget_id_for_node(tag),
            "Tag focus after routed edit entry; rect={rect:?}"
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        f.frame();
        let owner = f.tree.widget_id_for_node(tag).unwrap();
        let editor = f.tree.composite_editor_id(owner).unwrap();
        assert_eq!(f.tree.composite_editor_snapshot(&editor).unwrap().text, "query");
        let mut glyphs = Vec::new();
        collect(&f.retained.local_fragment(owner).unwrap(), &mut glyphs);
        collect(&f.retained.local_fragment(editor.child()).unwrap(), &mut glyphs);
        assert_eq!(glyphs.len(), 2, "one host chip and one tree-owned editor text run");
        let theme = &f.app.context.theme;
        for (label, font_size) in [
            ("A", theme.typography.font_size),
            ("query", theme.typography.input_font_size),
        ] {
            let glyph = glyphs
                .iter()
                .find(|glyph| glyph.text == label)
                .expect("allocated control text");
            assert_eq!(glyph.font_size, font_size);
            let metrics = f.text.font_metrics(glyph.font_size);
            let actual_baseline = glyph.position.y + metrics.ascent;
            let expected_baseline = rect.y
                + crate::core::text_engine::compute_control_text_baseline_y(
                    rect.height,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                );
            assert!(
                (actual_baseline - expected_baseline).abs() <= 0.001,
                "{label}: baseline={actual_baseline}, expected={expected_baseline}, rect={rect:?}"
            );
            assert!(
                glyph.position.y >= rect.y - 0.001
                    && actual_baseline + metrics.descent <= rect.bottom() + 0.001,
                "{label}: glyph metrics must fit the allocated control rect {rect:?}"
            );
        }
    }

    struct HideOnCapture {
        id: WidgetInternalId,
    }
    impl WidgetDowncast for HideOnCapture {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
    impl Widget for HideOnCapture {
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> crate::core::layout::LayoutResult {
            crate::core::layout::LayoutResult::fit(Size::new(300.0, 120.0))
        }
        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
        fn handle_event(
            &mut self,
            cx: &mut EventContext,
            event: &Event,
            phase: EventPhase,
        ) -> bool {
            if phase == EventPhase::Target
                && matches!(event, Event::Pointer(pe) if pe.kind == PointerEventKind::Down && pe.button == Some(PointerButton::Secondary))
            {
                cx.capture_pointer(PointerButton::Secondary);
                cx.request_cursor(CursorIcon::Hidden);
                true
            } else {
                false
            }
        }
    }

    #[test]
    fn dedicated_cursor_real_ui_thread_orders_batches_resize_hidden_and_shutdown() {
        use crate::widgets::split_pane::SplitPane;
        use std::sync::mpsc;
        use winit::window::CursorIcon as NativeCursor;

        fn receive_until(
            output: &mut UiThreadCursorOutput,
            events: &mut Vec<(Option<bool>, Option<NativeCursor>)>,
            ready: impl Fn(&[(Option<bool>, Option<NativeCursor>)]) -> bool,
        ) {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                output.drain_with(false, |visible, icon| events.push((visible, icon)));
                if ready(events) {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "cursor output timeout: {events:?}"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        let (input_tx, input_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (start_tx, start_rx) = mpsc::channel();
        let config = UiThreadConfig::new(
            Theme::dark(),
            Box::new(FontManagerTextEngine::new()),
            Constraints::loose(Size::new(300.0, 120.0)),
        )
        .with_ui_interaction_policy(UiInteractionPolicy::PointerDriven);
        let (worker, mut output) = spawn_ui_thread_with_cursor_output(
            config,
            move || {
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(SplitPane::horizontal(NodeId(0), NodeId(0))),
                    None,
                    0,
                );
                let leading = tree.add_child(root, Box::new(TextField::new()), None, 0);
                let trailing = tree.add_child(
                    root,
                    Box::new(HideOnCapture {
                        id: WidgetInternalId::new(),
                    }),
                    None,
                    1,
                );
                let split = tree
                    .node_mut(root)
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<SplitPane>()
                    .unwrap();
                split.leading = leading;
                split.trailing = trailing;
                let fixture = Fixture::from_tree(tree, root, UiInteractionPolicy::PointerDriven);
                let rect = fixture.tree.get_rect(leading).unwrap();
                ready_tx
                    .send(Point::new(rect.x + rect.width + 1.0, 10.0))
                    .unwrap();
                start_rx.recv().unwrap();
                (fixture.tree, root)
            },
            input_rx,
            SceneSwapchain::new(),
        );
        let handle = ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let text = Point::new(10.0, 10.0);
        let outside = Point::new(400.0, 10.0);
        let hide = Point::new(290.0, 10.0);
        // Queue the entire sequence before the real loop can drain its first batch.
        // The second Hidden publication witnesses all preceding routed transitions.
        for event in [
            moved(text),
            key(KeyState::Down),
            moved(outside),
            key(KeyState::Up),
            moved(text),
            key(KeyState::Down),
            key(KeyState::Up),
            button(handle, PointerButton::Primary, RawButtonState::Pressed),
            moved(Point::new(250.0, 10.0)),
            key(KeyState::Down),
            button(handle, PointerButton::Secondary, RawButtonState::Pressed),
            key(KeyState::Up),
            button(text, PointerButton::Primary, RawButtonState::Released),
            button(text, PointerButton::Secondary, RawButtonState::Released),
            moved(text),
            button(hide, PointerButton::Secondary, RawButtonState::Pressed),
            key(KeyState::Down),
            key(KeyState::Up),
            moved(Point::new(289.0, 10.0)),
            button(outside, PointerButton::Secondary, RawButtonState::Released),
            // Keep this a new single press, not the double-click policy path.
            button(
                Point::new(280.0, 10.0),
                PointerButton::Secondary,
                RawButtonState::Pressed,
            ),
        ] {
            input_tx.send(event).unwrap();
        }
        start_tx.send(()).unwrap();
        let mut events = Vec::new();
        receive_until(&mut output, &mut events, |events| {
            events
                .iter()
                .filter(|(visible, _)| *visible == Some(false))
                .count()
                == 2
        });
        let icons: Vec<_> = events.iter().filter_map(|(_, icon)| *icon).collect();
        assert_eq!(
            &icons[..3],
            [
                NativeCursor::Text,
                NativeCursor::Default,
                NativeCursor::Text
            ]
        );
        let resize = icons
            .iter()
            .position(|icon| *icon == NativeCursor::ColResize)
            .expect("actual SplitPane resize output");
        let text_after_resize = icons[resize..]
            .iter()
            .position(|icon| *icon == NativeCursor::Text)
            .map(|index| resize + index)
            .expect("text after release");
        assert!(
            icons[resize..text_after_resize]
                .iter()
                .all(|icon| *icon == NativeCursor::ColResize),
            "{events:?}"
        );
        let first_hidden = events
            .iter()
            .position(|(visible, _)| *visible == Some(false))
            .unwrap();
        assert_eq!(
            &events[first_hidden..],
            [
                (Some(false), None),
                (Some(true), Some(NativeCursor::Default)),
                (Some(false), None),
            ],
            "keyboard and captured motion must preserve Hidden until release"
        );
        // Input disconnection retires actual pointer capture and drops the producer.
        drop(input_tx);
        worker.join().expect("dedicated UI thread panicked");
        events.clear();
        output.drain_with(false, |visible, icon| events.push((visible, icon)));
        assert_eq!(events, [(Some(true), Some(NativeCursor::Default))]);
        events.clear();
        output.drain_with(false, |visible, icon| events.push((visible, icon)));
        assert!(
            events.is_empty(),
            "closed output must not restore repeatedly"
        );
    }

    #[test]
    fn dedicated_cursor_destroyed_capture_owner_restores_native_output() {
        use std::sync::mpsc;
        use winit::window::CursorIcon as NativeCursor;
        let (sender, mut output) = cursor_output::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (remove_tx, remove_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut fixture = Fixture::new(
                Box::new(HideOnCapture {
                    id: WidgetInternalId::new(),
                }),
                UiInteractionPolicy::PointerDriven,
            );
            fixture.cursor_sender = Some(sender);
            fixture.batch(vec![button(
                Point::new(10.0, 10.0),
                PointerButton::Secondary,
                RawButtonState::Pressed,
            )]);
            ready_tx.send(()).unwrap();
            remove_rx.recv().unwrap();
            fixture.tree.remove_node(fixture.root);
            let removed = fixture.tree.drain_pending_removals();
            fixture
                .router
                .on_widgets_destroyed(&removed, fixture.domains.primary_state_mut());
            fixture.settle(false, false);
            assert!(
                !fixture
                    .domains
                    .primary_state()
                    .pointer_capture_manager
                    .is_captured(PointerId::MOUSE)
            );
            ready_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
        });
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut events = Vec::new();
        output.drain_with(false, |visible, icon| events.push((visible, icon)));
        assert_eq!(events, [(Some(false), None)]);
        remove_tx.send(()).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        events.clear();
        output.drain_with(false, |visible, icon| events.push((visible, icon)));
        assert_eq!(events, [(Some(true), Some(NativeCursor::Default))]);
        finish_tx.send(()).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn cursor_host_preserves_hidden_restoration_relative_lease_and_resize_producers() {
        let mut f = Fixture::new(
            Box::new(HideOnCapture {
                id: WidgetInternalId::new(),
            }),
            UiInteractionPolicy::PointerDriven,
        );
        f.batch(vec![button(
            Point::new(10.0, 10.0),
            PointerButton::Secondary,
            RawButtonState::Pressed,
        )]);
        assert!(!f.visible && f.router.cursor_hidden_sticky);
        f.output.clear();
        f.batch(vec![
            moved(Point::new(400.0, 10.0)),
            key(KeyState::Down),
            key(KeyState::Up),
        ]);
        assert!(f.output.is_empty() && !f.visible);
        f.router.cancel_all_pointer_sessions(
            crate::core::event::CancellationReason::SystemCancel,
            f.domains.primary_state_mut(),
        );
        f.router.reset_cursor_request();
        f.settle(false, false);
        assert!(f.visible && !f.router.cursor_hidden_sticky);
        assert_eq!(f.icon, CursorIcon::Default);
        f.output.clear();
        f.settle(true, true);
        assert!(f.output.is_empty());

        use crate::widgets::split_pane::SplitPane;
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(SplitPane::horizontal(NodeId(0), NodeId(0))),
            None,
            0,
        );
        let leading = tree.add_child(root, Box::new(TextField::new()), None, 0);
        let trailing = tree.add_child(root, Box::new(TextField::new()), None, 1);
        let split = tree
            .node_mut(root)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<SplitPane>()
            .unwrap();
        split.leading = leading;
        split.trailing = trailing;
        let mut f = Fixture::from_tree(tree, root, UiInteractionPolicy::PointerDriven);
        let leading_rect = f.tree.get_rect(leading).unwrap();
        let handle = Point::new(leading_rect.x + leading_rect.width + 1.0, 10.0);
        f.hover(handle, CursorIcon::ColResize);
        f.batch(vec![
            button(handle, PointerButton::Primary, RawButtonState::Pressed),
            moved(Point::new(250.0, 10.0)),
            key(KeyState::Down),
            button(handle, PointerButton::Secondary, RawButtonState::Pressed),
            key(KeyState::Up),
        ]);
        assert_eq!(f.icon, CursorIcon::ColResize);
        assert_eq!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            f.tree.widget_id_for_node(root),
        );
        f.batch(vec![button(
            Point::new(250.0, 10.0),
            PointerButton::Primary,
            RawButtonState::Released,
        )]);
        assert_eq!(f.icon, CursorIcon::Text);
        assert!(
            !f.domains
                .primary_state()
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE)
        );
        assert!(f.fusion.button_state().secondary);
        f.batch(vec![button(
            Point::new(250.0, 10.0),
            PointerButton::Secondary,
            RawButtonState::Released,
        )]);
        f.hover(Point::new(400.0, 10.0), CursorIcon::Default);
    }
