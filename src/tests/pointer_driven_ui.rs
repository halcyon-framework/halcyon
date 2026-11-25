use crate::core::app::app_root_overlay_input_tests::{RecordingInputScreen, dispatch_physical_raw};
use crate::core::app::{
    AppConfig, AppContext, AppRoot, RetainedSceneTestHarness, Shortcut, UiThreadConfig,
};
use crate::core::appearance::{AppearanceRole, ButtonPaintCase, FrameworkAppearanceSnapshot};
use crate::core::event::{
    BindingKey, FusedInputEvent, InteractionDomains, KeyCode, KeyState, KeyboardAuthorityMode,
    KeyboardKeyReservation, Modifiers, PhysicalKeyCode, PointerButton, PointerDevice,
    RawButtonState, RawInputEvent, UiInteractionPolicy, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icon::IconRegistry;
use crate::core::input_fusion::InputFusion;
use crate::core::layout::Constraints;
use crate::core::math::{Point, Size};
use crate::core::theme::Theme;
use crate::core::widget::Widget;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::button::Button;
use crate::widgets::flex::Flex;
use crate::widgets::keybind_editor::KeybindEditor;
use crate::widgets::screen_host::ScreenHost;
use crate::widgets::text_field::TextField;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

#[test]
fn pointer_driven_treeview_activation() {
    use crate::tests::treeview_smoke::TreeActivationFixture;
    for policy in [
        UiInteractionPolicy::NavigationAndCapture,
        UiInteractionPolicy::PointerDriven,
    ] {
        let mut fixture = TreeActivationFixture::new(policy, true);
        fixture.click(1);
        assert_eq!(&*fixture.pending.borrow(), &[1]);
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            Some(fixture.owner(fixture.tree_node))
        );
        fixture.key(KeyCode::Enter, true, false, Modifiers::default());
        let expected = if policy == UiInteractionPolicy::NavigationAndCapture {
            2
        } else {
            1
        };
        assert_eq!(fixture.pending.borrow().len(), expected);
        let neighbor = fixture
            .tree
            .get_absolute_paint_rect(fixture.neighbor)
            .unwrap();
        let p = Point::new(
            neighbor.x + neighbor.width * 0.5,
            neighbor.y + neighbor.height * 0.5,
        );
        fixture.button(p, true);
        fixture.button(p, false);
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            Some(fixture.owner(fixture.neighbor))
        );
        fixture.key(KeyCode::Enter, true, false, Modifiers::default());
        assert_eq!(fixture.pending.borrow().len(), expected);
    }
}

struct Fixture {
    tree: WidgetTree,
    root: NodeId,
    nodes: [NodeId; 5],
    domains: InteractionDomains,
    app: AppRoot,
    router: EventRouter,
    fusion: InputFusion,
    text: FontManagerTextEngine,
    retained: RetainedSceneTestHarness,
    clicks: Rc<Cell<usize>>,
    recordings: Rc<RefCell<Vec<Option<crate::runtime::actions::Shortcut>>>>,
    screen_events: Rc<RefCell<Vec<FusedInputEvent>>>,
}

impl Fixture {
    fn new(policy: UiInteractionPolicy) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let clicks = Rc::new(Cell::new(0));
        let clicked = Rc::clone(&clicks);
        let recordings = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&recordings);
        let nodes = [
            tree.add_child(
                root,
                Box::new(
                    Button::with_text("First").on_click(move || clicked.set(clicked.get() + 1)),
                ),
                None,
                0,
            ),
            tree.add_child(root, Box::new(Button::with_text("Second")), None, 0),
            tree.add_child(root, Box::new(TextField::new().with_text("kept")), None, 0),
            tree.add_child(
                root,
                Box::new(TextField::new().with_text("neighbor")),
                None,
                0,
            ),
            tree.add_child(
                root,
                Box::new(
                    KeybindEditor::new()
                        .with_physical_keys(true)
                        .on_change(move |value| recorded.borrow_mut().push(value)),
                ),
                None,
                0,
            ),
        ];
        let screen_events = Rc::new(RefCell::new(Vec::new()));
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        app.push_screen(Box::new(RecordingInputScreen {
            host: ScreenHost::new(),
            events: Rc::clone(&screen_events),
        }));
        let mut fixture = Self {
            tree,
            root,
            nodes,
            domains: AppConfig::default()
                .with_ui_interaction_policy(policy)
                .interaction_domains(),
            app,
            router: EventRouter::new(),
            fusion: InputFusion::new(),
            text: FontManagerTextEngine::new(),
            retained: RetainedSceneTestHarness::new(),
            clicks,
            recordings,
            screen_events,
        };
        fixture.frame();
        fixture
    }

    fn frame(&mut self) {
        self.retained.frame_with_appearance(
            &mut self.tree,
            self.root,
            Constraints::loose(Size::new(500.0, 500.0)),
            &Theme::dark(),
            &mut self.text,
            &IconRegistry::new(&[]),
            &self.domains,
            WindowId::PRIMARY,
        );
    }

    fn raw(&mut self, event: RawInputEvent) {
        dispatch_physical_raw(
            &mut self.app,
            &mut self.tree,
            &mut self.router,
            self.domains.primary_state_mut(),
            &mut self.fusion,
            &mut self.text,
            event,
        );
    }

    fn button_event(&mut self, node: usize, state: RawButtonState) {
        let rect = self.tree.get_absolute_paint_rect(self.nodes[node]).unwrap();
        self.raw(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state,
            position: Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5),
        });
    }

    fn click(&mut self, node: usize) {
        self.button_event(node, RawButtonState::Pressed);
        self.button_event(node, RawButtonState::Released);
    }

    fn key(&mut self, key_code: KeyCode) {
        self.raw(RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            state: KeyState::Down,
            repeat: false,
            modifiers: Modifiers::default(),
        });
    }

    fn widget<W: Widget + 'static>(&self, node: usize) -> &W {
        self.tree
            .node(self.nodes[node])
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn owner(&self, node: usize) -> crate::utils::id::TreeWidgetId {
        self.tree.widget_id_for_node(self.nodes[node]).unwrap()
    }

    fn facts(&self, node: usize) -> crate::core::appearance::FrameworkAppearanceFacts {
        FrameworkAppearanceSnapshot::capture(&self.domains, WindowId::PRIMARY, &self.tree)
            .unwrap()
            .for_owner(self.owner(node))
    }
}

#[test]
fn pointer_driven_button_keyboard_only_focus_cue_tracks_the_current_mount() {
    use crate::core::appearance::{
        AppearanceConfigurationId, AppearanceConfigurationOwner, AppearanceFallbackPolicy,
        ButtonAppearanceConfiguration, ButtonAppearanceContract, ButtonAppearanceDefinition,
        FocusCueModality, ProductAppearanceOwner,
    };
    use crate::core::paint::{Color, LAYER_FOCUS};
    use crate::core::scene::SceneNode;

    let mut f = Fixture::new(UiInteractionPolicy::NavigationAndCapture);
    let contract = ButtonAppearanceContract::standard();
    let selector = contract.selectors()[0].clone();
    let mut values = Theme::dark().button_appearance_values(&contract);
    for paint in [
        &mut values.resting,
        &mut values.hovered,
        &mut values.pressed,
        &mut values.disabled,
    ]
    .into_iter()
    .flatten()
    {
        paint.radius = 6.0;
    }
    let gold = Color::from_hex(0xEBC597);
    values.focus.as_mut().unwrap().color = gold;
    let style = ButtonAppearanceConfiguration::new(
        AppearanceConfigurationId {
            owner: AppearanceConfigurationOwner::Product(ProductAppearanceOwner(42)),
            generation: 1,
        },
        contract,
        vec![ButtonAppearanceDefinition {
            selector: selector.clone(),
            values,
        }],
        vec![],
    )
    .unwrap()
    .prepare_style(selector, AppearanceFallbackPolicy::Reject)
    .unwrap();
    let clicks = Rc::clone(&f.clicks);
    let logo = || {
        let clicks = Rc::clone(&clicks);
        let mut button = Button::with_text("Home")
            .with_focus_cue_modality(FocusCueModality::KeyboardOnly)
            .on_click(move || clicks.set(clicks.get() + 1));
        button.set_appearance(style.clone(), None).unwrap();
        Box::new(button)
    };
    f.tree.replace_widget_only(f.nodes[0], logo());
    f.frame();
    let owner = f.owner(0);
    let rect = f.tree.get_absolute_paint_rect(f.nodes[0]).unwrap();
    let cue = |f: &Fixture, node| {
        f.retained
            .local_fragment(f.owner(node))
            .unwrap()
            .iter()
            .find_map(|node| match node {
                SceneNode::BorderRect(ring) if ring.z_index == LAYER_FOCUS + 1 => {
                    Some((ring.rect, ring.radius, ring.border_color))
                }
                _ => None,
            })
    };
    let gold_ring = Some((rect, 6.0, gold.into()));
    let key = |f: &mut Fixture, key_code, physical_key, state, shift| {
        f.raw(RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key,
            reservation: None,
            state,
            repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
        });
    };
    let tab = |f: &mut Fixture, shift| {
        for state in [KeyState::Down, KeyState::Up] {
            key(f, KeyCode::Tab, PhysicalKeyCode::Tab, state, shift);
        }
        f.frame();
    };
    let off = Point::new(480.0, 480.0);
    let leave = |f: &mut Fixture| {
        f.raw(RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            position: off,
            delta: (400.0, 400.0),
        });
    };

    tab(&mut f, false);
    assert!(f.facts(0).focused());
    assert_eq!(cue(&f, 0), gold_ring);
    f.click(0);
    leave(&mut f);
    f.frame();
    assert_eq!(clicks.get(), 1);
    assert!(f.facts(0).focused());
    assert!(!f.facts(0).pointer_captured());
    assert_eq!(cue(&f, 0), None);

    tab(&mut f, false);
    assert!(f.facts(1).focused());
    f.click(1);
    f.frame();
    assert!(
        cue(&f, 1).is_some(),
        "default Button cue still shows after a click"
    );
    tab(&mut f, true);
    assert!(f.facts(0).focused());
    assert_eq!(cue(&f, 0), gold_ring);

    f.button_event(0, RawButtonState::Pressed);
    leave(&mut f);
    f.raw(RawInputEvent::PointerButton {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        state: RawButtonState::Released,
        position: off,
    });
    f.frame();
    assert_eq!(clicks.get(), 1, "release outside cancels activation");
    assert!(f.facts(0).focused());
    assert!(!f.facts(0).pointer_captured());
    assert_eq!(cue(&f, 0), None);

    for (code, physical) in [
        (KeyCode::Enter, PhysicalKeyCode::Enter),
        (KeyCode::Space, PhysicalKeyCode::Space),
    ] {
        assert_eq!(cue(&f, 0), None);
        let before = clicks.get();
        key(&mut f, code, physical, KeyState::Down, false);
        f.frame();
        assert_eq!(clicks.get(), before);
        assert_eq!(f.owner(0), owner);
        assert_eq!(
            cue(&f, 0),
            gold_ring,
            "keyboard cue appears on the same focused owner"
        );
        key(&mut f, code, physical, KeyState::Up, false);
        f.frame();
        assert_eq!(clicks.get(), before + 1);
        assert_eq!(cue(&f, 0), gold_ring);
        if code == KeyCode::Enter {
            f.click(0);
            leave(&mut f);
            f.frame();
        }
    }

    // Replacing the concrete mount retains the logical owner and focus.
    f.tree.replace_widget_only(f.nodes[0], logo());
    f.frame();
    assert_eq!(f.owner(0), owner);
    assert!(f.facts(0).focused());
    assert_eq!(cue(&f, 0), None, "the new mount cannot inherit the old cue");
    tab(&mut f, false);
    tab(&mut f, true);
    assert_eq!(cue(&f, 0), gold_ring);
    for active in [false, true] {
        crate::core::app::handle_window_focus_changed(
            &mut f.domains,
            &mut f.router,
            &mut f.tree,
            WindowId::PRIMARY,
            active,
            &mut f.text,
        );
        f.frame();
        assert_eq!(cue(&f, 0), None);
    }
    f.router
        .request_focus_for_widget(&mut f.tree, owner, f.domains.primary_state_mut());
    f.frame();
    assert!(f.facts(0).focused());
    assert_eq!(
        cue(&f, 0),
        None,
        "programmatic focus has no keyboard provenance"
    );
}

#[test]
fn pointer_driven_button_pointer_lifecycle_and_keyboard_boundary() {
    let mut fixture = Fixture::new(UiInteractionPolicy::PointerDriven);
    let rect = fixture
        .tree
        .get_absolute_paint_rect(fixture.nodes[0])
        .unwrap();
    fixture.raw(RawInputEvent::PointerMoved {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        position: Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5),
        delta: (1.0, 1.0),
    });
    fixture.frame();
    assert_eq!(
        fixture
            .widget::<Button>(0)
            .resolved_appearance()
            .unwrap()
            .paint_case(),
        ButtonPaintCase::Hovered
    );
    fixture.button_event(0, RawButtonState::Pressed);
    fixture.frame();
    let appearance = fixture.widget::<Button>(0).resolved_appearance().unwrap();
    assert_eq!(appearance.paint_case(), ButtonPaintCase::Pressed);
    assert!(appearance.request().input.framework().focused());
    assert!(
        !appearance
            .request()
            .input
            .framework()
            .navigation_cue_eligible()
    );
    assert!(
        !appearance
            .composition()
            .iter()
            .any(|layer| layer.role == AppearanceRole::FocusCue)
    );
    fixture.button_event(0, RawButtonState::Released);
    fixture.frame();
    assert_eq!(fixture.clicks.get(), 1);
    assert!(
        !fixture
            .widget::<Button>(0)
            .resolved_appearance()
            .unwrap()
            .composition()
            .iter()
            .any(|layer| layer.role == AppearanceRole::FocusCue)
    );
    for key in [
        KeyCode::Tab,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Character('h'),
        KeyCode::Character('j'),
        KeyCode::Character('k'),
        KeyCode::Character('l'),
        KeyCode::Character('i'),
        KeyCode::Enter,
        KeyCode::Space,
    ] {
        let time = Instant::now();
        fixture.raw(RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time,
            key_code: key,
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            state: KeyState::Down,
            repeat: false,
            modifiers: Modifiers::default(),
        });
        fixture.raw(RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time,
            text: "i".into(),
            modifiers: Modifiers::default(),
        });
        fixture.raw(RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time,
            key_code: key,
            physical_key: PhysicalKeyCode::Unknown,
            reservation: None,
            state: KeyState::Up,
            repeat: false,
            modifiers: Modifiers::default(),
        });
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            Some(fixture.owner(0))
        );
        assert_eq!(
            fixture.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert_eq!(fixture.clicks.get(), 1);
        assert!(!fixture.widget::<Button>(0).keyboard_pulse_active.get());
    }
    let commands = Rc::new(Cell::new(0));
    let count = Rc::clone(&commands);
    fixture
        .app
        .bind_shortcut(Shortcut::key(KeyCode::Enter), move |_| {
            count.set(count.get() + 1);
            true
        });
    fixture.key(KeyCode::Enter);
    assert_eq!(commands.get(), 1);
    assert_eq!(fixture.clicks.get(), 1);
    let original = Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    fixture.raw(RawInputEvent::Key {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Character('h'),
        physical_key: PhysicalKeyCode::KeyH,
        reservation: None,
        state: KeyState::Down,
        repeat: true,
        modifiers: original,
    });
    assert!(
        matches!(fixture.screen_events.borrow().last(), Some(FusedInputEvent::KeyDown {
        key_code: KeyCode::Character('h'), physical_key: PhysicalKeyCode::KeyH, modifiers, repeat: true, ..
    }) if *modifiers == original)
    );
}

#[test]
fn pointer_driven_preserves_default_navigation_activation_and_cue() {
    let mut fixture = Fixture::new(UiInteractionPolicy::NavigationAndCapture);
    fixture.click(0);
    fixture.frame();
    assert!(
        fixture
            .widget::<Button>(0)
            .resolved_appearance()
            .unwrap()
            .composition()
            .iter()
            .any(|layer| layer.role == AppearanceRole::FocusCue)
    );
    fixture.key(KeyCode::Enter);
    assert!(fixture.widget::<Button>(0).keyboard_pulse_active.get());
    fixture.raw(RawInputEvent::Key {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Enter,
        physical_key: PhysicalKeyCode::Enter,
        reservation: None,
        state: KeyState::Up,
        repeat: false,
        modifiers: Modifiers::default(),
    });
    assert_eq!(fixture.clicks.get(), 2);
    fixture.key(KeyCode::Tab);
    assert_eq!(
        fixture.domains.primary_state().focus_manager.focused(),
        Some(fixture.owner(1))
    );
    fixture.click(2);
    assert_eq!(
        fixture.domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Capture
    );
    fixture.key(KeyCode::Tab);
    assert_eq!(
        fixture.domains.primary_state().focus_manager.focused(),
        Some(fixture.owner(3))
    );
    assert_eq!(
        fixture.domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Capture
    );
}

#[test]
fn pointer_driven_text_capture_edit_escape_and_unhandled_tab() {
    let mut fixture = Fixture::new(UiInteractionPolicy::PointerDriven);
    fixture.click(2);
    fixture.frame();
    assert!(fixture.facts(2).keyboard_captured());
    assert!(fixture.facts(2).focus_cue_visible());
    fixture.key(KeyCode::End);
    fixture.raw(RawInputEvent::TextInput {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        text: "漢x".into(),
        modifiers: Modifiers::default(),
    });
    assert_eq!(fixture.widget::<TextField>(2).text(), "kept漢x");
    fixture.screen_events.borrow_mut().clear();
    fixture.key(KeyCode::Escape);
    assert!(fixture.screen_events.borrow().is_empty());
    assert_eq!(fixture.widget::<TextField>(2).text(), "kept漢x");
    assert_eq!(
        fixture.domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Navigation
    );
    assert_eq!(
        fixture.domains.primary_state().focus_manager.focused(),
        Some(fixture.owner(2))
    );
    fixture.key(KeyCode::Character('i'));
    fixture.raw(RawInputEvent::TextInput {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        text: "leak".into(),
        modifiers: Modifiers::default(),
    });
    assert_eq!(fixture.widget::<TextField>(2).text(), "kept漢x");
    fixture.key(KeyCode::Escape);
    assert_eq!(
        fixture
            .screen_events
            .borrow()
            .iter()
            .filter(|event| matches!(
                event,
                FusedInputEvent::KeyDown {
                    key_code: KeyCode::Escape,
                    ..
                }
            ))
            .count(),
        1
    );
    fixture.click(2);
    fixture.key(KeyCode::Tab);
    assert_eq!(
        fixture.domains.primary_state().focus_manager.focused(),
        Some(fixture.owner(2))
    );
    assert_eq!(
        fixture.domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Navigation
    );
    assert_eq!(fixture.widget::<TextField>(2).text(), "kept漢x");
    assert_eq!(fixture.widget::<TextField>(3).text(), "neighbor");
    fixture.frame();
    assert!(fixture.facts(2).focused());
    assert!(!fixture.facts(2).focus_cue_visible());
}

#[test]
fn pointer_driven_physical_editor_records_once_and_preserves_reserved_cancel() {
    let mut fixture = Fixture::new(UiInteractionPolicy::PointerDriven);
    fixture.click(4);
    fixture.frame();
    assert!(fixture.facts(4).keyboard_captured());
    assert!(fixture.facts(4).focus_cue_visible());
    let modifiers = Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    fixture.raw(RawInputEvent::Key {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Character('ф'),
        physical_key: PhysicalKeyCode::KeyA,
        reservation: None,
        state: KeyState::Down,
        repeat: false,
        modifiers,
    });
    let recorded = fixture.widget::<KeybindEditor>(4).value().unwrap();
    assert_eq!(recorded.key, BindingKey::Physical(PhysicalKeyCode::KeyA));
    assert!(recorded.ctrl && recorded.shift && !recorded.alt && !recorded.meta);
    assert_eq!(fixture.recordings.borrow().as_slice(), &[Some(recorded)]);
    fixture.raw(RawInputEvent::Key {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Character('ф'),
        physical_key: PhysicalKeyCode::KeyA,
        reservation: None,
        state: KeyState::Down,
        repeat: true,
        modifiers,
    });
    assert_eq!(fixture.recordings.borrow().len(), 1);
    for key in [KeyCode::Tab, KeyCode::Escape] {
        fixture.click(4);
        fixture.key(key);
        assert_eq!(fixture.widget::<KeybindEditor>(4).value(), Some(recorded));
        assert_eq!(
            fixture.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            Some(fixture.owner(4))
        );
    }
    fixture.click(4);
    fixture.raw(RawInputEvent::Key {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::F11,
        physical_key: PhysicalKeyCode::F11,
        reservation: Some(KeyboardKeyReservation::Fullscreen),
        state: KeyState::Down,
        repeat: false,
        modifiers: Modifiers::default(),
    });
    assert_eq!(fixture.widget::<KeybindEditor>(4).value(), Some(recorded));
    assert_eq!(fixture.recordings.borrow().len(), 1);
    assert_eq!(
        fixture.domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Navigation
    );
}

#[test]
fn pointer_driven_public_configs_propagate_to_primary_and_registered_domains() {
    for policy in [
        UiInteractionPolicy::NavigationAndCapture,
        UiInteractionPolicy::PointerDriven,
    ] {
        let app = AppConfig::default().with_ui_interaction_policy(policy);
        let ui = UiThreadConfig::new(
            Theme::dark(),
            Box::new(FontManagerTextEngine::new()),
            Constraints::loose(Size::new(100.0, 100.0)),
        )
        .with_ui_interaction_policy(policy);
        for mut domains in [app.interaction_domains(), ui.interaction_domains()] {
            assert_eq!(domains.primary_state().ui_interaction_policy(), policy);
            let registered = domains.register_window(WindowId(42));
            assert_eq!(
                domains.state(registered).unwrap().ui_interaction_policy(),
                policy
            );
            assert_eq!(
                domains.state(registered).unwrap().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            assert!(!domains.should_route_event(&FusedInputEvent::KeyDown {
                window_id: WindowId(42),
                time: Instant::now(),
                key_code: KeyCode::Enter,
                physical_key: PhysicalKeyCode::Enter,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            }));
        }
    }
    assert_eq!(
        AppConfig::new("default").ui_interaction_policy,
        UiInteractionPolicy::NavigationAndCapture
    );
    assert_eq!(
        UiThreadConfig::new(
            Theme::dark(),
            Box::new(FontManagerTextEngine::new()),
            Constraints::loose(Size::new(100.0, 100.0))
        )
        .ui_interaction_policy,
        UiInteractionPolicy::NavigationAndCapture
    );
}
