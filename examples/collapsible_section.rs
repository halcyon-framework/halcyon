// SPEC-HAL-215: CollapsibleSection Demo
// SPEC-HAL-018: Uses App + Screen pattern

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::widget::Widget;
use halcyon::model::collapsible_section::CollapsibleSection;
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::section_header::SectionHeader;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== CollapsibleSection Demo ===");
    println!("Press Space to toggle | Close the window to exit");
    println!("================================\n");

    let app = App::new(AppConfig::new("CollapsibleSection Demo").with_size(500.0, 500.0));
    app.run(CollapsibleSectionDemoScreen::new());
}

struct CollapsibleSectionDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    controller: CollapsibleSection,
}

impl CollapsibleSectionDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            controller: CollapsibleSection::new(true),
        }
    }
}

impl Screen for CollapsibleSectionDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree.add_child(
            root,
            Box::new(Label::new("CollapsibleSection Demo")),
            None,
            1,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 2);

        // Header (always present)
        cx.tree
            .add_child(root, Box::new(SectionHeader::new("Settings")), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 4);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 5);

        // Content (only added when expanded)
        if self.controller.is_expanded() {
            cx.tree
                .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);
            cx.tree.add_child(
                root,
                Box::new(Label::new("This content is visible when expanded.")),
                None,
                7,
            );
            cx.tree.add_child(
                root,
                Box::new(Label::new("It is removed from layout when collapsed.")),
                None,
                8,
            );
            cx.tree.add_child(
                root,
                Box::new(Label::new("No height hack — nodes are simply not added.")),
                None,
                9,
            );
        }

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 10);

        let status = if self.controller.is_expanded() {
            "Status: EXPANDED | Press Space to collapse"
        } else {
            "Status: COLLAPSED | Press Space to expand"
        };
        cx.tree
            .add_child(root, Box::new(Label::new(status)), None, 11);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 12);
        cx.tree
            .add_child(root, Box::new(Label::new("Close the window to exit")), None, 13);

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::KeyDown {
                key_code: KeyCode::Space,
                ..
            } => {
                self.controller.toggle();
                self.needs_rebuild = true;
                true
            }

            _ => false,
        }
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
