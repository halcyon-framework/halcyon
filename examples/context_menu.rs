// SPEC-HAL-217: ContextMenu Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - Right-click invokes context menu at pointer position
// - New handle + new sink per invocation (no reuse)
// - Selection handled exclusively by screen (external sink)
// - Dismissal clears instance completely

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, PointerButton};
use halcyon::core::widget::Widget;
use halcyon::model::context_menu::{Anchor, ContextMenuHandle, SelectionSink, context_menu_handle};
use halcyon::model::menu_item::MenuItem;
use halcyon::prelude::*;
use halcyon::widgets::context_menu::ContextMenuScreen;
use halcyon::widgets::divider::Divider;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== ContextMenu Demo ===");
    println!("Right-click to open menu | Close the window to exit");
    println!("========================\n");

    let app = App::new(AppConfig::new("ContextMenu Demo").with_size(500.0, 500.0));
    app.run(ContextMenuDemoScreen::new());
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
            label: "Inspect Element".into(),
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
        5 => "Inspect Element",
        _ => "Unknown",
    }
}

struct ContextMenuDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    /// Active context menu instance. None when no menu is open.
    /// NEW handle + NEW sink created per invocation (SPEC-HAL-217 INV-B3).
    active_menu: Option<(ContextMenuHandle, SelectionSink)>,
    last_selection: Option<String>,
}

impl ContextMenuDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            active_menu: None,
            last_selection: None,
        }
    }
}

impl Screen for ContextMenuDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree
            .add_child(root, Box::new(Label::new("ContextMenu Demo")), None, 1);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Right-click anywhere to open context menu")),
            None,
            5,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        let menu_open = self
            .active_menu
            .as_ref()
            .map_or(false, |(h, _)| h.borrow().is_open());
        let status = if menu_open {
            "Menu: OPEN".to_string()
        } else if let Some(ref sel) = self.last_selection {
            format!("Last selection: {}", sel)
        } else {
            "No selection yet".to_string()
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&status)), None, 7);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 8);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 10);
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 11);

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::PointerDown {
                button: PointerButton::Secondary,
                pos,
                ..
            } => {
                // New invocation: new handle + new sink (SPEC-HAL-217 INV-B3)
                let (handle, sink) = context_menu_handle();
                handle
                    .borrow_mut()
                    .show(Anchor::Point(*pos), build_menu_items());
                cx.nav_open_modal(Box::new(ContextMenuScreen::new(handle.clone())));
                self.active_menu = Some((handle, sink));
                self.needs_rebuild = true;
                true
            }
            _ => false,
        }
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn flush_pending_actions(&mut self, _cx: &mut ScreenContext<'_>) {
        let selection_id = self.active_menu.as_ref().and_then(|(_, sink)| sink.take());
        let menu_dismissed = self
            .active_menu
            .as_ref()
            .is_some_and(|(handle, _)| !handle.borrow().is_open());

        if let Some(id) = selection_id {
            let label = label_for_id(id);
            println!("Selected: {} (id={})", label, id);
            self.last_selection = Some(label.to_string());
            self.active_menu = None;
            self.needs_rebuild = true;
        } else if menu_dismissed {
            self.active_menu = None;
            self.needs_rebuild = true;
        }
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
