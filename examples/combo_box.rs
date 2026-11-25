// ComboBox Demo — Interactive test harness for ComboBox features.
//
// Tests:
//   - Popup open/close (Enter, Space, Down, Escape, blur, outside-click, affordance toggle)
//   - Highlight navigation (Up/Down, pointer hover)
//   - Commit paths (Selection via click/Enter, InputConfirm via typed text + Enter)
//   - Type-to-search (prefix-match highlight bias)
//   - Close-on-text-click
//   - Resolver commit/reject (strict: exact match only, no custom values)
//   - Debounced async search (provider-side debounce + artificial delay)
//
// No ListView, TreeView, TabularSubstrate, adapters, data mutation, or persistence.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::layout::FlexParams;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::model::default_selection_resolver::DefaultSelectionResolver;
use halcyon::model::search_projection_harness::{DebouncedSearchHarness, MatchMode};
use halcyon::model::selection_resolver::{CommittedValue, OptionId, ResolverHandle};
use halcyon::prelude::*;
use halcyon::widgets::ComboBox;
use std::time::Duration;

// Configurable debounce/delay for demo tuning.
// Set DEBOUNCE_MS to 0 to disable debounce (instant search).
// Set DELAY_MS to 0 to disable artificial provider delay.
const DEBOUNCE_MS: u64 = 100;
const DELAY_MS: u64 = 80;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== ComboBox Demo (100k stress test, debounced search) ===");
    println!();
    println!("Options: Item 000000 … Item 099999 (100,000 items)");
    println!("Resolver: DefaultSelectionResolver::strict()");
    println!("Debounce: {} ms, Delay: {} ms", DEBOUNCE_MS, DELAY_MS);
    println!("Close-on-text-click: ON");
    println!();
    println!("Controls:");
    println!("  Click or Tab to focus");
    println!("  Enter/Down to open popup");
    println!("  Up/Down to navigate options");
    println!("  Enter to commit highlighted option (Selection path)");
    println!("  Type text + Enter to commit via exact match (InputConfirm path)");
    println!("  Click option to commit (Selection path)");
    println!("  Escape to close without commit");
    println!("  Click outside popup to dismiss");
    println!("  Click affordance to toggle popup");
    println!("  Click text area while open to close popup");
    println!();
    println!("Close window to exit.");
    println!("======================\n");

    let app = App::new(AppConfig::new("ComboBox Demo").with_size(400.0, 300.0));
    app.run(ComboBoxDemo::new());
}

struct ComboBoxDemo {
    placeholder: Label,
    combo_node_id: Option<NodeId>,
    built: bool,
    // Debounced search harness (external to ComboBox — provider-side timing)
    debounced_harness: Option<DebouncedSearchHarness>,
    // Last query text seen — used to detect text changes in flush_pending_actions
    last_query: String,
}

impl ComboBoxDemo {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            combo_node_id: None,
            built: false,
            debounced_harness: None,
            last_query: String::new(),
        }
    }
}

impl Screen for ComboBoxDemo {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        // 100k items: "Item 000000" … "Item 099999", zero-padded for lexicographic sort.
        let labels: Vec<String> = (0..100_000).map(|i| format!("Item {:06}", i)).collect();
        let options: Vec<(OptionId, &str)> = labels
            .iter()
            .enumerate()
            .map(|(i, s)| (OptionId::from_raw(i as u64), s.as_str()))
            .collect();

        let resolver = ResolverHandle::new(DefaultSelectionResolver::strict());
        let mut combo = ComboBox::new(resolver);
        combo.set_options(&options);
        combo.set_close_on_text_click(true);
        // No sync harness attached — search is driven externally via debounced harness.

        self.debounced_harness = Some(DebouncedSearchHarness::new(
            &options,
            Duration::from_millis(DEBOUNCE_MS),
            Duration::from_millis(DELAY_MS),
        ));

        // Widget tree:
        // Root (Flex column)
        //   ├─ Spacer (top)
        //   ├─ Row (horizontal centering)
        //   │   ├─ Spacer (left margin)
        //   │   ├─ ComboBox (flexible fill)
        //   │   └─ Spacer (right margin)
        //   ├─ Label (instructions)
        //   └─ Spacer (bottom)
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        let top = cx
            .tree
            .add_child(root, Box::new(FlexSpacer::new()), None, 0);
        cx.tree.set_flex(top, FlexParams::flexible(1.0));

        let row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 1);
        cx.tree
            .add_child(row, Box::new(FixedSpacer::horizontal(32.0)), None, 0);

        let combo_id = cx.tree.add_child(row, Box::new(combo), None, 1);
        cx.tree.set_flex(combo_id, FlexParams::flexible(1.0));
        self.combo_node_id = Some(combo_id);

        cx.tree
            .add_child(row, Box::new(FixedSpacer::horizontal(32.0)), None, 2);

        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Tab to focus | Enter to open | Escape to cancel",
            )),
            None,
            2,
        );

        let bottom = cx
            .tree
            .add_child(root, Box::new(FlexSpacer::new()), None, 3);
        cx.tree.set_flex(bottom, FlexParams::flexible(1.0));

        &mut self.placeholder
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let Some(combo_id) = self.combo_node_id else {
            return;
        };
        let Some(tree) = cx.tree.as_mut() else { return };
        let Some(node) = tree.node_mut(combo_id) else {
            return;
        };
        let Some(combo) = node.widget_mut().as_any_mut().downcast_mut::<ComboBox>() else {
            return;
        };

        // 1. Committed value retrieval (existing behavior)
        if let Some(committed) = combo.take_committed() {
            match &committed {
                CommittedValue::ExistingOption(id) => {
                    println!(
                        "[combo_box_demo] Commit: ExistingOption(OptionId({}))",
                        id.raw()
                    );
                }
                CommittedValue::CustomValue(text) => {
                    println!("[combo_box_demo] Commit: CustomValue(\"{}\")", text);
                }
            }
        }

        // 2. Debounced search: detect text changes, submit to harness, poll for results
        if let Some(ref mut harness) = self.debounced_harness {
            let current_text = combo.input_text().to_owned();

            // New text → submit to debounced harness (cancels any in-flight request)
            if current_text != self.last_query {
                self.last_query = current_text.clone();
                harness.submit(current_text, MatchMode::Prefix);
                combo.set_loading(true);
            }

            // Poll for results
            if let Some(output) = harness.poll() {
                combo.apply_projection_output(output);
                combo.set_loading(false);
            }
        }
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.built = false;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        if !self.built { true } else { false }
    }
}
