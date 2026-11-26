// =============================================================================
// Timeline Strip Demo (SPEC-HAL-334 / 335 / 336)
// =============================================================================

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::widgets::label::Label;

#[path = "temporal_demos/strip.rs"]
mod strip;
use strip::{TimelineStripDemo, WINDOW_WIDTH};

const WINDOW_HEIGHT: f32 = 420.0;

struct TimelineStripScreen {
    demo: TimelineStripDemo,
    placeholder: Label,
    built: bool,
}

impl TimelineStripScreen {
    fn new() -> Self {
        Self {
            demo: TimelineStripDemo::new(),
            placeholder: Label::new(""),
            built: false,
        }
    }
}

impl Screen for TimelineStripScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if !self.built {
            let parent = cx.tree.root().expect("Tree must have root");
            self.demo.mount(cx.tree, parent);
            self.built = true;
        }
        &mut self.placeholder
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        self.demo.update(tree);
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== Timeline Strip Demo (SPEC-HAL-334/335/336) ===");
        println!();
        println!("Interaction:");
        println!("  Ctrl+wheel  zoom about the cursor");
        println!("  Shift+wheel scroll horizontally");
        println!("  Click       select (accent outline); Cmd/Ctrl+click toggles");
        println!("  Click empty clear selection");
        println!("  Right-click ContextMenu intent (logged)");
        println!("  Double-click Activate intent (logged)");
        println!();
        println!("Close window to exit.");
        println!("==================================================");
    }

    fn mark_rebuild(&mut self) {}

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(
        AppConfig::new("Timeline Strip Demo (SPEC-HAL-336)").with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(TimelineStripScreen::new());
}
