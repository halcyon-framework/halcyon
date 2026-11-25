// SPEC-HAL-210: Slider Demo
// Demonstrates Slider widget with continuous updates, stepping, value display,
// FormBinder binding, and disabled state.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::model::form_binder::FormBinder;
use halcyon::prelude::*;
use halcyon::widgets::slider::Slider;
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Slider Demo ===");
    println!("Features:");
    println!("  - Click/drag track to set value");
    println!("  - Tab to focus, Left/Right/PageUp/PageDown/Home/End to adjust");
    println!("  - Watch console for value callbacks");
    println!("Close the window to exit");
    println!("====================\n");

    let app = App::new(AppConfig::new("Slider Demo").with_size(500.0, 400.0));
    app.run(SliderDemoScreen::new());
}

struct SliderDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    change_count: Rc<RefCell<u32>>,
    binder: FormBinder,
}

impl SliderDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            change_count: Rc::new(RefCell::new(0)),
            binder: FormBinder::new(),
        }
    }
}

impl Screen for SliderDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("Slider Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Content column
        let content_col =
            cx.tree
                .add_child(root, Box::new(Flex::column().with_spacing(12.0)), None, 3);

        // Slider 1: basic continuous
        let count = self.change_count.clone();
        let slider1 = Slider::new(0.0, 100.0)
            .with_label("Volume")
            .with_value(50.0)
            .with_value_display()
            .on_value_changed(move |v| {
                let mut c = count.borrow_mut();
                *c += 1;
                println!("[CALLBACK] Volume: {:.1}, changes={}", v, *c);
            });
        cx.tree.add_child(content_col, Box::new(slider1), None, 0);

        // Slider 2: stepped
        let count2 = self.change_count.clone();
        let slider2 = Slider::new(0.0, 10.0)
            .with_step(1.0)
            .with_label("Rating")
            .with_value(5.0)
            .with_value_display()
            .with_value_formatter(|v| format!("{:.0}/10", v))
            .on_value_changed(move |v| {
                let mut c = count2.borrow_mut();
                *c += 1;
                println!("[CALLBACK] Rating: {:.0}, changes={}", v, *c);
            });
        cx.tree.add_child(content_col, Box::new(slider2), None, 1);

        // Slider 3: bound to FormBinder
        let count3 = self.change_count.clone();
        let mut slider3 = Slider::new(0.0, 1.0)
            .with_label("Opacity")
            .with_value(0.75)
            .with_value_display()
            .with_value_formatter(|v| format!("{:.0}%", v * 100.0))
            .on_value_changed(move |v| {
                let mut c = count3.borrow_mut();
                *c += 1;
                println!("[CALLBACK] Opacity (bound): {:.2}, changes={}", v, *c);
            });
        slider3.set_binding("opacity", self.binder.writer());
        cx.tree.add_child(content_col, Box::new(slider3), None, 2);

        // Slider 4: disabled
        let mut slider4 = Slider::new(0.0, 100.0)
            .with_label("Locked")
            .with_value(30.0)
            .with_value_display();
        slider4.set_disabled(true);
        cx.tree.add_child(content_col, Box::new(slider4), None, 3);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 4);

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Click/drag to set value | Arrow keys to adjust | Close the window to exit",
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
