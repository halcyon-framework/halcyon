// SPEC-HAL-211: ProgressBar Demo
// Demonstrates determinate progress bar with external value updates,
// optional labels, and disabled state.
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::progress_bar::ProgressBar;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== ProgressBar Demo ===");
    println!("Features:");
    println!("  - External value updates (auto-advancing)");
    println!("  - Optional labels");
    println!("  - Disabled visual state");
    println!("Close the window to exit");
    println!("========================\n");

    let app = App::new(AppConfig::new("ProgressBar Demo").with_size(500.0, 400.0));
    app.run(ProgressBarDemoScreen::new());
}

struct ProgressBarDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    /// Simulated progress value, advanced each frame.
    progress: f32,
}

impl ProgressBarDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            progress: 0.0,
        }
    }
}

impl Screen for ProgressBarDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Advance progress externally (application layer drives value)
        self.progress += 0.002;
        if self.progress > 1.0 {
            self.progress = 0.0;
        }

        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree
            .add_child(root, Box::new(Label::new("ProgressBar Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        // Content column
        let content_col =
            cx.tree
                .add_child(root, Box::new(Flex::column().with_spacing(12.0)), None, 3);

        // ProgressBar 1: auto-advancing with label
        let pb1 = ProgressBar::new()
            .with_value(self.progress)
            .with_label("Downloading...");
        cx.tree.add_child(content_col, Box::new(pb1), None, 0);

        // ProgressBar 2: fixed at 75%
        let pb2 = ProgressBar::new()
            .with_value(0.75)
            .with_label("Build progress");
        cx.tree.add_child(content_col, Box::new(pb2), None, 1);

        // ProgressBar 3: no label
        let pb3 = ProgressBar::new().with_value(self.progress * 0.5);
        cx.tree.add_child(content_col, Box::new(pb3), None, 2);

        // ProgressBar 4: disabled
        let mut pb4 = ProgressBar::new()
            .with_value(0.4)
            .with_label("Paused (disabled)");
        pb4.set_disabled(true);
        cx.tree.add_child(content_col, Box::new(pb4), None, 3);

        // Spacer
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 4);

        // Help text
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 5);

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
