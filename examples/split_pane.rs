// SPEC-HAL-267: SplitPane Demo
// Demonstrates horizontal and vertical split panes with drag resize,
// keyboard navigation, double-click reset, min constraints, and nesting.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::split_pane::SplitPane;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== SplitPane Demo ===");
    println!("Features:");
    println!("  - Drag handle to resize panels");
    println!("  - Tab to focus handle, arrow keys to adjust");
    println!("  - Double-click handle to reset to initial ratio");
    println!("  - Escape during drag cancels resize");
    println!("  - Nested split: left panel contains a vertical split");
    println!("Close the window to exit");
    println!("=======================\n");

    let mut config = AppConfig::new("SplitPane Demo").with_size(800.0, 600.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(SplitPaneDemoScreen::new());
}

struct SplitPaneDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl SplitPaneDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for SplitPaneDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Layout:
        //   ┌─────────────┬─────────────────┐
        //   │  Top-Left   │                  │
        //   │  (labels)   │   Right Panel    │
        //   ├─────────────┤   (buttons)      │
        //   │ Bottom-Left │                  │
        //   │  (labels)   │                  │
        //   └─────────────┴─────────────────┘
        //
        // Outer: horizontal split (left_split | right_panel)
        // Left:  vertical split (top_left | bottom_left)

        // ── Outer horizontal split (placeholder NodeIds, replaced below) ──
        let outer_split = cx.mount_child(Box::new(
            SplitPane::horizontal(NodeId(0), NodeId(0))
                .initial_ratio(0.4)
                .leading_min(150.0)
                .trailing_min(200.0)
                .on_ratio_changed(|r| println!("[SPLIT] Horizontal ratio: {:.2}", r)),
        ));
        let tree = &mut cx.tree;

        // ── Left side: nested vertical split (placeholder NodeIds) ──
        let left_split = tree.add_child(
            outer_split,
            Box::new(
                SplitPane::vertical(NodeId(0), NodeId(0))
                    .initial_ratio(0.5)
                    .leading_min(80.0)
                    .trailing_min(80.0)
                    .on_ratio_changed(|r| println!("[SPLIT] Vertical ratio: {:.2}", r)),
            ),
            None,
            0,
        );

        // ── Top-left content: vertical overflow for scroll testing ──
        let top_left_scroll =
            tree.add_child(left_split, Box::new(ScrollArea::new(NodeId(0))), None, 0);
        let top_left_col = tree.add_child(
            top_left_scroll,
            Box::new(
                Flex::column()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(16.0)),
            ),
            None,
            0,
        );
        for i in 0..30 {
            tree.add_child(
                top_left_col,
                Box::new(Label::new(&format!("Top-Left Item {}", i + 1))),
                None,
                i,
            );
        }
        // Fix up ScrollArea child reference
        replace_widget!(tree, top_left_scroll, ScrollArea::new(top_left_col));

        // ── Bottom-left content: horizontal overflow for scroll testing ──
        let bottom_left_scroll =
            tree.add_child(left_split, Box::new(ScrollArea::new(NodeId(0))), None, 1);
        let bottom_left_col = tree.add_child(
            bottom_left_scroll,
            Box::new(
                Flex::column()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(16.0)),
            ),
            None,
            0,
        );
        for i in 0..15 {
            tree.add_child(
                bottom_left_col,
                Box::new(Label::new(&format!(
                    "Bottom-Left Item {} — this label is intentionally wide to force horizontal scrolling when the panel is narrow",
                    i + 1
                ))),
                None,
                i as i16,
            );
        }
        replace_widget!(
            tree,
            bottom_left_scroll,
            ScrollArea::new(bottom_left_col).enable_horizontal_scrollbar(true)
        );

        // Replace vertical split with correct child NodeIds
        replace_widget!(
            tree,
            left_split,
            SplitPane::vertical(top_left_scroll, bottom_left_scroll)
                .initial_ratio(0.5)
                .leading_min(80.0)
                .trailing_min(80.0)
                .on_ratio_changed(|r| println!("[SPLIT] Vertical ratio: {:.2}", r))
        );

        // ── Right panel content: wrapped in ScrollArea for vertical overflow ──
        let right_scroll =
            tree.add_child(outer_split, Box::new(ScrollArea::new(NodeId(0))), None, 1);
        let right_col = tree.add_child(
            right_scroll,
            Box::new(
                Flex::column()
                    .with_spacing(12.0)
                    .with_padding(EdgeInsets::all(16.0)),
            ),
            None,
            0,
        );
        tree.add_child(right_col, Box::new(Label::new("Right Panel")), None, 0);
        tree.add_child(right_col, Box::new(FixedSpacer::vertical(8.0)), None, 1);

        for i in 0..5i16 {
            let btn = Button::new(NodeId(0));
            let btn_id = tree.add_child(right_col, Box::new(btn), None, 10 + i);
            let label_id = tree.add_child(
                btn_id,
                Box::new(Label::new(&format!("Button {}", i + 1)).with_color(Color::WHITE)),
                None,
                0,
            );
            replace_widget!(
                tree,
                btn_id,
                Button::new(label_id).on_click(move || {
                    println!("[CLICK] Button {}", i + 1);
                })
            );
        }
        replace_widget!(tree, right_scroll, ScrollArea::new(right_col));

        // Replace outer split with correct child NodeIds
        replace_widget!(
            tree,
            outer_split,
            SplitPane::horizontal(left_split, right_scroll)
                .initial_ratio(0.4)
                .leading_min(150.0)
                .trailing_min(200.0)
                .on_ratio_changed(|r| println!("[SPLIT] Horizontal ratio: {:.2}", r))
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
