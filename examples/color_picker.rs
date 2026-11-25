// SPEC-HAL-272: ColorPicker Demo
// Demonstrates the interactive HSV color picker widget.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::color_picker::ColorPicker;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Color Picker Demo ===");
    println!("Features:");
    println!("  - Click/drag SV area to pick saturation/value");
    println!("  - Click/drag hue strip to pick hue");
    println!("  - Swatch and hex label update in real time");
    println!("==========================\n");

    let app = App::new(AppConfig::new("Color Picker Demo").with_size(400.0, 350.0));
    app.run(ColorPickerDemoScreen::new());
}

struct ColorPickerDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl ColorPickerDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for ColorPickerDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 0);

        // Center row for the picker
        let center_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 1);
        cx.tree
            .add_child(center_row, Box::new(FlexSpacer::new()), None, 0);
        cx.tree
            .add_child(center_row, Box::new(ColorPicker::new()), None, 1);
        cx.tree
            .add_child(center_row, Box::new(FlexSpacer::new()), None, 2);

        // Swatch row
        let swatch_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 2);
        cx.tree
            .add_child(swatch_row, Box::new(FlexSpacer::new()), None, 0);
        cx.tree.add_child(
            swatch_row,
            Box::new(Label::new("Selected color swatch appears in SV area")),
            None,
            1,
        );
        cx.tree
            .add_child(swatch_row, Box::new(FlexSpacer::new()), None, 2);

        // Help text
        let help_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 3);
        cx.tree
            .add_child(help_row, Box::new(FlexSpacer::new()), None, 0);
        cx.tree.add_child(
            help_row,
            Box::new(Label::new("Drag SV area or hue strip to change color")),
            None,
            1,
        );
        cx.tree
            .add_child(help_row, Box::new(FlexSpacer::new()), None, 2);

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        if let FusedInputEvent::KeyDown { key_code, .. } = event {
            if matches!(key_code, KeyCode::Escape) {
                // ESC ignored per demo law
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
