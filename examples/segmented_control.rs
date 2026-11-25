// SPEC-HAL-274: SegmentedControl Demo

use std::cell::Cell;
use std::rc::Rc;

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::segmented_control::{Segment, SegmentedControl};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== SegmentedControl Demo ===");
    println!("Features:");
    println!("  - Click to select a segment");
    println!("  - Keyboard: arrows move focus, Enter/Space selects");
    println!("  - Equal-width mode on second control");
    println!("  - Disabled segment (Card)");
    println!("=============================\n");

    let app = App::new(AppConfig::new("SegmentedControl Demo").with_size(500.0, 200.0));
    app.run(SegmentedControlDemoScreen::new());
}

struct SegmentedControlDemoScreen {
    needs_rebuild: bool,
    sel1: Rc<Cell<usize>>,
    sel2: Rc<Cell<usize>>,
    rebuild_flag: Rc<Cell<bool>>,
    placeholder: Label,
}

impl SegmentedControlDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            sel1: Rc::new(Cell::new(0)),
            sel2: Rc::new(Cell::new(1)),
            rebuild_flag: Rc::new(Cell::new(false)),
            placeholder: Label::new(""),
        }
    }
}

impl Screen for SegmentedControlDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 0);

        // Row 1: Day/Week/Month
        let row1 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 1);
        cx.tree
            .add_child(row1, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row1, Box::new(Label::new("View:")), None, 1);
        {
            let sel = self.sel1.clone();
            let flag = self.rebuild_flag.clone();
            cx.tree.add_child(
                row1,
                Box::new(
                    SegmentedControl::new(
                        vec![
                            Segment::new("Day"),
                            Segment::new("Week"),
                            Segment::new("Month"),
                        ],
                        self.sel1.get(),
                    )
                    .on_change(move |idx| {
                        sel.set(idx);
                        flag.set(true);
                    }),
                ),
                None,
                2,
            );
        }

        // Row 2: List/Grid/Card(disabled)/Table — equal width
        let row2 = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(12.0)), None, 2);
        cx.tree
            .add_child(row2, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree
            .add_child(row2, Box::new(Label::new("Layout:")), None, 1);
        {
            let sel = self.sel2.clone();
            let flag = self.rebuild_flag.clone();
            cx.tree.add_child(
                row2,
                Box::new(
                    SegmentedControl::new(
                        vec![
                            Segment::new("List"),
                            Segment::new("Grid"),
                            Segment::new("Card").with_enabled(false),
                            Segment::new("Table"),
                        ],
                        self.sel2.get(),
                    )
                    .with_equal_width(true)
                    .on_change(move |idx| {
                        sel.set(idx);
                        flag.set(true);
                    }),
                ),
                None,
                2,
            );
        }

        // Status label
        let label_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(0.0)), None, 3);
        cx.tree
            .add_child(label_row, Box::new(FixedSpacer::horizontal(20.0)), None, 0);
        cx.tree.add_child(
            label_row,
            Box::new(Label::new(&format!(
                "Selected: {}, {}",
                self.sel1.get(),
                self.sel2.get()
            ))),
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
        if self.rebuild_flag.get() {
            self.rebuild_flag.set(false);
            return true;
        }
        std::mem::take(&mut self.needs_rebuild)
    }
}
