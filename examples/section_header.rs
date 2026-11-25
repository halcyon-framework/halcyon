// SPEC-HAL-214: SectionHeader Demo
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::section_header::SectionHeader;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== SectionHeader Demo ===");
    println!("Close the window to exit");
    println!("==========================\n");

    let app = App::new(AppConfig::new("SectionHeader Demo").with_size(500.0, 500.0));
    app.run(SectionHeaderDemoScreen::new());
}

struct SectionHeaderDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl SectionHeaderDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for SectionHeaderDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree
            .add_child(root, Box::new(Label::new("SectionHeader Demo")), None, 1);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 2);

        // Section 1: Account Settings
        cx.tree.add_child(
            root,
            Box::new(SectionHeader::new("Account Settings")),
            None,
            3,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 4);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 5);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Content belonging to this section...")),
            None,
            7,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 8);

        // Section 2: Notifications (with secondary text)
        cx.tree.add_child(
            root,
            Box::new(
                SectionHeader::new("Notifications")
                    .with_secondary("Configure how you receive alerts"),
            ),
            None,
            9,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 10);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 11);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 12);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Notification options would go here...")),
            None,
            13,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 14);

        // Section 3: Privacy (with secondary text)
        cx.tree.add_child(
            root,
            Box::new(
                SectionHeader::new("Privacy").with_secondary("Manage your data and visibility"),
            ),
            None,
            15,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 16);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 17);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 18);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Privacy controls would go here...")),
            None,
            19,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 20);

        // Section 4: Danger Zone
        cx.tree
            .add_child(root, Box::new(SectionHeader::new("Danger Zone")), None, 21);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 22);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 23);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 24);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Destructive actions would go here...")),
            None,
            25,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 26);

        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Headers are inert — click/tab does nothing | Close the window to exit",
            )),
            None,
            27,
        );

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
