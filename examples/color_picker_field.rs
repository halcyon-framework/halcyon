// SPEC-HAL-272: ColorPickerField Demo
// Demonstrates the compact swatch trigger with popup color picker.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::color_picker_field::ColorPickerField;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Color Picker Field Demo ===");
    println!("Features:");
    println!("  - Click swatch to open color picker popup");
    println!("  - Click/drag SV area and hue strip inside popup");
    println!("  - Click outside popup to dismiss");
    println!("  - Escape closes popup");
    println!("================================\n");

    let app = App::new(AppConfig::new("Color Picker Field Demo").with_size(500.0, 400.0));
    app.run(ColorPickerFieldDemoScreen::new());
}

struct ColorPickerFieldDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl ColorPickerFieldDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for ColorPickerFieldDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(40.0)), None, 0);

        // Row with color picker field
        let field_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 1);
        cx.tree
            .add_child(field_row, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(field_row, Box::new(Label::new("Color:")), None, 1);
        cx.tree
            .add_child(field_row, Box::new(ColorPickerField::new()), None, 2);

        // Help text
        let help_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 2);
        cx.tree
            .add_child(help_row, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree.add_child(
            help_row,
            Box::new(Label::new("Click the swatch to open the color picker")),
            None,
            1,
        );

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
