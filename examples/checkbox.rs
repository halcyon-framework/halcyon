// SPEC-HAL-111 + SPEC-HAL-208: Checkbox Demo
// Demonstrates Checkbox widget with integrated labels and toggle callback.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::checkbox::Checkbox;
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Checkbox Demo ===");
    println!("Features:");
    println!("  - Click checkbox or label to toggle");
    println!("  - Tab to focus, Space/Enter to toggle when focused");
    println!("  - Watch console for toggle callbacks");
    println!("Close the window to exit");
    println!("=====================\n");

    let app = App::new(AppConfig::new("Checkbox Demo").with_size(400.0, 300.0));
    app.run(CheckboxDemoScreen::new());
}

struct CheckboxDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    toggle_count: Rc<RefCell<u32>>,
}

impl CheckboxDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            toggle_count: Rc::new(RefCell::new(0)),
        }
    }
}

impl Screen for CheckboxDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: vertical Flex column
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("Checkbox Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Content column with margins
        let content_col =
            cx.tree
                .add_child(root, Box::new(Flex::column().with_spacing(8.0)), None, 3);

        // SPEC-HAL-208: Checkbox with integrated label — no sibling Label widget
        let toggle_count = self.toggle_count.clone();
        let checkbox = Checkbox::new()
            .with_label("Enable feature")
            .on_toggle_changed(move |checked| {
                let mut count = toggle_count.borrow_mut();
                *count += 1;
                println!(
                    "[CALLBACK] Checkbox toggled: checked={}, total_toggles={}",
                    checked, *count
                );
            });
        cx.tree.add_child(content_col, Box::new(checkbox), None, 0);

        // Second checkbox (initially checked) with integrated label
        let toggle_count2 = self.toggle_count.clone();
        let checkbox2 = Checkbox::checked(true)
            .with_label("Auto-save (initially checked)")
            .on_toggle_changed(move |checked| {
                let mut count = toggle_count2.borrow_mut();
                *count += 1;
                println!(
                    "[CALLBACK] Checkbox 2 toggled: checked={}, total_toggles={}",
                    checked, *count
                );
            });
        cx.tree.add_child(content_col, Box::new(checkbox2), None, 1);

        // Third checkbox (disabled) with integrated label
        let mut checkbox3 = Checkbox::checked(true).with_label("Locked option (disabled)");
        checkbox3.set_disabled(true);
        cx.tree.add_child(content_col, Box::new(checkbox3), None, 2);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 4);

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Click checkbox or label to toggle | Close the window to exit",
            )),
            None,
            5,
        );

        // Bottom padding
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
