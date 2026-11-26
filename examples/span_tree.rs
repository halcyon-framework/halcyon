// =============================================================================
// SpanTree Synthetic Trace — drill-in (SPEC-HAL-341)
// =============================================================================

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::widgets::label::Label;

#[path = "temporal_demos/span_tree.rs"]
mod span_tree;
use span_tree::{SpanTreeDemo, NUM_RUNS, WINDOW_WIDTH};

const WINDOW_HEIGHT: f32 = 640.0;

struct SpanTreeScreen {
    demo: SpanTreeDemo,
    placeholder: Label,
    built: bool,
}

impl SpanTreeScreen {
    fn new() -> Self {
        Self {
            demo: SpanTreeDemo::new(),
            placeholder: Label::new(""),
            built: false,
        }
    }
}

impl Screen for SpanTreeScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if !self.built {
            let parent = cx.parent_node;
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
        println!("=== SpanTree Synthetic Trace (SPEC-HAL-341) ===");
        println!();
        println!(
            "{} pipeline runs at the top level, lane-packed. Each run nests",
            NUM_RUNS
        );
        println!("stages, parallel crate builds, test shards, and deploy regions —");
        println!("more than 3,000 nested spans in total.");
        println!();
        println!("  Double-click   enter a span");
        println!("  Wheel          zoom (zoom out past a span pops up a level)");
        println!("  Shift+wheel    pan the time axis");
        println!();
        println!("Close window to exit.");
        println!("===============================================");
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
        AppConfig::new("SpanTree Synthetic Trace (SPEC-HAL-341)")
            .with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(SpanTreeScreen::new());
}
