// List Demo
//
// Demonstrates List widget as a TabularSubstrate-backed list with List v1 interaction semantics.
// Shows: selection, Shift+drag range selection, focus tracking, activation callback.

use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::ordered_selection::SelectionMode;
use halcyon::prelude::*;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::list::List;
use halcyon::widgets::scroll_area::ScrollArea;

/// Number of items in the list.
const ITEM_COUNT: usize = 1000000;
// SPEC-HAL-094/096C: Row height is now theme-token-driven.
// Do NOT hardcode row height — List reads from cx.theme.list_geom.row_height.
// Compact density = 24px, Regular = 28px, Comfortable = 32px.

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== List Demo ===");
    println!("  - {} items", ITEM_COUNT);
    println!("  - Click for single selection");
    println!("  - Shift+click/drag for range selection (base union)");
    println!("  - Ctrl+click to toggle individual items");
    println!("  - Arrow keys for navigation");
    println!("  - Space to toggle focused item");
    println!("  - Enter to activate focused item");
    println!("  - Escape to clear selection");
    println!("==================\n");

    let app = App::new(AppConfig::new("List Demo").with_size(400.0, 600.0));
    app.run(ListDemoScreen::new());
}

struct ListDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl ListDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for ListDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Build List with multi-selection mode.
        // SPEC-HAL-094/096C: Do NOT call set_item_height() — let List use theme tokens.
        // Theme (Compact density) provides row_height=24.0.
        let mut list = List::new(SelectionMode::Multi);
        list.set_item_count(ITEM_COUNT);

        list.set_on_selection_changed(|selection| {
            println!(
                "[SELECTION] {} items: {:?}",
                selection.len(),
                selection.iter().take(10).collect::<Vec<_>>()
            );
        });

        list.set_on_activate(|index| {
            println!("[ACTIVATE] Item {}", index);
        });

        // Create the tree structure:
        // Root (Flex column)
        //   └─ ScrollArea (flexible, fills remaining space)
        //        └─ List
        let tree_root = cx.tree.root().expect("Tree must have root");
        let root = cx
            .tree
            .add_child(tree_root, Box::new(Flex::column()), None, 0);

        // ScrollArea with deferred wiring
        // SPEC-HAL-094: Use theme's Compact row_height (24.0) for content size estimate.
        // List's layout will provide the authoritative content_size.
        let expected_row_height = 24.0; // theme.list_geom.row_height for Compact
        let content_height = ITEM_COUNT as f32 * expected_row_height;
        let scroll_area = ScrollArea::new_unwired().with_content_size(380.0, content_height);
        let scroll_area_id = cx.tree.add_child(root, Box::new(scroll_area), None, 0);
        cx.tree.set_flex(scroll_area_id, FlexParams::flexible(1.0));

        // Add List as child of ScrollArea
        let list_id = cx.tree.add_child(scroll_area_id, Box::new(list), None, 0);

        // Wire ScrollArea content to List
        if let Some(node) = cx.tree.node_mut(scroll_area_id) {
            if let Some(scroll_area) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                scroll_area.wire_content(list_id);
            }
        }

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // Let all input pass through to the List widget
        false
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
