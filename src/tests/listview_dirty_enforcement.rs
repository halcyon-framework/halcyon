//! Mounted ListView residency through the current provider, HTS and CMS owners.

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::app::{RetainedSceneTestFrame, RetainedSceneTestHarness};
use crate::core::composite_text_edit::{CompositeEditOutcome, CompositeEditorId};
use crate::core::event::{
    FusedInputEvent, InteractionDomains, KeyCode, KeyboardAuthorityMode, Modifiers,
    PhysicalKeyCode, PointerButton, PointerDevice, PointerId, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icons::builtin_registry;
use crate::core::layout::Constraints;
use crate::core::math::{Point, RectF, Size};
use crate::core::scene::SceneNode;
use crate::core::theme::Theme;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::engines::cell_mechanics::{CompositeChoiceReadiness, CompositeChoiceRoleKey};
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, ExpansionPolicy};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::actions::ActionId;
use crate::runtime::binding_registry::BindingRegistry;
use crate::utils::id::TreeWidgetId;
use crate::widgets::checkbox::Checkbox;
use crate::widgets::label::Label;
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::tabular_view::column::{Column, ColumnId, ColumnScrollDomain};
use crate::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, ColumnInvalidationScope, DataInvalidationEvent, DataSource,
    Extent, OrderAccess, OrderRevision, OrderWindow, OrderWindowRequest, PolicyInvalidationScope,
    UniverseMode, ValueState,
};
use crate::widgets::tabular_view::intents::{ExpansionIntent, TabularIntent};
use crate::widgets::tabular_view::row::RowId;
use crate::widgets::{ComboBox, ListView};
use web_time::Instant;

const ROW_COUNT: usize = 80;
const ROW_HEIGHT: f32 = 24.0;
const VIEW_HEIGHT: f32 = 168.0;
const PICKED: &str = "Picked";

struct Provider {
    rows: Vec<RowId>,
    revision: OrderRevision,
    extent: Extent,
    changed_value: Option<RowId>,
    denied: Option<RowId>,
    bool_column: Option<ColumnId>,
    locator_reply: Option<(RowId, ValueState<Option<usize>>)>,
    writes: Vec<(RowId, ColumnId, CellValue)>,
    events: Vec<DataInvalidationEvent>,
}

impl Provider {
    fn new() -> Self {
        Self {
            rows: (0..ROW_COUNT)
                .map(|i| RowId::from_raw(10_000 + i as u64 * 17))
                .collect(),
            revision: OrderRevision(1),
            extent: Extent::LowerBound(ROW_COUNT - 1),
            changed_value: None,
            denied: None,
            bool_column: None,
            locator_reply: None,
            writes: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl DataSource for Provider {
    fn order_access(&self) -> OrderAccess {
        OrderAccess::Windowed
    }

    fn order_revision(&self) -> OrderRevision {
        self.revision
    }

    fn order_window(&self, request: OrderWindowRequest) -> ValueState<OrderWindow> {
        if request.revision != self.revision {
            return ValueState::Error {
                error: Some("stale order revision".into()),
            };
        }
        let start = request.start.min(self.rows.len());
        let end = start.saturating_add(request.length).min(self.rows.len());
        ValueState::Ready(OrderWindow {
            revision: self.revision,
            start,
            row_ids: self.rows[start..end].to_vec(),
        })
    }

    fn locate_row(&self, revision: OrderRevision, row: RowId) -> ValueState<Option<usize>> {
        if revision != self.revision {
            return ValueState::Error {
                error: Some("stale order revision".into()),
            };
        }
        if let Some((_, reply)) = self.locator_reply.as_ref().filter(|(id, _)| *id == row) {
            return reply.clone();
        }
        ValueState::Ready(self.rows.iter().position(|id| *id == row))
    }

    fn extent(&self) -> Extent {
        self.extent
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        if let Some((_, _, value)) = self
            .writes
            .iter()
            .rev()
            .find(|(written_row, written_column, _)| *written_row == row && *written_column == column)
        {
            return ValueState::Ready(value.clone());
        }
        if self.bool_column == Some(column) {
            return ValueState::Ready(CellValue::Bool(false));
        }
        let mut text = row.raw().to_string();
        if self.changed_value == Some(row) {
            text.push_str(" updated");
        }
        ValueState::Ready(CellValue::String(text))
    }

    fn capabilities(&self, row: RowId, column: ColumnId) -> CellCapabilities {
        CellCapabilities {
            editable: true,
            committable: self.denied != Some(row) || self.bool_column == Some(column),
            ..CellCapabilities::default()
        }
    }

    fn commit_cell(&mut self, row: RowId, column: ColumnId, value: CellValue) -> bool {
        // Record and accept even a misrouted attempt. Provider rejection must
        // never be what makes an unready-input assertion pass.
        self.writes.push((row, column, value));
        self.events.push(DataInvalidationEvent::CellsInvalidated {
            rows: vec![row],
            columns: ColumnInvalidationScope::Specific(vec![column]),
        });
        true
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.events)
    }
}

struct MountedList {
    tree: WidgetTree,
    root: NodeId,
    scroll: NodeId,
    body: NodeId,
    choice_column: ColumnId,
    bool_column: Option<ColumnId>,
    rows: Vec<RowId>,
    router: EventRouter,
    domains: InteractionDomains,
    bindings: BindingRegistry,
    text: FontManagerTextEngine,
    frames: RetainedSceneTestHarness,
    animation_time_secs: f64,
}

impl MountedList {
    fn new() -> Self {
        Self::with_columns(ColumnScrollDomain::Scrollable, false)
    }

    fn with_columns(choice_domain: ColumnScrollDomain, with_bool: bool) -> Self {
        let provider = Provider::new();
        let rows = provider.rows.clone();
        let mut list = ListView::new(provider);
        let choice_column =
            list.add_column_raw("Choice", Column::new(120.0).with_scroll_domain(choice_domain));
        let bool_column = with_bool.then(|| list.add_column("Flag", 80.0));
        list.adapter_mut().source_mut().bool_column = bool_column;
        let options = std::iter::once(PICKED.to_string()).chain(rows
            .iter()
            .flat_map(|row| [row.raw().to_string(), format!("{} updated", row.raw())]))
            .map(|value| (value.clone(), value))
            .collect();
        list.set_column_combo_options(choice_column, options);
        list.set_expansion_policy(ExpansionPolicy::SingleExpand);
        list.set_expanded_height_policy(ExpandedHeightPolicy::Fixed(24.0));
        list.set_row_widget_factory(|row| Box::new(Label::new(format!("Detail {}", row.raw()))));
        let scroll = list.take_body_scroll_area();
        let body = list.create_body_widget();
        let footer = list.create_footer_widget();

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(list), None, 0);
        let scroll = tree.add_child(root, Box::new(scroll), None, 0);
        let body = tree.add_child(scroll, Box::new(body), None, 0);
        tree.node_mut(scroll)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body);
        tree.add_child(root, Box::new(footer), None, 0);

        let mut list = Self {
            tree,
            root,
            scroll,
            body,
            choice_column,
            bool_column,
            rows,
            router: EventRouter::new(),
            domains: InteractionDomains::new(),
            bindings: BindingRegistry::new(),
            text: FontManagerTextEngine::new(),
            frames: RetainedSceneTestHarness::new(),
            animation_time_secs: 0.0,
        };
        list.settle();
        list
    }

    fn list(&self) -> &ListView<Provider> {
        self.tree
            .node(self.root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn change_provider(&mut self, change: impl FnOnce(&mut Provider)) {
        {
            let node = self.tree.node_mut(self.root).unwrap();
            let list = node
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<ListView<Provider>>()
                .unwrap();
            change(list.adapter_mut().source_mut());
        }
        self.tree.mark_layout_dirty(self.body);
    }

    fn phase_two(&mut self) {
        self.tree.process_composite_choice_work(
            &mut self.router,
            self.domains.primary_state_mut(),
            &mut self.text,
            (self.animation_time_secs * 1000.0) as u64,
        );
    }

    fn compose(&mut self) -> RetainedSceneTestFrame {
        self.frames.frame_with_appearance(
            &mut self.tree,
            self.root,
            Constraints::tight(Size::new(240.0, VIEW_HEIGHT)),
            &Theme::light(),
            &mut self.text,
            &builtin_registry(),
            &self.domains,
            WindowId::PRIMARY,
        )
    }

    fn frame(&mut self) -> RetainedSceneTestFrame {
        self.phase_two();
        self.compose()
    }

    fn settle(&mut self) {
        // Layout publishes the current window. The next Phase 2 settles its mounts.
        self.frame();
        self.frame();
    }

    fn scroll_to(&mut self, y: f32) {
        self.request_scroll(y);
        self.settle();
    }

    fn request_scroll(&mut self, y: f32) {
        self.tree
            .node_mut(self.scroll)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .scroll_to(0.0, y);
        self.tree.mark_layout_dirty(self.body);
    }

    fn focus(&mut self, owner: TreeWidgetId) {
        self.router.request_focus_for_widget(
            &mut self.tree,
            owner,
            self.domains.primary_state_mut(),
        );
    }

    fn clear_focus(&mut self) {
        self.router
            .clear_focus(&mut self.tree, self.domains.primary_state_mut());
    }

    fn mouse_capture(&self) -> Option<TreeWidgetId> {
        self.domains.primary_state().pointer_capture_manager.current_capture(PointerId::MOUSE)
    }

    fn route(&mut self, input: FusedInputEvent) {
        self.router.route_input_event(
            &mut self.tree,
            input,
            Some(self.root),
            &mut self.text,
            Some(self.frames.animation_context()),
            self.domains.primary_state_mut(),
            &mut self.bindings,
        );
    }

    fn enter(&mut self) {
        let time = Instant::now();
        self.route(FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time,
            key_code: KeyCode::Enter,
            physical_key: PhysicalKeyCode::Enter,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        });
        self.route(FusedInputEvent::KeyUp {
            window_id: WindowId::PRIMARY,
            time,
            key_code: KeyCode::Enter,
            physical_key: PhysicalKeyCode::Enter,
            reservation: None,
            modifiers: Modifiers::default(),
        });
    }

    fn pointer_down(&mut self, pos: Point) {
        self.route(FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        });
    }

    fn pointer_up(&mut self, pos: Point) {
        self.route(FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        });
    }

    fn open_popup(&mut self, owner: TreeWidgetId) -> (Point, RetainedSceneTestFrame) {
        self.focus(owner);
        assert!(self.router.handle_engine_action(
            &mut self.tree,
            ActionId::ENTER_WIDGET,
            Some(self.root),
            self.domains.primary_state_mut(),
            &mut self.text,
            Some(self.frames.animation_context()),
        ));
        self.frame();
        self.animation_time_secs += 0.25;
        self.frames.advance_transition_time(self.animation_time_secs);
        let frame = self.frame();
        let geometry = self.tree.combo_popup_input_geometry(owner).expect("ready popup");
        let host = self.tree.get_absolute_paint_rect(self.tree.find_by_id(owner).unwrap()).unwrap();
        let first_row = Point::new(
            host.x + geometry.rows.x + 5.0 - geometry.scroll_offset.0,
            host.y + geometry.rows.y + 5.0 - geometry.scroll_offset.1,
        );
        assert!(self.tree.hit_test(first_row.x, first_row.y).is_some_and(|hit| hit.widget_id == owner));
        (first_row, frame)
    }

    fn relation(&self, owner: TreeWidgetId) -> CompositeEditorId {
        self.tree.composite_editor_id(owner).expect("live Choice relation")
    }

    fn non_ready(&self, row: RowId) -> Option<CompositeChoiceReadiness> {
        let core = self.list().test_core_weak().upgrade().unwrap();
        core.borrow().cms.test_composite_choice_non_ready(CompositeChoiceRoleKey {
            row_id: row.raw(),
            column_id: self.choice_column,
        })
    }

    fn writes(&self) -> Vec<(RowId, ColumnId, CellValue)> {
        let core = self.list().test_core_weak().upgrade().unwrap();
        core.borrow().adapter.source().writes.clone()
    }

    fn combo(&self, owner: TreeWidgetId) -> &ComboBox {
        self.tree.node(self.tree.find_by_id(owner).unwrap()).unwrap()
            .widget.as_any().downcast_ref().unwrap()
    }

    fn cells(&self) -> HashMap<String, TreeWidgetId> {
        // Each provider value is unique in this single Choice column.
        let choices = self
            .tree
            .children(self.body)
            .into_iter()
            .filter_map(|node| {
                let node = self.tree.node(node).unwrap();
                let widget = node.widget.as_any().downcast_ref::<ComboBox>()?;
                Some((widget.input_text().to_owned(), node.id))
            });
        let mut cells = HashMap::new();
        for (value, owner) in choices {
            assert!(cells.insert(value, owner).is_none(), "duplicate cell mount");
        }
        cells
    }

    fn row_detail(&self) -> TreeWidgetId {
        self.tree
            .children(self.body)
            .into_iter()
            .map(|node| self.tree.node(node).unwrap())
            .find(|node| node.widget.as_any().is::<Label>())
            .expect("the expanded row must have a real detail widget")
            .id
    }
}

fn scene_contains(nodes: &[SceneNode], predicate: &impl Fn(&SceneNode) -> bool) -> bool {
    nodes.iter().any(|node| {
        predicate(node) || {
            let children = match node {
                SceneNode::RetainedFragment(node) => node.children.as_ref(),
                SceneNode::Clip(node) => node.children.as_slice(),
                SceneNode::Scroll(node) => node.children.as_slice(),
                SceneNode::Layer(node) => node.children.as_slice(),
                _ => return false,
            };
            scene_contains(children, predicate)
        }
    })
}

fn scene_has_text(frame: &RetainedSceneTestFrame, text: &str) -> bool {
    scene_contains(&frame.scene_root, &|node| matches!(node, SceneNode::Text(run) if run.text == text))
}

#[test]
fn listview_dirty_mounted_reuses_rows_and_cells_on_updates_and_reorder() {
    let mut mounted = MountedList::new();
    let row = mounted.rows[0];
    let peer = mounted.rows[1];
    let core = mounted.list().test_core_weak().upgrade().unwrap();
    core.borrow_mut()
        .dispatch_intent(&TabularIntent::Expansion(ExpansionIntent::Expand {
            row_id: row,
        }));
    mounted.tree.mark_layout_dirty(mounted.body);
    mounted.settle();

    let detail = mounted.row_detail();
    let before = mounted.cells();
    let row_value = row.raw().to_string();
    let peer_value = peer.raw().to_string();
    let row_mount = before[&row_value];
    let peer_mount = before[&peer_value];
    let clean = mounted.frames.local_fragment(row_mount).unwrap();
    mounted.frame();
    assert!(Arc::ptr_eq(
        &clean,
        &mounted.frames.local_fragment(row_mount).unwrap(),
    ));

    let column = mounted.choice_column;
    mounted.change_provider(|provider| {
        provider.changed_value = Some(row);
        provider
            .events
            .push(DataInvalidationEvent::CellsInvalidated {
                rows: vec![row],
                columns: ColumnInvalidationScope::Specific(vec![column]),
            });
    });
    mounted.settle();
    let updated_value = format!("{} updated", row.raw());
    let cells = mounted.cells();
    assert_eq!(cells[&updated_value], row_mount);
    assert_eq!(cells[&peer_value], peer_mount);
    assert_eq!(mounted.row_detail(), detail);

    // Coalesced extent and cell invalidation retain the existing instances.
    assert_eq!(mounted.list().row_count(), ROW_COUNT - 1);
    mounted.change_provider(|provider| {
        provider.changed_value = None;
        provider
            .events
            .push(DataInvalidationEvent::CellsInvalidated {
                rows: vec![row],
                columns: ColumnInvalidationScope::Specific(vec![column]),
            });
        provider.extent = Extent::Exact(ROW_COUNT);
        provider.events.push(DataInvalidationEvent::ExtentChanged);
    });
    mounted.settle();
    assert_eq!(mounted.list().row_count(), ROW_COUNT);
    assert_eq!(mounted.cells()[&row_value], row_mount);
    assert_eq!(mounted.cells()[&peer_value], peer_mount);
    assert_eq!(mounted.row_detail(), detail);

    let old_epoch = core.borrow().cms.hts().base_row_order_epoch();
    mounted.change_provider(|provider| {
        provider.rows.swap(0, 1);
        provider.revision = OrderRevision(2);
        provider
            .events
            .push(DataInvalidationEvent::BaseOrderReplaced);
    });
    mounted.settle();

    let new_projection = mounted.list().test_realized_row_projections();
    assert_eq!(new_projection[0].row_id, peer);
    assert_eq!(new_projection[1].row_id, row);
    assert_eq!(
        core.borrow().cms.hts().base_row_order_epoch(),
        old_epoch + 1
    );
    assert_eq!(mounted.cells()[&row_value], row_mount);
    assert_eq!(mounted.cells()[&peer_value], peer_mount);
    assert_eq!(mounted.row_detail(), detail);

    mounted.scroll_to(ROW_HEIGHT);
    assert_eq!(mounted.cells()[&row_value], row_mount);
    assert_eq!(mounted.row_detail(), detail);
}

#[test]
fn listview_dirty_scrolled_cells_retain_overlap_and_evict_unpinned() {
    let mut mounted = MountedList::new();
    let row = mounted.rows[0];
    let row_value = row.raw().to_string();
    let core = mounted.list().test_core_weak().upgrade().unwrap();
    core.borrow_mut().action_select_row(row);
    let initial = mounted.cells();
    let first_mount = initial[&row_value];
    let limit = (VIEW_HEIGHT / ROW_HEIGHT).ceil() as usize + 5;
    assert!(initial.len() <= limit);

    mounted.scroll_to(ROW_HEIGHT * 3.0);
    let overlapping = mounted.cells();
    assert!(overlapping.len() <= limit);
    let mut survivor_count = 0;
    for (value, owner) in &overlapping {
        if let Some(original) = initial.get(value) {
            assert_eq!(owner, original, "a cell survived the window shift");
            survivor_count += 1;
        }
    }
    assert!(survivor_count > 0);

    for row_offset in [20.0, 40.0, 60.0] {
        mounted.scroll_to(ROW_HEIGHT * row_offset);
        let scrolled = mounted.cells();
        assert!(!scrolled.is_empty());
        assert!(scrolled.len() <= limit);
        assert!(!scrolled.contains_key(&row_value));
    }
    assert!(core.borrow().selection.is_selected(row));

    mounted.scroll_to(0.0);
    assert_ne!(mounted.cells()[&row_value], first_mount);
    assert!(core.borrow().selection.is_selected(row));
}

#[test]
fn listview_dirty_focused_choice_remains_resident_outside_scroll_window() {
    let mut mounted = MountedList::new();
    let row_value = mounted.rows[0].raw().to_string();
    let owner = mounted.cells()[&row_value];
    let target = mounted.relation(owner);
    mounted.focus(owner);
    assert_eq!(
        mounted.domains.primary_state().focus_manager.focused(),
        Some(owner),
    );

    mounted.request_scroll(ROW_HEIGHT * 30.0);
    mounted.frame();
    let visible = mounted.list().test_realized_row_projections();
    assert!(!visible.iter().any(|row| row.row_id == mounted.rows[0]));
    let domain = mounted.domains.primary_state();
    assert_eq!(domain.focus_manager.focused(), Some(owner));
    assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
    assert!(domain.pointer_capture_manager.active_pointers().is_empty());

    mounted.phase_two();
    assert_eq!(
        mounted.cells().get(&row_value),
        Some(&owner),
        "HAL-189 §7 pins the concrete editor while it owns focus",
    );
    assert_eq!(
        mounted.domains.primary_state().focus_manager.focused(),
        Some(owner),
    );
    assert_eq!(mounted.relation(owner), target);
    let scene = mounted.compose();
    assert_eq!(mounted.tree.get_rect(mounted.tree.find_by_id(owner).unwrap()), Some(RectF::zero()));
    assert!(!scene_has_text(&scene, &row_value), "offscreen mount has no old placement");

    mounted.clear_focus();
    assert!(mounted.domains.primary_state().focus_manager.focused().is_none());
    assert!(mounted.domains.primary_state().pointer_capture_manager.active_pointers().is_empty());
    assert!(mounted.combo(owner).popup_input_revision().is_none());
    mounted.frame();
    assert_eq!(mounted.tree.validate_composite_editor(&target), Err(CompositeEditOutcome::Retired));
    assert!(mounted.tree.find_by_id(owner).is_none());
}

#[test]
fn listview_dirty_policy_invalidation_preserves_resident_choice_identity() {
    for choice_domain in [ColumnScrollDomain::Scrollable, ColumnScrollDomain::PinnedLeft] {
        let mut mounted = MountedList::with_columns(choice_domain, true);
        let [row, peer] = [mounted.rows[0], mounted.rows[1]];
        let [row_value, peer_value] = [row.raw().to_string(), peer.raw().to_string()];
        let cells = mounted.cells();
        let [owner, peer_owner] = [cells[&row_value], cells[&peer_value]];
        let [target, peer_target] = [mounted.relation(owner), mounted.relation(peer_owner)];
        let flag_column = mounted.bool_column.unwrap();
        let choice_column = mounted.choice_column;
        let host_node = mounted.tree.find_by_id(owner).unwrap();
        let host_rect = mounted.tree.get_absolute_paint_rect(host_node).unwrap();
        let (flag, flag_point) = mounted.tree.children(mounted.body).into_iter().find_map(|node_id| {
            let node = mounted.tree.node(node_id).unwrap();
            let rect = mounted.tree.get_absolute_paint_rect(node_id).unwrap();
            let point = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
            (node.widget.as_any().is::<Checkbox>() && point.y >= host_rect.y && point.y < host_rect.bottom())
                .then_some((node.id, point))
        }).expect("real Bool cell in the same row");
        let flag_mount = mounted.tree.concrete_mount(flag).unwrap();

        let (option, before) = mounted.open_popup(owner);
        assert!(scene_has_text(&before, &row_value));
        assert!(scene_has_text(&before, PICKED), "the already-open popup is in the composed scene");
        let old_local = mounted.frames.local_fragment(owner).unwrap();
        let old_overlay = mounted.frames.overlay_fragment(owner).unwrap();
        let old_fragment = |node: &SceneNode| matches!(node, SceneNode::RetainedFragment(fragment)
            if Arc::ptr_eq(&fragment.children, &old_local) || Arc::ptr_eq(&fragment.children, &old_overlay));
        assert!(scene_contains(&before.scene_root, &old_fragment));
        mounted.pointer_down(option);
        assert_eq!(mounted.mouse_capture(), Some(owner));
        assert_eq!(mounted.domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);

        mounted.change_provider(|provider| {
            provider.denied = Some(row);
            provider.events.push(DataInvalidationEvent::PolicyInvalidated {
                scope: PolicyInvalidationScope::Rows(vec![row]),
            });
        });
        mounted.phase_two();
        assert_eq!(mounted.non_ready(row), Some(CompositeChoiceReadiness::PolicyDenied));
        assert_eq!(mounted.relation(owner), target);
        assert!(!mounted.tree.composite_editor_ready(&target));
        assert!(mounted.combo(owner).is_enabled(), "readiness leaves enabled policy alone");
        assert!(mounted.tree.composite_editor_ready(&peer_target));
        assert!(mounted.tree.combo_popup_input_geometry(owner).is_none());
        assert!(mounted.tree.hit_test(option.x, option.y).is_none_or(|hit| hit.widget_id != owner),
            "the old popup entry has no input authority even before successor paint");

        mounted.enter();
        mounted.pointer_up(option);
        assert_eq!(mounted.mouse_capture(), None, "ordinary pointer release still settles");
        assert!(mounted.writes().is_empty(), "withheld input cannot reach the provider");
        assert_eq!(mounted.tree.composite_editor_snapshot(&target).unwrap().text, row_value);

        let successor = mounted.compose();
        assert!(successor.scene_built);
        assert!(mounted.list().test_realized_row_projections().iter().any(|p| p.row_id == row));
        assert_eq!(mounted.cells().get(&row_value), Some(&owner));
        assert!(!scene_contains(&successor.scene_root, &old_fragment));
        assert!(!scene_has_text(&successor, &row_value), "the cell stays claimed without fallback");
        assert!(!scene_has_text(&successor, PICKED), "no stale popup in the composed scene");
        assert!(scene_has_text(&successor, &peer_value), "the ready sibling still paints");
        assert!(mounted.tree.combo_popup_input_geometry(owner).is_none());

        assert!(mounted.tree.hit_test(flag_point.x, flag_point.y).is_some_and(|hit| hit.widget_id == flag));
        mounted.pointer_down(flag_point);
        mounted.pointer_up(flag_point);
        assert_eq!(mounted.writes(), [(row, flag_column, CellValue::Bool(true))]);
        let (peer_option, _) = mounted.open_popup(peer_owner);
        mounted.pointer_down(peer_option);
        mounted.pointer_up(peer_option);
        assert_eq!(mounted.writes(), [
            (row, flag_column, CellValue::Bool(true)),
            (peer, choice_column, CellValue::String(PICKED.into())),
        ]);
        assert_eq!(mounted.relation(peer_owner), peer_target);
        assert_eq!(mounted.tree.concrete_mount(flag).unwrap(), flag_mount);
        assert_eq!(mounted.non_ready(row), Some(CompositeChoiceReadiness::PolicyDenied));

        let refreshed = format!("{row_value} updated");
        mounted.change_provider(|provider| {
            provider.denied = None;
            provider.changed_value = Some(row);
            provider.events.push(DataInvalidationEvent::PolicyInvalidated {
                scope: PolicyInvalidationScope::Rows(vec![row]),
            });
        });
        let recovery = mounted.frame();
        assert_eq!(mounted.relation(owner), target);
        assert!(mounted.tree.composite_editor_ready(&target));
        assert_eq!(mounted.non_ready(row), None);
        assert_eq!(mounted.tree.composite_editor_snapshot(&target).unwrap().text, refreshed);
        assert!(scene_has_text(&recovery, &refreshed));
        assert!(mounted.tree.combo_popup_input_geometry(owner).is_none());
        assert_eq!(mounted.writes().len(), 2);

        let (fresh_option, _) = mounted.open_popup(owner);
        mounted.pointer_down(fresh_option);
        mounted.pointer_up(fresh_option);
        assert_eq!(mounted.writes()[2..], [(row, choice_column, CellValue::String(PICKED.into()))]);
        assert_eq!(mounted.relation(owner), target);
        assert_eq!(mounted.relation(peer_owner), peer_target);
        assert_eq!(mounted.tree.concrete_mount(flag).unwrap(), flag_mount);

        let cold = mounted.rows[30];
        let cold_value = cold.raw().to_string();
        mounted.change_provider(|provider| {
            provider.denied = Some(cold);
            provider.events.push(DataInvalidationEvent::PolicyInvalidated {
                scope: PolicyInvalidationScope::Rows(vec![cold]),
            });
        });
        mounted.request_scroll(ROW_HEIGHT * 30.0);
        mounted.frame();
        let cold_scene = mounted.frame();
        assert_eq!(mounted.non_ready(cold), Some(CompositeChoiceReadiness::PolicyDenied));
        assert!(!mounted.cells().contains_key(&cold_value));
        assert!(scene_has_text(&cold_scene, &cold_value), "a cell without an editor still paints its ordinary value");
    }
}

#[test]
fn listview_dirty_pointer_capture_pins_choice_without_focus() {
    let mut mounted = MountedList::new();
    let row_value = mounted.rows[0].raw().to_string();
    let owner = mounted.cells()[&row_value];
    let target = mounted.relation(owner);
    let (option, _) = mounted.open_popup(owner);
    mounted.pointer_down(option);
    assert_eq!(mounted.mouse_capture(), Some(owner));
    mounted.clear_focus();
    assert!(mounted.combo(owner).popup_input_revision().is_none());
    mounted.request_scroll(ROW_HEIGHT * 30.0);
    mounted.frame();
    assert!(mounted.domains.primary_state().focus_manager.focused().is_none());
    assert_eq!(mounted.domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Navigation);
    assert_eq!(mounted.mouse_capture(), Some(owner));
    mounted.phase_two();
    assert_eq!(mounted.relation(owner), target);
    assert_eq!(mounted.mouse_capture(), Some(owner));
    mounted.compose();
    mounted.pointer_up(option);
    assert_eq!(mounted.mouse_capture(), None);
    assert!(mounted.writes().is_empty());
    mounted.frame();
    assert_eq!(mounted.tree.validate_composite_editor(&target), Err(CompositeEditOutcome::Retired));
}

#[test]
fn listview_dirty_pinned_choice_waits_for_locator_and_retires_on_confirmed_absence() {
    let mut mounted = MountedList::new();
    let row = mounted.rows[0];
    let owner = mounted.cells()[&row.raw().to_string()];
    let target = mounted.relation(owner);
    mounted.focus(owner);
    mounted.scroll_to(ROW_HEIGHT * 30.0);

    for (reply, reason) in [
        (ValueState::Pending, CompositeChoiceReadiness::Pending),
        (ValueState::Denied { reason: None }, CompositeChoiceReadiness::Denied),
        (ValueState::Error { error: None }, CompositeChoiceReadiness::Error),
        (ValueState::Missing, CompositeChoiceReadiness::Missing),
    ] {
        mounted.change_provider(|provider| {
            provider.locator_reply = Some((row, reply));
            provider.revision = OrderRevision(provider.revision.0 + 1);
            provider.events.push(DataInvalidationEvent::BaseOrderReplaced);
        });
        mounted.settle();
        assert_eq!(mounted.relation(owner), target);
        assert_eq!(mounted.non_ready(row), Some(reason));
        assert!(!mounted.tree.composite_editor_ready(&target));
        assert_eq!(mounted.domains.primary_state().focus_manager.focused(), Some(owner));
        assert!(mounted.tree.combo_popup_input_geometry(owner).is_none());
    }

    mounted.change_provider(|provider| provider.locator_reply = None);
    mounted.frame();
    assert_eq!(mounted.relation(owner), target);
    assert!(mounted.tree.composite_editor_ready(&target));
    mounted.change_provider(|provider| {
        provider.rows.retain(|id| *id != row);
        provider.revision = OrderRevision(provider.revision.0 + 1);
        provider.events.push(DataInvalidationEvent::RowsRemoved { rows: vec![row] });
    });
    assert_eq!(mounted.domains.primary_state().focus_manager.focused(), Some(owner));
    mounted.phase_two();
    assert_eq!(mounted.tree.validate_composite_editor(&target), Err(CompositeEditOutcome::Retired));
    assert!(mounted.tree.find_by_id(owner).is_none());
    assert!(mounted.tree.find_by_id(target.child()).is_none());
    assert!(mounted.tree.drain_pending_removals().contains(&owner));
}
