//! Native TextArea demo for multiline editing, selection, and scrolling (HAL-109).
//!
//! The console lists controls to try in the window. The mounted example test
//! checks the document and scroll offset after routed input.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::text_area::TextArea;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== TextArea Multi-Line Demo ===");
    println!("Try these controls in the native window.");
    println!("  - Type text with Enter for newlines");
    println!("  - Arrow keys for navigation (Up/Down/Left/Right)");
    println!("  - Shift+Arrow for selection");
    println!("  - Cmd/Ctrl+A to select all");
    println!("  - Cmd/Ctrl+C/X/V for copy/cut/paste");
    println!("  - Cmd/Ctrl+Z for undo, Shift+Cmd+Z for redo");
    println!("  - Mouse click/drag to select");
    println!("  - Double-click to select word");
    println!("  - Triple-click to select line");
    println!("  - Scroll with trackpad/mousewheel");
    println!("================================\n");

    let app = App::new(AppConfig::new("TextArea Demo").with_size(700.0, 500.0));
    app.run(TextAreaDemoScreen::new());
}

struct TextAreaDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl TextAreaDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for TextAreaDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: vertical Flex column
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 0);

        // Title
        cx.tree.add_child(
            root,
            Box::new(Label::new("TextArea Demo - Multi-Line Plain Text Editor")),
            None,
            1,
        );

        // Main content row with margins
        let content_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 2);
        cx.tree.set_flex(content_row, FlexParams::flexible(1.0));

        // Left margin
        cx.tree.add_child(
            content_row,
            Box::new(FixedSpacer::horizontal(24.0)),
            None,
            0,
        );

        // TextArea with sample content (enough to require scrolling)
        let sample_text = "Welcome to TextArea!\n\
            \n\
            This is a multi-line text editor that supports:\n\
            - Typing and editing across multiple lines\n\
            - Vertical scrolling when content exceeds viewport\n\
            - Selection with mouse drag or Shift+Arrow keys\n\
            - Copy, Cut, and Paste operations\n\
            - Undo and Redo\n\
            \n\
            Try scrolling down to see more content.\n\
            \n\
            Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
            Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
            Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.\n\
            \n\
            Press Enter to insert new lines.\n\
            Use Backspace to delete backwards.\n\
            Use Delete to delete forwards.\n\
            \n\
            === Additional Content for Scrollbar Testing ===\n\
            \n\
            Line 1: The quick brown fox jumps over the lazy dog.\n\
            Line 2: Pack my box with five dozen liquor jugs.\n\
            Line 3: How vexingly quick daft zebras jump!\n\
            Line 4: The five boxing wizards jump quickly.\n\
            Line 5: Sphinx of black quartz, judge my vow.\n\
            \n\
            Line 6: Two driven jocks help fax my big quiz.\n\
            Line 7: The jay, pig, fox, zebra and my wolves quack!\n\
            Line 8: Sympathizing would fix Quaker objectives.\n\
            Line 9: A wizard's job is to vex chumps quickly in fog.\n\
            Line 10: Watch Jeopardy, Alex Trebek's fun TV quiz game.\n\
            \n\
            Line 11: By Jove, my quick study of lexicography won a prize!\n\
            Line 12: Crazy Frederick bought many very exquisite opal jewels.\n\
            Line 13: Sixty zippers were quickly picked from the woven jute bag.\n\
            Line 14: A quick movement of the enemy will jeopardize six gunboats.\n\
            Line 15: The job requires extra pluck and zeal from every young wage earner.\n\
            \n\
            Line 16: Just keep examining every low bid quoted for zinc etchings.\n\
            Line 17: How razorback-jumping frogs can level six piqued gymnasts!\n\
            Line 18: Cozy lummox gives smart squid who asks for job pen.\n\
            Line 19: Adjusting quiver and bow, Zompyc killed the fox.\n\
            Line 20: My faxed joke won a pager in the cable TV quiz show.\n\
            \n\
            === End of Sample Content ===";

        let text_area_node = cx.tree.add_child(
            content_row,
            Box::new(
                TextArea::new()
                    .with_text(sample_text)
                    .with_placeholder("Enter text..."),
            ),
            None,
            1,
        );
        cx.tree.set_flex(text_area_node, FlexParams::flexible(1.0));

        // Right margin
        cx.tree.add_child(
            content_row,
            Box::new(FixedSpacer::horizontal(24.0)),
            None,
            2,
        );

        // Help text
        let help_text = "Up/Down to navigate lines | Cmd/Ctrl+Z to undo";
        cx.tree
            .add_child(root, Box::new(Label::new(help_text)), None, 3);

        // Bottom padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
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
    use halcyon::core::rect_authority::LayoutAuthorityGuard;
    use halcyon::runtime::actions::ActionId;
    use halcyon::runtime::binding_registry::BindingRegistry;
    use halcyon::widgets::screen_host::ScreenHost;
    use std::time::Instant;

    fn mounted_area(tree: &WidgetTree, node: NodeId) -> &TextArea {
        tree.node(node)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<TextArea>()
            .unwrap()
    }

    #[test]
    fn mounted_text_area_routes_editing_selection_and_wheel() {
        let theme = Theme::default();
        let mut app = AppContext::new(theme.clone());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let mut screen = TextAreaDemoScreen::new();
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
                Constraints::tight(Size::new(700.0, 500.0)),
                &theme,
                &mut text_engine,
                None,
            );
        }
        let column = tree.children(root)[0];
        let row = tree.children(column)[2];
        let area_node = tree.children(row)[1];
        let owner = tree.widget_id_for_node(area_node).unwrap();
        let initial = mounted_area(&tree, area_node).text().to_owned();
        assert!(initial.ends_with("=== End of Sample Content ==="));

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
        let mut route = |tree: &mut WidgetTree, raw| {
            let mut events = Vec::new();
            fusion.process_raw_with_gesture_sessions(
                raw,
                domain.focus_manager.interaction_domain_id(),
                &mut domain.gesture_registry,
                tree,
                None,
                &mut events,
            );
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
            }
            (domain.focus_manager.focused(), domain.keyboard_mode)
        };

        let area = mounted_area(&tree, area_node);
        let scroll_before = area.scroll_y_for_test();
        let caret_before = area.cursor_byte_pos_for_test();
        let selection_before = area.selection_range_for_test();
        let rect = tree.get_absolute_paint_rect(area_node).unwrap();
        let position = Point::new(rect.x + 30.0, rect.y + 30.0);
        assert_eq!(tree.hit_test(position.x, position.y).unwrap().widget_id, owner);
        route(
            &mut tree,
            RawInputEvent::PointerWheel {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                device: PointerDevice::Mouse,
                delta: WheelDelta::Pixels(0.0, -24.0),
                position,
            },
        );
        let area = mounted_area(&tree, area_node);
        assert!(area.scroll_y_for_test() > scroll_before);
        assert_eq!(area.cursor_byte_pos_for_test(), caret_before);
        assert_eq!(area.selection_range_for_test(), selection_before);
        assert_eq!(area.text(), initial);

        let key = |key_code, physical_key, modifiers| {
            [KeyState::Down, KeyState::Up].map(|state| RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key,
                reservation: None,
                state,
                repeat: false,
                modifiers,
            })
        };
        let text = |text: &str| RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: text.into(),
            modifiers: Modifiers::default(),
        };
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        for raw in key(KeyCode::Home, PhysicalKeyCode::Home, ctrl) {
            route(&mut tree, raw);
        }
        assert_eq!(mounted_area(&tree, area_node).cursor_byte_pos_for_test(), 0);
        for raw in key(KeyCode::End, PhysicalKeyCode::End, ctrl) {
            route(&mut tree, raw);
        }
        let area = mounted_area(&tree, area_node);
        assert_eq!(area.cursor_byte_pos_for_test(), initial.len());
        assert_eq!(area.text(), initial);

        // The native window converter filters Enter's control text from TextInput.
        for raw in key(KeyCode::Enter, PhysicalKeyCode::Enter, Modifiers::default()) {
            route(&mut tree, raw);
        }
        assert_eq!(mounted_area(&tree, area_node).text(), format!("{initial}\n"));
        route(&mut tree, text("λ"));
        let inserted = format!("{initial}\nλ");
        assert_eq!(mounted_area(&tree, area_node).text(), inserted);

        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        for _ in 0..3 {
            for raw in key(KeyCode::Left, PhysicalKeyCode::ArrowLeft, shift) {
                route(&mut tree, raw);
            }
        }
        let area = mounted_area(&tree, area_node);
        let (anchor, head) = area.selection_range_for_test();
        let selected_caret = area.cursor_byte_pos_for_test();
        assert_ne!(anchor, head);
        assert_eq!(area.text(), inserted);

        route(&mut tree, text("Ω"));
        let prefix = initial.strip_suffix('=').unwrap();
        let expected = format!("{prefix}Ω");
        let replacement_caret = expected.len();
        let replacement_selection = (replacement_caret, replacement_caret);
        let area = mounted_area(&tree, area_node);
        assert_eq!(area.text(), expected);
        assert_eq!(area.cursor_byte_pos_for_test(), replacement_caret);
        assert_eq!(area.selection_range_for_test(), replacement_selection);

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
            // The native converter reserves the key and emits printable text at the press time.
            [
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

        let captured = (Some(owner), KeyboardAuthorityMode::Capture);
        for (redo, document, caret, selection) in [
            (false, inserted.as_str(), selected_caret, (anchor, head)),
            (
                true,
                expected.as_str(),
                replacement_caret,
                replacement_selection,
            ),
            (false, inserted.as_str(), selected_caret, (anchor, head)),
        ] {
            for raw in history_shortcut(redo) {
                assert_eq!(route(&mut tree, raw), captured);
            }
            let area = mounted_area(&tree, area_node);
            assert_eq!(area.text(), document);
            assert_eq!(area.cursor_byte_pos_for_test(), caret);
            assert_eq!(area.selection_range_for_test(), selection);
        }

        assert_eq!(route(&mut tree, text("β")), captured);
        let new_edit = format!("{prefix}β");
        assert_eq!(mounted_area(&tree, area_node).text(), new_edit);
        for raw in history_shortcut(true) {
            assert_eq!(route(&mut tree, raw), captured);
        }
        let area = mounted_area(&tree, area_node);
        assert_eq!(area.text(), new_edit);
        assert_eq!(area.cursor_byte_pos_for_test(), new_edit.len());
        assert_eq!(
            area.selection_range_for_test(),
            (new_edit.len(), new_edit.len()),
        );
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Capture);
    }
}
