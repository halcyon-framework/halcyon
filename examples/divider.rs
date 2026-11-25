// SPEC-HAL-213: Divider Demo
// Demonstrates horizontal and vertical dividers as inert visual separators.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Divider Demo ===");
    println!("Shows horizontal and vertical dividers.");
    println!("Dividers are visual-only: no interaction, no focus, no events.");
    println!("Close the window to exit");
    println!("====================\n");

    let app = App::new(AppConfig::new("Divider Demo").with_size(500.0, 400.0));
    app.run(DividerDemoScreen::new());
}

struct DividerDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl DividerDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for DividerDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("Divider Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Section 1: Horizontal dividers in a column
        cx.tree
            .add_child(root, Box::new(Label::new("Horizontal dividers:")), None, 3);

        cx.tree
            .add_child(root, Box::new(Label::new("Above divider")), None, 4);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 5);
        cx.tree
            .add_child(root, Box::new(Label::new("Between dividers")), None, 6);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 7);
        cx.tree
            .add_child(root, Box::new(Label::new("Below divider")), None, 8);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 9);

        // Section 2: Vertical dividers in a row
        cx.tree.add_child(
            root,
            Box::new(Label::new("Vertical dividers in a row:")),
            None,
            10,
        );

        let row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 11);
        cx.tree
            .add_child(row, Box::new(Label::new("Left")), None, 0);
        cx.tree
            .add_child(row, Box::new(Divider::vertical()), None, 1);
        cx.tree
            .add_child(row, Box::new(Label::new("Center")), None, 2);
        cx.tree
            .add_child(row, Box::new(Divider::vertical()), None, 3);
        cx.tree
            .add_child(row, Box::new(Label::new("Right")), None, 4);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 12);

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Dividers are inert — click/tab does nothing | Close the window to exit",
            )),
            None,
            13,
        );

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
