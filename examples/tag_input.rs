// SPEC-HAL-276: TagInput Demo
// Demonstrates TagInput with Full, CountBadge, and ToggleOnly modes.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::tag_input::{
    TagCreationMode, TagDisplayMode, TagInput, TagInputPolicy, TagItem, TagTriggerKeys,
};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== TagInput Demo ===");
    println!("Click to open popup. Space toggles. Enter commits.");
    println!("=====================\n");

    let app = App::new(AppConfig::new("TagInput Demo").with_size(500.0, 400.0));
    app.run(TagInputDemoScreen::new());
}

fn language_items() -> Vec<TagItem> {
    vec![
        TagItem {
            id: 1,
            label: "Rust".into(),
            group: Some("Systems".into()),
            enabled: true,
        },
        TagItem {
            id: 2,
            label: "C++".into(),
            group: Some("Systems".into()),
            enabled: true,
        },
        TagItem {
            id: 3,
            label: "Go".into(),
            group: Some("Systems".into()),
            enabled: true,
        },
        TagItem {
            id: 4,
            label: "Python".into(),
            group: Some("Scripting".into()),
            enabled: true,
        },
        TagItem {
            id: 5,
            label: "TypeScript".into(),
            group: Some("Scripting".into()),
            enabled: true,
        },
        TagItem {
            id: 6,
            label: "Ruby".into(),
            group: Some("Scripting".into()),
            enabled: true,
        },
        TagItem {
            id: 7,
            label: "Java".into(),
            group: Some("JVM".into()),
            enabled: true,
        },
        TagItem {
            id: 8,
            label: "Kotlin".into(),
            group: Some("JVM".into()),
            enabled: true,
        },
        TagItem {
            id: 9,
            label: "C#".into(),
            group: Some(".NET".into()),
            enabled: true,
        },
        TagItem {
            id: 10,
            label: "F#".into(),
            group: Some(".NET".into()),
            enabled: true,
        },
        TagItem {
            id: 11,
            label: "Swift".into(),
            group: Some("Apple".into()),
            enabled: true,
        },
        TagItem {
            id: 12,
            label: "COBOL".into(),
            group: Some("Legacy".into()),
            enabled: false,
        },
    ]
}

fn priority_items() -> Vec<TagItem> {
    let names = ["Low", "Medium", "High", "Critical", "Blocker"];
    names
        .iter()
        .enumerate()
        .map(|(i, name)| TagItem {
            id: 100 + i as u64,
            label: name.to_string(),
            group: None,
            enabled: true,
        })
        .collect()
}

struct TagInputDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl TagInputDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for TagInputDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 0);

        // 1. Full policy — Chips mode
        let row1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 1);
        cx.tree
            .add_child(row1, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row1, Box::new(Label::new("Languages:")), None, 1);
        cx.tree.add_child(
            row1,
            Box::new(
                TagInput::new()
                    .with_items(language_items())
                    .with_selection(&[1, 3, 5])
                    .with_placeholder("Select languages...")
                    .on_change(|entries| {
                        tracing::info!("Languages changed: {:?}", entries);
                    }),
            ),
            None,
            2,
        );

        // 2. Count badge mode
        let row2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 2);
        cx.tree
            .add_child(row2, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row2, Box::new(Label::new("Count badge:")), None, 1);
        cx.tree.add_child(
            row2,
            Box::new(
                TagInput::new()
                    .with_items(language_items())
                    .with_selection(&[2, 4, 6, 8])
                    .with_display_mode(TagDisplayMode::CountBadge)
                    .with_placeholder("Select languages..."),
            ),
            None,
            2,
        );

        // 3. ToggleOnly policy
        let row3 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 3);
        cx.tree
            .add_child(row3, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row3, Box::new(Label::new("Priority:")), None, 1);
        cx.tree.add_child(
            row3,
            Box::new(
                TagInput::new()
                    .with_items(priority_items())
                    .with_policy(TagInputPolicy::ToggleOnly)
                    .with_placeholder("Select priority..."),
            ),
            None,
            2,
        );

        // 4. FreeFormOnly — inline text input, no popup
        let row4 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 4);
        cx.tree
            .add_child(row4, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row4, Box::new(Label::new("Free-form:")), None, 1);
        cx.tree.add_child(
            row4,
            Box::new(
                TagInput::new()
                    .with_tag_creation(TagCreationMode::FreeFormOnly)
                    .with_placeholder("Type and press Enter..."),
            ),
            None,
            2,
        );

        // 5. Hybrid — inline text for custom tags + Down opens item popup
        let row5 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 5);
        cx.tree
            .add_child(row5, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree.add_child(
            row5,
            Box::new(Label::new("Hybrid (type + browse):")),
            None,
            1,
        );
        cx.tree.add_child(
            row5,
            Box::new(
                TagInput::new()
                    .with_items(language_items())
                    .with_tag_creation(TagCreationMode::Hybrid)
                    .with_trigger_keys(TagTriggerKeys {
                        enter: true,
                        comma: true,
                        tab: false,
                    })
                    .with_placeholder("Type tag or Down for list..."),
            ),
            None,
            2,
        );

        // Help text
        let help_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 6);
        cx.tree
            .add_child(help_row, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree.add_child(
            help_row,
            Box::new(Label::new(
                "Type to filter. Enter toggles/creates. Ctrl/Opt+Backspace word-delete. Paste splits on comma.",
            )),
            None,
            1,
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
