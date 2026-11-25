// SPEC-HAL-226A: Tooltip Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - Hover-based tooltips with framework-managed delay (500ms)
// - Instant dismiss on pointer leave
// - Reshow shortcut (quick hover across adjacent controls)
// - Dismiss on click and Escape
// - Auto-placement (below by default)

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{Event, EventPhase};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::tooltip::TooltipPayload;
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::divider::Divider;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Tooltip Demo (SPEC-HAL-226A) ===");
    println!("Hover over labels to see tooltips");
    println!("Click or press Escape to dismiss");
    println!("===================================\n");

    let app = App::new(AppConfig::new("Tooltip Demo (SPEC-HAL-226A)").with_size(500.0, 300.0));
    app.run(TooltipDemoScreen::new());
}

/// A label that registers a tooltip trigger during paint.
struct TooltipLabel {
    id: WidgetInternalId,
    text: String,
    tooltip_text: String,
}

impl TooltipLabel {
    fn new(text: &str, tooltip_text: &str) -> Self {
        Self {
            id: WidgetInternalId::new(),
            text: text.to_string(),
            tooltip_text: tooltip_text.to_string(),
        }
    }
}

impl WidgetDowncast for TooltipLabel {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TooltipLabel {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(&self.text, font_size);
        let w = run.width.min(constraints.max_width);
        let h = run.height.min(constraints.max_height);
        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(&self.text, font_size);
        let y = rect.y + (rect.height - run.height) / 2.0;
        cx.text_engine.draw(
            &run,
            rect.x,
            y,
            cx.theme.colors.typography.primary,
            halcyon::core::paint::LAYER_CONTENT,
            cx.scene,
        );

        // Register tooltip trigger for this label's bounds
        cx.register_tooltip(rect, TooltipPayload::Text(self.tooltip_text.clone()));
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

struct TooltipDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl TooltipDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for TooltipDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Tooltip Demo (SPEC-HAL-226A)")),
            None,
            1,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        // Labels with tooltips
        cx.tree.add_child(
            root,
            Box::new(TooltipLabel::new(
                "Hover me for tooltip #1",
                "This is the first tooltip — appears after 500ms delay",
            )),
            None,
            5,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        cx.tree.add_child(
            root,
            Box::new(TooltipLabel::new(
                "Hover me for tooltip #2",
                "Second tooltip — move quickly from #1 for instant reshow",
            )),
            None,
            7,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 8);

        cx.tree.add_child(
            root,
            Box::new(TooltipLabel::new(
                "Hover me for tooltip #3",
                "Third tooltip — click anywhere or press Escape to dismiss",
            )),
            None,
            9,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 10);

        cx.tree.add_child(
            root,
            Box::new(Label::new("(Regular label — no tooltip)")),
            None,
            11,
        );

        &mut self.placeholder
    }

    fn handle_input(
        &mut self,
        _event: &halcyon::core::event::FusedInputEvent,
        _cx: &mut ScreenContext<'_>,
    ) -> bool {
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
