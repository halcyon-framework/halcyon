// SPEC-HAL-218: DropdownMenu Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - Trigger button opens action menu anchored below
// - Keyboard: Enter/Space opens, Up/Down navigates, Enter selects, Escape dismisses
// - Selection emitted to external sink, printed by screen
// - Trigger never displays last-selected action (INV-D3)

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::dropdown_menu::{DropdownMenuHandle, SelectionSink, dropdown_menu_handle};
use halcyon::model::menu_item::MenuItem;
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::dropdown_menu::DropdownMenu;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== DropdownMenu Demo ===");
    println!("Click trigger or press Enter/Space to open");
    println!("Up/Down to navigate, Enter to select, Escape to dismiss");
    println!("=========================\n");

    let app = App::new(AppConfig::new("DropdownMenu Demo").with_size(500.0, 500.0));
    app.run(DropdownMenuDemoScreen::new());
}

fn build_actions() -> Vec<MenuItem> {
    vec![
        MenuItem::ActionRow {
            id: 1,
            label: "New File".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::ActionRow {
            id: 2,
            label: "Open File".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::ActionRow {
            id: 3,
            label: "Save".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::Separator,
        MenuItem::ActionRow {
            id: 4,
            label: "Export as PDF".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::ActionRow {
            id: 5,
            label: "Print".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
        MenuItem::Separator,
        MenuItem::ActionRow {
            id: 6,
            label: "Close".into(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        },
    ]
}

fn label_for_id(id: u32) -> &'static str {
    match id {
        1 => "New File",
        2 => "Open File",
        3 => "Save",
        4 => "Export as PDF",
        5 => "Print",
        6 => "Close",
        _ => "Unknown",
    }
}

struct DropdownMenuDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    handle: DropdownMenuHandle,
    sink: SelectionSink,
    last_action: Option<String>,
}

impl DropdownMenuDemoScreen {
    fn new() -> Self {
        let (handle, sink) = dropdown_menu_handle();
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            handle,
            sink,
            last_action: None,
        }
    }
}

impl Screen for DropdownMenuDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree
            .add_child(root, Box::new(Label::new("DropdownMenu Demo")), None, 1);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        // Trigger control — label is static, never shows last action (INV-D3)
        let mut menu = DropdownMenu::new(self.handle.clone(), "File");
        menu.set_actions(build_actions());
        cx.tree.add_child(root, Box::new(menu), None, 5);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 6);

        let status = if let Some(ref action) = self.last_action {
            format!("Last action: {}", action)
        } else {
            "No action yet".to_string()
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&status)), None, 7);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 8);
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 9);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // Poll selection sink
        if let Some(id) = self.sink.take() {
            let label = label_for_id(id);
            println!("Action: {} (id={})", label, id);
            self.last_action = Some(label.to_string());
            self.needs_rebuild = true;
        }

        // Detect dismissal without selection (for rebuild if needed)
        let menu_dismissed = !self.handle.borrow().is_open();
        if menu_dismissed && self.needs_rebuild {
            // Already flagged
        }

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
