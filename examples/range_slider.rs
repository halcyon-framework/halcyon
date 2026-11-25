// SPEC-HAL-275: RangeSlider Demo

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::paint::TrackDisplayStyle;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::range_slider::RangeSlider;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== RangeSlider Demo ===");
    println!("Features:");
    println!("  - Drag handles to select a range");
    println!("  - Keyboard: arrows adjust focused handle, Tab switches handles");
    println!("  - Magnetic snap: handles stick to anchor points (25/50/75)");
    println!("  - Stepped slider snaps to integer boundaries");
    println!("  - Bar style: center-drag slides entire range");
    println!("=============================\n");

    let app = App::new(AppConfig::new("RangeSlider Demo").with_size(500.0, 300.0));
    app.run(RangeSliderDemoScreen::new());
}

struct RangeSliderDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl RangeSliderDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for RangeSliderDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 0);

        // Slider 1: Price range (continuous)
        cx.tree.add_child(
            root,
            Box::new(
                RangeSlider::new(0.0, 100.0, 20.0, 80.0)
                    .with_label("Price range")
                    .with_anchors(vec![25.0, 50.0, 75.0])
                    .on_change(|rv| {
                        println!(
                            "[CALLBACK] Price range: low={:.1}, high={:.1}",
                            rv.low, rv.high
                        );
                    }),
            ),
            None,
            1,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Slider 2: Rating (stepped)
        cx.tree.add_child(
            root,
            Box::new(
                RangeSlider::new(0.0, 10.0, 3.0, 7.0)
                    .with_step(1.0)
                    .with_label("Rating")
                    .on_change(|rv| {
                        println!("[CALLBACK] Rating: low={:.0}, high={:.0}", rv.low, rv.high);
                    }),
            ),
            None,
            3,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 4);

        // Slider 3: Bar style (chart viewport selector)
        cx.tree.add_child(
            root,
            Box::new(
                RangeSlider::new(0.0, 100.0, 30.0, 70.0)
                    .with_display_style(TrackDisplayStyle::Bar)
                    .with_label("Viewport")
                    .with_anchors(vec![50.0])
                    .on_change(|rv| {
                        println!(
                            "[CALLBACK] Viewport: low={:.1}, high={:.1}",
                            rv.low, rv.high
                        );
                    }),
            ),
            None,
            5,
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
