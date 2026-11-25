//! Single-line TextField editing and selection (SPEC-HAL-101).
//!
//! Click to focus and place the caret. Type or delete text, or drag to select
//! a range. The status label counts the field's text-change notifications.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== TextField Editing Demo ===");
    println!("Click the field to focus it and place the blinking caret.");
    println!("Type or press Backspace to edit. Drag across text to select it.");
    println!("The label counts the field's text-change notifications.");
    println!("==============================\n");

    let app = App::new(AppConfig::new("TextField Demo").with_size(600.0, 200.0));
    app.run(TextFieldDemoScreen::new());
}

struct TextFieldDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    text_changes: Rc<Cell<u64>>,
    change_status: Option<NodeId>,
    displayed_changes: u64,
}

impl TextFieldDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            text_changes: Rc::new(Cell::new(0)),
            change_status: None,
            displayed_changes: 0,
        }
    }
}

impl Screen for TextFieldDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: vertical Flex column with padding
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top spacer for vertical centering
        let spacer1 = cx
            .tree
            .add_child(root, Box::new(FlexSpacer::new()), None, 0);
        cx.tree.set_flex(spacer1, FlexParams::flexible(1.0));

        // Create a row to hold the text field with horizontal margins
        let row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 1);

        // Left margin
        cx.tree
            .add_child(row, Box::new(FixedSpacer::horizontal(32.0)), None, 0);

        let mut field = TextField::new()
            .with_text("Click here and select some text")
            .with_placeholder("Enter text...");
        let changes = Rc::clone(&self.text_changes);
        field.set_on_change(move |_| {
            let next = changes
                .get()
                .checked_add(1)
                .expect("text change count overflow");
            changes.set(next);
        });
        cx.tree.add_child(row, Box::new(field), None, 1);

        // Right margin
        cx.tree
            .add_child(row, Box::new(FixedSpacer::horizontal(32.0)), None, 2);

        // Bottom spacer for vertical centering
        let spacer2 = cx
            .tree
            .add_child(root, Box::new(FlexSpacer::new()), None, 2);
        cx.tree.set_flex(spacer2, FlexParams::flexible(1.0));

        self.displayed_changes = self.text_changes.get();
        self.change_status = Some(cx.tree.add_child(
            root,
            Box::new(Label::new(format!("Text changes: {}", self.displayed_changes))),
            None,
            3,
        ));

        // Bottom padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let count = self.text_changes.get();
        if count == self.displayed_changes {
            return;
        }
        let Some(status) = self.change_status else {
            return;
        };
        tree.node_mut(status)
            .expect("current text change status mount")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Label>()
            .expect("text change status Label")
            .set_text(format!("Text changes: {count}"));
        tree.mark_layout_dirty(status);
        self.displayed_changes = count;
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halcyon::core::event_router::EventRouter;
    use halcyon::core::input_fusion::InputFusion;
    use halcyon::core::rect_authority::{LayoutAuthorityGuard, UpdateAuthorityGuard};
    use halcyon::runtime::actions::ActionId;
    use halcyon::runtime::binding_registry::BindingRegistry;
    use halcyon::widgets::screen_host::ScreenHost;
    use halcyon::widgets::text_widget_test_harness::TextWidgetTestHarnessRead;
    use std::time::Instant;

    #[test]
    fn mounted_text_field_counts_two_changes_before_screen_update() {
        let theme = Theme::default();
        let mut app = AppContext::new(theme.clone());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut screen = TextFieldDemoScreen::new();
        screen.build(&mut ScreenFrameContext {
            app: &mut app,
            tree: &mut tree,
            parent_node: root,
        });

        let mut text_engine = FontManagerTextEngine::new_with_system_ui();
        let mut ticker = EngineTicker::new(Instant::now());
        ticker.begin_frame();
        {
            let _layout = LayoutAuthorityGuard::enter();
            layout_tree(
                &mut tree,
                root,
                Constraints::tight(Size::new(600.0, 200.0)),
                &theme,
                &mut text_engine,
                None,
            );
        }
        let column = tree.children(root)[0];
        let row = tree.children(column)[1];
        let field = tree.children(row)[1];
        let owner = tree.widget_id_for_node(field).unwrap();
        let mut domain = InteractionDomainState::new();
        let mut router = EventRouter::new();
        for action in [ActionId::FOCUS_NEXT, ActionId::ENTER_WIDGET] {
            assert!(router.handle_engine_action(
                &mut tree,
                action,
                Some(root),
                &mut domain,
                &mut text_engine,
                None,
            ));
        }
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);

        let mut bindings = BindingRegistry::new();
        let mut fusion = InputFusion::new();
        let mut route = |tree: &mut WidgetTree, inputs: Vec<RawInputEvent>| {
            let mut events = Vec::new();
            for raw in inputs {
                fusion.process_raw_with_gesture_sessions(
                    raw,
                    domain.focus_manager.interaction_domain_id(),
                    &mut domain.gesture_registry,
                    tree,
                    None,
                    &mut events,
                );
            }
            fusion.flush_with_gesture_sessions(&mut events);
            for event in events {
                router.route_session_input_event(
                    tree,
                    event,
                    Some(root),
                    &mut text_engine,
                    None,
                    &mut domain,
                    &mut bindings,
                );
                assert_eq!(domain.focus_manager.focused(), Some(owner));
                assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);
            }
        };
        let insert = |text: &str| RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: text.into(),
            modifiers: Modifiers::default(),
        };
        let snapshot = |tree: &WidgetTree| {
            let text_field = tree
                .node(field)
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<TextField>()
                .unwrap();
            (text_field.text().to_owned(), text_field.cursor_ref().pos)
        };

        let initial = snapshot(&tree);
        assert_eq!(initial.0, "Click here and select some text");
        assert_eq!(initial.1, initial.0.len());
        route(&mut tree, vec![insert("x"), insert("y")]);
        let typed = (format!("{}xy", initial.0), initial.1 + 2);
        assert_eq!(snapshot(&tree), typed);
        assert_eq!(screen.text_changes.get(), 2);
        assert_eq!(screen.displayed_changes, 0);

        let history_shortcut = |redo| {
            let modifiers = Modifiers {
                meta: cfg!(target_os = "macos"),
                ctrl: !cfg!(target_os = "macos"),
                shift: redo,
                ..Modifiers::default()
            };
            let pressed_at = Instant::now();
            let key = |state, time| RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time,
                key_code: KeyCode::Character('z'),
                physical_key: PhysicalKeyCode::KeyZ,
                reservation: Some(KeyboardKeyReservation::Clipboard),
                state,
                repeat: false,
                modifiers,
            };
            // The native converter reserves the key and pairs the text with its press time.
            vec![
                key(KeyState::Down, pressed_at),
                RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: pressed_at,
                    text: (if redo { "Z" } else { "z" }).into(),
                    modifiers,
                },
                key(KeyState::Up, Instant::now()),
            ]
        };
        for (redo, expected, changes) in [
            (false, &initial, 3),
            (true, &typed, 4),
            (false, &initial, 5),
        ] {
            route(&mut tree, history_shortcut(redo));
            assert_eq!(&snapshot(&tree), expected);
            assert_eq!(screen.text_changes.get(), changes);
        }

        route(&mut tree, vec![insert("λ")]);
        let fresh = (format!("{}λ", initial.0), initial.1 + 'λ'.len_utf8());
        assert_eq!(snapshot(&tree), fresh);
        assert_eq!(screen.text_changes.get(), 6);
        route(&mut tree, history_shortcut(true));
        assert_eq!(snapshot(&tree), fresh);
        assert_eq!(screen.text_changes.get(), 6);
        assert_eq!(screen.displayed_changes, 0);

        let _update = UpdateAuthorityGuard::enter();
        screen.update(&mut tree);
        assert_eq!(screen.displayed_changes, 6);
    }
}
