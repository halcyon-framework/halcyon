// SPEC-HAL-021 Stage 5/6: Button Animation Example
// Demonstrates hover and pressed animations using the capability injection pipeline.
// Single button, visual confirmation only.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::replace_widget;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== SPEC-HAL-021: Button Animation Demo ===");
    println!("Hover: White overlay fades in/out (0.12s)");
    println!("Press: Dark overlay appears on click (0.08s)");
    println!("Close the window to exit.");
    println!("===========================================\n");

    let mut config = AppConfig::new("Button Animation Demo").with_size(400.0, 300.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(ButtonHoverScreen::new());
}

struct ButtonHoverScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl ButtonHoverScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for ButtonHoverScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: centered column
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top spacer to push button toward center
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(100.0)), None, 0);

        // Centered row for button
        let center_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 1);

        // Left spacer
        cx.tree.add_child(
            center_row,
            Box::new(FixedSpacer::horizontal(120.0)),
            None,
            0,
        );

        // Button with label child
        let button_node =
            cx.tree
                .add_child(center_row, Box::new(Label::new("placeholder")), None, 1);
        let label_node = cx
            .tree
            .add_child(button_node, Box::new(Label::new("Click Me")), None, 0);
        replace_widget!(
            cx.tree,
            button_node,
            Button::new(label_node).with_padding(16.0)
        );

        // Right spacer
        cx.tree.add_child(
            center_row,
            Box::new(FixedSpacer::horizontal(120.0)),
            None,
            2,
        );

        // Bottom spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(100.0)), None, 2);

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new("Hover and click button | Close the window to exit")),
            None,
            3,
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
