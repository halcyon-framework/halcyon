// SPEC-HAL-308: Sparkline Demo
// Demonstrates miniature inline charts with filled area rendering.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::sparkline::{Sparkline, SparklineConfig};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("Sparkline Demo").with_size(400.0, 300.0));
    app.run(SparklineDemoScreen::new());
}

struct SparklineDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl SparklineDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for SparklineDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        cx.tree
            .add_child(root, Box::new(Label::new("Sparkline Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Row 1: Revenue (green)
        let row1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 3);
        cx.tree
            .add_child(row1, Box::new(Label::new("Revenue")), None, 0);
        cx.tree.add_child(
            row1,
            Box::new(
                Sparkline::new()
                    .with_config(SparklineConfig {
                        width: 80.0,
                        height: 20.0,
                        color: Some(Color::from_hex(0x2ECC71)),
                        fill: true,
                        live: false,
                    })
                    .with_data(vec![10.0, 15.0, 12.0, 18.0, 22.0, 19.0, 25.0, 30.0]),
            ),
            None,
            1,
        );

        // Row 2: Latency (red)
        let row2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 4);
        cx.tree
            .add_child(row2, Box::new(Label::new("Latency")), None, 0);
        cx.tree.add_child(
            row2,
            Box::new(
                Sparkline::new()
                    .with_config(SparklineConfig {
                        width: 80.0,
                        height: 20.0,
                        color: Some(Color::from_hex(0xE74C3C)),
                        fill: true,
                        live: false,
                    })
                    .with_data(vec![5.0, 8.0, 3.0, 12.0, 6.0, 4.0, 9.0, 2.0]),
            ),
            None,
            1,
        );

        // Row 3: Default color (theme accent)
        let row3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 5);
        cx.tree
            .add_child(row3, Box::new(Label::new("Default color")), None, 0);
        cx.tree.add_child(
            row3,
            Box::new(Sparkline::new().with_data(vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0])),
            None,
            1,
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
