// SPEC-HAL-025: TextBlock Multi-line Text Showcase
// Demonstrates text wrapping, paragraphs, and alignment
// SPEC-HAL-018: Migrated to App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();

    let app = App::new(AppConfig::new("TextBlock Showcase").with_size(800.0, 600.0));
    app.run(TextBlockShowcaseScreen::new());
}

struct TextBlockShowcaseScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl TextBlockShowcaseScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for TextBlockShowcaseScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: vertical Flex column
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(20.0)));

        // Example 1: Basic wrapping text
        cx.tree.add_child(
            root,
            Box::new(
                TextBlock::new(
                    "This is a long piece of text that should wrap automatically when it reaches the edge of its container. \
                     The TextBlock widget handles line breaking and respects word boundaries.",
                )
                .with_size(26.0)
                .with_color(Color::WHITE)
                .with_align(TextAlign::Left),
            ),
            None,
            0,
        );

        // Example 2: Multiple paragraphs
        cx.tree.add_child(
            root,
            Box::new(
                TextBlock::new(
                    "First paragraph with some content.\n\n\
                     Second paragraph starts here after a blank line.\n\n\
                     Third paragraph demonstrates hard line breaks.",
                )
                .with_size(21.0)
                .with_color(Color::from_hex(0x5bc0ff))
                .with_align(TextAlign::Left),
            ),
            None,
            0,
        );

        // Example 3: Center aligned
        cx.tree.add_child(
            root,
            Box::new(
                TextBlock::new(
                    "Center aligned text\nwith multiple lines\nthat should be centered.",
                )
                .with_size(24.0)
                .with_color(Color::from_hex(0xff9f43))
                .with_align(TextAlign::Center),
            ),
            None,
            0,
        );

        // Example 4: Right aligned
        cx.tree.add_child(
            root,
            Box::new(
                TextBlock::new("Right aligned text\naligned to the right edge.")
                    .with_size(22.0)
                    .with_color(Color::from_hex(0x10ac84))
                    .with_align(TextAlign::Right),
            ),
            None,
            0,
        );

        // Example 5: No wrapping
        cx.tree.add_child(
            root,
            Box::new(
                TextBlock::new(
                    "This text has wrapping disabled. It will extend beyond its container if necessary.",
                )
                .with_size(16.0)
                .with_color(Color::from_hex(0xee5a24))
                .with_wrap(false),
            ),
            None,
            0,
        );

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
