// =============================================================================
// Bound Timeline Composition (SPEC-HAL-337 §8.6 / SPEC-HAL-336 §8)
// =============================================================================

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::widgets::label::Label;

#[path = "temporal_demos/bound.rs"]
mod bound;
use bound::{BoundTimelineDemo, WINDOW_WIDTH};

const WINDOW_HEIGHT: f32 = 640.0;

struct BoundTimelineScreen {
    demo: BoundTimelineDemo,
    placeholder: Label,
    built: bool,
}

impl BoundTimelineScreen {
    fn new() -> Self {
        Self {
            demo: BoundTimelineDemo::new(),
            placeholder: Label::new(""),
            built: false,
        }
    }
}

impl Screen for BoundTimelineScreen {
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

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        self.demo.handle_input(event)
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== Bound Timeline Composition (SPEC-HAL-337 §8.6) ===");
        println!();
        println!("Left: TreeView over SharedHts.  Right: TimelineHeader + TimelineCanvas.");
        println!("Row geometry crosses via hts_rows once per frame (FORBID-337-2).");
        println!();
        println!("Verify:");
        println!("  Wheel              dy scrolls rows, dx pans time (no modifiers)");
        println!("  Cmd/Ctrl+wheel     zoom about cursor (up = in)");
        println!("  Shift+wheel        horizontal time pan");
        println!("  Space + drag       hand tool, both axes");
        println!("  Click bar          tree row selects (Ctrl toggles, Shift extends)");
        println!("  Click empty        tree selection clears; DeselectAll logged");
        println!("  Drag splitter      both panes stay coherent");
        println!();
        println!("Close window to exit.");
        println!("=======================================================");
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
        AppConfig::new("Bound Timeline (SPEC-HAL-337 §8.6)").with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(BoundTimelineScreen::new());
}
