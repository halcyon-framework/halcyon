// SPEC-HAL-228: NavigationHost Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - NavigationHost as sole navigation state authority
// - Intent-driven state updates (NavigateTo, NavigateUp)
// - State projection (path, current node) without widget modification
// - Deterministic: same intent sequence → same state sequence
// - No widgets modified — Breadcrumbs/TabBar remain untouched

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::navigation_host::{NavigationHost, NavigationIntent, NavigationNodeId};
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== NavigationHost Demo (SPEC-HAL-228) ===");
    println!("1-4   — NavigateTo node 1-4");
    println!("B     — NavigateUp");
    println!("Close the window to exit");
    println!("=========================================\n");

    let app = App::new(AppConfig::new("NavigationHost Demo").with_size(600.0, 300.0));
    app.run(NavigationHostDemoScreen::new());
}

fn label_for_node(id: u32) -> &'static str {
    match id {
        1 => "Home",
        2 => "Library",
        3 => "Electronics",
        4 => "Laptops",
        _ => "Unknown",
    }
}

struct NavigationHostDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    host: NavigationHost,
    last_intent: Option<String>,
}

impl NavigationHostDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            host: NavigationHost::new(NavigationNodeId(1)),
            last_intent: None,
        }
    }
}

impl Screen for NavigationHostDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree.add_child(
            root,
            Box::new(Label::new("NavigationHost Demo (SPEC-HAL-228)")),
            None,
            1,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        // Path projection (what Breadcrumbs would read)
        let path_labels: Vec<&str> = self
            .host
            .path()
            .iter()
            .map(|n| label_for_node(n.0))
            .collect();
        let path_str = format!("Path: {}", path_labels.join(" > "));
        cx.tree
            .add_child(root, Box::new(Label::new(&path_str)), None, 5);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        // Current node
        let current_str = format!(
            "Current: {} (id={})",
            label_for_node(self.host.current().0),
            self.host.current().0
        );
        cx.tree
            .add_child(root, Box::new(Label::new(&current_str)), None, 7);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 8);

        // Last intent
        let intent_str = match &self.last_intent {
            Some(msg) => msg.clone(),
            None => "No intent yet".to_string(),
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&intent_str)), None, 9);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 10);

        cx.tree.add_child(
            root,
            Box::new(Label::new("1-4=NavigateTo, B=Back. Close the window to exit")),
            None,
            11,
        );

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::TextInput { text, .. } => {
                let intent = match text.as_str() {
                    "1" => Some(NavigationIntent::NavigateTo(NavigationNodeId(1).into())),
                    "2" => Some(NavigationIntent::NavigateTo(NavigationNodeId(2).into())),
                    "3" => Some(NavigationIntent::NavigateTo(NavigationNodeId(3).into())),
                    "4" => Some(NavigationIntent::NavigateTo(NavigationNodeId(4).into())),
                    "b" | "B" => Some(NavigationIntent::NavigateUp),
                    _ => None,
                };
                if let Some(intent) = intent {
                    let desc = match intent {
                        NavigationIntent::NavigateTo(ref entry) => {
                            let id = entry.node_id();
                            format!("NavigateTo({} id={})", label_for_node(id.0), id.0)
                        }
                        NavigationIntent::NavigateUp => "NavigateUp".to_string(),
                        NavigationIntent::NavigatePath(_)
                        | NavigationIntent::HistoryBack
                        | NavigationIntent::HistoryForward => unreachable!(),
                        #[allow(deprecated)]
                        NavigationIntent::NavigateBack => "NavigateUp".to_string(),
                    };
                    let state = self.host.apply(intent);
                    println!(
                        "Intent: {} → Current: {} Path: {:?}",
                        desc,
                        label_for_node(state.current().node_id().0),
                        state
                            .path()
                            .iter()
                            .map(|entry| entry.node_id().0)
                            .collect::<Vec<_>>()
                    );
                    self.last_intent = Some(desc);
                    self.needs_rebuild = true;
                    true
                } else {
                    false
                }
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
