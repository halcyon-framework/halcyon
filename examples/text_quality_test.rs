// Text Quality Test — Visual debugging for font rendering
// Renders 50 lines of pangram text from size 6 to 55
// SPEC-HAL-018: Migrated to App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::scroll_area::ScrollArea;

fn main() {
    tracing_subscriber::fmt::init();

    let app = App::new(AppConfig::new("Text Quality Test").with_size(1200.0, 1600.0));
    app.run(TextQualityScreen::new());
}

struct TextQualityScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl TextQualityScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for TextQualityScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let scroll = cx.mount_child(Box::new(
            ScrollArea::new_unwired().enable_horizontal_scrollbar(true),
        ));
        let samples =
            cx.tree
                .add_child(scroll, Box::new(Flex::column().with_spacing(20.0)), None, 0);
        cx.tree
            .node_mut(scroll)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(samples);

        // Create text blocks, sizes 6 through 55
        for size in 6..=55 {
            let text = format!(
                "The quick brown fox jumps over the lazy dog — SIZE: {}",
                size
            );
            // Measure the unwrapped sample at its natural width for horizontal scrolling.
            let row = cx.tree.add_child(samples, Box::new(Flex::row()), None, 0);
            let sample = cx.tree.add_child(
                row,
                Box::new(
                    TextBlock::new(&text)
                        .with_size(size as f32)
                        .with_align(TextAlign::Left)
                        .with_wrap(false),
                ),
                None,
                0,
            );
            cx.tree.set_flex(
                sample,
                FlexParams {
                    shrink: 0.0,
                    ..FlexParams::default()
                },
            );
        }

        &mut self.placeholder
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
