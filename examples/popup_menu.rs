// SPEC-HAL-216: PopupMenu Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - Stateless reopen (items injected per invocation)
// - Selection handled exclusively by screen (external sink)
// - No controller memory between openings

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::menu_item::MenuItem;
use halcyon::model::popup_menu::{PopupMenuHandle, SelectionSink, popup_menu_handle};
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::popup_menu::PopupMenu;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== PopupMenu Demo ===");
    println!("Press M to open menu | Close the window to exit");
    println!("=======================\n");

    let app = App::new(AppConfig::new("PopupMenu Demo").with_size(500.0, 500.0));
    app.run(PopupMenuDemoScreen::new());
}

/// Build menu items fresh per invocation. No reuse across openings.
fn build_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::ActionRow {
            id: 1,
            label: "Cut".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::ActionRow {
            id: 2,
            label: "Copy".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::ActionRow {
            id: 3,
            label: "Paste".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::Separator,
        MenuItem::ActionRow {
            id: 4,
            label: "Select All".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::Separator,
        MenuItem::ActionRow {
            id: 5,
            label: "Find and Replace...".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
    ]
}

fn label_for_id(id: u32) -> &'static str {
    match id {
        1 => "Cut",
        2 => "Copy",
        3 => "Paste",
        4 => "Select All",
        5 => "Find and Replace...",
        _ => "Unknown",
    }
}

struct PopupMenuDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    controller: PopupMenuHandle,
    selection_sink: SelectionSink,
    last_selection: Option<String>,
}

impl PopupMenuDemoScreen {
    fn new() -> Self {
        let (controller, selection_sink) = popup_menu_handle();

        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            controller,
            selection_sink,
            last_selection: None,
        }
    }
}

impl Screen for PopupMenuDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree
            .add_child(root, Box::new(Label::new("PopupMenu Demo")), None, 1);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);
        cx.tree
            .add_child(root, Box::new(Label::new("Press M to open menu")), None, 5);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        let status = if self.controller.borrow().is_open() {
            "Menu: OPEN".to_string()
        } else if let Some(ref sel) = self.last_selection {
            format!("Last selection: {}", sel)
        } else {
            "Menu: CLOSED (no selection yet)".to_string()
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&status)), None, 7);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 8);

        // PopupMenu widget (zero-size anchor for overlay rendering)
        cx.tree.add_child(
            root,
            Box::new(PopupMenu::new(self.controller.clone())),
            None,
            9,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 10);
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 11);

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // Read selection from external sink (screen-owned, not controller-owned)
        if let Some(id) = self.selection_sink.take() {
            let label = label_for_id(id);
            println!("Selected: {} (id={})", label, id);
            self.last_selection = Some(label.to_string());
            self.needs_rebuild = true;
        }

        match event {
            // Character keys arrive as TextInput, not KeyDown (platform routing)
            FusedInputEvent::TextInput { text, .. } if text == "m" || text == "M" => {
                let mut ctrl = self.controller.borrow_mut();
                if ctrl.is_open() {
                    ctrl.dismiss();
                } else {
                    // Items injected per invocation — fresh slate every opening
                    ctrl.show(build_menu_items());
                }
                drop(ctrl);
                self.needs_rebuild = true;
                true
            }
            _ => false,
        }
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
