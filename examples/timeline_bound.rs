// =============================================================================
// Bound Timeline Composition (SPEC-HAL-337 §8.6 / SPEC-HAL-336 §8)
// =============================================================================
//
// The stage-6 composition — the Gantt / resource-board consumer shape:
// one shared topology substrate, projected by a TreeView (left) and a
// TimelineHeader + TimelineCanvas pair (right).
//
//   Screen mints SharedHts (SPEC-HAL-337 §2)
//   SplitPane
//   ├── Left: TreeView::with_hts(handle)  — HtsOwnership::External
//   └── Right: Flex column
//        ├── TimelineHeader               — same TemporalSubstrate (INV-336-1)
//        └── TimelineCanvas               — rows via hts_rows(&handle)
//
// Per frame (Screen::update, pre-layout — SPEC-HAL-335 §10):
//   1. hts_rows(&hts) → viewport-space RowGeometry (FORBID-337-2 adapter)
//   2. canvas.set_row_geometry(rows)      — the SPEC-HAL-335 §7 seam
//   3. substrate.begin_frame(source, Some(&row_ids)); drain intents
//   4. Select / DeselectAll intents reduce the TREE's row selection
//      (SPEC-HAL-336 §8: unified row-level selection, normative direction)
//
// Visual verification:
//   [ ] Tree rows and canvas bars share one vertical geometry: scroll the
//       tree — bars track exactly, including partial top rows
//   [ ] Collapse "Release 2412" — its children's bars vanish same frame
//   [ ] Header tiers (Month | Day) pixel-aligned with canvas gridlines
//   [ ] Click a bar — the corresponding tree row selects; Ctrl+click toggles
//   [ ] Click empty canvas — tree selection clears; console logs DeselectAll
//   [ ] Critical arrow Build → Deploy; Freeze marker at Jun 8; now at Jun 12
//   [ ] Splitter drags; both panes stay coherent
//   [ ] Plain wheel over the canvas: dy scrolls the tree rows, dx pans time
//   [ ] Cmd/Ctrl+wheel zooms about the cursor (up = in); Shift+wheel pans time
//   [ ] Space held + drag: hand tool pans both axes, content follows pointer
// =============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::layout::FlexParams;
use halcyon::core::ordered_selection::{
    OrderedSelectionAction, SelectionMode, reduce_ordered_selection,
};
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::engines::hierarchical::{HierarchicalTabularSubstrate, NodeMeta, SharedHts};
use halcyon::engines::temporal::{
    ArrowDescriptor, IntervalDescriptor, IntervalEdge, IntervalId, IntervalLayer, IntervalSource,
    IntervalStyle, LanePolicy, MarkerDescriptor, MarkerStyle, TemporalDescriptors, TemporalIntent,
    TemporalInvalidationEvent, TemporalSubstrate, TimeGeometryEngine, TimeScale, hts_rows,
};
use halcyon::model::date_value::DateValue;
use halcyon::model::time::{TimeFrame, TimeRange, Timestamp, from_civil};
use halcyon::model::time_value::TimeValue;
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::split_pane::SplitPane;
use halcyon::widgets::tabular_view::column::{Column, ColumnModel};
use halcyon::widgets::timeline_canvas::{TemporalCanvasPolicy, TimelineCanvas};
use halcyon::widgets::timeline_header::TimelineHeader;
use halcyon::widgets::tree_view::TreeView;

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 640.0;

fn ts_d(day: u8, hour: u8) -> Timestamp {
    let date = DateValue::new(2026, 6, day).expect("valid demo date");
    from_civil(date, TimeValue::new(hour, 0, None), TimeFrame::default())
}

fn span_d(d0: u8, d1: u8) -> TimeRange {
    TimeRange::new(ts_d(d0, 0), ts_d(d1, 0))
}

fn node(node_id: u64, parent_index: Option<usize>, depth: u32, label: &str) -> NodeMeta {
    NodeMeta {
        node_id,
        parent_index,
        depth,
        has_children: false, // fixed by the second pass
        label: label.to_string(),
    }
}

/// 13-node programme tree. Interval ids equal node ids for the Primary layer.
fn build_project_tree() -> Vec<NodeMeta> {
    let mut nodes = vec![
        node(0, None, 0, "Programme 2412"),
        node(1, Some(0), 1, "Release 2412"),
        node(2, Some(1), 2, "Build"),
        node(3, Some(1), 2, "Integration"),
        node(4, Some(1), 2, "Soak"),
        node(5, Some(1), 2, "Deploy"),
        node(6, Some(0), 1, "Infra"),
        node(7, Some(6), 2, "Provision"),
        node(8, Some(6), 2, "Migration"),
        node(9, Some(6), 2, "Cutover"),
        node(10, Some(0), 1, "QA"),
        node(11, Some(10), 2, "Regression"),
        node(12, Some(10), 2, "Sign-off"),
    ];
    let mut has_child = vec![false; nodes.len()];
    for n in &nodes {
        if let Some(p) = n.parent_index {
            has_child[p] = true;
        }
    }
    for (i, n) in nodes.iter_mut().enumerate() {
        n.has_children = has_child[i];
    }
    nodes
}

fn bar(
    id: u64,
    row: u64,
    range: TimeRange,
    color: u32,
    label: &str,
    progress: f32,
    style: IntervalStyle,
    layer: IntervalLayer,
) -> IntervalDescriptor {
    IntervalDescriptor {
        id: IntervalId(id),
        row,
        range,
        color: Color::from_hex(color),
        label: if label.is_empty() {
            None
        } else {
            Some(label.to_string())
        },
        progress,
        style,
        layer,
        lane: None,
    }
}

fn build_demo_data() -> TemporalDescriptors {
    let mut data = TemporalDescriptors::default();
    let s = IntervalStyle::Solid;
    let p = IntervalLayer::Primary;

    data.intervals.push(bar(
        0,
        0,
        span_d(1, 19),
        0x8FA6C4,
        "Programme 2412",
        0.45,
        s,
        p,
    ));
    data.intervals.push(bar(
        1,
        1,
        span_d(2, 10),
        0x5E9CFF,
        "Release 2412",
        0.7,
        s,
        p,
    ));
    data.intervals
        .push(bar(2, 2, span_d(2, 4), 0x3DCC8E, "Build", 1.0, s, p));
    data.intervals
        .push(bar(3, 3, span_d(3, 6), 0x3DCC8E, "Integration", 0.8, s, p));
    // Planned window for Integration: baseline ghost.
    data.intervals.push(bar(
        100,
        3,
        span_d(4, 7),
        0x3DCC8E,
        "",
        0.0,
        s,
        IntervalLayer::Baseline,
    ));
    data.intervals.push(bar(
        4,
        4,
        span_d(6, 9),
        0xE8A33D,
        "Soak",
        0.0,
        IntervalStyle::Hatched,
        p,
    ));
    data.intervals.push(bar(
        5,
        5,
        span_d(9, 10),
        0xC678F0,
        "Deploy",
        0.0,
        IntervalStyle::Hollow,
        p,
    ));
    data.intervals
        .push(bar(6, 6, span_d(4, 15), 0x5E9CFF, "Infra", 0.3, s, p));
    data.intervals
        .push(bar(7, 7, span_d(4, 7), 0x3DCC8E, "Provision", 1.0, s, p));
    data.intervals
        .push(bar(8, 8, span_d(7, 12), 0xE8A33D, "Migration", 0.4, s, p));
    data.intervals.push(bar(
        9,
        9,
        span_d(12, 15),
        0xC678F0,
        "Cutover",
        0.0,
        IntervalStyle::Hollow,
        p,
    ));
    data.intervals
        .push(bar(10, 10, span_d(5, 18), 0x5E9CFF, "QA", 0.2, s, p));
    data.intervals.push(bar(
        11,
        11,
        span_d(5, 16),
        0x3DCC8E,
        "Regression",
        0.3,
        s,
        p,
    ));
    // Sign-off milestone (zero duration).
    data.intervals.push(bar(
        12,
        12,
        TimeRange::new(ts_d(18, 0), ts_d(18, 0)),
        0xE05A50,
        "",
        0.0,
        IntervalStyle::Milestone,
        p,
    ));

    data.arrows.push(ArrowDescriptor {
        from: IntervalId(2),
        from_edge: IntervalEdge::End,
        to: IntervalId(5),
        to_edge: IntervalEdge::Start,
        critical: true,
        color: None,
    });

    data.markers.push(MarkerDescriptor {
        ts: ts_d(8, 0),
        color: Color::from_hex(0xA8B0BC),
        label: Some("Freeze".to_string()),
        style: MarkerStyle::VerticalLine,
    });

    data
}

/// Static demo source: stable ids, no invalidation traffic. Over-returning
/// is permitted; the substrate culls precisely (SPEC-HAL-335 §6).
struct DemoSource {
    data: TemporalDescriptors,
}

impl IntervalSource for DemoSource {
    fn collect(&self, _rows: Option<&[u64]>, _range: TimeRange, out: &mut TemporalDescriptors) {
        out.intervals.extend(self.data.intervals.iter().cloned());
        out.bands.extend(self.data.bands.iter().copied());
        out.arrows.extend(self.data.arrows.iter().copied());
        out.markers.extend(self.data.markers.iter().cloned());
    }

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
        Vec::new()
    }

    fn interval_exists(&self, id: IntervalId) -> bool {
        self.data.intervals.iter().any(|d| d.id == id)
    }
}

struct BoundTimelineScreen {
    placeholder: Label,
    built: bool,
    hts: SharedHts,
    substrate: Option<Rc<RefCell<TemporalSubstrate>>>,
    source: DemoSource,
    canvas_id: Option<NodeId>,
    tree_view_id: Option<NodeId>,
    scroll_area_id: Option<NodeId>,
    space_held: bool,
}

impl BoundTimelineScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
            // Screen mints the topology substrate (SPEC-HAL-337 §2).
            hts: Rc::new(RefCell::new(HierarchicalTabularSubstrate::new())),
            substrate: None,
            source: DemoSource {
                data: build_demo_data(),
            },
            canvas_id: None,
            tree_view_id: None,
            scroll_area_id: None,
            space_held: false,
        }
    }
}

impl Screen for BoundTimelineScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        // Outer split (placeholder NodeIds; fixed by replace_widget! below).
        let split = cx.mount_child(Box::new(
            SplitPane::horizontal(NodeId(0), NodeId(0))
                .initial_ratio(0.30)
                .leading_min(240.0)
                .trailing_min(360.0),
        ));
        let tree = &mut cx.tree;

        // ---- Left pane: TreeView projecting the shared topology ----
        let mut tree_view = TreeView::with_hts(Rc::clone(&self.hts));
        tree_view.set_tree_data(build_project_tree());

        let mut col_model = ColumnModel::new();
        let tree_col = Column::new(220.0);
        let owner_col = Column::new(120.0);
        let tree_col_id = tree_col.id;
        let owner_col_id = owner_col.id;
        col_model.push(tree_col);
        col_model.push(owner_col);
        tree_view.set_columns(col_model);
        tree_view.set_tree_column(tree_col_id);
        tree_view.set_column_title(tree_col_id, "Task");
        tree_view.set_column_title(owner_col_id, "Owner");

        // Expand by VISIBLE ROW INDEX, top-down (indices shift as rows appear):
        // expand(0) Programme → [0:Programme 1:Release 2:Infra 3:QA]
        // expand(1) Release   → Infra moves to index 6, QA to 7
        // expand(6) Infra     → QA moves to index 10
        // expand(10) QA       → all 13 rows visible
        tree_view.expand(0);
        tree_view.expand(1);
        tree_view.expand(6);
        tree_view.expand(10);

        let hscroll_link = tree_view.horizontal_scroll_link();
        let body_widget = tree_view.create_body_widget();

        let tree_view_id = tree.add_child(split, Box::new(tree_view), None, 0);

        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_content_owned_vertical_translation();
        let scroll_area_id = tree.add_child(tree_view_id, Box::new(scroll_area), None, 0);
        let body_id = tree.add_child(scroll_area_id, Box::new(body_widget), None, 0);
        if let Some(n) = tree.node_mut(scroll_area_id) {
            if let Some(sa) = n.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        // ---- Right pane: header + canvas over one TemporalSubstrate ----
        let right = tree.add_child(split, Box::new(Flex::column()), None, 1);

        let mut engine =
            TimeGeometryEngine::new(ts_d(1, 0), TimeScale::Days, 40.0, WINDOW_WIDTH * 0.70);
        engine.set_now(Some(ts_d(12, 0)));
        let substrate = Rc::new(RefCell::new(TemporalSubstrate::new(
            engine,
            SelectionMode::Multi,
            LanePolicy::Auto,
        )));
        self.substrate = Some(substrate.clone());

        let header = TimelineHeader::new(substrate.clone());
        let canvas =
            TimelineCanvas::new(substrate.clone()).with_policy(TemporalCanvasPolicy::Select);

        tree.add_child(right, Box::new(header), None, 0);
        let canvas_id = tree.add_child(right, Box::new(canvas), None, 1);
        tree.set_flex(canvas_id, FlexParams::flexible(1.0));

        // Fix the split's children to the real subtrees.
        replace_widget!(
            tree,
            split,
            SplitPane::horizontal(tree_view_id, right)
                .initial_ratio(0.30)
                .leading_min(240.0)
                .trailing_min(360.0)
        );

        self.canvas_id = Some(canvas_id);
        self.tree_view_id = Some(tree_view_id);
        self.scroll_area_id = Some(scroll_area_id);

        &mut self.placeholder
    }

    /// Per-frame drive, pre-layout (SPEC-HAL-335 §10; SPEC-HAL-337 §4.1).
    fn update(&mut self, tree: &mut WidgetTree) {
        let (Some(canvas_id), Some(tree_view_id), Some(scroll_area_id), Some(substrate)) = (
            self.canvas_id,
            self.tree_view_id,
            self.scroll_area_id,
            self.substrate.as_ref(),
        ) else {
            return;
        };

        // 1. Row geometry from the shared topology (FORBID-337-2 adapter).
        let rows = hts_rows(&self.hts);
        let row_ids: Vec<u64> = rows.iter().map(|r| r.id).collect();

        // 2. Feed the SPEC-HAL-335 §7 seam + the §7 hand-tool modifier.
        if let Some(n) = tree.node_mut(canvas_id) {
            if let Some(canvas) = n.widget_mut().as_any_mut().downcast_mut::<TimelineCanvas>() {
                canvas.set_row_geometry(rows);
                canvas.set_pan_modifier(self.space_held);
            }
        }

        // 3. Temporal frame + intent drain (INV-335-I1: application drains).
        let mut actions: Vec<OrderedSelectionAction<u64>> = Vec::new();
        let mut pan_dy: f32 = 0.0;
        {
            let mut s = substrate.borrow_mut();
            s.begin_frame(&mut self.source, Some(&row_ids));
            for intent in s.drain_intents() {
                if !matches!(intent, TemporalIntent::VerticalPanBy { .. }) {
                    println!("[intent] {:?}", intent);
                }
                match intent {
                    TemporalIntent::Select { id, modifiers } => {
                        // SPEC-HAL-336 §8: interval → its row, reduce row selection.
                        if let Some(row) = s.snapshot().interval(id).map(|d| d.row) {
                            let action = if modifiers.ctrl || modifiers.meta {
                                OrderedSelectionAction::Toggle { key: row }
                            } else if modifiers.shift {
                                OrderedSelectionAction::ExtendTo { key: row }
                            } else {
                                OrderedSelectionAction::Select { key: row }
                            };
                            actions.push(action);
                        }
                    }
                    TemporalIntent::DeselectAll => {
                        actions.push(OrderedSelectionAction::ClearSelection);
                    }
                    TemporalIntent::VerticalPanBy { dy } => {
                        pan_dy += dy;
                    }
                    _ => {}
                }
            }
        }

        // 3b. Apply vertical pan through the row provider's single scroll
        //     path (SPEC-HAL-336 §7 / INV-337-S1 / FORBID-337-4).
        if pan_dy != 0.0 {
            if let Some(n) = tree.node_mut(scroll_area_id) {
                if let Some(sa) = n.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                    let y = sa.target_scroll_y() + pan_dy;
                    sa.scroll_to_y(y);
                }
            }
        }

        // 4. Reduce the tree's row selection (oracle = the shared HTS;
        //    selection core and hts are distinct RefCells — no double borrow).
        if actions.is_empty() {
            return;
        }
        if let Some(n) = tree.node_mut(tree_view_id) {
            if let Some(tv) = n.widget_mut().as_any_mut().downcast_mut::<TreeView>() {
                let oracle = self.hts.borrow();
                for action in actions {
                    reduce_ordered_selection(&mut tv.selection_state_mut(), action, &*oracle);
                }
            }
        }
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // §7 hand tool: track Space-held (fed to the canvas per frame via
        // set_pan_modifier in update()) and CONSUME the Space key so it never
        // reaches the focused tree row — whose Space-toggle would otherwise
        // flicker the row selection while the modifier is held.
        match event {
            FusedInputEvent::KeyDown { key_code, .. } if *key_code == KeyCode::Space => {
                self.space_held = true;
                true
            }
            FusedInputEvent::KeyUp { key_code, .. } if *key_code == KeyCode::Space => {
                self.space_held = false;
                true
            }
            FusedInputEvent::FocusChanged { focused: false, .. } => {
                self.space_held = false;
                false
            }
            _ => false,
        }
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== Bound Timeline Composition (SPEC-HAL-337 §8.6) ===");
        println!();
        println!("Left: TreeView over SharedHts.  Right: TimelineHeader + TimelineCanvas.");
        println!("Row geometry crosses via hts_rows once per frame (FORBID-337-2).");
        println!();
        println!("Verify:");
        println!("  Wheel              dy scrolls rows, dx pans time (no modifiers)");
        println!("  Cmd/Ctrl+wheel     zoom about cursor (up = in)");
        println!("  Shift+wheel        horizontal time pan");
        println!("  Space + drag       hand tool, both axes");
        println!("  Click bar          tree row selects (Ctrl toggles, Shift extends)");
        println!("  Click empty        tree selection clears; DeselectAll logged");
        println!("  Drag splitter      both panes stay coherent");
        println!();
        println!("Close window to exit.");
        println!("=======================================================");
    }

    fn mark_rebuild(&mut self) {}

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(
        AppConfig::new("Bound Timeline (SPEC-HAL-337 §8.6)").with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(BoundTimelineScreen::new());
}
