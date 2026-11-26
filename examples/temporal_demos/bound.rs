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

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::layout::{Constraints, FlexParams, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::ordered_selection::{
    OrderedSelectionAction, SelectionMode, reduce_ordered_selection,
};
use halcyon::core::paint::Color;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::engines::hierarchical::{HierarchicalTabularSubstrate, NodeMeta, SharedHts};
use halcyon::engines::temporal::{
    ArrowDescriptor, IntervalDescriptor, IntervalEdge, IntervalId, IntervalLayer, IntervalSource,
    IntervalStyle, LanePolicy, MarkerDescriptor, MarkerStyle, RowGeometry, TemporalDescriptors,
    TemporalIntent, TemporalInvalidationEvent, TemporalSubstrate, TimeGeometryEngine,
    TimeGeometryResolution, TimeScale, hts_rows,
};
use halcyon::model::date_value::DateValue;
use halcyon::model::time::{TimeFrame, TimeRange, Timestamp, from_civil};
use halcyon::model::time_value::TimeValue;
use halcyon::replace_widget;
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::split_pane::SplitPane;
use halcyon::widgets::tabular_view::column::{Column, ColumnModel};
use halcyon::widgets::timeline_canvas::{TemporalCanvasPolicy, TimelineCanvas};
use halcyon::widgets::timeline_header::TimelineHeader;
use halcyon::widgets::tree_view::TreeView;

pub(super) const WINDOW_WIDTH: f32 = 1280.0;
const TREE_PANE_MIN: f32 = 240.0;
const TIMELINE_PANE_MIN: f32 = 360.0;
#[allow(dead_code, reason = "Used by the hosted Showcase; standalone timeline_bound uses the shared pane minima directly.")]
pub(super) const MIN_SPLIT_WIDTH: f32 = TREE_PANE_MIN + TIMELINE_PANE_MIN + 3.0;

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

struct BoundPaneInset {
    id: WidgetInternalId,
    top: f32,
}

impl BoundPaneInset {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            top: 0.0,
        }
    }
}

impl WidgetDowncast for BoundPaneInset {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for BoundPaneInset {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            constraints.min_height
        };
        let inset = self.top.min(height);
        let remaining = Constraints::new(
            constraints.min_width,
            constraints.max_width,
            (constraints.min_height - inset).max(0.0),
            height - inset,
        );
        let content = children[0];
        let size = cx.layout_child(content, remaining);
        cx.set_child_rect(content, RectF::new(0.0, inset, size.width, size.height));
        LayoutResult::fit(constraints.constrain(Size::new(size.width, inset + size.height)))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

struct BoundTimelineLayout {
    id: WidgetInternalId,
    panes: [NodeId; 2],
    viewports: [NodeId; 2],
    needs_settlement: Rc<Cell<bool>>,
}

impl WidgetDowncast for BoundTimelineLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for BoundTimelineLayout {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let split = children[0];
        let mut size = cx.layout_child(split, constraints);
        // Local rects come from TreeView and the timeline column; neither
        // includes the inset applied to its parent pane.
        let origins = self
            .viewports
            .map(|node| cx.tree.get_rect(node).expect("mounted row viewport").y);
        let common = origins[0].max(origins[1]);
        let mut changed = false;
        for (node, origin) in self.panes.into_iter().zip(origins) {
            let pane = cx
                .tree
                .node_mut(node)
                .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<BoundPaneInset>())
                .expect("mounted bound pane");
            let inset = common - origin;
            if pane.top != inset {
                pane.top = inset;
                cx.tree.mark_layout_dirty(node);
                changed = true;
            }
        }
        if changed {
            size = cx.layout_child(split, constraints);
        }
        cx.set_child_rect(split, RectF::new(0.0, 0.0, size.width, size.height));
        // Update runs before layout. A real layout pass may have settled new
        // HTS rows or time width, so admit a successor update through the tree.
        self.needs_settlement.set(true);
        LayoutResult::fit(size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn needs_viewport_constraint(&self) -> bool {
        true
    }

    fn needs_update(&self) -> bool {
        self.needs_settlement.get()
    }
}

#[derive(Clone, Copy)]
struct BoundTimelineNodes {
    tree_view: NodeId,
    body: NodeId,
    scroll_area: NodeId,
    header: NodeId,
    canvas: NodeId,
}

pub(super) struct BoundTimelineDemo {
    hts: SharedHts,
    substrate: Option<Rc<RefCell<TemporalSubstrate>>>,
    source: DemoSource,
    nodes: Option<BoundTimelineNodes>,
    needs_settlement: Rc<Cell<bool>>,
    published_rows: Vec<RowGeometry>,
    published_resolution: Option<TimeGeometryResolution>,
    published_selection: HashSet<IntervalId>,
    published_cursor: Option<Timestamp>,
    space_held: bool,
}

impl BoundTimelineDemo {
    pub(super) fn new() -> Self {
        Self {
            // Screen mints the topology substrate (SPEC-HAL-337 §2).
            hts: Rc::new(RefCell::new(HierarchicalTabularSubstrate::new())),
            substrate: None,
            source: DemoSource {
                data: build_demo_data(),
            },
            nodes: None,
            needs_settlement: Rc::new(Cell::new(false)),
            published_rows: Vec::new(),
            published_resolution: None,
            published_selection: HashSet::new(),
            published_cursor: None,
            space_held: false,
        }
    }

    pub(super) fn mount(&mut self, tree: &mut WidgetTree, parent: NodeId) -> NodeId {
        let layout = tree.add_child(parent, Box::new(Label::new("")), None, 0);
        let split = tree.add_child(layout, Box::new(Label::new("")), None, 0);
        let left_pane = tree.add_child(split, Box::new(BoundPaneInset::new()), None, 0);
        let right_pane = tree.add_child(split, Box::new(BoundPaneInset::new()), None, 1);

        // ---- Left pane: TreeView projecting the shared topology ----
        let mut tree_view = TreeView::with_hts(Rc::clone(&self.hts));
        tree_view.set_tree_data(build_project_tree());

        let mut col_model = ColumnModel::new();
        let tree_col = Column::new(220.0);
        let tree_col_id = tree_col.id;
        col_model.push(tree_col);
        tree_view.set_columns(col_model);
        tree_view.set_tree_column(tree_col_id);
        tree_view.set_column_title(tree_col_id, "Task");

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

        let tree_view_id = tree.add_child(left_pane, Box::new(tree_view), None, 0);

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
        let right = tree.add_child(right_pane, Box::new(Flex::column()), None, 0);

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

        let header_id = tree.add_child(right, Box::new(header), None, 0);
        let canvas_id = tree.add_child(right, Box::new(canvas), None, 1);
        tree.set_flex(canvas_id, FlexParams::flexible(1.0));

        // Fix the split's children to the real subtrees.
        replace_widget!(
            tree,
            split,
            SplitPane::horizontal(left_pane, right_pane)
                .initial_ratio(0.30)
                .leading_min(TREE_PANE_MIN)
                .trailing_min(TIMELINE_PANE_MIN)
        );
        replace_widget!(
            tree,
            layout,
            BoundTimelineLayout {
                id: WidgetInternalId::new(),
                panes: [left_pane, right_pane],
                viewports: [scroll_area_id, canvas_id],
                needs_settlement: Rc::clone(&self.needs_settlement),
            }
        );

        self.nodes = Some(BoundTimelineNodes {
            tree_view: tree_view_id,
            body: body_id,
            scroll_area: scroll_area_id,
            header: header_id,
            canvas: canvas_id,
        });

        layout
    }

    /// Per-frame drive, pre-layout (SPEC-HAL-335 §10; SPEC-HAL-337 §4.1).
    pub(super) fn update(&mut self, tree: &mut WidgetTree) {
        let (Some(nodes), Some(substrate)) = (self.nodes, self.substrate.as_ref()) else {
            return;
        };
        self.needs_settlement.set(false);

        // 1. Row geometry from the shared topology (FORBID-337-2 adapter).
        let rows = hts_rows(&self.hts);
        let row_ids: Vec<u64> = rows.iter().map(|r| r.id).collect();
        let rows_changed = rows != self.published_rows;

        // 2. Feed the SPEC-HAL-335 §7 seam + the §7 hand-tool modifier.
        if let Some(n) = tree.node_mut(nodes.canvas) {
            if let Some(canvas) = n.widget_mut().as_any_mut().downcast_mut::<TimelineCanvas>() {
                if rows_changed {
                    self.published_rows.clone_from(&rows);
                    canvas.set_row_geometry(rows);
                }
                canvas.set_pan_modifier(self.space_held);
            }
        }

        // 3. Temporal frame + intent drain (INV-335-I1: application drains).
        let mut actions: Vec<OrderedSelectionAction<u64>> = Vec::new();
        let mut pan_dy: f32 = 0.0;
        {
            let mut s = substrate.borrow_mut();
            s.begin_frame(&mut self.source, Some(&row_ids));
            // The source is static. A different row set or time resolution
            // accounts for every change to its culled interval projection.
            let resolution = s.resolution();
            let resolution_changed = self.published_resolution.as_ref() != Some(resolution);
            if resolution_changed {
                let tiers = resolution.tiers.len().max(1);
                let tiers_changed = self
                    .published_resolution
                    .as_ref()
                    .is_none_or(|previous| previous.tiers.len().max(1) != tiers);
                self.published_resolution = Some(resolution.clone());
                if tiers_changed {
                    tree.mark_layout_dirty(nodes.header);
                } else {
                    tree.mark_paint_dirty(nodes.header);
                }
            }
            let selection = &s.selection().selected_keys;
            let selection_changed = &self.published_selection != selection;
            if selection_changed {
                self.published_selection.clone_from(selection);
            }
            let cursor = s.time_context().cursor;
            let cursor_changed = self.published_cursor != cursor;
            self.published_cursor = cursor;
            if rows_changed || resolution_changed || selection_changed || cursor_changed {
                tree.mark_paint_dirty(nodes.canvas);
            }

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
            if let Some(n) = tree.node_mut(nodes.scroll_area) {
                if let Some(sa) = n.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                    let previous = sa.target_scroll_y();
                    sa.scroll_to_y(previous + pan_dy);
                    if sa.target_scroll_y() != previous {
                        tree.mark_layout_dirty(nodes.scroll_area);
                    }
                }
            }
        }

        // 4. Reduce the tree's row selection (oracle = the shared HTS;
        //    selection core and hts are distinct RefCells — no double borrow).
        if actions.is_empty() {
            return;
        }
        if let Some(n) = tree.node(nodes.tree_view) {
            if let Some(tv) = n.widget().as_any().downcast_ref::<TreeView>() {
                let oracle = self.hts.borrow();
                let mut changed = false;
                for action in actions {
                    let result =
                        reduce_ordered_selection(&mut tv.selection_state_mut(), action, &*oracle);
                    changed |= result.selection_changed || result.focus_changed;
                }
                if changed {
                    tree.mark_paint_dirty(nodes.body);
                    tree.mark_paint_dirty(nodes.tree_view);
                }
            }
        }
    }

    #[allow(dead_code, reason = "Used by standalone timeline_bound; Showcase preserves HAL-328 keyboard activation.")]
    pub(super) fn handle_input(&mut self, event: &FusedInputEvent) -> bool {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use halcyon::core::rect_authority::LayoutAuthorityGuard;
    use halcyon::core::theme::Theme;
    use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;

    #[test]
    fn bound_rows_settle_at_the_same_viewport_origin_and_height() {
        let mut tree = WidgetTree::new();
        let parent = tree.create_root(Box::new(Flex::column()), None, 0);
        let mut demo = BoundTimelineDemo::new();
        let root = demo.mount(&mut tree, parent);
        let nodes = demo.nodes.unwrap();
        let mut text_engine = FontManagerTextEngine::new();
        let theme = Theme::default();
        let viewport_top = |tree: &WidgetTree, mut node| {
            let mut y = 0.0;
            while node != root {
                y += tree.get_rect(node).unwrap().y;
                node = tree.node(node).unwrap().parent().unwrap();
            }
            y
        };

        for (width, height) in [(780.0, 600.0), (1280.0, 390.0)] {
            demo.update(&mut tree);
            {
                let _layout = LayoutAuthorityGuard::enter();
                let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
                cx.layout_child(root, Constraints::new(width, width, height, height));
            }
            assert_eq!(
                viewport_top(&tree, nodes.scroll_area),
                viewport_top(&tree, nodes.canvas),
            );
            assert_eq!(
                tree.get_rect(nodes.scroll_area).unwrap().height,
                tree.get_rect(nodes.canvas).unwrap().height,
            );
            assert!(demo.needs_settlement.get());
            demo.update(&mut tree);
            assert!(!demo.needs_settlement.get());
            assert!(!demo.published_rows.is_empty());
            let substrate = demo.substrate.as_ref().unwrap().borrow();
            assert!(substrate.snapshot().interval(IntervalId(2)).is_some());
        }
    }
}
