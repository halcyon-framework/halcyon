// SPEC-HAL-212: SpinBox Demo
// Demonstrates discrete numeric editor with increment/decrement,
// optional labels, bounds, FormBinder binding, and disabled state.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::form_binder::FormBinder;
use halcyon::prelude::*;
use halcyon::widgets::spin_box::SpinBox;
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== SpinBox Demo ===");
    println!("Features:");
    println!("  - Click -/+ buttons to increment/decrement");
    println!("  - Tab to focus, Up/Down arrow keys to adjust");
    println!("  - Watch console for value callbacks");
    println!("Close the window to exit");
    println!("====================\n");

    let app = App::new(AppConfig::new("SpinBox Demo").with_size(500.0, 400.0));
    app.run(SpinBoxDemoScreen::new());
}

struct SpinBoxDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    change_count: Rc<RefCell<u32>>,
    binder: FormBinder,
}

impl SpinBoxDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            change_count: Rc::new(RefCell::new(0)),
            binder: FormBinder::new(),
        }
    }
}

impl Screen for SpinBoxDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("SpinBox Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Content column
        let content_col =
            cx.tree
                .add_child(root, Box::new(Flex::column().with_spacing(12.0)), None, 3);

        // SpinBox 1: basic, unbounded, step=1
        let count1 = self.change_count.clone();
        let sb1 = SpinBox::new()
            .with_label("Counter")
            .on_value_changed(move |v| {
                let mut c = count1.borrow_mut();
                *c += 1;
                println!("[CALLBACK] Counter: {}, changes={}", v, *c);
            });
        cx.tree.add_child(content_col, Box::new(sb1), None, 0);

        // SpinBox 2: bounded 0..100, step=5
        let count2 = self.change_count.clone();
        let sb2 = SpinBox::new()
            .with_min(0)
            .with_max(100)
            .with_step(5)
            .with_value(50)
            .with_label("Percent")
            .on_value_changed(move |v| {
                let mut c = count2.borrow_mut();
                *c += 1;
                println!("[CALLBACK] Percent: {}, changes={}", v, *c);
            });
        cx.tree.add_child(content_col, Box::new(sb2), None, 1);

        // SpinBox 3: bound to FormBinder
        let count3 = self.change_count.clone();
        let mut sb3 = SpinBox::new()
            .with_min(-50)
            .with_max(50)
            .with_value(0)
            .with_label("Bound")
            .on_value_changed(move |v| {
                let mut c = count3.borrow_mut();
                *c += 1;
                println!("[CALLBACK] Bound: {} (via binder), changes={}", v, *c);
            });
        sb3.set_binding("spin_value", self.binder.writer());
        cx.tree.add_child(content_col, Box::new(sb3), None, 2);

        // SpinBox 4: disabled
        let mut sb4 = SpinBox::new()
            .with_value(30)
            .with_label("Locked (disabled)");
        sb4.set_disabled(true);
        cx.tree.add_child(content_col, Box::new(sb4), None, 3);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 4);

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Click -/+ or focus and use Up/Down keys | Close the window to exit",
            )),
            None,
            5,
        );

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
