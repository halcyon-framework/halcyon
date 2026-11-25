use super::*;
use crate::core::app::{
    FrameReason, FrameTime, RetainedSceneTestHarness, next_qualified_widget_input,
};
use crate::core::event::PointerDevice;
use crate::core::event::{
    InteractionDomains, KeyState, RawButtonState, RawInputEvent, UiInteractionPolicy, WindowId,
};
use crate::core::layout::{Constraints, EdgeInsets};
use crate::core::math::Size;
use crate::core::text_engine::TextEngine;
use crate::core::widget_availability::{apply_pending_widget_availability, effective};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::{actions::ActionId, binding_registry::BindingRegistry};
use crate::widgets::{
    button::Button, flex::Flex, label::Label, text_area::TextArea, text_edit::TextEdit,
    text_field::TextField,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

struct Fixture {
    tree: WidgetTree,
    root: NodeId,
    controls: [NodeId; 4],
    peer: NodeId,
    clicks: Rc<Cell<usize>>,
    router: EventRouter,
    fusion: crate::core::input_fusion::InputFusion,
    domains: InteractionDomains,
    bindings: BindingRegistry,
    text: FontManagerTextEngine,
    retained: RetainedSceneTestHarness,
    deferred: Vec<crate::core::event::SessionQualifiedInput>,
    input_time: Instant,
}

impl Fixture {
    fn new(policy: UiInteractionPolicy, read_only: bool) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(12.0))
                    .with_spacing(8.0),
            ),
            None,
            0,
        );
        let clicks = Rc::new(Cell::new(0));
        let sink = clicks.clone();
        let button = tree.add_child(
            root,
            Box::new(Button::with_text("Apply").on_click(move || sink.set(sink.get() + 1))),
            None,
            0,
        );
        let field = tree.add_child(
            root,
            Box::new(
                TextField::new()
                    .with_text("field")
                    .with_read_only(read_only),
            ),
            None,
            0,
        );
        let area = tree.add_child(
            root,
            Box::new(
                TextArea::new()
                    .with_text("one\ntwo\nthree\nfour\nfive\nsix")
                    .with_fixed_height(90.0)
                    .with_read_only(read_only),
            ),
            None,
            0,
        );
        let edit = tree.add_child(
            root,
            Box::new(
                TextEdit::with_text("wrapper")
                    .with_fixed_height(60.0)
                    .with_read_only(read_only),
            ),
            None,
            0,
        );
        let peer = tree.add_child(root, Box::new(Label::new("retained peer")), None, 0);
        let mut router = EventRouter::new();
        router
            .clipboard
            .as_mut()
            .expect("clipboard facade")
            .use_memory_transport_for_test("untouched");
        let mut f = Self {
            tree,
            root,
            controls: [button, field, area, edit],
            peer,
            clicks,
            router,
            fusion: crate::core::input_fusion::InputFusion::new(),
            domains: InteractionDomains::with_ui_interaction_policy(policy),
            bindings: BindingRegistry::new(),
            text: FontManagerTextEngine::new(),
            retained: RetainedSceneTestHarness::new(),
            deferred: Vec::new(),
            input_time: Instant::now(),
        };
        f.frame();
        f.frame();
        f
    }

    fn owner(&self, node: NodeId) -> TreeWidgetId {
        self.tree.node(node).unwrap().id
    }
    fn frame(&mut self) -> crate::core::app::RetainedSceneTestFrame {
        crate::core::app::test_begin_frame_layout_tracking();
        let time = FrameTime {
            now: Instant::now(),
            dt: Duration::from_millis(16),
            frame_index: 1,
            reason: FrameReason::Input,
        };
        self.retained.frame_with_gesture_sessions(
            &mut self.tree,
            self.root,
            Constraints::tight(Size::new(500.0, 450.0)),
            &crate::core::theme::Theme::light(),
            &mut self.text,
            &crate::core::icons::builtin_registry(),
            &time,
            None,
            &mut self.domains,
            WindowId::PRIMARY,
            &mut self.router,
            &mut self.fusion,
        )
    }
    fn settle(&mut self) -> Vec<TreeWidgetId> {
        let result = apply_pending_widget_availability(
            &mut self.tree,
            &mut self.router,
            &mut self.fusion,
            self.domains.primary_state_mut(),
            &mut self.bindings,
            &mut self.text,
        );
        assert!(result.rejected.is_empty());
        result.changed
    }
    fn route(&mut self, event: FusedInputEvent) {
        assert!(!self.tree.has_pending_widget_availability());
        self.router.route_input_event(
            &mut self.tree,
            event,
            Some(self.root),
            &mut self.text,
            None,
            self.domains.primary_state_mut(),
            &mut self.bindings,
        );
    }
    fn raw(&mut self, event: RawInputEvent) {
        let state = self.domains.primary_state_mut();
        let gate = EventRouter::relative_ingress_gate(state);
        self.fusion.process_raw_with_gesture_sessions(
            event,
            state.focus_manager.interaction_domain_id(),
            &mut state.gesture_registry,
            &self.tree,
            gate.as_ref(),
            &mut self.deferred,
        );
    }
    fn drain(&mut self) {
        let mut intake = std::mem::take(&mut self.deferred).into_iter();
        while let Some(qualified) =
            next_qualified_widget_input(&self.tree, &mut intake, &mut self.deferred)
        {
            self.router.route_session_input_event(
                &mut self.tree,
                qualified,
                Some(self.root),
                &mut self.text,
                None,
                self.domains.primary_state_mut(),
                &mut self.bindings,
            );
        }
    }
    fn pointer(&mut self, node: NodeId, state: RawButtonState) {
        let r = self.tree.get_absolute_paint_rect(node).unwrap();
        self.input_time += Duration::from_secs(1);
        self.raw(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: self.input_time,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state,
            position: crate::core::math::Point::new(r.x + r.width * 0.5, r.y + r.height * 0.5),
        });
        self.drain();
    }
    fn click(&mut self, node: NodeId) {
        self.pointer(node, RawButtonState::Pressed);
        self.pointer(node, RawButtonState::Released);
    }
    fn command(&mut self, text: &str) {
        self.command_with_shift(text, false);
    }
    fn redo(&mut self) {
        if cfg!(target_os = "macos") {
            self.command_with_shift("z", true);
        } else {
            self.command("y");
        }
    }
    fn command_with_shift(&mut self, text: &str, shift: bool) {
        let modifiers = if cfg!(target_os = "macos") {
            Modifiers {
                meta: true,
                shift,
                ..Default::default()
            }
        } else {
            Modifiers {
                ctrl: true,
                shift,
                ..Default::default()
            }
        };
        self.route(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: text.to_owned(),
            modifiers,
        });
    }
    fn text(&mut self, text: &str) {
        self.route(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: text.to_owned(),
            modifiers: Modifiers::default(),
        });
    }
    fn key(&mut self, key_code: KeyCode, state: KeyState, repeat: bool) {
        self.key_with_modifiers(key_code, state, repeat, Modifiers::default());
    }
    fn key_with_modifiers(
        &mut self,
        key_code: KeyCode,
        state: KeyState,
        repeat: bool,
        modifiers: Modifiers,
    ) {
        self.input_time += Duration::from_millis(16);
        self.raw(RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time: self.input_time,
            key_code,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            state,
            repeat,
            modifiers,
        });
        self.drain();
    }
    fn wheel(&mut self, node: NodeId) {
        let r = self.tree.get_absolute_paint_rect(node).unwrap();
        self.input_time += Duration::from_millis(16);
        self.raw(RawInputEvent::PointerWheel {
            window_id: WindowId::PRIMARY,
            time: self.input_time,
            device: PointerDevice::Mouse,
            delta: crate::core::event::WheelDelta::Pixels(0.0, -24.51),
            position: Point::new(r.x + 10.0, r.y + 10.0),
        });
        self.fusion.flush_with_gesture_sessions(&mut self.deferred);
        self.drain();
    }
    fn content(&self, node: NodeId) -> String {
        let widget = &self.tree.node(node).unwrap().widget;
        if let Some(w) = widget.as_any().downcast_ref::<TextField>() {
            w.text().to_owned()
        } else if let Some(w) = widget.as_any().downcast_ref::<TextArea>() {
            w.text().to_owned()
        } else {
            widget
                .as_any()
                .downcast_ref::<TextEdit>()
                .unwrap()
                .text()
                .to_owned()
        }
    }
    fn ime(&mut self, node: NodeId, event: crate::core::event::ImeEvent) {
        let owner = self.owner(node);
        self.router
            .enter_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "ime_policy");
        let text_ptr = &mut self.text as &mut dyn TextEngine as *mut dyn TextEngine;
        self.router.route_to_widget(
            &mut self.tree,
            owner,
            &Event::Ime(event),
            Some(text_ptr),
            self.domains.primary_state_mut(),
            None,
        );
        self.router
            .exit_phase(InputPipelinePhase::Routing, PointerId::MOUSE, "ime_policy");
    }
}

#[test]
fn atomic_availability_readonly_keyboard_requires_capture() {
    for policy in [
        UiInteractionPolicy::NavigationAndCapture,
        UiInteractionPolicy::PointerDriven,
    ] {
        let mut f = Fixture::new(policy, true);
        for node in [f.controls[1], f.controls[2], f.controls[3]] {
            let owner = f.owner(node);
            let content = f.content(node);
            f.router
                .request_focus_for_widget(&mut f.tree, owner, f.domains.primary_state_mut());
            for released in [false, true] {
                if released {
                    f.command("a");
                    f.command("c");
                    assert_eq!(
                        f.router.clipboard.as_mut().unwrap().get_text().unwrap(),
                        content
                    );
                    assert!(f.router.handle_engine_action(
                        &mut f.tree,
                        ActionId::RELEASE_CAPTURE,
                        Some(f.root),
                        f.domains.primary_state_mut(),
                        &mut f.text,
                        None,
                    ));
                }
                assert_eq!(
                    f.domains.primary_state().focus_manager.focused(),
                    Some(owner)
                );
                assert_eq!(
                    f.domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Navigation
                );
                f.frame();
                f.router.reset_continuous_frames_request();
                assert!(!f.frame().scene_built);
                assert!(!f.tree.node(node).unwrap().widget.needs_animation());
                assert!(!f.tree.any_widget_needs_animation());
                assert_eq!(f.domains.primary_state().swallow_text_at, None);

                let local = f.retained.local_fragment(owner).unwrap();
                let peer = f.retained.local_fragment(f.owner(f.peer)).unwrap();
                let root = f.retained.local_fragment(f.owner(f.root)).unwrap();
                let geometry = f.tree.get_absolute_paint_rect(node).unwrap();
                let layout_epoch = f.tree.node(node).unwrap().layout_epoch;
                let clipboard = f.router.clipboard.as_mut().unwrap().get_text().unwrap();
                for key in [KeyCode::Backspace, KeyCode::Delete] {
                    f.key(key, KeyState::Down, false);
                    let swallowed = f.domains.primary_state().swallow_text_at;
                    if policy == UiInteractionPolicy::NavigationAndCapture {
                        assert_eq!(swallowed, None, "Navigation editor did not consume {key:?}");
                    } else {
                        assert_eq!(
                            swallowed,
                            Some(f.input_time),
                            "PointerDriven's own suppression remains"
                        );
                    }
                    f.key(key, KeyState::Up, false);
                    assert!(!f.tree.node(node).unwrap().dirty_paint);
                    assert!(!f.router.continuous_frames_requested());
                }
                f.text("outside capture");
                f.command("a");
                f.command("c");
                f.ime(
                    node,
                    crate::core::event::ImeEvent::PreEdit {
                        text: "outside capture".into(),
                        cursor: None,
                    },
                );
                f.ime(
                    node,
                    crate::core::event::ImeEvent::Commit {
                        text: "outside capture".into(),
                    },
                );
                assert_eq!(f.content(node), content);
                assert_eq!(
                    f.router.clipboard.as_mut().unwrap().get_text().unwrap(),
                    clipboard
                );
                assert!(!f.tree.node(node).unwrap().dirty_paint);
                assert!(!f.router.continuous_frames_requested());
                assert!(!f.frame().scene_built);
                assert!(Arc::ptr_eq(
                    &local,
                    &f.retained.local_fragment(owner).unwrap()
                ));
                assert!(Arc::ptr_eq(
                    &peer,
                    &f.retained.local_fragment(f.owner(f.peer)).unwrap()
                ));
                assert!(Arc::ptr_eq(
                    &root,
                    &f.retained.local_fragment(f.owner(f.root)).unwrap()
                ));
                assert_eq!(f.tree.get_absolute_paint_rect(node).unwrap(), geometry);
                assert_eq!(f.tree.node(node).unwrap().layout_epoch, layout_epoch);
                assert_eq!(
                    f.domains.primary_state().focus_manager.focused(),
                    Some(owner)
                );
                assert_eq!(
                    f.domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Navigation
                );

                assert!(f.router.handle_engine_action(
                    &mut f.tree,
                    ActionId::ENTER_WIDGET,
                    Some(f.root),
                    f.domains.primary_state_mut(),
                    &mut f.text,
                    None,
                ));
                assert_eq!(
                    f.domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
                // Observe preserved cursor/selection through actual editing behavior,
                // including TextEdit's private inner editor, without a harness setter.
                let (key, selected) = if released {
                    (KeyCode::Left, &content[..content.len() - 1])
                } else if node == f.controls[1] {
                    // TextField::set_text starts at the end; multiline builders start at zero.
                    (KeyCode::Left, &content[content.len() - 1..])
                } else {
                    (KeyCode::Right, &content[..1])
                };
                f.key_with_modifiers(
                    key,
                    KeyState::Down,
                    false,
                    Modifiers {
                        shift: true,
                        ..Default::default()
                    },
                );
                f.key_with_modifiers(
                    key,
                    KeyState::Up,
                    false,
                    Modifiers {
                        shift: true,
                        ..Default::default()
                    },
                );
                f.command("c");
                assert_eq!(
                    f.router.clipboard.as_mut().unwrap().get_text().unwrap(),
                    selected,
                    "Navigation preserved cursor/selection before genuine Capture re-entry"
                );
                for key in [
                    KeyCode::Backspace,
                    KeyCode::Delete,
                    KeyCode::Space,
                    KeyCode::Enter,
                ] {
                    f.key(key, KeyState::Down, false);
                    f.key(key, KeyState::Up, false);
                }
                for shortcut in ["x", "v", "z", "y"] {
                    f.command(shortcut);
                }
                f.redo();
                f.text("readonly capture");
                f.ime(
                    node,
                    crate::core::event::ImeEvent::Commit {
                        text: "readonly capture".into(),
                    },
                );
                assert_eq!(f.content(node), content);
                assert_eq!(
                    f.router.clipboard.as_mut().unwrap().get_text().unwrap(),
                    selected
                );
                f.command("c");
                assert_eq!(
                    f.router.clipboard.as_mut().unwrap().get_text().unwrap(),
                    selected
                );
                assert_eq!(
                    f.domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
                println!(
                    "ATOMIC CAPTURE GATE {policy:?} {} released={released}: Navigation unconsumed/clean; Capture selection/copy and readonly mutation gate PASS",
                    f.tree.node(node).unwrap().widget.debug_type_name()
                );
            }
            f.router.handle_engine_action(
                &mut f.tree,
                ActionId::RELEASE_CAPTURE,
                Some(f.root),
                f.domains.primary_state_mut(),
                &mut f.text,
                None,
            );
            f.frame();
        }
    }
}

#[test]
fn atomic_availability_mounted_editors_and_retained_policy() {
    for policy in [
        UiInteractionPolicy::NavigationAndCapture,
        UiInteractionPolicy::PointerDriven,
    ] {
        let mut f = Fixture::new(policy, true);
        let [button, field, area, edit] = f.controls;
        let root_local = f.retained.local_fragment(f.owner(f.root)).unwrap();
        let peer_local = f.retained.local_fragment(f.owner(f.peer)).unwrap();
        f.tree.request_widget_enabled(button, true).unwrap();
        assert!(f.settle().is_empty());
        assert!(!f.frame().scene_built);
        assert!(Arc::ptr_eq(
            &root_local,
            &f.retained.local_fragment(f.owner(f.root)).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_local,
            &f.retained.local_fragment(f.owner(f.peer)).unwrap()
        ));
        assert_eq!(
            f.tree.request_widget_read_only(button, true),
            Err(crate::core::widget_tree::WidgetAvailabilityError::ReadOnlyUnsupported)
        );
        assert!(!f.tree.has_pending_widget_availability());

        for node in [field, area, edit] {
            let original = f.content(node);
            f.click(node);
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            assert_eq!(
                f.domains.primary_state().focus_manager.focused(),
                Some(f.owner(node))
            );
            f.command("a");
            f.command("c");
            assert_eq!(
                f.router.clipboard.as_mut().unwrap().get_text().unwrap(),
                original
            );
            let frame = f.frame();
            assert!(frame.scene_built);
            assert!(f.tree.node(node).unwrap().widget.needs_animation());
            for shortcut in ["x", "v", "z", "y", "Z"] {
                f.command(shortcut);
            }
            f.redo();
            f.text("denied");
            f.ime(
                node,
                crate::core::event::ImeEvent::PreEdit {
                    text: "draft".into(),
                    cursor: None,
                },
            );
            f.ime(
                node,
                crate::core::event::ImeEvent::Commit {
                    text: "denied ime".into(),
                },
            );
            assert_eq!(f.content(node), original);
            assert_eq!(
                f.router.clipboard.as_mut().unwrap().get_text().unwrap(),
                original
            );
            f.tree.request_widget_read_only(node, false).unwrap();
            assert!(
                effective(f.tree.node(node).unwrap().widget.as_ref())
                    .unwrap()
                    .read_only
                    .unwrap()
            );
            f.settle();
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            f.route(FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::End,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            });
            f.text("!");
            let edited = f.content(node);
            assert_ne!(edited, original);
            f.ime(
                node,
                crate::core::event::ImeEvent::PreEdit {
                    text: "uncommitted".into(),
                    cursor: None,
                },
            );
            f.tree.request_widget_read_only(node, true).unwrap();
            f.settle();
            assert_eq!(
                f.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            for shortcut in ["x", "v", "z", "y"] {
                f.command(shortcut);
            }
            f.redo();
            f.ime(
                node,
                crate::core::event::ImeEvent::Commit {
                    text: "late".into(),
                },
            );
            assert_eq!(f.content(node), edited);
            f.tree.request_widget_read_only(node, false).unwrap();
            f.settle();
            f.command("z");
            assert_eq!(
                f.content(node),
                original,
                "read-only operations did not create history"
            );
            f.redo();
            assert_eq!(f.content(node), edited);
            f.command("a");
            f.command("x");
            assert_eq!(f.content(node), "");
            f.command("v");
            assert_eq!(
                f.content(node),
                edited,
                "enabled clipboard mutation remains admitted"
            );
            f.ime(
                node,
                crate::core::event::ImeEvent::PreEdit {
                    text: "Ω".into(),
                    cursor: None,
                },
            );
            f.ime(
                node,
                crate::core::event::ImeEvent::Commit { text: "Ω".into() },
            );
            assert_ne!(
                f.content(node),
                edited,
                "enabled IME commit remains admitted"
            );
            f.command("z");
            assert_eq!(f.content(node), edited);
            f.router.handle_engine_action(
                &mut f.tree,
                ActionId::RELEASE_CAPTURE,
                Some(f.root),
                f.domains.primary_state_mut(),
                &mut f.text,
                None,
            );
            f.frame();
        }

        f.click(field);
        f.frame();
        let rects = f
            .controls
            .map(|node| f.tree.get_absolute_paint_rect(node).unwrap());
        let epochs = f
            .controls
            .map(|node| f.tree.node(node).unwrap().layout_epoch);
        let peer = f.retained.local_fragment(f.owner(f.peer)).unwrap();
        let root = f.retained.local_fragment(f.owner(f.root)).unwrap();
        f.tree.request_widget_enabled(field, false).unwrap();
        assert!(f.tree.node(field).unwrap().widget.focusable());
        f.settle();
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(f.owner(area))
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert!(!f.tree.node(field).unwrap().widget.focusable());
        assert!(!f.tree.node(field).unwrap().widget.needs_animation());
        f.frame();
        assert_eq!(
            f.controls
                .map(|node| f.tree.get_absolute_paint_rect(node).unwrap()),
            rects
        );
        assert_eq!(
            f.controls
                .map(|node| f.tree.node(node).unwrap().layout_epoch),
            epochs
        );
        assert!(Arc::ptr_eq(
            &peer,
            &f.retained.local_fragment(f.owner(f.peer)).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root,
            &f.retained.local_fragment(f.owner(f.root)).unwrap()
        ));
        f.click(field);
        assert_ne!(
            f.domains.primary_state().focus_manager.focused(),
            Some(f.owner(field))
        );
        f.tree.request_widget_enabled(field, true).unwrap();
        f.settle();
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        f.click(field);
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );

        f.tree.request_widget_enabled(area, false).unwrap();
        f.settle();
        f.tree.request_widget_enabled(field, false).unwrap();
        f.tree.request_widget_enabled(area, true).unwrap();
        f.settle();
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(f.owner(area)),
            "next eligibility uses the final atomic batch"
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );

        f.click(edit);
        f.tree.request_widget_enabled(edit, false).unwrap();
        f.settle();
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            None,
            "no implicit wrap"
        );
        f.tree.request_widget_enabled(edit, true).unwrap();
        f.settle();
        assert_eq!(f.domains.primary_state().focus_manager.focused(), None);
        f.domains
            .primary_state_mut()
            .focus_manager
            .push_scope(f.root);
        f.click(edit);
        f.tree.request_widget_enabled(edit, false).unwrap();
        f.settle();
        f.tree.request_widget_enabled(edit, true).unwrap();
        f.settle();
        f.router
            .ensure_focus_within_active_scope(&mut f.tree, f.domains.primary_state_mut());
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            None,
            "re-enable does not resurrect focus in the existing scope"
        );
        println!(
            "ATOMIC AVAILABILITY {policy:?}: mounted readonly/copy/edits/IME/history, deferred focus, paint-only peer reuse PASS"
        );
    }
}

fn visit(
    nodes: &[crate::core::scene::SceneNode],
    f: &mut impl FnMut(&crate::core::scene::SceneNode),
) {
    use crate::core::scene::SceneNode;
    for node in nodes {
        f(node);
        match node {
            SceneNode::Clip(n) => visit(&n.children, f),
            SceneNode::Scroll(n) => visit(&n.children, f),
            SceneNode::RetainedFragment(n) => visit(&n.children, f),
            _ => {}
        }
    }
}

#[test]
fn atomic_availability_combo_box_retires_popup_and_preserves_surface() {
    use crate::core::appearance::InputSurfaceAppearance;
    use crate::core::icon::IconId;
    use crate::core::paint::{Color, ColorStop, GradientDef, GradientKind, SurfaceGradient};
    use crate::core::scene::SceneNode;
    use crate::core::widget::Widget;
    use crate::widgets::combo_box::ComboBox;

    let surface = SurfaceGradient::new(GradientDef {
        kind: GradientKind::Linear,
        p0: (0.0, 0.0),
        p1: (0.0, 1.0),
        stops: vec![
            ColorStop {
                offset: 0.0,
                color: Color::from_hex(0x41484F),
            },
            ColorStop {
                offset: 1.0,
                color: Color::from_hex(0x20262C),
            },
        ],
    })
    .unwrap();
    let appearance = InputSurfaceAppearance::try_new(
        surface.clone(),
        Color::from_hex(0x515961),
        Color::from_hex(0x32393F),
        Color::from_hex(0x3A4249),
        Color::from_hex(0x252D34),
        Color::from_hex(0x3D4A55),
        11.0,
    )
    .unwrap();
    let icons = crate::core::icons::builtin_registry();
    let icon_layers = icons.get(IconId::GLOBE).unwrap().layers.len();
    for policy in [
        UiInteractionPolicy::NavigationAndCapture,
        UiInteractionPolicy::PointerDriven,
    ] {
        let mut f = Fixture::new(policy, false);
        let commits = Rc::new(Cell::new(0));
        let sink = commits.clone();
        let mut combo = ComboBox::simple_with_values(vec![
            ("Alpha".into(), "alpha".into()),
            ("Beta".into(), "beta".into()),
        ])
        .with_input_surface_appearance(appearance.clone())
        .with_leading_icon(IconId::GLOBE);
        combo.set_input_text("Alpha".into()).unwrap();
        combo.set_on_committed(move |_| sink.set(sink.get() + 1));
        let node = f.tree.add_child(f.root, Box::new(combo), None, 0);
        let owner = f.owner(node);
        macro_rules! combo {
            () => {
                f.tree
                    .node(node)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<ComboBox>()
                    .unwrap()
            };
        }
        let tap_key = |f: &mut Fixture, key| {
            f.key(key, KeyState::Down, false);
            f.key(key, KeyState::Up, false);
        };
        f.frame();
        let rect = f.tree.get_absolute_paint_rect(node).unwrap();
        let assert_surface = |f: &Fixture| {
            assert_eq!(f.tree.get_absolute_paint_rect(node), Some(rect));
            let local = f.retained.local_fragment(owner).unwrap();
            let mut has_surface = false;
            let mut filled_paths = 0;
            visit(&local, &mut |node| match node {
                SceneNode::GradientRect(gradient) => {
                    has_surface |= &gradient.gradient == surface.definition();
                }
                SceneNode::Path(path) if !path.is_stroke => filled_paths += 1,
                _ => {}
            });
            assert!(has_surface);
            assert_eq!(filled_paths, icon_layers);
        };
        assert_surface(&f);

        f.click(node);
        assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        tap_key(&mut f, KeyCode::Home);
        tap_key(&mut f, KeyCode::Enter);
        assert_eq!(combo!().committed_value_string().as_deref(), Some("alpha"));
        assert_eq!(commits.get(), 1);

        f.click(node);
        f.frame();
        f.frame();
        assert!(combo!().wants_scroll());
        assert!(!f.retained.overlay_fragment(owner).unwrap().is_empty());
        f.tree.request_widget_enabled(node, false).unwrap();
        assert_eq!(f.settle(), vec![owner]);
        f.frame();
        assert!(!combo!().wants_scroll());
        assert!(f.retained.overlay_fragment(owner).unwrap().is_empty());
        assert_surface(&f);

        f.click(node);
        f.router
            .request_focus_for_widget(&mut f.tree, owner, f.domains.primary_state_mut());
        for key in [KeyCode::Down, KeyCode::Enter, KeyCode::Backspace] {
            tap_key(&mut f, key);
        }
        f.text("denied");
        assert_ne!(f.domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert!(!combo!().capture_eligible());
        assert!(!combo!().wants_scroll());
        assert_eq!(combo!().input_text(), "Alpha");
        assert_eq!(combo!().committed_value_string().as_deref(), Some("alpha"));
        assert_eq!(commits.get(), 1);

        f.tree.request_widget_enabled(node, true).unwrap();
        assert_eq!(f.settle(), vec![owner]);
        f.frame();
        assert_ne!(f.domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert!(f.retained.overlay_fragment(owner).unwrap().is_empty());
        assert_surface(&f);
        f.click(node);
        tap_key(&mut f, KeyCode::End);
        tap_key(&mut f, KeyCode::Enter);
        assert_eq!(combo!().input_text(), "Beta");
        assert_eq!(combo!().committed_value_string().as_deref(), Some("beta"));
        assert_eq!(commits.get(), 2);
    }
}

#[test]
fn atomic_availability_disabled_output_scroll_and_keyboard_custody() {
    use crate::core::appearance::{ButtonAppearanceContract, ButtonPaintCase};
    use crate::core::scene::SceneNode;
    use crate::core::theme::{ControlChromeState, Theme};
    for policy in [
        UiInteractionPolicy::NavigationAndCapture,
        UiInteractionPolicy::PointerDriven,
    ] {
        let mut f = Fixture::new(policy, true);
        let [button, field, area, edit] = f.controls;
        f.click(area);
        f.frame();
        let before = f.retained.local_fragment(f.owner(area)).unwrap();
        f.wheel(area);
        for _ in 0..8 {
            f.frame();
        }
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        let scrolled = f.retained.local_fragment(f.owner(area)).unwrap();
        assert!(
            !Arc::ptr_eq(&before, &scrolled),
            "readonly scrolling updates owner paint"
        );
        assert_eq!(
            f.domains.primary_state().scroll_capture_owner,
            Some(f.owner(area))
        );
        let peer = f.retained.local_fragment(f.owner(f.peer)).unwrap();
        let root = f.retained.local_fragment(f.owner(f.root)).unwrap();
        let geometry = f.controls.map(|node| {
            (
                f.tree.get_absolute_paint_rect(node).unwrap(),
                f.tree.node(node).unwrap().layout_epoch,
            )
        });
        for node in [button, field, area, edit] {
            f.tree.request_widget_enabled(node, false).unwrap();
        }
        assert_eq!(f.settle().len(), 4);
        assert_eq!(f.domains.primary_state().focus_manager.focused(), None);
        assert_eq!(f.domains.primary_state().scroll_capture_owner, None);
        f.frame();
        assert_eq!(
            geometry,
            f.controls.map(|node| (
                f.tree.get_absolute_paint_rect(node).unwrap(),
                f.tree.node(node).unwrap().layout_epoch
            ))
        );
        let theme = Theme::light();
        for node in [field, area, edit] {
            let chrome = if node == field {
                theme.input_chrome(ControlChromeState::Disabled).chrome
            } else {
                theme.text_area_chrome(ControlChromeState::Disabled).chrome
            };
            let fragment = f.retained.local_fragment(f.owner(node)).unwrap();
            let mut shell = false;
            let mut text = false;
            visit(&fragment, &mut |n| match n {
                SceneNode::RoundedRect(r) if r.z_index == 2 => {
                    assert_eq!(r.color, chrome.border_color.into());
                    shell = true;
                }
                SceneNode::Text(t) => {
                    text = true;
                    assert!(t.backgrounds.is_empty(), "disabled selection/caret absent");
                }
                _ => {}
            });
            assert!(shell && text);
            assert!(!f.tree.node(node).unwrap().widget.needs_animation());
            assert!(!f.tree.node(node).unwrap().widget.capture_eligible());
        }
        let expected_content_color = theme
            .button_appearance_values(&ButtonAppearanceContract::standard())
            .disabled
            .as_ref()
            .expect("standard Button disabled appearance")
            .content_color;
        let appearance = f
            .tree
            .node(button)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Button>()
            .expect("fixture Button")
            .resolved_appearance()
            .expect("published Button appearance");
        assert_eq!(appearance.paint_case(), ButtonPaintCase::Disabled);
        assert_eq!(appearance.content_color(), expected_content_color);
        let mut button_text = false;
        visit(
            &f.retained.local_fragment(f.owner(button)).unwrap(),
            &mut |n| {
                if let SceneNode::Text(t) = n {
                    assert_eq!(t.color, expected_content_color.into());
                    button_text = true;
                }
            },
        );
        assert!(button_text);
        let content = [field, area, edit].map(|n| f.content(n));
        f.click(button);
        f.click(area);
        let r = f.tree.get_absolute_paint_rect(area).unwrap();
        f.raw(RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: f.input_time + Duration::from_millis(1),
            device: PointerDevice::Mouse,
            position: Point::new(r.x + 8.0, r.y + 8.0),
            delta: (1.0, 1.0),
        });
        f.fusion.flush_with_gesture_sessions(&mut f.deferred);
        f.drain();
        assert_eq!(f.domains.primary_state().hovered_widget, None);
        f.wheel(area);
        f.key(KeyCode::Space, KeyState::Down, false);
        f.text("disabled");
        f.ime(
            area,
            crate::core::event::ImeEvent::Commit {
                text: "disabled ime".into(),
            },
        );
        f.frame();
        assert_eq!([field, area, edit].map(|n| f.content(n)), content);
        assert_eq!(f.clicks.get(), 0);
        assert!(Arc::ptr_eq(
            &peer,
            &f.retained.local_fragment(f.owner(f.peer)).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root,
            &f.retained.local_fragment(f.owner(f.root)).unwrap()
        ));
        assert_eq!(f.domains.primary_state().scroll_capture_owner, None);
        f.tree.request_widget_enabled(button, true).unwrap();
        f.settle();
        let owner = f.owner(button);
        f.router
            .request_focus_for_widget(&mut f.tree, owner, f.domains.primary_state_mut());
        if policy == UiInteractionPolicy::NavigationAndCapture {
            f.key(KeyCode::Space, KeyState::Down, false);
            f.tree.request_widget_enabled(button, false).unwrap();
            f.settle();
            f.tree.request_widget_enabled(button, true).unwrap();
            f.settle();
            f.router
                .request_focus_for_widget(&mut f.tree, owner, f.domains.primary_state_mut());
            f.key(KeyCode::Space, KeyState::Down, true);
            f.key(KeyCode::Space, KeyState::Up, false);
            assert_eq!(
                f.clicks.get(),
                0,
                "retired held key cannot activate after re-enable"
            );
            f.key(KeyCode::Space, KeyState::Down, false);
            f.key(KeyCode::Space, KeyState::Up, false);
            assert_eq!(f.clicks.get(), 1, "fresh keyboard activation is admitted");
        } else {
            f.key(KeyCode::Space, KeyState::Down, false);
            f.key(KeyCode::Space, KeyState::Up, false);
            assert_eq!(f.clicks.get(), 0, "PointerDriven remains authoritative");
        }
        println!(
            "ATOMIC AVAILABILITY {policy:?}: disabled scene facts/caret/scroll, paint-only geometry and keyboard custody PASS"
        );
    }
}

#[test]
fn atomic_availability_intake_mount_and_capture_cancellation() {
    for policy in [
        UiInteractionPolicy::NavigationAndCapture,
        UiInteractionPolicy::PointerDriven,
    ] {
        let mut f = Fixture::new(policy, false);
        let button = f.controls[0];
        f.pointer(button, RawButtonState::Pressed);
        assert_eq!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(f.owner(button))
        );
        let r = f.tree.get_absolute_paint_rect(button).unwrap();
        f.raw(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state: RawButtonState::Released,
            position: Point::new(r.x + 1.0, r.y + 1.0),
        });
        let queued = f.deferred.len();
        assert!(queued > 0);
        f.tree.request_widget_enabled(button, false).unwrap();
        f.drain();
        assert_eq!(
            f.deferred.len(),
            queued,
            "original qualified release retained at cutoff"
        );
        f.settle();
        assert!(
            !f.domains
                .primary_state()
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE)
        );
        f.drain();
        assert_eq!(f.clicks.get(), 0);
        f.tree.request_widget_enabled(button, true).unwrap();
        f.settle();
        assert_eq!(f.clicks.get(), 0);
        f.click(button);
        assert_eq!(f.clicks.get(), 1, "fresh activation after re-enable");

        {
            let owner = f.owner(button);
            let foreign_owner = f.owner(f.controls[2]);
            let tree_ptr = &mut f.tree as *mut WidgetTree;
            let mut cx = crate::core::widget::EventContext::new(
                owner,
                tree_ptr,
                0,
                std::ptr::null_mut(),
                None,
                None,
                false,
                false,
                KeyboardAuthorityMode::Navigation,
            );
            cx.widget_id = foreign_owner;
            assert_eq!(
                cx.request_read_only(true),
                Err(crate::core::widget_tree::WidgetAvailabilityError::ReadOnlyUnsupported)
            );
            cx.request_enabled(false).unwrap();
            cx.request_enabled(true).unwrap();
        }
        assert!(
            f.settle().is_empty(),
            "self-only requests coalesce to equal effective policy"
        );

        let area = f.controls[2];
        let area_owner = f.owner(area);
        let button_owner = f.owner(button);
        f.pointer(area, RawButtonState::Pressed);
        let touch = PointerId::MOUSE;
        assert_eq!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(touch),
            Some(area_owner)
        );
        let press = f
            .domains
            .primary_state()
            .gesture_registry
            .physical_press(touch)
            .unwrap();
        // Exercise the registry's candidate-only removal boundary on a live foreign capture.
        assert!(
            f.domains
                .primary_state_mut()
                .gesture_registry
                .register_candidate_request(
                    touch,
                    button_owner,
                    crate::core::event::GestureKind::Scroll,
                    None
                )
        );
        let foreign = f.domains.register_window(WindowId(55));
        let foreign_pointer = PointerId(400);
        f.domains
            .primary_state_mut()
            .pointer_capture_manager
            .request_capture(foreign_pointer, area_owner)
            .unwrap();
        f.domains
            .state_mut(foreign)
            .unwrap()
            .pointer_capture_manager
            .request_capture(foreign_pointer, area_owner)
            .unwrap();
        f.tree.request_widget_enabled(button, false).unwrap();
        f.settle();
        assert_eq!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(foreign_pointer),
            Some(area_owner)
        );
        assert_eq!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(touch),
            Some(area_owner)
        );
        assert!(Rc::ptr_eq(
            &press,
            &f.domains
                .primary_state()
                .gesture_registry
                .physical_press(touch)
                .unwrap()
        ));
        assert!(
            !f.domains
                .primary_state()
                .gesture_registry
                .has_candidate_for_widget(button_owner)
        );
        assert_eq!(
            f.domains
                .state(foreign)
                .unwrap()
                .pointer_capture_manager
                .current_capture(foreign_pointer),
            Some(area_owner)
        );
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(area_owner)
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );

        let field = f.controls[1];
        f.tree.request_widget_read_only(field, true).unwrap();
        let old = f.tree.snapshot_tree_lifetime();
        let old_owner = f.owner(field);
        f.tree.remove_node(field);
        assert!(!f.tree.has_pending_widget_availability());
        assert!(!f.tree.is_live_mount(&old, old_owner));
        assert_eq!(
            f.tree.request_widget_enabled(field, false),
            Err(crate::core::widget_tree::WidgetAvailabilityError::TargetUnavailable)
        );
        let replacement = WidgetTree::new();
        assert!(!replacement.is_live_mount(&old, old_owner));
        println!(
            "ATOMIC AVAILABILITY {policy:?}: qualified intake cutoff, down-disable-up, fresh activation, foreign pointer/domain and mount eviction PASS"
        );
    }
}
