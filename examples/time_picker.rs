// SPEC-HAL-273: TimePicker Demo
// Demonstrates standalone TimePicker with 24h, 12h, seconds, and working hours modes.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::time_picker::{TimeDisplayMode, TimeDomain, TimePicker, TimeValue};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== TimePicker Demo ===");
    println!("Features:");
    println!("  - Enter to edit, Escape to cancel, hjkl for vim navigation");
    println!("  - Up/Down adjusts focused column value with wrapping");
    println!("  - Left/Right navigates between columns");
    println!("  - Tab advances, Shift+Tab reverses");
    println!("  - Enter commits value");
    println!("  - Escape resets working value");
    println!("  - Scroll wheel adjusts column under cursor");
    println!("  - Digit typing for direct entry (e.g., 1 then 4 sets hour to 14)");
    println!("========================\n");

    let app = App::new(AppConfig::new("TimePicker Demo").with_size(400.0, 300.0));
    app.run(TimePickerDemoScreen::new());
}

struct TimePickerDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl TimePickerDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for TimePickerDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(40.0)), None, 0);

        // 24h mode picker
        let row_24h = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 1);
        cx.tree
            .add_child(row_24h, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row_24h, Box::new(Label::new("Time (24h):")), None, 1);
        cx.tree.add_child(
            row_24h,
            Box::new(TimePicker::new(TimeValue::new(14, 30, None))),
            None,
            2,
        );

        // 12h mode picker
        let row_12h = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 2);
        cx.tree
            .add_child(row_12h, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row_12h, Box::new(Label::new("Time (12h):")), None, 1);
        cx.tree.add_child(
            row_12h,
            Box::new(
                TimePicker::new(TimeValue::new(9, 15, None))
                    .with_display_mode(TimeDisplayMode::H12),
            ),
            None,
            2,
        );

        // 24h with seconds picker
        let row_sec = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 3);
        cx.tree
            .add_child(row_sec, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row_sec, Box::new(Label::new("Time (24h+s):")), None, 1);
        cx.tree.add_child(
            row_sec,
            Box::new(TimePicker::new(TimeValue::new(8, 45, Some(30))).with_seconds(true)),
            None,
            2,
        );

        // Working hours picker (09:00-17:00, 15-min steps)
        let row_work = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 4);
        cx.tree
            .add_child(row_work, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree.add_child(
            row_work,
            Box::new(Label::new("Working hours (15m step):")),
            None,
            1,
        );
        cx.tree.add_child(
            row_work,
            Box::new(
                TimePicker::new(TimeValue::new(10, 30, None))
                    .with_domain(TimeDomain::working_hours())
                    .with_step_minutes(15),
            ),
            None,
            2,
        );

        // Help text
        let help_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 5);
        cx.tree
            .add_child(help_row, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree.add_child(
            help_row,
            Box::new(Label::new(
                "Click a picker, then use arrows/digits to adjust",
            )),
            None,
            1,
        );

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        if let FusedInputEvent::KeyDown { key_code, .. } = event {
            if matches!(key_code, KeyCode::Escape) {
                return false;
            }
        }
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
