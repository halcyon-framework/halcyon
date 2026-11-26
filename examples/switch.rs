// SPEC-HAL-209 + SPEC-HAL-208: Switch Demo
// Demonstrates Switch widget with integrated labels, toggle callback, and binding.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::SwitchAppearance;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::model::form_binder::FormBinder;
use halcyon::prelude::*;
use halcyon::widgets::switch::Switch;
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Switch Demo ===");
    println!("Features:");
    println!("  - Click switch or label to toggle");
    println!("  - Tab to focus, Space/Enter to toggle when focused");
    println!("  - Third switch bound to FormBinder");
    println!("  - Watch console for toggle callbacks");
    println!("Close the window to exit");
    println!("====================\n");

    let app = App::new(AppConfig::new("Switch Demo").with_size(400.0, 400.0));
    app.run(SwitchDemoScreen::new());
}

struct SwitchDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    toggle_count: Rc<RefCell<u32>>,
    binder: FormBinder,
}

impl SwitchDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            toggle_count: Rc::new(RefCell::new(0)),
            binder: FormBinder::new(),
        }
    }
}

impl Screen for SwitchDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let bg_node = cx.mount_child(Box::new(Label::new("")));
        let root = cx.tree.add_child(
            bg_node,
            Box::new(Flex::column().with_spacing(16.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            bg_node,
            Container::new(root, 0.0).with_background(cx.app.theme.colors.surfaces.base0)
        );

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("Switch Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Content column
        let content_col =
            cx.tree
                .add_child(root, Box::new(Flex::column().with_spacing(8.0)), None, 3);

        // Switch 1: basic toggle with callback
        let toggle_count = self.toggle_count.clone();
        let switch1 = Switch::new()
            .with_label("Enable notifications")
            .on_toggle_changed(move |on| {
                let mut count = toggle_count.borrow_mut();
                *count += 1;
                println!("[CALLBACK] Switch 1: on={}, total_toggles={}", on, *count);
            });
        cx.tree.add_child(content_col, Box::new(switch1), None, 0);

        // Switch 2: initially on
        let toggle_count2 = self.toggle_count.clone();
        let switch2 = Switch::on(true)
            .with_label("Auto-save (initially on)")
            .on_toggle_changed(move |on| {
                let mut count = toggle_count2.borrow_mut();
                *count += 1;
                println!("[CALLBACK] Switch 2: on={}, total_toggles={}", on, *count);
            });
        cx.tree.add_child(content_col, Box::new(switch2), None, 1);

        // Switch 3: bound to FormBinder
        let toggle_count3 = self.toggle_count.clone();
        let mut switch3 = Switch::new()
            .with_label("Dark mode (bound)")
            .on_toggle_changed(move |on| {
                let mut count = toggle_count3.borrow_mut();
                *count += 1;
                println!(
                    "[CALLBACK] Switch 3 (bound): on={}, total_toggles={}",
                    on, *count
                );
            });
        switch3.set_binding("dark_mode", self.binder.writer());
        cx.tree.add_child(content_col, Box::new(switch3), None, 2);

        // Switch 4: disabled
        let mut switch4 = Switch::on(true).with_label("Locked option (disabled)");
        switch4.set_disabled(true);
        cx.tree.add_child(content_col, Box::new(switch4), None, 3);

        let appearance = SwitchAppearance::try_new(
            Color::from_hex(0x42484d),
            Color::from_hex(0x597c65),
            Color::from_hex(0xc4c8ca),
            Color::from_hex(0xe1e7df),
        )
        .expect("valid Switch example colors");
        let rounded = Switch::new()
            .with_label("Rounded appearance")
            .with_switch_appearance(appearance);
        cx.tree.add_child(content_col, Box::new(rounded), None, 4);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 4);

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Click switch or label to toggle | Close the window to exit",
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
