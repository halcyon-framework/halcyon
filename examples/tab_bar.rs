// SPEC-HAL-219: TabBar Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - 4 tabs, one disabled
// - External active tab tracking (TabBar does not own selection state)
// - Selection emission to external sink, printed by screen
// - Keyboard navigation: Tab focuses, Left/Right navigates, Enter/Space activates
// - No content switching inside TabBar (INV-A1)

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::tab_bar::{TabBarSink, tab_bar_sink};
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::tab_bar::{TabBar, TabId};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== TabBar Demo (SPEC-HAL-219) ===");
    println!("Click a tab or use Tab to focus, Left/Right to navigate, Enter/Space to activate");
    println!("Tab 'Disabled' is inert. Re-selecting active tab is a no-op.");
    println!("Close the window to exit");
    println!("=================================\n");

    let app = App::new(AppConfig::new("TabBar Demo").with_size(500.0, 300.0));
    app.run(TabBarDemoScreen::new());
}

fn label_for_id(id: u32) -> &'static str {
    match id {
        1 => "General",
        2 => "Network",
        3 => "Disabled",
        4 => "Advanced",
        _ => "Unknown",
    }
}

struct TabBarDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    sink: TabBarSink,
    active_tab: Option<TabId>,
    last_event: Option<String>,
}

impl TabBarDemoScreen {
    fn new() -> Self {
        let sink = tab_bar_sink();
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            sink,
            active_tab: Some(TabId(1)),
            last_event: None,
        }
    }
}

impl Screen for TabBarDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree.add_child(
            root,
            Box::new(Label::new("TabBar Demo (SPEC-HAL-219)")),
            None,
            1,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        // TabBar — externally wired, no content ownership
        let mut tab_bar = TabBar::new(self.sink.clone());
        tab_bar.set_tabs(&[
            (TabId(1), "General", true),
            (TabId(2), "Network", true),
            (TabId(3), "Disabled", false),
            (TabId(4), "Advanced", true),
        ]);
        tab_bar.set_active_tab(self.active_tab);
        cx.tree.add_child(root, Box::new(tab_bar), None, 5);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 6);

        // Status: active tab
        let active_label = match self.active_tab {
            Some(id) => format!("Active tab: {} (id={})", label_for_id(id.0), id.0),
            None => "No active tab".to_string(),
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&active_label)), None, 7);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 8);

        // Status: last selection event
        let event_label = match &self.last_event {
            Some(msg) => msg.clone(),
            None => "No selection yet".to_string(),
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&event_label)), None, 9);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 10);
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 11);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, _cx: &mut ScreenContext<'_>) {
        // Poll selection sink after all events have routed for this frame.
        // Runs once per frame regardless of whether the press emitted a
        // follow-up event (e.g., character keys like `i` produce only
        // TextInput; control keys like Enter produce KeyDown+KeyUp).
        if let Some(id) = self.sink.take() {
            let label = label_for_id(id.0);
            println!("TabSelected: {} (id={})", label, id.0);
            self.last_event = Some(format!("Selected: {} (id={})", label, id.0));
            self.active_tab = Some(id);
            self.needs_rebuild = true;
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
