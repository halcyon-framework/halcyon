use std::cell::RefCell;
use std::rc::Rc;

use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind, PointerId, PointerType,
};
use crate::core::layout::Constraints;
use crate::core::math::{Point, Size};
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::theme::Theme;
use crate::core::widget::EventContext;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::list_view::{ListView, ListViewBody, ListViewFooter, RowId};
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::tabular_view::column::{Column, ColumnId};
use crate::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use crate::widgets::tabular_view::intent_policy::TabularIntentPolicy;
use crate::widgets::tabular_view::{ViewConfigKey, ViewConfigStore};

#[derive(Clone, Default)]
struct SharedViewConfigStore {
    value: Rc<RefCell<Option<serde_json::Value>>>,
}

impl ViewConfigStore for SharedViewConfigStore {
    fn load(&self, _key: &ViewConfigKey) -> Option<serde_json::Value> {
        self.value.borrow().clone()
    }

    fn save(&self, _key: &ViewConfigKey, value: &serde_json::Value) {
        *self.value.borrow_mut() = Some(value.clone());
    }

    fn clear(&self, _key: &ViewConfigKey) {
        *self.value.borrow_mut() = None;
    }
}

struct InteractionSource {
    rows: Vec<RowId>,
}

impl InteractionSource {
    fn new(rows: Vec<RowId>) -> Self {
        Self { rows }
    }
}

impl DataSource for InteractionSource {
    fn row_ids(&self) -> &[RowId] {
        &self.rows
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, _column: ColumnId) -> ValueState<CellValue> {
        ValueState::Ready(CellValue::String(format!("Row {}", row.raw())))
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        Vec::new()
    }
}

struct InteractionTree {
    tree: WidgetTree,
    root: NodeId,
    body: NodeId,
}

fn build_tree(
    list_view: ListView<InteractionSource>,
    body: ListViewBody<InteractionSource>,
    footer: ListViewFooter<InteractionSource>,
) -> InteractionTree {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(list_view), None, 0);
    let scroll_area = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll_area, Box::new(body), None, 0);
    tree.node_mut(scroll_area)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(root, Box::new(footer), None, 0);

    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    test_begin_frame_layout_tracking();
    layout_tree(
        &mut tree,
        root,
        Constraints::tight(Size::new(320.0, 192.0)),
        &theme,
        &mut text_engine,
        None,
    );

    InteractionTree { tree, root, body }
}

fn dispatch(
    tree: &mut WidgetTree,
    node: NodeId,
    event: Event,
    keyboard_mode: KeyboardAuthorityMode,
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
        keyboard_mode,
    );
    tree.node_mut(node)
        .unwrap()
        .widget
        .handle_event(&mut cx, &event, EventPhase::Target)
}

fn pointer_event(kind: PointerEventKind, pos: Point) -> Event {
    pointer_event_with_modifiers(kind, pos, Modifiers::default())
}

fn pointer_event_with_modifiers(kind: PointerEventKind, pos: Point, modifiers: Modifiers) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (pos.x, pos.y),
        local_pos: (pos.x, pos.y),
        kind,
        button: Some(PointerButton::Primary),
        modifiers,
        pointer_type: PointerType::Mouse,
    })
}

fn key_event(key_code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        key_code,

        physical_key: crate::core::event::PhysicalKeyCode::Unknown,
        is_down: true,
        is_repeat: false,
        modifiers: Modifiers::default(),
        text: None,
    })
}

fn focused_row(tree: &WidgetTree, root: NodeId) -> Option<RowId> {
    tree.node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ListView<InteractionSource>>()
        .unwrap()
        .focused_row()
}

#[test]
fn committed_column_resize_survives_list_view_reconstruction_through_view_config_store() {
    let store = SharedViewConfigStore::default();
    let column_id = ColumnId::from_raw(70_001);

    let mut first = ListView::new(InteractionSource::new(Vec::new()))
        .with_app_namespace("example-app")
        .with_view_id("market-inspector")
        .with_config_store(Box::new(store.clone()));
    first.add_column_raw(
        "Value",
        Column::with_id(column_id, 120.0)
            .with_min_width(80.0)
            .with_max_width(220.0),
    );
    first.finalize_config();
    assert_eq!(
        first.test_commit_column_resize(column_id, 184.0),
        Some(184.0)
    );
    drop(first);

    let mut reconstructed = ListView::new(InteractionSource::new(Vec::new()))
        .with_app_namespace("example-app")
        .with_view_id("market-inspector")
        .with_config_store(Box::new(store));
    reconstructed.add_column_raw(
        "Value",
        Column::with_id(column_id, 120.0)
            .with_min_width(80.0)
            .with_max_width(220.0),
    );
    reconstructed.finalize_config();

    assert_eq!(
        reconstructed.test_column_preferred_width(column_id),
        Some(184.0)
    );
}

#[test]
fn single_primary_click_notifies_focus_after_borrow_release_and_activates_on_click() {
    let _guard = LayoutAuthorityGuard::enter();
    let rows: Vec<RowId> = (0..12).map(|_| RowId::new()).collect();
    let mut list_view = ListView::new(InteractionSource::new(rows.clone()));
    list_view.add_column("Value", 240.0);
    assert!(list_view.set_intent_policy(TabularIntentPolicy::single()));

    let focused = Rc::new(RefCell::new(Vec::new()));
    let activated = Rc::new(RefCell::new(Vec::new()));
    let weak_core = list_view.test_core_weak();
    let focused_log = focused.clone();
    list_view.set_focused_row_callback(move |row_id| {
        let core = weak_core
            .upgrade()
            .expect("ListView core must remain alive");
        let core = core.borrow();
        assert_eq!(core.focused_row(), row_id);
        if let Some(row_id) = row_id {
            assert!(core.is_selected(row_id));
        }
        focused_log.borrow_mut().push(row_id);
    });
    let activation_log = activated.clone();
    list_view.set_activation_callback(move |row_id| {
        activation_log.borrow_mut().push(row_id);
    });

    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();
    let mut fixture = build_tree(list_view, body, footer);
    let body_rect = fixture.tree.get_absolute_paint_rect(fixture.body).unwrap();
    let first_row = Point::new(body_rect.x + 12.0, body_rect.y + 12.0);

    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event(PointerEventKind::Down, first_row),
        KeyboardAuthorityMode::Navigation,
    );
    assert_eq!(focused_row(&fixture.tree, fixture.root), Some(rows[0]));
    assert_eq!(&*focused.borrow(), &[Some(rows[0])]);
    assert_eq!(&*activated.borrow(), &[rows[0]]);

    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event(PointerEventKind::Down, first_row),
        KeyboardAuthorityMode::Navigation,
    );
    assert_eq!(&*focused.borrow(), &[Some(rows[0])]);
    assert_eq!(&*activated.borrow(), &[rows[0], rows[0]]);
}

#[test]
fn range_drag_routes_through_policy_and_notifies_focused_row() {
    let _guard = LayoutAuthorityGuard::enter();
    let rows: Vec<RowId> = (0..6).map(|_| RowId::new()).collect();
    let mut list_view = ListView::new(InteractionSource::new(rows.clone()));
    list_view.add_column("Value", 240.0);
    assert!(list_view.set_intent_policy(TabularIntentPolicy::multi()));

    let focused = Rc::new(RefCell::new(Vec::new()));
    let focused_log = focused.clone();
    list_view.set_focused_row_callback(move |row_id| {
        focused_log.borrow_mut().push(row_id);
    });
    let activated = Rc::new(RefCell::new(Vec::new()));
    let activation_log = activated.clone();
    list_view.set_activation_callback(move |row_id| {
        activation_log.borrow_mut().push(row_id);
    });
    let weak_core = list_view.test_core_weak();

    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();
    let mut fixture = build_tree(list_view, body, footer);
    let body_rect = fixture.tree.get_absolute_paint_rect(fixture.body).unwrap();
    let first_row = Point::new(body_rect.x + 12.0, body_rect.y + 12.0);
    let third_row = Point::new(body_rect.x + 12.0, body_rect.y + 60.0);
    let fifth_row = Point::new(body_rect.x + 12.0, body_rect.y + 108.0);
    let mut shift = Modifiers::default();
    shift.shift = true;

    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event_with_modifiers(PointerEventKind::Down, first_row, shift),
        KeyboardAuthorityMode::Navigation,
    );
    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event_with_modifiers(PointerEventKind::Move, third_row, shift),
        KeyboardAuthorityMode::Navigation,
    );
    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event_with_modifiers(PointerEventKind::Move, fifth_row, shift),
        KeyboardAuthorityMode::Navigation,
    );

    assert_eq!(
        &*focused.borrow(),
        &[Some(rows[0]), Some(rows[2]), Some(rows[4])]
    );
    assert!(activated.borrow().is_empty());
    let core = weak_core
        .upgrade()
        .expect("ListView core must remain alive");
    let core = core.borrow();
    for row_id in rows.iter().take(5) {
        assert!(core.is_selected(*row_id));
    }
    assert!(!core.is_selected(rows[5]));
    assert_eq!(core.selection.selection_count(), 5);
}

#[test]
fn intent_policy_configuration_locks_after_first_routed_row_interaction() {
    let _guard = LayoutAuthorityGuard::enter();
    let rows: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    let mut list_view = ListView::new(InteractionSource::new(rows));
    list_view.add_column("Value", 240.0);
    assert!(list_view.set_intent_policy(TabularIntentPolicy::single()));
    let weak_core = list_view.test_core_weak();

    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();
    let mut fixture = build_tree(list_view, body, footer);
    let body_rect = fixture.tree.get_absolute_paint_rect(fixture.body).unwrap();
    let first_row = Point::new(body_rect.x + 12.0, body_rect.y + 12.0);
    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event(PointerEventKind::Down, first_row),
        KeyboardAuthorityMode::Navigation,
    );

    assert!(
        weak_core
            .upgrade()
            .expect("ListView core must remain alive")
            .borrow()
            .selection
            .is_interaction_model_locked()
    );
    let list_view = fixture
        .tree
        .node_mut(fixture.root)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ListView<InteractionSource>>()
        .unwrap();
    assert!(!list_view.set_intent_policy(TabularIntentPolicy::multi()));
    assert_eq!(list_view.intent_policy(), TabularIntentPolicy::single());
}

#[test]
fn keyboard_navigation_notifies_focused_row_without_activation() {
    let _guard = LayoutAuthorityGuard::enter();
    let rows: Vec<RowId> = (0..12).map(|_| RowId::new()).collect();
    let mut list_view = ListView::new(InteractionSource::new(rows));
    list_view.add_column("Value", 240.0);
    assert!(list_view.set_intent_policy(TabularIntentPolicy::single()));

    let focused = Rc::new(RefCell::new(Vec::new()));
    let focused_log = focused.clone();
    list_view.set_focused_row_callback(move |row_id| {
        focused_log.borrow_mut().push(row_id);
    });
    let activated = Rc::new(RefCell::new(Vec::new()));
    let activation_log = activated.clone();
    list_view.set_activation_callback(move |row_id| {
        activation_log.borrow_mut().push(row_id);
    });

    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();
    let mut fixture = build_tree(list_view, body, footer);

    let keys = [
        (KeyCode::Down, KeyboardAuthorityMode::Navigation),
        (KeyCode::Down, KeyboardAuthorityMode::Navigation),
        (KeyCode::Up, KeyboardAuthorityMode::Navigation),
        (KeyCode::End, KeyboardAuthorityMode::Navigation),
        (KeyCode::Home, KeyboardAuthorityMode::Navigation),
        (KeyCode::PageDown, KeyboardAuthorityMode::Navigation),
        (KeyCode::PageUp, KeyboardAuthorityMode::Navigation),
        (KeyCode::Character('j'), KeyboardAuthorityMode::Capture),
        (KeyCode::Character('k'), KeyboardAuthorityMode::Capture),
    ];

    for (index, (key, mode)) in keys.into_iter().enumerate() {
        assert!(dispatch(
            &mut fixture.tree,
            fixture.root,
            key_event(key),
            mode,
        ));
        let current = focused_row(&fixture.tree, fixture.root);
        assert_eq!(focused.borrow().len(), index + 1);
        assert_eq!(focused.borrow().last().copied(), Some(current));
    }
    assert!(activated.borrow().is_empty());
}

#[test]
fn double_click_and_enter_activation_callbacks_remain_intact() {
    let _guard = LayoutAuthorityGuard::enter();
    let rows: Vec<RowId> = (0..3).map(|_| RowId::new()).collect();
    let mut list_view = ListView::new(InteractionSource::new(rows.clone()));
    list_view.add_column("Value", 240.0);
    assert!(list_view.set_intent_policy(TabularIntentPolicy::multi()));

    let activated = Rc::new(RefCell::new(Vec::new()));
    let activation_log = activated.clone();
    list_view.set_activation_callback(move |row_id| {
        activation_log.borrow_mut().push(row_id);
    });

    let body = list_view.create_body_widget();
    let footer = list_view.create_footer_widget();
    let mut fixture = build_tree(list_view, body, footer);
    let body_rect = fixture.tree.get_absolute_paint_rect(fixture.body).unwrap();
    let first_row = Point::new(body_rect.x + 12.0, body_rect.y + 12.0);

    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event(PointerEventKind::Down, first_row),
        KeyboardAuthorityMode::Navigation,
    );
    assert!(activated.borrow().is_empty());

    dispatch(
        &mut fixture.tree,
        fixture.body,
        pointer_event(PointerEventKind::DoubleClick, first_row),
        KeyboardAuthorityMode::Navigation,
    );
    assert_eq!(&*activated.borrow(), &[rows[0]]);

    assert!(dispatch(
        &mut fixture.tree,
        fixture.root,
        key_event(KeyCode::Enter),
        KeyboardAuthorityMode::Navigation,
    ));
    assert_eq!(&*activated.borrow(), &[rows[0], rows[0]]);
}
