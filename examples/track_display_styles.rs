// SPEC-HAL-278: Track Display Styles Demo — Track vs Bar vs Pill vs Radial

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::paint::TrackDisplayStyle;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::busy_indicator::BusyIndicator;
use halcyon::widgets::progress_bar::ProgressBar;
use halcyon::widgets::slider::Slider;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Track Display Styles Demo ===");
    println!("Comparing Track vs Bar vs Radial display styles:");
    println!("  - Slider (Track + Bar + Radial)");
    println!("  - ProgressBar (Track + Bar + Pill + Radial)");
    println!("  - BusyIndicator (Track + Bar + Pill + Radial)");
    println!("==================================\n");

    let app = App::new(AppConfig::new("Track Display Styles Demo").with_size(500.0, 650.0));
    app.run(BarStyleDemoScreen::new());
}

struct BarStyleDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl BarStyleDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for BarStyleDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 0);

        // Slider: Track style
        cx.tree.add_child(
            root,
            Box::new(
                Slider::new(0.0, 100.0)
                    .with_value(40.0)
                    .with_label("Slider (Track)")
                    .with_value_display(),
            ),
            None,
            1,
        );

        // Slider: Bar style
        cx.tree.add_child(
            root,
            Box::new(
                Slider::new(0.0, 100.0)
                    .with_value(40.0)
                    .with_display_style(TrackDisplayStyle::Bar)
                    .with_label("Slider (Bar)")
                    .with_value_display(),
            ),
            None,
            2,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 3);

        // ProgressBar: Track style
        cx.tree.add_child(
            root,
            Box::new(
                ProgressBar::new()
                    .with_value(0.6)
                    .with_label("Progress (Track)"),
            ),
            None,
            4,
        );

        // ProgressBar: Bar style
        cx.tree.add_child(
            root,
            Box::new(
                ProgressBar::new()
                    .with_value(0.6)
                    .with_display_style(TrackDisplayStyle::Bar)
                    .with_label("Progress (Bar)"),
            ),
            None,
            5,
        );

        // ProgressBar: Pill style
        cx.tree.add_child(
            root,
            Box::new(
                ProgressBar::new()
                    .with_value(0.6)
                    .with_display_style(TrackDisplayStyle::Pill)
                    .with_label("Progress (Pill)"),
            ),
            None,
            6,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 7);

        // BusyIndicator: Track style
        cx.tree.add_child(
            root,
            Box::new(BusyIndicator::new().with_label("Busy (Track)")),
            None,
            8,
        );

        // BusyIndicator: Bar style
        cx.tree.add_child(
            root,
            Box::new(
                BusyIndicator::new()
                    .with_display_style(TrackDisplayStyle::Bar)
                    .with_label("Busy (Bar)"),
            ),
            None,
            9,
        );

        // BusyIndicator: Pill style
        cx.tree.add_child(
            root,
            Box::new(
                BusyIndicator::new()
                    .with_display_style(TrackDisplayStyle::Pill)
                    .with_label("Busy (Pill)"),
            ),
            None,
            10,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 11);

        // Radial style
        cx.tree.add_child(
            root,
            Box::new(
                Slider::new(0.0, 100.0)
                    .with_value(40.0)
                    .with_display_style(TrackDisplayStyle::Radial)
                    .with_label("Slider (Radial)")
                    .with_value_display(),
            ),
            None,
            12,
        );

        cx.tree.add_child(
            root,
            Box::new(
                ProgressBar::new()
                    .with_value(0.6)
                    .with_display_style(TrackDisplayStyle::Radial)
                    .with_label("Progress (Radial)"),
            ),
            None,
            13,
        );

        cx.tree.add_child(
            root,
            Box::new(
                BusyIndicator::new()
                    .with_display_style(TrackDisplayStyle::Radial)
                    .with_label("Busy (Radial)"),
            ),
            None,
            14,
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
