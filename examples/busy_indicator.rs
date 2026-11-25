// SPEC-HAL-211: BusyIndicator Demo
// Demonstrates indeterminate busy indicator with frame-driven animation,
// optional labels, and disabled state.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::busy_indicator::BusyIndicator;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== BusyIndicator Demo ===");
    println!("Features:");
    println!("  - Indeterminate animation (frame-driven)");
    println!("  - Optional labels");
    println!("  - Disabled visual state");
    println!("Close the window to exit");
    println!("==========================\n");

    let app = App::new(AppConfig::new("BusyIndicator Demo").with_size(500.0, 400.0));
    app.run(BusyIndicatorDemoScreen::new());
}

struct BusyIndicatorDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl BusyIndicatorDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for BusyIndicatorDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("BusyIndicator Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Content column
        let content_col =
            cx.tree
                .add_child(root, Box::new(Flex::column().with_spacing(12.0)), None, 3);

        // BusyIndicator 1: with label
        let bi1 = BusyIndicator::new().with_label("Loading...");
        cx.tree.add_child(content_col, Box::new(bi1), None, 0);

        // BusyIndicator 2: different label
        let bi2 = BusyIndicator::new().with_label("Processing data");
        cx.tree.add_child(content_col, Box::new(bi2), None, 1);

        // BusyIndicator 3: no label
        let bi3 = BusyIndicator::new();
        cx.tree.add_child(content_col, Box::new(bi3), None, 2);

        // BusyIndicator 4: disabled
        let mut bi4 = BusyIndicator::new().with_label("Suspended (disabled)");
        bi4.set_disabled(true);
        cx.tree.add_child(content_col, Box::new(bi4), None, 3);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 4);

        // Help text
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 5);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 6);

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
