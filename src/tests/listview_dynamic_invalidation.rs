use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::animation::Timeline;
use crate::core::app::{
    RetainedSceneTestFrame, RetainedSceneTestHarness, current_layout_epoch, layout_tree,
    paint_tree, pre_layout_sync_scroll_to_lists, process_linked_scroll_intents,
    test_begin_frame_layout_tracking,
};
use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind, PointerId, PointerType,
};
use crate::core::icons::builtin_registry;
use crate::core::layout::{Constraints, FlexParams};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::LAYER_SCROLLBAR;
use crate::core::rect_authority::{
    LayoutAuthorityGuard, PaintAuthorityGuard, UpdateAuthorityGuard,
};
use crate::core::scene::{RectNode, SceneNode, TextNode};
use crate::core::scroll_container::{HorizontalScrollLink, VerticalScrollLink};
use crate::core::scroll_link::{ScrollIntent, ScrollLinkCoordinator};
use crate::core::scroll_state::{AnchorId, ScrollAnchor, ScrollGroupId};
use crate::core::theme::Theme;
use crate::core::widget::{EventContext, LayoutContext, Widget};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, ExpansionPolicy};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::button::Button;
use crate::widgets::flex::Flex;
use crate::widgets::list_view::{ListView, RowId};
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::tabular_view::column::{Column, ColumnId, ColumnScrollDomain};
use crate::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, ColumnInvalidationScope, DataInvalidationEvent, DataSource,
    Extent, UniverseMode, ValueState,
};

#[derive(Default)]
struct DynamicSourceState {
    rows: Vec<RowId>,
    values: HashMap<(RowId, ColumnId), CellValue>,
    pending_events: Vec<DataInvalidationEvent>,
}

#[derive(Clone)]
struct DynamicSourceHandle(Rc<RefCell<DynamicSourceState>>);

impl DynamicSourceHandle {
    fn set_cell_value(&self, row: RowId, column: ColumnId, value: CellValue) {
        let mut state = self.0.borrow_mut();
        state.values.insert((row, column), value);
        state
            .pending_events
            .push(DataInvalidationEvent::CellsInvalidated {
                rows: vec![row],
                columns: ColumnInvalidationScope::Specific(vec![column]),
            });
    }

    fn set_row_heights(&self, changes: Vec<(RowId, f32)>) {
        self.0
            .borrow_mut()
            .pending_events
            .push(DataInvalidationEvent::RowHeightsChanged { changes });
    }

    fn replace_rows(&self, rows: Vec<RowId>) {
        let mut state = self.0.borrow_mut();
        state.rows = rows;
        state
            .pending_events
            .push(DataInvalidationEvent::BaseOrderReplaced);
    }
}

struct DynamicSource {
    shared: Rc<RefCell<DynamicSourceState>>,
    cached_rows: Vec<RowId>,
}

impl DynamicSource {
    fn new() -> (Self, DynamicSourceHandle) {
        let shared = Rc::new(RefCell::new(DynamicSourceState::default()));
        (
            Self {
                shared: Rc::clone(&shared),
                cached_rows: Vec::new(),
            },
            DynamicSourceHandle(shared),
        )
    }
}

impl DataSource for DynamicSource {
    fn row_ids(&self) -> &[RowId] {
        &self.cached_rows
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.shared.borrow().rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let state = self.shared.borrow();
        if state.rows.contains(&row) {
            ValueState::Ready(
                state
                    .values
                    .get(&(row, column))
                    .cloned()
                    .unwrap_or_else(|| CellValue::String(format!("Row {}", row.raw()))),
            )
        } else {
            ValueState::Missing
        }
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        let mut state = self.shared.borrow_mut();
        self.cached_rows = state.rows.clone();
        std::mem::take(&mut state.pending_events)
    }
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

fn collect_rects<'a>(nodes: &'a [SceneNode], out: &mut Vec<&'a RectNode>) {
    for node in nodes {
        match node {
            SceneNode::Rect(rect) => out.push(rect),
            SceneNode::RetainedFragment(fragment) => collect_rects(&fragment.children, out),
            SceneNode::Clip(clip) => collect_rects(&clip.children, out),
            SceneNode::Layer(layer) => collect_rects(&layer.children, out),
            SceneNode::Scroll(scroll) => collect_rects(&scroll.children, out),
            _ => {}
        }
    }
}

fn dispatch_primary_pointer(
    tree: &mut WidgetTree,
    node: NodeId,
    kind: PointerEventKind,
    position: Point,
) -> bool {
    let widget_id = tree.node(node).unwrap().id;
    let tree_ptr = tree as *mut WidgetTree;
    let mut cx = EventContext::new(
        widget_id,
        tree_ptr,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    let event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (position.x, position.y),
        local_pos: (position.x, position.y),
        kind,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });
    tree.node_mut(node)
        .unwrap()
        .widget
        .handle_event(&mut cx, &event, EventPhase::Target)
}

fn dispatch_key(tree: &mut WidgetTree, node: NodeId, key_code: KeyCode) -> bool {
    let widget_id = tree.node(node).unwrap().id;
    let tree_ptr = tree as *mut WidgetTree;
    let mut cx = EventContext::new(
        widget_id,
        tree_ptr,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    let event = Event::Key(KeyEvent {
        key_code,

        physical_key: crate::core::event::PhysicalKeyCode::Unknown,
        is_down: true,
        is_repeat: false,
        modifiers: Modifiers::default(),
        text: None,
    });
    tree.node_mut(node)
        .unwrap()
        .widget
        .handle_event(&mut cx, &event, EventPhase::Target)
}

fn collect_texts<'a>(nodes: &'a [SceneNode], out: &mut Vec<&'a TextNode>) {
    for node in nodes {
        match node {
            SceneNode::Text(text) => out.push(text),
            SceneNode::RetainedFragment(fragment) => collect_texts(&fragment.children, out),
            SceneNode::Clip(clip) => collect_texts(&clip.children, out),
            SceneNode::Layer(layer) => collect_texts(&layer.children, out),
            SceneNode::Scroll(scroll) => collect_texts(&scroll.children, out),
            _ => {}
        }
    }
}

fn collect_text_clip_stacks(
    nodes: &[SceneNode],
    text: &str,
    active_clips: &mut Vec<RectF>,
    out: &mut Vec<(Point, Vec<RectF>)>,
) {
    for node in nodes {
        match node {
            SceneNode::Text(text_node) if text_node.text == text => {
                out.push((text_node.position, active_clips.clone()));
            }
            SceneNode::RetainedFragment(fragment) => {
                collect_text_clip_stacks(&fragment.children, text, active_clips, out);
            }
            SceneNode::Clip(clip) => {
                active_clips.push(clip.rect);
                collect_text_clip_stacks(&clip.children, text, active_clips, out);
                active_clips.pop();
            }
            SceneNode::Layer(layer) => {
                collect_text_clip_stacks(&layer.children, text, active_clips, out);
            }
            SceneNode::Scroll(scroll) => {
                collect_text_clip_stacks(&scroll.children, text, active_clips, out);
            }
            _ => {}
        }
    }
}

fn clip_contains_point(rect: RectF, point: Point) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

fn build_retained_scroll_list(
    link: VerticalScrollLink,
    row_count: usize,
) -> (WidgetTree, NodeId, NodeId, NodeId) {
    let (source, handle) = DynamicSource::new();
    let rows: Vec<RowId> = (0..row_count).map(|_| RowId::new()).collect();
    handle.replace_rows(rows);

    let mut list_view = ListView::new(source);
    list_view.add_column("Symbol", 280.0);
    list_view.refresh_visible_cells();
    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(120.0));
    let scroll = tree.add_child(
        list,
        Box::new(
            ScrollArea::new_unwired()
                .enable_vertical_scrollbar(true)
                .with_vertical_scroll_link(link),
        ),
        None,
        0,
    );
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    (tree, root, scroll, body)
}

fn collect_scroll_offsets(nodes: &[SceneNode], out: &mut Vec<(f32, f32)>) {
    for node in nodes {
        match node {
            SceneNode::RetainedFragment(fragment) => {
                collect_scroll_offsets(&fragment.children, out)
            }
            SceneNode::Clip(clip) => collect_scroll_offsets(&clip.children, out),
            SceneNode::Layer(layer) => collect_scroll_offsets(&layer.children, out),
            SceneNode::Scroll(scroll) => {
                out.push(scroll.offset);
                collect_scroll_offsets(&scroll.children, out);
            }
            _ => {}
        }
    }
}

#[test]
fn consumer_contract_listview_natural_height_uses_owned_band_and_row_geometry() {
    let (source, handle) = DynamicSource::new();
    let mut list_view = ListView::new(source);

    assert_eq!(list_view.natural_height(), 24.0);

    let rows: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    handle.replace_rows(rows);
    list_view.refresh_visible_cells();
    assert_eq!(list_view.natural_height(), 96.0);

    list_view.set_footer_height(8.0);
    assert_eq!(list_view.natural_height(), 104.0);
}

#[test]
fn consumer_contract_hidden_listview_header_updates_owned_natural_height() {
    let (source, handle) = DynamicSource::new();
    let mut list_view = ListView::new(source);

    assert!(list_view.header_visible());
    assert_eq!(list_view.natural_height(), 24.0);

    list_view.set_header_visible(false);
    assert!(!list_view.header_visible());
    assert_eq!(list_view.natural_height(), 0.0);

    let rows: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    handle.replace_rows(rows);
    list_view.refresh_visible_cells();
    assert_eq!(list_view.natural_height(), 72.0);

    list_view.set_header_visible(true);
    assert!(list_view.header_visible());
    assert_eq!(list_view.natural_height(), 96.0);
}

#[test]
fn hidden_listview_header_moves_owned_body_to_top_edge() {
    let _guard = LayoutAuthorityGuard::enter();
    let (source, _) = DynamicSource::new();
    let mut list_view = ListView::new(source);
    list_view.set_header_visible(false);
    list_view.add_column("Value", 240.0);
    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(240.0));
    let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    test_begin_frame_layout_tracking();
    layout_tree(
        &mut tree,
        root,
        Constraints::tight(Size::new(320.0, 240.0)),
        &theme,
        &mut text_engine,
        None,
    );

    let scroll_rect = tree.get_rect(scroll).unwrap();
    assert_eq!(scroll_rect.y, 0.0);
    assert_eq!(scroll_rect.height, 240.0);
}

#[test]
fn listview_body_layout_size_stays_viewport_sized_after_hydration() {
    let _guard = LayoutAuthorityGuard::enter();
    let (source, handle) = DynamicSource::new();
    let rows: Vec<RowId> = (0..100).map(|_| RowId::new()).collect();
    handle.replace_rows(rows);

    let mut list_view = ListView::new(source);
    list_view.add_column("Symbol", 160.0);
    list_view.add_column("Venue", 160.0);
    list_view.refresh_visible_cells();
    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(120.0));
    let scroll = tree.add_child(
        list,
        Box::new(
            ScrollArea::new_unwired()
                .enable_vertical_scrollbar(true)
                .enable_horizontal_scrollbar(true),
        ),
        None,
        0,
    );
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    test_begin_frame_layout_tracking();
    layout_tree(
        &mut tree,
        root,
        Constraints::tight(Size::new(320.0, 120.0)),
        &theme,
        &mut text_engine,
        None,
    );

    let scroll_rect = tree.get_rect(scroll).unwrap();
    let body_rect = tree.get_rect(body).unwrap();
    let body_content = tree.get_content_size(body).unwrap();

    assert_eq!(scroll_rect.y, 24.0);
    assert_eq!(scroll_rect.height, 96.0);
    assert_eq!(body_rect.height, 96.0);
    assert_eq!(body_content.height, 2400.0);
}

#[test]
fn listview_scroll_area_reconstruction_restores_first_paint_and_thumb() {
    let _guard = LayoutAuthorityGuard::enter();
    let link = VerticalScrollLink::new();
    let constraints = Constraints::tight(Size::new(320.0, 120.0));
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let (mut old_tree, old_root, old_scroll, _) = build_retained_scroll_list(link.clone(), 100);
    pre_layout_sync_scroll_to_lists(&mut old_tree);
    test_begin_frame_layout_tracking();
    layout_tree(
        &mut old_tree,
        old_root,
        constraints,
        &theme,
        &mut text_engine,
        None,
    );
    old_tree
        .node_mut(old_scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .jump_to_y(1200.0);

    let (mut rebuilt_tree, rebuilt_root, rebuilt_scroll, _) = build_retained_scroll_list(link, 100);
    drop(old_tree);

    pre_layout_sync_scroll_to_lists(&mut rebuilt_tree);
    test_begin_frame_layout_tracking();
    layout_tree(
        &mut rebuilt_tree,
        rebuilt_root,
        constraints,
        &theme,
        &mut text_engine,
        None,
    );

    let scroll = rebuilt_tree
        .node(rebuilt_scroll)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScrollArea>()
        .unwrap();
    assert!(approx_eq(scroll.logical_offset().y, 1200.0));
    assert!(approx_eq(scroll.visual_scroll_y(), 1200.0));
    assert_eq!(Widget::scroll_offset(scroll), Some((0.0, 1200.0)));

    let scroll_rect = rebuilt_tree.get_rect(rebuilt_scroll).unwrap();
    let content_height = scroll.content_extent_height();
    let viewport_height = scroll_rect.height;
    let max_scroll = content_height - viewport_height;
    let track_height = scroll_rect.height;
    let thumb_height = (track_height * (track_height / content_height))
        .max(theme.scrollbar_geom.min_thumb_size)
        .min(track_height);
    let expected_thumb_y = scroll_rect.y + (track_height - thumb_height) * (1200.0 / max_scroll);

    let icon_registry = builtin_registry();
    let timeline = Timeline::new();
    let interpolated_rects = HashMap::new();
    let mut tooltip_triggers = Vec::new();
    let _paint_guard = PaintAuthorityGuard::enter();
    let (graph, _) = paint_tree(
        &rebuilt_tree,
        rebuilt_root,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        2.0,
        None,
        &mut tooltip_triggers,
        None,
        None,
        KeyboardAuthorityMode::Navigation,
    );

    let mut offsets = Vec::new();
    collect_scroll_offsets(&graph.root, &mut offsets);
    assert!(offsets.iter().any(|(_, y)| approx_eq(*y, 1200.0)));

    let mut rects = Vec::new();
    collect_rects(&graph.root, &mut rects);
    let thumb = rects
        .iter()
        .find(|node| {
            node.z_index == LAYER_SCROLLBAR
                && approx_eq(
                    node.rect.x,
                    scroll_rect.x + scroll_rect.width - theme.scrollbar_geom.thickness,
                )
                && approx_eq(node.rect.width, theme.scrollbar_geom.thickness)
                && approx_eq(node.rect.height, thumb_height)
        })
        .expect("first painted vertical scrollbar thumb");
    assert!(approx_eq(thumb.rect.y, expected_thumb_y));
    assert!(!approx_eq(thumb.rect.y, scroll_rect.y));
}

#[test]
fn fresh_listview_scroll_intent_cancels_pending_reconstruction_restore() {
    let _guard = LayoutAuthorityGuard::enter();
    let link = VerticalScrollLink::new();
    let constraints = Constraints::tight(Size::new(320.0, 120.0));
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let (mut old_tree, old_root, old_scroll, _) = build_retained_scroll_list(link.clone(), 100);
    pre_layout_sync_scroll_to_lists(&mut old_tree);
    test_begin_frame_layout_tracking();
    layout_tree(
        &mut old_tree,
        old_root,
        constraints,
        &theme,
        &mut text_engine,
        None,
    );
    old_tree
        .node_mut(old_scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .jump_to_y(1200.0);

    let (mut rebuilt_tree, rebuilt_root, rebuilt_scroll, _) = build_retained_scroll_list(link, 100);
    drop(old_tree);
    rebuilt_tree
        .node_mut(rebuilt_scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .jump_to_y(0.0);

    pre_layout_sync_scroll_to_lists(&mut rebuilt_tree);
    test_begin_frame_layout_tracking();
    layout_tree(
        &mut rebuilt_tree,
        rebuilt_root,
        constraints,
        &theme,
        &mut text_engine,
        None,
    );

    let scroll = rebuilt_tree
        .node(rebuilt_scroll)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScrollArea>()
        .unwrap();
    assert!(approx_eq(scroll.logical_offset().y, 0.0));
    assert!(approx_eq(scroll.visual_scroll_y(), 0.0));
}

#[test]
fn listview_scrollable_cells_remain_inside_content_clip_after_scroll() {
    let _guard = LayoutAuthorityGuard::enter();
    let (source, handle) = DynamicSource::new();
    let rows: Vec<RowId> = (0..100).map(|_| RowId::new()).collect();
    let target_text = format!("Row {}", rows[52].raw());
    handle.replace_rows(rows);

    let mut list_view = ListView::new(source);
    list_view.add_column_raw(
        "Pinned",
        Column::new(120.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft),
    );
    list_view.add_column("Scrollable", 180.0);
    list_view.refresh_visible_cells();
    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(120.0));
    let scroll = tree.add_child(
        list,
        Box::new(ScrollArea::new_unwired().enable_vertical_scrollbar(true)),
        None,
        0,
    );
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let constraints = Constraints::tight(Size::new(320.0, 120.0));
    test_begin_frame_layout_tracking();
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .scroll_to_y(1200.0);

    tree.node_mut(body).unwrap().widget.sync_scroll(1200.0);
    tree.mark_layout_dirty(body);
    test_begin_frame_layout_tracking();
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);
    drop(_guard);

    let icon_registry = builtin_registry();
    let timeline = Timeline::new();
    let interpolated_rects = HashMap::new();
    let mut tooltip_triggers = Vec::new();
    let (graph, _) = {
        let _paint = PaintAuthorityGuard::enter();
        paint_tree(
            &tree,
            root,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            2.0,
            None,
            &mut tooltip_triggers,
            None,
            None,
            KeyboardAuthorityMode::Navigation,
        )
    };

    let mut text_clip_stacks = Vec::new();
    collect_text_clip_stacks(
        &graph.root,
        &target_text,
        &mut Vec::new(),
        &mut text_clip_stacks,
    );

    assert!(
        text_clip_stacks.iter().any(|(position, clips)| {
            position.x >= 120.0
                && clips.iter().any(|clip| {
                    (clip.x - 120.0).abs() <= 0.5
                        && (clip.y - 24.0).abs() <= 0.5
                        && clip.width >= 199.0
                        && clip.height >= 2399.0
                        && clip_contains_point(*clip, *position)
                })
        }),
        "scrolled scrollable cell text `{}` must remain inside a content-height scrollable body clip: {:?}",
        target_text,
        text_clip_stacks
    );
}

#[test]
fn retained_listview_horizontal_scroll_repaints_linked_body_without_layout() {
    let (source, handle) = DynamicSource::new();
    let row_id = RowId::new();
    let target_text = format!("Row {}", row_id.raw());
    handle.replace_rows(vec![row_id]);

    let horizontal = HorizontalScrollLink::new();
    let freeze = Rc::new(std::cell::Cell::new(false));
    let mut list_view = ListView::new(source);
    list_view.add_column_raw(
        "Pinned",
        Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft),
    );
    list_view.add_column("Near", 160.0);
    list_view.add_column("Far", 160.0);
    list_view.set_horizontal_scroll_link(horizontal.clone(), freeze.clone());
    list_view.refresh_visible_cells();
    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(72.0));
    let scroll = tree.add_child(
        list,
        Box::new(
            ScrollArea::new_unwired()
                .enable_horizontal_scrollbar(true)
                .with_horizontal_scroll_link(horizontal.clone())
                .with_horizontal_freeze(freeze),
        ),
        None,
        0,
    );
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let constraints = Constraints::tight(Size::new(260.0, 72.0));
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let icons = builtin_registry();
    let mut harness = RetainedSceneTestHarness::new();

    let first = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );
    assert!(first.scene_built);
    let completed_layout_epoch = current_layout_epoch();

    {
        let _update = UpdateAuthorityGuard::enter();
        tree.node_mut(scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .scroll_to(120.0, 0.0);
    }

    let scrolled = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );

    assert!(scrolled.scene_built);
    assert!(scrolled.visual_changed);
    assert!(approx_eq(horizontal.get(), 120.0));
    assert_eq!(current_layout_epoch(), completed_layout_epoch);

    let mut text_clip_stacks = Vec::new();
    collect_text_clip_stacks(
        &scrolled.scene_root,
        &target_text,
        &mut Vec::new(),
        &mut text_clip_stacks,
    );
    assert!(
        text_clip_stacks.iter().any(|(position, clips)| {
            position.x >= 240.0
                && clips.iter().any(|clip| {
                    approx_eq(clip.x, 200.0)
                        && clip.width >= 179.0
                        && clip_contains_point(*clip, *position)
                })
        }),
        "the newly exposed far-column cell must be inside the repainted horizontal content clip: {:?}",
        text_clip_stacks
    );
}

#[test]
fn retained_listview_lifted_cell_values_refresh_during_stationary_drag() {
    use std::sync::Arc;

    use crate::core::event::{
        FusedInputEvent, InteractionDomains, PointerButtonsState, PointerDevice, WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::runtime::binding_registry::BindingRegistry;
    use web_time::Instant;

    let (source, handle) = DynamicSource::new();
    let row = RowId::new();
    handle.replace_rows(vec![row, RowId::new()]);
    let mut list_view = ListView::new(source);
    let lifted = list_view.add_column("Lifted", 140.0);
    let neighbor = list_view.add_column("Neighbor", 140.0);
    handle.set_cell_value(row, lifted, CellValue::Float(10.0));
    handle.set_cell_value(row, neighbor, CellValue::String("Neighbor before".into()));
    let body_widget = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(144.0));
    let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body_widget), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 1);
    let owner = tree.widget_id_for_node(list).unwrap();
    let body_owner = tree.widget_id_for_node(body).unwrap();

    let constraints = Constraints::tight(Size::new(300.0, 144.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = builtin_registry();
    let mut frames = RetainedSceneTestHarness::new();
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let mut bindings = BindingRegistry::new();
    let time = Instant::now();
    macro_rules! frame {
        () => {
            frames.frame_with_appearance(
                &mut tree,
                root,
                constraints,
                &theme,
                &mut text,
                &icons,
                &domains,
                WindowId::PRIMARY,
            )
        };
    }
    macro_rules! route {
        ($input:expr) => {
            router.route_input_event(
                &mut tree,
                $input,
                Some(root),
                &mut text,
                Some(frames.animation_context()),
                domains.primary_state_mut(),
                &mut bindings,
            )
        };
    }

    let refresh = |tree: &mut WidgetTree| {
        let _guard = UpdateAuthorityGuard::enter();
        tree.node_mut(list)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ListView<DynamicSource>>()
            .unwrap()
            .refresh_visible_cells();
        tree.mark_paint_dirty(body);
    };
    let capture = |domains: &InteractionDomains| {
        domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE)
    };
    let has_text = |nodes: &[SceneNode], expected: &str| {
        let mut texts = Vec::new();
        collect_texts(nodes, &mut texts);
        texts.iter().any(|node| node.text == expected)
    };
    let geometry = |tree: &WidgetTree| {
        let list_view = tree
            .node(list)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ListView<DynamicSource>>()
            .unwrap();
        let rows: Vec<_> = list_view
            .test_realized_row_projections()
            .into_iter()
            .map(|row| (row.row_id, row.row_index, row.y_offset, row.height))
            .collect();
        let rects = [list, scroll, body].map(|node| tree.get_absolute_paint_rect(node).unwrap());
        (list_view.test_column_probe_entries(), rows, rects)
    };

    frame!();
    let list_rect = tree.get_absolute_paint_rect(list).unwrap();
    let header_y = (list_rect.y + tree.get_absolute_paint_rect(scroll).unwrap().y) * 0.5;
    let start = Point::new(list_rect.x + 65.0, header_y);
    let held = Point::new(start.x - 12.0, start.y);
    let down = || FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time,
        pos: start,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    let drag_move = || FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time,
        pos: held,
        delta: (-12.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState {
            primary: true,
            ..PointerButtonsState::default()
        },
    };
    route!(down());
    assert_eq!(capture(&domains), Some(owner));
    route!(drag_move());
    frame!();
    frames.advance_transition_time(1.0);
    frame!();
    let original = frames.overlay_fragment(owner).unwrap();
    assert!(has_text(&original, "10.00"));
    let original_body = frames.local_fragment(body_owner).unwrap();
    assert!(!has_text(&original_body, "10.00"));
    assert!(has_text(&original_body, "Neighbor before"));
    frame!();
    assert!(Arc::ptr_eq(
        &original,
        &frames.overlay_fragment(owner).unwrap()
    ));
    let epoch = current_layout_epoch();
    let original_geometry = geometry(&tree);

    handle.set_cell_value(row, lifted, CellValue::Float(20.0));
    handle.set_cell_value(row, neighbor, CellValue::String("Neighbor after".into()));
    refresh(&mut tree);
    refresh(&mut tree);
    assert!(!tree.node(list).unwrap().dirty_paint);
    assert!(frame!().visual_changed);
    let refreshed = frames.overlay_fragment(owner).unwrap();
    assert!(!Arc::ptr_eq(&original, &refreshed));
    assert!(!has_text(&refreshed, "10.00"));
    assert!(has_text(&refreshed, "20.00"));
    let refreshed_body = frames.local_fragment(body_owner).unwrap();
    assert!(!has_text(&refreshed_body, "Neighbor before"));
    assert!(has_text(&refreshed_body, "Neighbor after"));
    assert!(!has_text(&refreshed_body, "20.00"));
    assert_eq!(capture(&domains), Some(owner));
    assert_eq!(geometry(&tree), original_geometry);
    assert_eq!(current_layout_epoch(), epoch);

    handle.set_cell_value(row, neighbor, CellValue::String("Neighbor only".into()));
    refresh(&mut tree);
    frame!();
    assert!(has_text(
        &frames.local_fragment(body_owner).unwrap(),
        "Neighbor only"
    ));
    assert!(Arc::ptr_eq(
        &refreshed,
        &frames.overlay_fragment(owner).unwrap()
    ));

    // A different typed value that still paints as "20.00" leaves the overlay clean.
    handle.set_cell_value(row, lifted, CellValue::Float(20.001));
    refresh(&mut tree);
    frame!();
    assert!(Arc::ptr_eq(
        &refreshed,
        &frames.overlay_fragment(owner).unwrap()
    ));
    refresh(&mut tree);
    frame!();
    assert!(Arc::ptr_eq(
        &refreshed,
        &frames.overlay_fragment(owner).unwrap()
    ));
    assert_eq!(capture(&domains), Some(owner));
    assert_eq!(geometry(&tree), original_geometry);
    assert_eq!(current_layout_epoch(), epoch);

    // Leave a refresh pending across each real drag termination path.
    for cancel in [false, true] {
        if cancel {
            router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
            route!(down());
            route!(drag_move());
            frame!();
            assert_eq!(capture(&domains), Some(owner));
        }
        let terminal_value = if cancel {
            "Cancel value"
        } else {
            "Release value"
        };
        handle.set_cell_value(row, lifted, CellValue::String(terminal_value.into()));
        refresh(&mut tree);
        if cancel {
            router.clear_focus(&mut tree, domains.primary_state_mut());
        } else {
            route!(FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time,
                pos: held,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            });
        }
        assert_eq!(capture(&domains), None);
        frame!();
        frames.advance_transition_time(if cancel { 3.0 } else { 2.0 });
        frame!();
        let idle = frames.overlay_fragment(owner).unwrap();
        assert!(!has_text(&idle, terminal_value));
        assert!(has_text(
            &frames.local_fragment(body_owner).unwrap(),
            terminal_value
        ));

        handle.set_cell_value(row, lifted, CellValue::String("Inactive update".into()));
        refresh(&mut tree);
        frame!();
        assert!(has_text(
            &frames.local_fragment(body_owner).unwrap(),
            "Inactive update"
        ));
        assert!(Arc::ptr_eq(&idle, &frames.overlay_fragment(owner).unwrap()));
    }
}

#[test]
fn retained_listview_pointer_and_keyboard_selection_repaint_body_without_layout() {
    let (source, handle) = DynamicSource::new();
    let rows: Vec<RowId> = (0..5).map(|_| RowId::new()).collect();
    handle.replace_rows(rows.clone());

    let mut list_view = ListView::new(source);
    list_view.add_column("Value", 240.0);
    list_view.refresh_visible_cells();
    let body_widget = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(168.0));
    let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body_widget), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let constraints = Constraints::tight(Size::new(320.0, 168.0));
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let icons = builtin_registry();
    let mut harness = RetainedSceneTestHarness::new();

    let first = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );
    assert!(first.scene_built);
    let completed_layout_epoch = current_layout_epoch();
    let body_rect = tree.get_absolute_paint_rect(body).unwrap();
    let second_row = Point::new(body_rect.x + 12.0, body_rect.y + 36.0);

    dispatch_primary_pointer(&mut tree, body, PointerEventKind::Down, second_row);

    assert!(
        tree.node(body).unwrap().dirty_paint,
        "selection intent must dirty the retained ListViewBody fragment"
    );

    let selected = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );
    assert!(selected.scene_built);
    assert!(selected.visual_changed);
    assert_eq!(current_layout_epoch(), completed_layout_epoch);

    let mut rects = Vec::new();
    collect_rects(&selected.scene_root, &mut rects);
    assert!(
        rects.iter().any(|rect| {
            rect.color == theme.colors.list.row_selected_inactive.into()
                && approx_eq(rect.rect.y, body_rect.y + 24.0)
                && approx_eq(rect.rect.height, 24.0)
        }),
        "the repainted retained body must present the newly selected second row"
    );

    let focused_row = tree
        .node(list)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ListView<DynamicSource>>()
        .unwrap()
        .focused_row();
    assert_eq!(focused_row, Some(rows[1]));

    assert!(dispatch_key(&mut tree, list, KeyCode::Down));
    assert!(
        tree.node(body).unwrap().dirty_paint,
        "keyboard selection intent must dirty the retained ListViewBody fragment"
    );

    let keyboard_selected = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );
    assert!(keyboard_selected.scene_built);
    assert!(keyboard_selected.visual_changed);
    assert_eq!(current_layout_epoch(), completed_layout_epoch);

    let mut rects = Vec::new();
    collect_rects(&keyboard_selected.scene_root, &mut rects);
    assert!(
        rects.iter().any(|rect| {
            rect.color == theme.colors.list.row_selected_inactive.into()
                && approx_eq(rect.rect.y, body_rect.y + 48.0)
                && approx_eq(rect.rect.height, 24.0)
        }),
        "the repainted retained body must present the keyboard-selected third row"
    );
    let mut texts = Vec::new();
    collect_texts(&keyboard_selected.scene_root, &mut texts);
    for row_id in &rows {
        let expected = format!("Row {}", row_id.raw());
        assert!(
            texts.iter().any(|text| text.text == expected),
            "selection repaint must retain the complete five-row body; missing {expected}"
        );
    }

    let focused_row = tree
        .node(list)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ListView<DynamicSource>>()
        .unwrap()
        .focused_row();
    assert_eq!(focused_row, Some(rows[2]));

    dispatch_primary_pointer(&mut tree, body, PointerEventKind::Down, second_row);
    assert!(
        tree.node(body).unwrap().dirty_paint,
        "changing the selection back to the second row must repaint"
    );
    let _ = harness.frame(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text_engine,
        &icons,
    );
    dispatch_primary_pointer(&mut tree, body, PointerEventKind::Down, second_row);
    assert!(
        !tree.node(body).unwrap().dirty_paint,
        "reselecting the already selected row must not dirty retained paint"
    );
}

#[test]
fn listview_paints_fixed_header_chrome_above_hydrated_body() {
    let (source, handle) = DynamicSource::new();
    let row_id = RowId::new();
    let target_text = format!("Row {}", row_id.raw());
    let rows = vec![row_id];
    handle.replace_rows(rows);

    let mut list_view = ListView::new(source);
    list_view.add_column("Symbol", 160.0);
    list_view.add_column("Venue", 160.0);
    list_view.refresh_visible_cells();
    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(48.0));
    let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    test_begin_frame_layout_tracking();
    {
        let _guard = LayoutAuthorityGuard::enter();
        layout_tree(
            &mut tree,
            root,
            Constraints::tight(Size::new(320.0, 48.0)),
            &theme,
            &mut text_engine,
            None,
        );
    }

    assert_eq!(tree.get_rect(list).unwrap().height, 48.0);
    assert_eq!(tree.get_rect(scroll).unwrap().y, 24.0);

    let icon_registry = builtin_registry();
    let timeline = Timeline::new();
    let interpolated_rects = HashMap::new();
    let mut tooltip_triggers = Vec::new();
    let (graph, _) = {
        let _paint = PaintAuthorityGuard::enter();
        paint_tree(
            &tree,
            root,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            2.0,
            None,
            &mut tooltip_triggers,
            None,
            None,
            KeyboardAuthorityMode::Navigation,
        )
    };

    let mut rects = Vec::new();
    collect_rects(&graph.root, &mut rects);
    let mut texts = Vec::new();
    collect_texts(&graph.root, &mut texts);

    let row_color = theme.colors.list.row_background.into();
    let row_index = rects.iter().position(|node| {
        node.color == row_color
            && approx_eq(node.rect.x, 0.0)
            && approx_eq(node.rect.y, 24.0)
            && approx_eq(node.rect.width, 320.0)
            && approx_eq(node.rect.height, 24.0)
    });
    let header_color = theme.colors.list.header_background.into();
    let header_index = rects.iter().position(|node| {
        node.color == header_color
            && approx_eq(node.rect.x, 0.0)
            && approx_eq(node.rect.y, 0.0)
            && approx_eq(node.rect.width, 320.0)
            && approx_eq(node.rect.height, 24.0)
    });
    let boundary_color = theme.colors.borders.strong.into();
    let boundary_index = rects.iter().position(|node| {
        node.color == boundary_color
            && node.z_index > LAYER_SCROLLBAR
            && approx_eq(node.rect.x, 0.0)
            && approx_eq(node.rect.y, 23.5)
            && approx_eq(node.rect.width, 320.0)
            && approx_eq(node.rect.height, 0.5)
    });
    let header_text = texts
        .iter()
        .find(|node| node.text == "Symbol" && node.position.y < 24.0)
        .expect("header text");
    let row_text = texts
        .iter()
        .find(|node| node.text == target_text)
        .expect("hydrated row text");

    assert!(row_index.is_some());
    assert!(header_index.is_some());
    assert!(boundary_index.is_some());
    assert!(header_index.unwrap() > row_index.unwrap());
    assert!(boundary_index.unwrap() > row_index.unwrap());
    assert!(header_text.z_index > row_text.z_index);
    assert!(rects[header_index.unwrap()].z_index > LAYER_SCROLLBAR);
    assert!(rects[boundary_index.unwrap()].z_index > LAYER_SCROLLBAR);
}

#[test]
fn empty_listview_realizes_rows_after_flex_height_growth() {
    let _guard = LayoutAuthorityGuard::enter();
    let (source, handle) = DynamicSource::new();
    let mut list_view = ListView::new(source);
    list_view.add_column("Value", 240.0);
    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_view), None, 0);
    tree.set_flex(list, FlexParams::fixed(24.0));
    let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let constraints = Constraints::tight(Size::new(320.0, 240.0));
    test_begin_frame_layout_tracking();
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    assert_eq!(tree.get_rect(list).unwrap().height, 24.0);
    assert_eq!(tree.get_rect(scroll).unwrap().height, 0.0);
    assert_eq!(tree.get_content_size(body).unwrap().height, 0.0);
    assert!(!tree.node(root).unwrap().dirty_layout);

    tree.set_flex(list, FlexParams::fixed(24.0));
    assert!(!tree.node(list).unwrap().dirty_layout);
    assert!(!tree.node(root).unwrap().dirty_layout);

    let rows: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    handle.replace_rows(rows);
    tree.set_flex(list, FlexParams::fixed(24.0 + 3.0 * 24.0));

    assert!(tree.node(list).unwrap().dirty_layout);
    assert!(tree.node(root).unwrap().dirty_layout);

    tree.node_mut(list)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ListView<DynamicSource>>()
        .unwrap()
        .refresh_visible_cells();

    test_begin_frame_layout_tracking();
    layout_tree(&mut tree, root, constraints, &theme, &mut text_engine, None);

    assert_eq!(tree.get_rect(list).unwrap().height, 96.0);
    assert_eq!(tree.get_content_size(body).unwrap().height, 72.0);
}

struct RetainedXYFixture {
    tree: WidgetTree,
    root: NodeId,
    list: NodeId,
    scroll: NodeId,
    footer: NodeId,
    body: NodeId,
    rows: Vec<RowId>,
    source: DynamicSourceHandle,
    size: Size,
}

impl RetainedXYFixture {
    fn new(
        horizontal: &HorizontalScrollLink,
        area: ScrollArea,
        rows: usize,
        column_width: f32,
    ) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let (list, scroll, footer, body, rows, source) =
            Self::mount(&mut tree, root, horizontal, area, rows, column_width);
        Self {
            tree,
            root,
            list,
            scroll,
            footer,
            body,
            rows,
            source,
            size: Size::new(260.0, 120.0),
        }
    }

    fn mount(
        tree: &mut WidgetTree,
        root: NodeId,
        horizontal: &HorizontalScrollLink,
        area: ScrollArea,
        rows: usize,
        column_width: f32,
    ) -> (
        NodeId,
        NodeId,
        NodeId,
        NodeId,
        Vec<RowId>,
        DynamicSourceHandle,
    ) {
        let (source, handle) = DynamicSource::new();
        let rows: Vec<_> = (0..rows).map(|_| RowId::new()).collect();
        handle.replace_rows(rows.clone());
        let freeze = Rc::new(std::cell::Cell::new(false));
        let mut list = ListView::new(source);
        list.add_column_raw(
            "Pinned",
            Column::new(80.0).with_scroll_domain(ColumnScrollDomain::PinnedLeft),
        );
        list.add_column("Near", column_width);
        list.add_column("Far", column_width);
        list.set_footer_height(16.0);
        list.set_horizontal_scroll_link(horizontal.clone(), freeze.clone());
        list.refresh_visible_cells();
        let body = list.create_body_widget();
        let footer = list.create_footer_widget();
        let list = tree.add_child(root, Box::new(list), None, 0);
        tree.set_flex(list, FlexParams::fixed(120.0));
        let scroll = tree.add_child(list, Box::new(area.with_horizontal_freeze(freeze)), None, 0);
        let body = tree.add_child(scroll, Box::new(body), None, 0);
        tree.node_mut(scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body);
        let footer = tree.add_child(list, Box::new(footer), None, 0);
        (list, scroll, footer, body, rows, handle)
    }

    fn replace(
        &mut self,
        horizontal: &HorizontalScrollLink,
        area: ScrollArea,
        rows: usize,
        column_width: f32,
    ) {
        let root_owner = self.tree.node(self.root).unwrap().id;
        let removed: Vec<_> = self
            .tree
            .iter_nodes()
            .filter(|(node, _)| *node != self.root)
            .map(|(_, node)| node.id)
            .collect();
        self.tree.remove_node(self.list);
        assert!(self.tree.children(self.root).is_empty());
        let removals = self.tree.drain_pending_removals();
        assert_eq!(removals.len(), removed.len());
        for owner in &removed {
            assert!(removals.contains(owner));
            assert!(self.tree.find_by_id(*owner).is_none());
        }
        assert_eq!(self.tree.drain_pending_node_removals().len(), removed.len());

        (
            self.list,
            self.scroll,
            self.footer,
            self.body,
            self.rows,
            self.source,
        ) = Self::mount(
            &mut self.tree,
            self.root,
            horizontal,
            area,
            rows,
            column_width,
        );
        self.tree.mark_layout_dirty(self.root);
        assert_eq!(self.tree.node(self.root).unwrap().id, root_owner);
        assert_eq!(self.tree.children(self.root), vec![self.list]);
        for owner in removed {
            assert!(self.tree.find_by_id(owner).is_none());
        }
        assert_eq!(
            self.tree
                .iter_nodes()
                .filter(|(_, node)| node.widget.as_any().is::<ListView<DynamicSource>>())
                .count(),
            1
        );
        assert_eq!(
            self.tree
                .iter_nodes()
                .filter(|(_, node)| node.widget.as_any().is::<ScrollArea>())
                .count(),
            1
        );
    }

    fn area(&self) -> &ScrollArea {
        self.tree
            .node(self.scroll)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn area_mut(&mut self) -> &mut ScrollArea {
        self.tree
            .node_mut(self.scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    fn scroll_to(&mut self, x: f32, y: f32) {
        let _update = UpdateAuthorityGuard::enter();
        self.area_mut().scroll_to(x, y);
    }

    fn assert_settled(&self, x: f32, y: f32) {
        let area = self.area();
        assert_eq!(area.logical_offset(), Point::new(x, y));
        assert_eq!((area.target_scroll_x(), area.target_scroll_y()), (x, y));
        assert_eq!(area.scroll_container().unwrap().target_offset(), (x, y));
        assert_eq!(area.scroll_container().unwrap().visual_offset(), (x, y));
        assert_eq!(area.scroll_offset(), Some((x, y)));
    }
}

struct RetainedXYFrames {
    harness: RetainedSceneTestHarness,
    theme: Theme,
    text: FontManagerTextEngine,
}

impl RetainedXYFrames {
    fn new() -> Self {
        Self {
            harness: RetainedSceneTestHarness::new(),
            theme: Theme::default(),
            text: FontManagerTextEngine::new(),
        }
    }

    fn frame(&mut self, fixture: &mut RetainedXYFixture) -> RetainedSceneTestFrame {
        self.harness.frame(
            &mut fixture.tree,
            fixture.root,
            Constraints::tight(fixture.size),
            &self.theme,
            &mut self.text,
            &builtin_registry(),
        )
    }

    fn seed(
        &mut self,
        horizontal: &HorizontalScrollLink,
        vertical: &VerticalScrollLink,
        x: f32,
        y: f32,
    ) -> (RetainedXYFixture, Vec<SceneNode>) {
        let mut predecessor = RetainedXYFixture::new(
            horizontal,
            ScrollArea::new_unwired()
                .enable_horizontal_scrollbar(true)
                .enable_vertical_scrollbar(true)
                .with_horizontal_scroll_link(horizontal.clone())
                .with_vertical_scroll_link(vertical.clone()),
            100,
            160.0,
        );
        let initial = self.frame(&mut predecessor);
        assert!(initial.scene_built);
        predecessor.scroll_to(x, y);
        let scrolled = self.frame(&mut predecessor);
        assert!(scrolled.scene_built);
        predecessor.assert_settled(x, y);
        assert_eq!(horizontal.get(), x);
        (predecessor, initial.scene_root)
    }
}

fn assert_retained_xy_scene(
    fixture: &RetainedXYFixture,
    frame: &RetainedSceneTestFrame,
    theme: &Theme,
    x: f32,
    y: f32,
) {
    assert!(
        frame.scene_built,
        "the first restored frame must contain its scene"
    );
    fixture.assert_settled(x, y);
    let mut offsets = Vec::new();
    collect_scroll_offsets(&frame.scene_root, &mut offsets);
    assert!(
        offsets.contains(&(x, y)),
        "actual nested retained Scroll projection: {offsets:?}"
    );
    let rect = fixture.tree.get_rect(fixture.scroll).unwrap();
    let smooth = fixture.area().scroll_container().unwrap();
    let thickness = theme.scrollbar_geom.thickness;
    let track_width = rect.width - thickness;
    let track_height = rect.height - thickness;
    let thumb_width = (track_width * track_width / smooth.content_size.width)
        .max(theme.scrollbar_geom.min_thumb_size)
        .min(track_width);
    let thumb_height = (track_height * track_height / smooth.content_size.height)
        .max(theme.scrollbar_geom.min_thumb_size)
        .min(track_height);
    let thumb_x = rect.x
        + (track_width - thumb_width) * x
            / (smooth.content_size.width - smooth.viewport_size.width);
    let thumb_y = rect.y
        + (track_height - thumb_height) * y
            / (smooth.content_size.height - smooth.viewport_size.height);
    let mut rects = Vec::new();
    collect_rects(&frame.scene_root, &mut rects);
    assert!(
        rects.iter().any(|node| node.z_index == LAYER_SCROLLBAR
            && approx_eq(node.rect.x, thumb_x)
            && approx_eq(node.rect.y, rect.y + rect.height - thickness)
            && approx_eq(node.rect.width, thumb_width)
            && approx_eq(node.rect.height, thickness)),
        "horizontal thumb uses the restored visual coordinate"
    );
    assert!(
        rects.iter().any(|node| node.z_index == LAYER_SCROLLBAR
            && approx_eq(node.rect.x, rect.x + rect.width - thickness)
            && approx_eq(node.rect.y, thumb_y)
            && approx_eq(node.rect.width, thickness)
            && approx_eq(node.rect.height, thumb_height)),
        "vertical thumb uses the restored visual coordinate"
    );
    let owner = fixture.tree.node(fixture.scroll).unwrap().id;
    for (px, py) in [
        (
            thumb_x + thumb_width / 2.0,
            rect.y + rect.height - thickness / 2.0,
        ),
        (
            rect.x + rect.width - thickness / 2.0,
            thumb_y + thumb_height / 2.0,
        ),
    ] {
        let hit = fixture
            .tree
            .hit_test(px, py)
            .expect("published scrollbar overlay hit");
        assert_eq!(hit.widget_id, owner);
        assert_eq!(
            hit.absolute_bounds,
            (rect.x, rect.y, rect.width, rect.height)
        );
        assert!(approx_eq(hit.local_pos.0, px - rect.x));
        assert!(approx_eq(hit.local_pos.1, py - rect.y));
    }
}

fn assert_retained_xy_linked_chrome(
    fixture: &RetainedXYFixture,
    frame: &RetainedSceneTestFrame,
    initial_near_x: f32,
    x: f32,
) {
    let mut texts = Vec::new();
    collect_texts(&frame.scene_root, &mut texts);
    let near_x = texts
        .iter()
        .find(|text| text.text == "Near")
        .unwrap()
        .position
        .x;
    assert!(approx_eq(near_x, initial_near_x - x));
    let footer_rect = fixture.tree.get_rect(fixture.footer).unwrap();
    let mut rects = Vec::new();
    collect_rects(&frame.scene_root, &mut rects);
    assert!(
        rects.iter().any(|node| approx_eq(node.rect.x, 239.0 - x)
            && approx_eq(node.rect.y, footer_rect.y)
            && approx_eq(node.rect.height, 16.0)
            && approx_eq(node.rect.width, 1.0)),
        "fixed footer consumes the same restored X as header/body"
    );
}

#[test]
fn retained_x_reconstruction_restores_first_xy_paint_and_projections() {
    let horizontal = HorizontalScrollLink::new();
    let vertical = VerticalScrollLink::new();
    let mut frames = RetainedXYFrames::new();
    let (mut replacement, initial) = frames.seed(&horizontal, &vertical, 90.0, 240.0);
    let mut baseline_text = Vec::new();
    collect_texts(&initial, &mut baseline_text);
    let initial_near_x = baseline_text
        .iter()
        .find(|text| text.text == "Near")
        .unwrap()
        .position
        .x;
    replacement.replace(
        &horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_retained_horizontal_scroll_link(horizontal.clone())
            .with_vertical_scroll_link(vertical),
        100,
        160.0,
    );
    replacement.tree.sync_linked_horizontal_scroll_projections();
    assert_eq!(horizontal.get(), 90.0);
    {
        let _layout = LayoutAuthorityGuard::enter();
        test_begin_frame_layout_tracking();
        let mut cx = LayoutContext::new(&mut replacement.tree, &mut frames.text, &frames.theme);
        cx.layout_child_full(
            replacement.scroll,
            Constraints::loose(Size::new(260.0, 80.0)),
        );
    }
    replacement.tree.sync_linked_horizontal_scroll_projections();
    assert_eq!(horizontal.get(), 90.0);
    assert_eq!(replacement.area().logical_offset(), Point::new(0.0, 0.0));
    let restored = frames.frame(&mut replacement);
    assert_retained_xy_scene(&replacement, &restored, &frames.theme, 90.0, 240.0);
    assert_eq!(horizontal.get(), 90.0);
    assert_retained_xy_linked_chrome(&replacement, &restored, initial_near_x, 90.0);
    let current_owners: Vec<_> = replacement
        .tree
        .iter_nodes()
        .map(|(_, node)| node.id)
        .collect();
    let epoch = current_layout_epoch();
    // frame_dt is zero; the active scrollbar timer may still request repaint.
    let unchanged_content = frames.frame(&mut replacement);
    assert_eq!(current_layout_epoch(), epoch);
    assert_eq!(
        replacement
            .tree
            .iter_nodes()
            .map(|(_, node)| node.id)
            .collect::<Vec<_>>(),
        current_owners
    );
    replacement.assert_settled(90.0, 240.0);
    assert_eq!(horizontal.get(), 90.0);
    if unchanged_content.scene_built {
        assert_retained_xy_scene(&replacement, &unchanged_content, &frames.theme, 90.0, 240.0);
        assert_retained_xy_linked_chrome(&replacement, &unchanged_content, initial_near_x, 90.0);
    }
}

#[test]
fn retained_x_reconstruction_clamps_to_final_occlusion_bounds() {
    let horizontal = HorizontalScrollLink::new();
    let vertical = VerticalScrollLink::new();
    let mut frames = RetainedXYFrames::new();
    let (mut replacement, _) = frames.seed(&horizontal, &vertical, 140.0, 1200.0);
    replacement.replace(
        &horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_vertical_scroll_link(vertical)
            .with_retained_horizontal_scroll_link(horizontal.clone()),
        10,
        110.0,
    );
    let restored = frames.frame(&mut replacement);
    let thickness = frames.theme.scrollbar_geom.thickness;
    let rect = replacement.tree.get_rect(replacement.scroll).unwrap();
    let expected_x = 300.0 - (rect.width - thickness);
    let expected_y = 240.0 - (rect.height - thickness);
    assert_retained_xy_scene(
        &replacement,
        &restored,
        &frames.theme,
        expected_x,
        expected_y,
    );
    assert_eq!(horizontal.get(), expected_x);
    assert_eq!(
        replacement.area().scroll_container().unwrap().viewport_size,
        Size::new(rect.width - thickness, rect.height - thickness)
    );
    let final_row = *replacement.rows.last().unwrap();
    assert_presented_row_and_select(&mut replacement, &restored, final_row);
}

#[test]
fn retained_x_reconstruction_fresh_intent_wins() {
    let horizontal = HorizontalScrollLink::new();
    let vertical = VerticalScrollLink::new();
    let mut frames = RetainedXYFrames::new();
    let (mut replacement, _) = frames.seed(&horizontal, &vertical, 90.0, 240.0);
    replacement.replace(
        &horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_retained_horizontal_scroll_link(horizontal.clone())
            .with_vertical_scroll_link(vertical.clone()),
        100,
        160.0,
    );
    replacement.scroll_to(0.0, 0.0);
    let reset = frames.frame(&mut replacement);
    assert_retained_xy_scene(&replacement, &reset, &frames.theme, 0.0, 0.0);
    assert_eq!(horizontal.get(), 0.0);
    replacement.scroll_to(70.0, 120.0);
    let scrolled = frames.frame(&mut replacement);
    assert_retained_xy_scene(&replacement, &scrolled, &frames.theme, 70.0, 120.0);
    assert_eq!(horizontal.get(), 70.0);

    let group = ScrollGroupId::new(366);
    replacement.replace(
        &horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_scroll_group(group)
            .with_retained_horizontal_scroll_link(horizontal.clone())
            .with_vertical_scroll_link(vertical),
        100,
        160.0,
    );
    let mut coordinator = ScrollLinkCoordinator::new();
    coordinator.begin_frame();
    let token = coordinator.generate_token();
    coordinator.emit_intent(
        group,
        ScrollIntent::horizontal_anchor_sync(
            Some(ScrollAnchor::new(
                AnchorId::new(366),
                Point::new(10.0, 0.0),
                Point::new(0.0, 0.0),
            )),
            10.0,
            token,
        ),
    );
    {
        let _update = UpdateAuthorityGuard::enter();
        process_linked_scroll_intents(&mut coordinator, &mut replacement.tree);
    }
    assert_eq!(coordinator.pending_count(), 0);
    assert!(replacement.area().has_anchor());
    let frame = frames.frame(&mut replacement);
    assert_retained_xy_scene(&replacement, &frame, &frames.theme, 0.0, 0.0);
    assert_eq!(
        horizontal.get(),
        0.0,
        "accepted linked intent retires stale restoration even when its target cannot resolve"
    );
}

#[test]
fn retained_x_reconstruction_preserves_independent_vertical_convergence() {
    let horizontal = HorizontalScrollLink::new();
    let vertical = VerticalScrollLink::new();
    let mut frames = RetainedXYFrames::new();
    let (mut replacement, _) = frames.seed(&horizontal, &vertical, 90.0, 240.0);
    let local_horizontal = HorizontalScrollLink::new();
    replacement.replace(
        &local_horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_horizontal_scroll_link(local_horizontal.clone())
            .with_vertical_scroll_link(vertical),
        100,
        160.0,
    );
    assert!(frames.frame(&mut replacement).scene_built);
    {
        let _update = UpdateAuthorityGuard::enter();
        replacement.area_mut().scroll_to_y(480.0);
        replacement
            .area_mut()
            .set_retained_horizontal_scroll_link(horizontal.clone());
        replacement.tree.mark_layout_dirty(replacement.scroll);
    }
    let before = replacement
        .area()
        .scroll_container()
        .unwrap()
        .visual_offset()
        .1;
    assert!(before < 480.0);
    let frame = frames.frame(&mut replacement);
    assert!(frame.scene_built);
    let area = replacement.area();
    assert_eq!(area.logical_offset(), Point::new(90.0, 480.0));
    assert_eq!(
        area.scroll_container().unwrap().target_offset(),
        (90.0, 480.0)
    );
    assert_eq!(
        area.scroll_container().unwrap().visual_offset(),
        (90.0, before)
    );
    assert_eq!(horizontal.get(), 90.0);
    {
        let _update = UpdateAuthorityGuard::enter();
        replacement
            .area_mut()
            .scroll_container_mut()
            .unwrap()
            .update_visual(0.016);
    }
    let after = replacement
        .area()
        .scroll_container()
        .unwrap()
        .visual_offset()
        .1;
    assert!(after > before && after < 480.0);
    assert_eq!(replacement.area().horizontal_scroll_offset(), 90.0);
}

#[test]
fn retained_x_reconstruction_legacy_live_alignment_stays_publish_only() {
    let horizontal = HorizontalScrollLink::new();
    let vertical = VerticalScrollLink::new();
    let mut frames = RetainedXYFrames::new();
    let (mut replacement, _) = frames.seed(&horizontal, &vertical, 90.0, 240.0);
    replacement.replace(
        &horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_retained_horizontal_scroll_link(horizontal.clone())
            .with_vertical_scroll_link(vertical.clone())
            .with_horizontal_scroll_link(horizontal.clone()),
        100,
        160.0,
    );
    let first = frames.frame(&mut replacement);
    assert_retained_xy_scene(&replacement, &first, &frames.theme, 0.0, 240.0);
    assert_eq!(horizontal.get(), 0.0);
    replacement.scroll_to(80.0, 240.0);
    assert!(frames.frame(&mut replacement).scene_built);
    assert_eq!(horizontal.get(), 80.0);
    let mut area = ScrollArea::new_unwired()
        .enable_horizontal_scrollbar(true)
        .enable_vertical_scrollbar(true)
        .with_vertical_scroll_link(vertical)
        .with_retained_horizontal_scroll_link(horizontal.clone());
    area.set_horizontal_scroll_link(horizontal.clone());
    replacement.replace(&horizontal, area, 100, 160.0);
    let first = frames.frame(&mut replacement);
    assert_retained_xy_scene(&replacement, &first, &frames.theme, 0.0, 240.0);
    assert_eq!(horizontal.get(), 0.0);
    let epoch = current_layout_epoch();
    replacement.scroll_to(60.0, 240.0);
    let scrolled = frames.frame(&mut replacement);
    assert_retained_xy_scene(&replacement, &scrolled, &frames.theme, 60.0, 240.0);
    assert_eq!(horizontal.get(), 60.0);
    assert_eq!(current_layout_epoch(), epoch);
}

fn collect_presented_row_text(
    nodes: &[SceneNode],
    target: &str,
    offset: (f32, f32),
    clips: &mut Vec<RectF>,
    out: &mut Vec<Point>,
) {
    for node in nodes {
        match node {
            SceneNode::Text(text) if text.text == target => {
                let point = Point::new(text.position.x - offset.0, text.position.y - offset.1);
                if clips.iter().all(|clip| clip_contains_point(*clip, point)) {
                    out.push(point);
                }
            }
            SceneNode::RetainedFragment(fragment) => {
                collect_presented_row_text(&fragment.children, target, offset, clips, out);
            }
            SceneNode::Layer(layer) => {
                collect_presented_row_text(&layer.children, target, offset, clips, out);
            }
            SceneNode::Clip(clip) => {
                clips.push(RectF::new(
                    clip.rect.x - offset.0,
                    clip.rect.y - offset.1,
                    clip.rect.width,
                    clip.rect.height,
                ));
                collect_presented_row_text(&clip.children, target, offset, clips, out);
                clips.pop();
            }
            SceneNode::Scroll(scroll) => {
                collect_presented_row_text(
                    &scroll.children,
                    target,
                    (offset.0 + scroll.offset.0, offset.1 + scroll.offset.1),
                    clips,
                    out,
                );
            }
            _ => {}
        }
    }
}

fn assert_presented_row_and_select(
    fixture: &mut RetainedXYFixture,
    frame: &RetainedSceneTestFrame,
    row_id: RowId,
) {
    assert!(
        frame.scene_built,
        "row proof uses the first canonical scene"
    );
    let body_rect = fixture.tree.get_absolute_paint_rect(fixture.body).unwrap();
    let mut positions = Vec::new();
    collect_presented_row_text(
        &frame.scene_root,
        &format!("Row {}", row_id.raw()),
        (0.0, 0.0),
        &mut Vec::new(),
        &mut positions,
    );
    assert!(
        positions
            .iter()
            .any(|point| clip_contains_point(body_rect, *point)),
        "successor row text must actually be visible after cumulative scroll/clips: {positions:?}"
    );
    let list = fixture
        .tree
        .node(fixture.list)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ListView<DynamicSource>>()
        .unwrap();
    let row = list
        .test_realized_row_projections()
        .into_iter()
        .find(|row| row.row_id == row_id)
        .expect("current realized successor RowId");
    let point = Point::new(
        body_rect.x + 40.0,
        body_rect.y + row.y_offset - fixture.area().visual_scroll_y() + 12.0,
    );
    let hit = fixture
        .tree
        .hit_test(point.x, point.y)
        .expect("visible non-separator row hit");
    assert_eq!(hit.widget_id, fixture.tree.node(fixture.body).unwrap().id);
    dispatch_primary_pointer(
        &mut fixture.tree,
        fixture.body,
        PointerEventKind::Down,
        point,
    );
    assert_eq!(
        fixture
            .tree
            .node(fixture.list)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ListView<DynamicSource>>()
            .unwrap()
            .focused_row(),
        Some(row_id),
    );
}

#[test]
fn retained_x_reconstruction_initializes_current_variable_metrics_before_realization() {
    let horizontal = HorizontalScrollLink::new();
    let vertical = VerticalScrollLink::new();
    let mut frames = RetainedXYFrames::new();
    let (mut replacement, _) = frames.seed(&horizontal, &vertical, 140.0, 1200.0);
    replacement.replace(
        &horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_retained_horizontal_scroll_link(horizontal.clone())
            .with_vertical_scroll_link(vertical),
        10,
        110.0,
    );
    replacement.source.set_row_heights(
        replacement
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| (*row, if index % 2 == 0 { 24.0 } else { 48.0 }))
            .collect(),
    );
    let restored = frames.frame(&mut replacement);
    let rect = replacement.tree.get_rect(replacement.scroll).unwrap();
    let thickness = frames.theme.scrollbar_geom.thickness;
    let expected_x = 300.0 - (rect.width - thickness);
    let expected_y = 360.0 - (rect.height - thickness);
    assert_eq!(replacement.area().content_extent_height(), 360.0);
    assert_eq!(
        replacement
            .tree
            .get_content_size(replacement.body)
            .unwrap()
            .height,
        360.0
    );
    assert_eq!(horizontal.get(), expected_x);
    assert_retained_xy_scene(
        &replacement,
        &restored,
        &frames.theme,
        expected_x,
        expected_y,
    );
    let final_row = *replacement.rows.last().unwrap();
    assert_presented_row_and_select(&mut replacement, &restored, final_row);
}

#[test]
fn retained_x_reconstruction_preserves_known_expansion_metrics_before_realization() {
    let horizontal = HorizontalScrollLink::new();
    let mut frames = RetainedXYFrames::new();
    let mut fixture = RetainedXYFixture::new(
        &horizontal,
        ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .enable_vertical_scrollbar(true)
            .with_horizontal_scroll_link(horizontal.clone()),
        10,
        160.0,
    );
    {
        let list = fixture
            .tree
            .node_mut(fixture.list)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ListView<DynamicSource>>()
            .unwrap();
        list.set_expansion_policy(ExpansionPolicy::SingleExpand);
        list.set_expanded_height_policy(ExpandedHeightPolicy::Fixed(64.0));
        list.set_row_widget_factory(|row| {
            Box::new(Button::with_text(format!("Expanded {}", row.raw())))
        });
    }
    let initial = frames.frame(&mut fixture);
    let expanded_row = fixture.rows[0];
    assert_presented_row_and_select(&mut fixture, &initial, expanded_row);
    assert!(dispatch_key(
        &mut fixture.tree,
        fixture.list,
        KeyCode::Enter
    ));
    let expanded = frames.frame(&mut fixture);
    assert!(expanded.scene_built);
    assert_eq!(fixture.area().content_extent_height(), 304.0);
    let expanded_child = fixture.tree.children(fixture.body)[0];
    let child_owner = fixture.tree.node(expanded_child).unwrap().id;
    let row_owner = fixture.tree.node(fixture.body).unwrap().id;
    assert_eq!(fixture.tree.get_rect(expanded_child).unwrap().height, 64.0);
    fixture.scroll_to(90.0, 0.0);
    assert!(frames.frame(&mut fixture).scene_built);
    assert_eq!(horizontal.get(), 90.0);
    let child_layout_epoch = fixture.tree.node(expanded_child).unwrap().layout_epoch;
    {
        let _update = UpdateAuthorityGuard::enter();
        fixture
            .area_mut()
            .set_retained_horizontal_scroll_link(horizontal.clone());
        fixture.tree.mark_layout_dirty(fixture.scroll);
    }
    let restored = frames.frame(&mut fixture);
    assert_retained_xy_scene(&fixture, &restored, &frames.theme, 90.0, 0.0);
    assert_eq!(fixture.area().content_extent_height(), 304.0);
    assert_eq!(
        fixture.tree.get_content_size(fixture.body).unwrap().height,
        304.0
    );
    assert_eq!(fixture.tree.node(fixture.body).unwrap().id, row_owner);
    assert_eq!(fixture.tree.children(fixture.body), vec![expanded_child]);
    assert_eq!(fixture.tree.node(expanded_child).unwrap().id, child_owner);
    assert_eq!(fixture.tree.get_rect(expanded_child).unwrap().height, 64.0);
    assert_eq!(
        fixture.tree.node(expanded_child).unwrap().layout_epoch,
        child_layout_epoch
    );
    assert!(!fixture.tree.node(expanded_child).unwrap().dirty_layout);
    let body_rect = fixture.tree.get_absolute_paint_rect(fixture.body).unwrap();
    let hit = fixture
        .tree
        .hit_test(body_rect.x + 40.0, body_rect.y + 24.0 + 32.0)
        .expect("existing expanded child hit in the first restored scene");
    assert_eq!(hit.widget_id, child_owner);
    assert_presented_row_and_select(&mut fixture, &restored, expanded_row);
}
