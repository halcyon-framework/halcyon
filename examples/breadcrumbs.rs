// SPEC-HAL-220 + SPEC-HAL-228: Breadcrumbs Demo (NavigationHost-wired)
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - Breadcrumbs as pure projection of NavigationHost state (INV-228-D1)
// - Click a crumb → NavigateTo intent → host truncates path deterministically
// - No local path mutation — path comes from host.path()
// - Double-click safe — idempotent by construction
// - Disabled crumb behavior (INV-220-F2)
// - Optional dropdown on current crumb (INV-220-C3)
// - Keyboard: N=deeper, B=back, Tab/Left/Right/Enter/Space for crumb nav

use std::cell::Cell;
use std::rc::Rc;

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::breadcrumbs::{BreadcrumbsSink, breadcrumbs_sink};
use halcyon::model::dropdown_menu::{DropdownMenuHandle, dropdown_menu_handle};
use halcyon::model::menu_item::{MenuItem, MenuItemId};
use halcyon::model::navigation_host::{NavigationHost, NavigationIntent, NavigationNodeId};
use halcyon::prelude::*;
use halcyon::widgets::breadcrumbs::{Breadcrumbs, CrumbId};
use halcyon::widgets::divider::Divider;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Breadcrumbs Demo (SPEC-HAL-220 + SPEC-HAL-228) ===");
    println!("Click a past crumb to navigate back.");
    println!("N     — navigate deeper");
    println!("B     — navigate back");
    println!("Crumb 'Archived' is disabled (inert).");
    println!("Current crumb has a dropdown (click or Enter to open).");
    println!("Close the window to exit");
    println!("====================================================\n");

    let app = App::new(AppConfig::new("Breadcrumbs Demo").with_size(600.0, 300.0));
    app.run(BreadcrumbsDemoScreen::new());
}

fn node_label(id: u32) -> &'static str {
    match id {
        1 => "Home",
        2 => "Library",
        3 => "Archived",
        4 => "Laptops",
        _ => "Unknown",
    }
}

fn node_enabled(id: u32) -> bool {
    match id {
        3 => false, // Archived is disabled (inert)
        _ => true,
    }
}

/// Canonical tree: Home → Library → Archived → Laptops
fn next_child(id: u32) -> Option<u32> {
    match id {
        1 => Some(2),
        2 => Some(3),
        3 => Some(4),
        _ => None,
    }
}

struct BreadcrumbsDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    host: NavigationHost,
    sink: BreadcrumbsSink,
    dropdown_handle: DropdownMenuHandle,
    dropdown_sink: Rc<Cell<Option<MenuItemId>>>,
    last_event: Option<String>,
}

impl BreadcrumbsDemoScreen {
    fn new() -> Self {
        // Build initial path: Home → Library → Archived → Laptops
        let mut host = NavigationHost::new(NavigationNodeId(1));
        host.apply(NavigationIntent::NavigateTo(NavigationNodeId(2).into()));
        host.apply(NavigationIntent::NavigateTo(NavigationNodeId(3).into()));
        host.apply(NavigationIntent::NavigateTo(NavigationNodeId(4).into()));

        let sink = breadcrumbs_sink();
        let (dropdown_handle, dropdown_sink) = dropdown_menu_handle();
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            host,
            sink,
            dropdown_handle,
            dropdown_sink,
            last_event: None,
        }
    }
}

impl Screen for BreadcrumbsDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Breadcrumbs Demo (SPEC-HAL-220 + SPEC-HAL-228)")),
            None,
            1,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        // Breadcrumbs — pure projection of NavigationHost state
        let mut breadcrumbs = Breadcrumbs::new(self.sink.clone());
        let crumbs: Vec<(CrumbId, &str, bool)> = self
            .host
            .path()
            .iter()
            .map(|n| (CrumbId(n.0), node_label(n.0), node_enabled(n.0)))
            .collect();
        breadcrumbs.set_crumbs(&crumbs);
        breadcrumbs.set_current_crumb(Some(CrumbId(self.host.current().0)));

        // Optional dropdown on current crumb (INV-220-C3)
        breadcrumbs.enable_dropdown(
            self.dropdown_handle.clone(),
            self.dropdown_sink.clone(),
            vec![
                MenuItem::ActionRow {
                    id: 1,
                    label: "Home".to_string(),
                    icon: None,
                    shortcut: None,
                    disabled: false,
                    submenu: None,
                },
                MenuItem::ActionRow {
                    id: 2,
                    label: "Library".to_string(),
                    icon: None,
                    shortcut: None,
                    disabled: false,
                    submenu: None,
                },
            ],
        );

        cx.tree.add_child(root, Box::new(breadcrumbs), None, 5);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 6);

        // Status: current node (from host)
        let current_label = format!(
            "Current: {} (id={})",
            node_label(self.host.current().0),
            self.host.current().0
        );
        cx.tree
            .add_child(root, Box::new(Label::new(&current_label)), None, 7);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 8);

        // Status: path (from host)
        let path_labels: Vec<&str> = self.host.path().iter().map(|n| node_label(n.0)).collect();
        let path_str = format!("Path: {}", path_labels.join(" > "));
        cx.tree
            .add_child(root, Box::new(Label::new(&path_str)), None, 9);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 10);

        // Status: last navigation event
        let event_label = match &self.last_event {
            Some(msg) => msg.clone(),
            None => "No navigation yet".to_string(),
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&event_label)), None, 11);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 12);
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Click crumb=navigate, N=deeper, B=back. Close the window to exit",
            )),
            None,
            13,
        );

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // Poll breadcrumb sink → apply to host
        if let Some(crumb_id) = self.sink.take() {
            let node_id = NavigationNodeId(crumb_id.0);
            let state = self
                .host
                .apply(NavigationIntent::NavigateTo(node_id.into()));
            let label = node_label(state.current().node_id().0);
            println!(
                "Crumb click: NavigateTo {} → Path: {:?}",
                label,
                state
                    .path()
                    .iter()
                    .map(|entry| entry.node_id().0)
                    .collect::<Vec<_>>()
            );
            self.last_event = Some(format!(
                "Navigated to: {} (id={})",
                label,
                state.current().node_id().0
            ));
            self.needs_rebuild = true;
        }

        // Poll dropdown sink → apply to host
        if let Some(menu_id) = self.dropdown_sink.take() {
            let node_id = NavigationNodeId(menu_id);
            let state = self
                .host
                .apply(NavigationIntent::NavigateTo(node_id.into()));
            let label = node_label(state.current().node_id().0);
            println!(
                "Dropdown: NavigateTo {} → Path: {:?}",
                label,
                state
                    .path()
                    .iter()
                    .map(|entry| entry.node_id().0)
                    .collect::<Vec<_>>()
            );
            self.last_event = Some(format!(
                "Dropdown to: {} (id={})",
                label,
                state.current().node_id().0
            ));
            self.needs_rebuild = true;
        }

        match event {
            FusedInputEvent::TextInput { text, .. } if text == "n" || text == "N" => {
                if let Some(child_id) = next_child(self.host.current().0) {
                    let state = self.host.apply(NavigationIntent::NavigateTo(
                        NavigationNodeId(child_id).into(),
                    ));
                    let label = node_label(state.current().node_id().0);
                    println!(
                        "Deeper: NavigateTo {} → Path: {:?}",
                        label,
                        state
                            .path()
                            .iter()
                            .map(|entry| entry.node_id().0)
                            .collect::<Vec<_>>()
                    );
                    self.last_event = Some(format!(
                        "Navigated deeper to: {} (id={})",
                        label,
                        state.current().node_id().0
                    ));
                    self.needs_rebuild = true;
                }
                true
            }
            FusedInputEvent::TextInput { text, .. } if text == "b" || text == "B" => {
                let state = self.host.apply(NavigationIntent::NavigateUp);
                let label = node_label(state.current().node_id().0);
                println!(
                    "Back → Path: {:?}",
                    state
                        .path()
                        .iter()
                        .map(|entry| entry.node_id().0)
                        .collect::<Vec<_>>()
                );
                self.last_event = Some(format!(
                    "Navigated back to: {} (id={})",
                    label,
                    state.current().node_id().0
                ));
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
