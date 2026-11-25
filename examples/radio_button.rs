// SPEC-HAL-130/131 + SPEC-HAL-208: RadioButton Demo
// Demonstrates RadioGroup widget with integrated labels and selection callback.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::radio_group::RadioId;
use halcyon::widgets::radio_group_widget::{RadioGroup, RadioGroupId};
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== RadioButton Demo ===");
    println!("Features:");
    println!("  - Click radio button or label to select");
    println!("  - Tab to focus group, Up/Down to navigate");
    println!("  - Space/Enter to select when focused");
    println!("  - Only one selection per group");
    println!("  - Watch console for selection callbacks");
    println!("Close the window to exit");
    println!("========================\n");

    let app = App::new(AppConfig::new("RadioButton Demo").with_size(400.0, 600.0));
    app.run(RadioButtonDemoScreen::new());
}

struct RadioButtonDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    selection_count: Rc<RefCell<u32>>,
}

impl RadioButtonDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            selection_count: Rc::new(RefCell::new(0)),
        }
    }
}

impl Screen for RadioButtonDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: vertical Flex column
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("RadioButton Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Section: Color selection
        cx.tree
            .add_child(root, Box::new(Label::new("Select a color:")), None, 3);

        // SPEC-HAL-208: RadioGroup with integrated labels — no parallel label columns
        let selection_count = self.selection_count.clone();
        let color_group = RadioGroup::with_labeled_options(
            RadioGroupId(1),
            &[
                (RadioId(1), "Red"),
                (RadioId(2), "Green"),
                (RadioId(3), "Blue"),
            ],
        )
        .on_selection_changed(move |group_id, selected| {
            let mut count = selection_count.borrow_mut();
            *count += 1;
            let color_name = match selected {
                Some(RadioId(1)) => "Red",
                Some(RadioId(2)) => "Green",
                Some(RadioId(3)) => "Blue",
                _ => "None",
            };
            println!(
                "[CALLBACK] Group {:?}: selected={} ({}), total_selections={}",
                group_id,
                color_name,
                selected.map(|r| r.0).unwrap_or(0),
                *count
            );
        });
        cx.tree.add_child(root, Box::new(color_group), None, 4);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 5);

        // Section: Size selection (with initial selection)
        cx.tree
            .add_child(root, Box::new(Label::new("Select a size:")), None, 6);

        let selection_count2 = self.selection_count.clone();
        let mut size_group = RadioGroup::with_labeled_options(
            RadioGroupId(2),
            &[
                (RadioId(10), "Small"),
                (RadioId(20), "Medium (default)"),
                (RadioId(30), "Large"),
            ],
        )
        .on_selection_changed(move |group_id, selected| {
            let mut count = selection_count2.borrow_mut();
            *count += 1;
            let size_name = match selected {
                Some(RadioId(10)) => "Small",
                Some(RadioId(20)) => "Medium",
                Some(RadioId(30)) => "Large",
                _ => "None",
            };
            println!(
                "[CALLBACK] Group {:?}: selected={} ({}), total_selections={}",
                group_id,
                size_name,
                selected.map(|r| r.0).unwrap_or(0),
                *count
            );
        });
        // Set initial selection
        size_group.set_selected(Some(RadioId(20)));
        cx.tree.add_child(root, Box::new(size_group), None, 7);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 8);

        // Section: Disabled group
        cx.tree
            .add_child(root, Box::new(Label::new("Disabled group:")), None, 9);

        let mut disabled_group = RadioGroup::with_labeled_options(
            RadioGroupId(3),
            &[
                (RadioId(100), "Option A (locked)"),
                (RadioId(200), "Option B"),
            ],
        );
        disabled_group.set_selected(Some(RadioId(100)));
        disabled_group.set_disabled(true);
        cx.tree.add_child(root, Box::new(disabled_group), None, 10);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 11);

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Click or Tab+Up/Down+Space to select | Close the window to exit",
            )),
            None,
            12,
        );

        // Bottom padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 13);

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
