//! TreeView smoke tests (SPEC-HAL-259)
//!
//! Validates that TreeView is driven by HierarchicalTabularSubstrate
//! and correctly exposes tree state via engine composition.
//!
//! INV-TV-1: HTS is sole topology authority.
//! INV-TV-6: No physical row_index search in hit-test.

#![cfg(test)]

use crate::core::app::{
    layout_tree, pre_layout_sync_scroll_to_lists, test_begin_frame_layout_tracking,
};
use crate::core::event::KeyboardAuthorityMode;
use crate::core::layout::{Constraints, FlexParams};
use crate::core::math::Size;
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::scroll_container::VerticalScrollLink;
use crate::core::theme::Theme;
use crate::core::widget::{EventContext, Widget};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::engines::hierarchical::NodeMeta;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::utils::id::TreeWidgetId;
use crate::widgets::flex::Flex;
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::tabular_view::column::{Column, ColumnModel};
use crate::widgets::tree_view::TreeView;

/// The same mounted native composite is used by pointer and App-policy proof.
pub(super) struct TreeActivationFixture {
    pub(super) tree: WidgetTree,
    pub(super) root: NodeId,
    pub(super) tree_node: NodeId,
    pub(super) body: NodeId,
    pub(super) neighbor: NodeId,
    pub(super) domains: crate::core::event::InteractionDomains,
    pub(super) router: crate::core::event_router::EventRouter,
    pub(super) pending: std::rc::Rc<std::cell::RefCell<Vec<u64>>>,
    app: crate::core::app::AppRoot,
    fusion: crate::core::input_fusion::InputFusion,
    text: FontManagerTextEngine,
    retained: crate::core::app::RetainedSceneTestHarness,
}

impl TreeActivationFixture {
    pub(super) fn new(policy: crate::core::event::UiInteractionPolicy, callback: bool) -> Self {
        Self::with_view(
            policy,
            callback,
            make_tree_view(build_simple_tree()),
            ScrollArea::new_unwired(),
        )
    }

    fn with_view(
        policy: crate::core::event::UiInteractionPolicy,
        callback: bool,
        mut view: TreeView,
        scroll_area: ScrollArea,
    ) -> Self {
        use crate::core::app::app_root_overlay_input_tests::RecordingInputScreen;
        use crate::core::app::{AppConfig, AppContext, AppRoot, RetainedSceneTestHarness};
        use crate::widgets::{screen_host::ScreenHost, text_field::TextField};
        use std::{cell::RefCell, rc::Rc};
        let pending = Rc::new(RefCell::new(Vec::new()));
        view.set_header_visible(false);
        if callback {
            let output = Rc::clone(&pending);
            let topology = view.hts_handle();
            view.set_activation_callback(move |id| {
                // Public source reads must be lawful inside callback delivery.
                let _ = topology.borrow().revision();
                output.borrow_mut().push(id);
            });
        }
        let body = view.create_body_widget();
        let footer = view.create_footer_widget();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let tree_node = tree.add_child(root, Box::new(view), None, 0);
        tree.set_flex(tree_node, FlexParams::fixed(120.0));
        let scroll = tree.add_child(tree_node, Box::new(scroll_area), None, 0);
        let body = tree.add_child(scroll, Box::new(body), None, 0);
        tree.node_mut(scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body);
        tree.add_child(tree_node, Box::new(footer), None, 0);
        let neighbor = tree.add_child(
            root,
            Box::new(TextField::new().with_text("neighbor")),
            None,
            0,
        );
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        app.push_screen(Box::new(RecordingInputScreen {
            host: ScreenHost::new(),
            events: Rc::new(RefCell::new(Vec::new())),
        }));
        let mut fixture = Self {
            tree,
            root,
            tree_node,
            body,
            neighbor,
            domains: AppConfig::default()
                .with_ui_interaction_policy(policy)
                .interaction_domains(),
            router: crate::core::event_router::EventRouter::new(),
            pending,
            app,
            fusion: crate::core::input_fusion::InputFusion::new(),
            text: FontManagerTextEngine::new(),
            retained: RetainedSceneTestHarness::new(),
        };
        fixture.frame();
        fixture
    }

    pub(super) fn frame(&mut self) {
        self.retained.frame_with_appearance(
            &mut self.tree,
            self.root,
            Constraints::tight(Size::new(320.0, 200.0)),
            &Theme::dark(),
            &mut self.text,
            &crate::core::icon::IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        );
    }

    pub(super) fn owner(&self, node: NodeId) -> TreeWidgetId {
        self.tree.widget_id_for_node(node).unwrap()
    }

    pub(super) fn view(&self) -> &TreeView {
        self.tree
            .node(self.tree_node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn view_mut(&mut self) -> &mut TreeView {
        self.tree
            .node_mut(self.tree_node)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    pub(super) fn row_point(&self, id: u64) -> crate::core::math::Point {
        let body = self
            .tree
            .node(self.body)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::tree_view::TreeViewBody>()
            .unwrap();
        let row = body
            .test_realized_row_probe_entries()
            .into_iter()
            .find(|row| row.row_id == id)
            .unwrap();
        let rect = self.tree.get_absolute_paint_rect(self.body).unwrap();
        crate::core::math::Point::new(rect.x + 80.0, rect.y + row.rect.y + row.rect.height * 0.5)
    }

    pub(super) fn raw(&mut self, raw: crate::core::event::RawInputEvent) {
        crate::core::app::app_root_overlay_input_tests::dispatch_physical_raw(
            &mut self.app,
            &mut self.tree,
            &mut self.router,
            self.domains.primary_state_mut(),
            &mut self.fusion,
            &mut self.text,
            raw,
        );
    }

    pub(super) fn button(&mut self, point: crate::core::math::Point, down: bool) {
        self.raw(crate::core::event::RawInputEvent::PointerButton {
            window_id: crate::core::event::WindowId::PRIMARY,
            time: std::time::Instant::now(),
            device: crate::core::event::PointerDevice::Mouse,
            button: crate::core::event::PointerButton::Primary,
            state: if down {
                crate::core::event::RawButtonState::Pressed
            } else {
                crate::core::event::RawButtonState::Released
            },
            position: point,
        });
    }

    pub(super) fn click(&mut self, id: u64) {
        let p = self.row_point(id);
        self.button(p, true);
        self.button(p, false);
    }

    fn move_to(&mut self, point: crate::core::math::Point) {
        self.raw(crate::core::event::RawInputEvent::PointerMoved {
            window_id: crate::core::event::WindowId::PRIMARY,
            time: std::time::Instant::now(),
            device: crate::core::event::PointerDevice::Mouse,
            position: point,
            delta: (40.0, 40.0),
        });
    }

    pub(super) fn key(
        &mut self,
        key: crate::core::event::KeyCode,
        down: bool,
        repeat: bool,
        modifiers: crate::core::event::Modifiers,
    ) {
        self.raw(crate::core::event::RawInputEvent::Key {
            window_id: crate::core::event::WindowId::PRIMARY,
            time: std::time::Instant::now(),
            key_code: key,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            state: if down {
                crate::core::event::KeyState::Down
            } else {
                crate::core::event::KeyState::Up
            },
            repeat,
            modifiers,
        });
    }
}

#[test]
fn hal371_treeview_focus_owned_node_activation() {
    use crate::core::event::{KeyCode, Modifiers, PointerId, UiInteractionPolicy};
    let mut f = TreeActivationFixture::new(UiInteractionPolicy::NavigationAndCapture, true);
    let p = f.row_point(1);
    f.button(p, true);
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(f.owner(f.tree_node))
    );
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner(f.body))
    );
    assert!(f.view().selection_state().selected_keys.contains(&1));
    assert!(f.pending.borrow().is_empty());
    f.frame(); // A normal retained frame between press and release must not cancel the click.
    f.button(p, false);
    assert_eq!(&*f.pending.borrow(), &[1]);
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    f.button(p, false);
    assert_eq!(&*f.pending.borrow(), &[1]);
    f.click(1); // The router replaces this physical Down with DoubleClick.
    f.click(1); // The third physical Down is TripleClick.
    assert_eq!(&*f.pending.borrow(), &[1, 1, 1]);
    f.pending.borrow_mut().clear();
    f.click(2);
    assert_eq!(&*f.pending.borrow(), &[2]);
    f.key(KeyCode::Enter, true, false, Modifiers::default());
    f.key(KeyCode::Enter, true, true, Modifiers::default());
    f.key(KeyCode::Enter, false, false, Modifiers::default());
    f.key(KeyCode::F2, true, false, Modifiers::default());
    assert_eq!(&*f.pending.borrow(), &[2, 2]);
    f.click(0);
    f.key(KeyCode::Enter, true, false, Modifiers::default());
    assert_eq!(&*f.pending.borrow(), &[2, 2]);
    assert!(!f.view().header_visible());
    f.click(1);
    f.pending.borrow_mut().clear();
    f.view_mut().collapse(0);
    f.frame();
    // The old row-axis identity must not authorize a now-hidden leaf.
    f.key(KeyCode::Enter, true, false, Modifiers::default());
    assert!(f.pending.borrow().is_empty());
}

#[test]
fn hal371_treeview_focus_owned_node_activation_cancellation() {
    use crate::core::event::{CancellationReason, PointerId, UiInteractionPolicy};
    for case in 0..8 {
        let mut f = TreeActivationFixture::new(UiInteractionPolicy::NavigationAndCapture, true);
        let p = f.row_point(1);
        f.button(p, true);
        match case {
            0 => {
                let other = f.row_point(2);
                f.button(other, false);
            }
            1 => {
                f.move_to(crate::core::math::Point::new(-10.0, -10.0));
                f.move_to(p);
            }
            2 => {
                f.move_to(crate::core::math::Point::new(p.x + 40.0, p.y));
                assert_eq!(
                    f.domains
                        .primary_state()
                        .pointer_capture_manager
                        .current_capture(PointerId::MOUSE),
                    None,
                    "DragStart must terminate the press"
                );
            }
            3 => {
                f.view_mut().set_tree_data(build_simple_tree());
                f.frame();
            }
            4 => {
                f.view_mut().collapse(0);
                f.frame();
            }
            5 => {
                f.tree.remove_node(f.tree_node);
                f.frame();
            }
            6 => {
                f.router.cancel_pointer_session_with_tree(
                    &mut f.tree,
                    PointerId::MOUSE,
                    CancellationReason::SystemCancel,
                    f.domains.primary_state_mut(),
                );
            }
            7 => f.button(p, true), // A second physical Down cannot restart the held press.
            _ => unreachable!(),
        }
        f.button(p, false);
        assert!(f.pending.borrow().is_empty(), "canceled case {case}");
        assert_eq!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
    }
}

#[test]
fn hal371_treeview_focus_owned_node_activation_excluded_routes() {
    use crate::core::event::{
        KeyCode, Modifiers, PointerButton, PointerDevice, PointerId, RawButtonState, RawInputEvent,
        UiInteractionPolicy, WindowId,
    };
    let mut f = TreeActivationFixture::new(UiInteractionPolicy::NavigationAndCapture, true);
    f.click(1);
    f.pending.borrow_mut().clear();
    let mut modifiers = Modifiers::default();
    modifiers.ctrl = true;
    f.key(KeyCode::Control, true, false, modifiers);
    // A different row makes this a real single Down, not native DoubleClick.
    let p = f.row_point(2);
    f.button(p, true);
    assert!(f.view().selection_state().selected_keys.contains(&1));
    assert!(f.view().selection_state().selected_keys.contains(&2));
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    f.button(p, false);
    f.key(KeyCode::Control, false, false, Modifiers::default());
    assert!(f.pending.borrow().is_empty());

    let p = f.row_point(1);
    f.button(p, true);
    f.raw(RawInputEvent::PointerButton {
        window_id: WindowId::PRIMARY,
        time: std::time::Instant::now(),
        device: PointerDevice::Mouse,
        button: PointerButton::Secondary,
        state: RawButtonState::Pressed,
        position: p,
    });
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    f.raw(RawInputEvent::PointerButton {
        window_id: WindowId::PRIMARY,
        time: std::time::Instant::now(),
        device: PointerDevice::Mouse,
        button: PointerButton::Secondary,
        state: RawButtonState::Released,
        position: p,
    });
    f.button(p, false);
    assert!(f.pending.borrow().is_empty());

    let body = f
        .tree
        .node(f.body)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<crate::widgets::tree_view::TreeViewBody>()
        .unwrap();
    let zone = body
        .test_disclosure_hit_zone_probe_entries()
        .into_iter()
        .find(|zone| zone.node_id == 0)
        .unwrap()
        .rect;
    let rect = f.tree.get_absolute_paint_rect(f.body).unwrap();
    let p = crate::core::math::Point::new(
        rect.x + zone.x + zone.width * 0.5,
        rect.y + zone.y + zone.height * 0.5,
    );
    f.button(p, true);
    f.button(p, false);
    assert!(f.pending.borrow().is_empty());
}

#[test]
fn hal371_treeview_descendant_capture_blur_retirement() {
    use crate::core::event::{KeyCode, Modifiers, PointerId, UiInteractionPolicy};
    let mut f = TreeActivationFixture::new(UiInteractionPolicy::NavigationAndCapture, true);
    let p = f.row_point(1);
    f.button(p, true);
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner(f.body))
    );
    f.key(KeyCode::Tab, true, false, Modifiers::default());
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(f.owner(f.neighbor))
    );
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    f.move_to(p);
    f.button(p, false);
    f.key(KeyCode::Enter, true, false, Modifiers::default());
    assert!(f.pending.borrow().is_empty());
    f.click(1);
    assert_eq!(&*f.pending.borrow(), &[1]);
}

#[test]
fn hal371_treeview_descendant_capture_blur_retirement_authentication() {
    use crate::core::event::{CaptureMode, InteractionDomains, PointerId};
    use std::{
        any::Any,
        cell::{Cell, RefCell},
        rc::Rc,
    };
    struct Probe {
        id: crate::utils::id::WidgetInternalId,
        request: Rc<Cell<Option<(PointerId, TreeWidgetId)>>>,
        events: Rc<RefCell<Vec<&'static str>>>,
    }
    impl crate::core::widget::WidgetDowncast for Probe {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }
    impl Widget for Probe {
        fn id(&self) -> crate::utils::id::WidgetInternalId {
            self.id
        }
        fn layout(
            &mut self,
            _cx: &mut crate::core::widget::LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> crate::core::layout::LayoutResult {
            crate::core::layout::LayoutResult::new(Size::new(100.0, 30.0), Size::new(100.0, 30.0))
        }
        fn paint(
            &self,
            _cx: &mut crate::core::widget::PaintContext,
            _rect: crate::core::math::RectF,
            _children: &[NodeId],
        ) {
        }
        fn focusable(&self) -> bool {
            true
        }
        fn on_focus(&mut self, _cx: &mut EventContext) {
            self.events.borrow_mut().push("focus");
        }
        fn on_blur(&mut self, cx: &mut EventContext) {
            if let Some((pointer, body)) = self.request.get() {
                cx.retire_descendant_pointer_capture(pointer, body);
            }
        }
        fn on_pointer_capture_cancel(
            &mut self,
            _cx: &mut EventContext,
            _pointer: PointerId,
            _reason: crate::core::event::CancellationReason,
        ) {
            self.events.borrow_mut().push("cancel");
        }
    }
    for case in 0..4 {
        let events = Rc::new(RefCell::new(Vec::new()));
        let request = Rc::new(Cell::new(None));
        let make = |request| Probe {
            id: crate::utils::id::WidgetInternalId::new(),
            request,
            events: Rc::clone(&events),
        };
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let parent = tree.add_child(root, Box::new(make(Rc::clone(&request))), None, 0);
        let body = tree.add_child(parent, Box::new(make(Rc::new(Cell::new(None)))), None, 0);
        let sibling = tree.add_child(root, Box::new(make(Rc::new(Cell::new(None)))), None, 0);
        let parent = tree.widget_id_for_node(parent).unwrap();
        let body = tree.widget_id_for_node(body).unwrap();
        let sibling = tree.widget_id_for_node(sibling).unwrap();
        let mut domains = InteractionDomains::new();
        let state = domains.primary_state_mut();
        let mut router = crate::core::event_router::EventRouter::new();
        router.request_focus_for_widget(&mut tree, parent, state);
        let (holder, requested) = match case {
            0 => (body, body),
            1 => (sibling, body), // ancestry alone cannot retire a different holder
            2 => (sibling, sibling), // holder match alone cannot retire a sibling
            3 => (parent, parent), // strict descendant, not self
            _ => unreachable!(),
        };
        state
            .pointer_capture_manager
            .request_capture_with_mode(PointerId::MOUSE, holder, CaptureMode::Hard)
            .unwrap();
        state
            .pointer_capture
            .capture(holder, crate::core::event::PointerButton::Primary);
        request.set(Some((PointerId::MOUSE, requested)));
        events.borrow_mut().clear();
        router.request_focus_for_widget(&mut tree, sibling, state);
        assert_eq!(
            state
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            if case == 0 { None } else { Some(holder) }
        );
        assert_eq!(
            &*events.borrow(),
            if case == 0 {
                &["cancel", "focus"][..]
            } else {
                &["focus"][..]
            }
        );
    }
}

#[test]
fn hal371_treeview_focus_owned_node_activation_no_callback_and_shift() {
    use crate::core::event::{KeyCode, Modifiers, PointerId, UiInteractionPolicy};
    let mut f = TreeActivationFixture::new(UiInteractionPolicy::NavigationAndCapture, false);
    let p = f.row_point(1);
    f.button(p, true);
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    f.button(p, false);
    f.key(KeyCode::Enter, true, false, Modifiers::default());
    assert!(f.pending.borrow().is_empty());
    assert!(f.view().selection_state().selected_keys.contains(&1));

    let mut f = TreeActivationFixture::new(UiInteractionPolicy::NavigationAndCapture, true);
    f.click(1);
    f.pending.borrow_mut().clear();
    let mut modifiers = Modifiers::default();
    modifiers.shift = true;
    f.key(KeyCode::Shift, true, false, modifiers);
    let p = f.row_point(2);
    f.button(p, true);
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner(f.body))
    );
    assert!(f.view().selection_state().selected_keys.contains(&1));
    assert!(f.view().selection_state().selected_keys.contains(&2));
    f.key(KeyCode::Shift, false, false, Modifiers::default());
    f.key(KeyCode::Tab, true, false, Modifiers::default());
    // No HAL371 arm: parent Blur must not become blanket capture cancellation.
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner(f.body))
    );
    f.button(p, false);
    assert!(f.pending.borrow().is_empty());
}

/// Build a simple 3-node tree: root(0) → child(1), child(2).
fn build_simple_tree() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 0,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Root".to_string(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: Some(0),
            depth: 1,
            has_children: false,
            label: "Child 1".to_string(),
        },
        NodeMeta {
            node_id: 2,
            parent_index: Some(0),
            depth: 1,
            has_children: false,
            label: "Child 2".to_string(),
        },
    ]
}

/// Build a deeper tree: root(0) → child(1) → grandchild(2).
fn build_deep_tree() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 0,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Root".to_string(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: "Child".to_string(),
        },
        NodeMeta {
            node_id: 2,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "Grandchild".to_string(),
        },
    ]
}

/// Build a deep tree for hit-test validation:
///   root(0)
///     ├─ child(1) [has_children]
///     │   ├─ grandchild(3)
///     │   └─ grandchild(4)
///     └─ child(2) [has_children]
///         └─ grandchild(5)
///             └─ great_grandchild(6)
fn build_hit_test_tree() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 0,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Root".to_string(),
        },
        NodeMeta {
            node_id: 1,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: "Child 1".to_string(),
        },
        NodeMeta {
            node_id: 2,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: "Child 2".to_string(),
        },
        NodeMeta {
            node_id: 3,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "Grandchild 3".to_string(),
        },
        NodeMeta {
            node_id: 4,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "Grandchild 4".to_string(),
        },
        NodeMeta {
            node_id: 5,
            parent_index: Some(2),
            depth: 2,
            has_children: true,
            label: "Grandchild 5".to_string(),
        },
        NodeMeta {
            node_id: 6,
            parent_index: Some(5),
            depth: 3,
            has_children: false,
            label: "Great-grandchild 6".to_string(),
        },
    ]
}

fn make_tree_view(nodes: Vec<NodeMeta>) -> TreeView {
    let mut tv = TreeView::new();
    tv.set_tree_data(nodes);
    let mut model = ColumnModel::new();
    let tree_col = Column::new(400.0);
    let tree_col_id = tree_col.id;
    model.push(tree_col);
    tv.set_columns(model);
    tv.set_tree_column(tree_col_id);
    tv
}

fn build_retained_scroll_tree(
    link: VerticalScrollLink,
    row_count: usize,
) -> (WidgetTree, NodeId, NodeId) {
    let nodes = (0..row_count)
        .map(|index| NodeMeta {
            node_id: index as u64,
            parent_index: None,
            depth: 0,
            has_children: false,
            label: format!("Node {index}"),
        })
        .collect();
    let mut tree_view = make_tree_view(nodes);
    let body = tree_view.create_body_widget();
    let footer = tree_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let tree_node = tree.add_child(root, Box::new(tree_view), None, 0);
    tree.set_flex(tree_node, FlexParams::fixed(120.0));
    let scroll = tree.add_child(
        tree_node,
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
    tree.add_child(tree_node, Box::new(footer), None, 0);

    (tree, root, scroll)
}

// =============================================================================
// Construction
// =============================================================================

#[test]
fn test_treeview_construction() {
    let tv = TreeView::new();
    assert_eq!(tv.revision(), 0);
}

#[test]
fn test_treeview_set_tree_data_bumps_revision() {
    let mut tv = TreeView::new();
    let r0 = tv.revision();
    tv.set_tree_data(build_simple_tree());
    assert!(tv.revision() > r0);
}

#[test]
fn treeview_body_hover_leave_clears_identity_hover_projection() {
    let tree_view = make_tree_view(build_simple_tree());
    let mut body = tree_view.create_body_widget();
    body.test_set_hovered_node(Some(1));
    assert_eq!(body.test_hovered_node(), Some(1));

    let mut tree = WidgetTree::new();
    let mut cx = EventContext::new(
        TreeWidgetId::from_raw(1),
        &mut tree as *mut WidgetTree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    Widget::on_hover_leave(&mut body, &mut cx);

    assert_eq!(body.test_hovered_node(), None);
}

#[test]
fn tree_view_hover_invalidates_paint_without_layout() {
    use crate::core::event::{
        PointerDevice, RawInputEvent, UiInteractionPolicy, WheelDelta, WindowId,
    };
    use crate::core::math::{Point, RectF};
    use crate::core::scene::SceneNode;
    use crate::widgets::{disclosure_toggle::DisclosureToggle, tree_view::TreeViewBody};
    use std::sync::Arc;

    fn body(f: &TreeActivationFixture) -> &TreeViewBody {
        f.tree
            .node(f.body)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }
    #[track_caller]
    fn projection(f: &TreeActivationFixture, hovered: Option<u64>, dirty: bool) {
        assert_eq!(body(f).test_hovered_node(), hovered);
        assert_eq!(f.tree.node(f.body).unwrap().dirty_paint, dirty);
        let scroll = f.tree.node(f.body).unwrap().parent.unwrap();
        for id in [f.body, scroll, f.tree_node, f.root] {
            assert!(
                !f.tree.node(id).unwrap().dirty_layout,
                "hover dirtied layout: {id:?}"
            );
        }
    }
    #[track_caller]
    fn paint(f: &mut TreeActivationFixture, hovered: Option<u64>) -> Arc<[SceneNode]> {
        f.frame();
        projection(f, hovered, false);
        let fragment = f.retained.local_fragment(f.owner(f.body)).unwrap();
        let hover_color: crate::core::scene::Color = Theme::dark().colors.list.row_hover.into();
        let bands: Vec<_> = fragment
            .iter()
            .filter_map(|node| match node {
                SceneNode::Rect(rect) if rect.color == hover_color => Some(rect.rect),
                _ => None,
            })
            .collect();
        if let Some(id) = hovered {
            let row = body(f)
                .test_realized_row_probe_entries()
                .into_iter()
                .find(|row| row.row_id == id)
                .unwrap();
            let origin = f.tree.get_absolute_paint_rect(f.body).unwrap();
            assert_eq!(
                bands,
                vec![RectF::new(
                    origin.x,
                    origin.y + row.rect.y,
                    row.rect.width.max(origin.width),
                    row.rect.height,
                )],
                "only the current row has a retained hover band"
            );
        } else {
            assert!(
                bands.is_empty(),
                "old row hover survived the retained refresh"
            );
        }
        fragment
    }

    let mut f = TreeActivationFixture::new(UiInteractionPolicy::NavigationAndCapture, false);
    projection(&f, None, false);
    let a = f.row_point(1);
    let b = f.row_point(2);
    f.move_to(a);
    projection(&f, Some(1), true);
    let old = paint(&mut f, Some(1));
    f.move_to(b);
    projection(&f, Some(2), true);
    let new = paint(&mut f, Some(2));
    assert!(!Arc::ptr_eq(&old, &new));

    f.move_to(Point::new(b.x + 1.0, b.y + 1.0));
    projection(&f, Some(2), false);
    assert!(Arc::ptr_eq(&new, &paint(&mut f, Some(2))));
    let rect = f.tree.get_absolute_paint_rect(f.body).unwrap();
    let empty = Point::new(b.x, rect.y + rect.height - 2.0);
    assert!(empty.y > b.y);
    f.move_to(empty);
    projection(&f, None, true);
    let clean = paint(&mut f, None);
    f.move_to(Point::new(empty.x + 1.0, empty.y));
    projection(&f, None, false);
    assert!(Arc::ptr_eq(&clean, &paint(&mut f, None)));

    f.move_to(a);
    projection(&f, Some(1), true);
    paint(&mut f, Some(1));
    // A real disclosure remains the target and owns its hover/click. Entering
    // that child leaves the body, clearing the prior row without layout.
    let disclosure = f
        .tree
        .children(f.body)
        .into_iter()
        .find(|id| {
            f.tree
                .node(*id)
                .unwrap()
                .widget
                .as_any()
                .is::<DisclosureToggle>()
        })
        .expect("mounted root disclosure");
    let rect = f.tree.get_absolute_paint_rect(disclosure).unwrap();
    f.move_to(Point::new(
        rect.x + rect.width * 0.5,
        rect.y + rect.height * 0.5,
    ));
    assert_eq!(
        f.domains.primary_state().hovered_widget,
        Some(f.owner(disclosure))
    );
    projection(&f, None, true);
    paint(&mut f, None);
    assert!(f.view().selection_state().selected_keys.is_empty());
    assert!(f.pending.borrow().is_empty());

    f.move_to(a);
    projection(&f, Some(1), true);
    paint(&mut f, Some(1));
    let neighbor = f.tree.get_absolute_paint_rect(f.neighbor).unwrap();
    let outside = Point::new(neighbor.x + 80.0, neighbor.y + neighbor.height * 0.5);
    f.move_to(outside); // The router invokes the actual body's hover-leave lifecycle.
    projection(&f, None, true);
    let clean = paint(&mut f, None);
    f.move_to(outside);
    projection(&f, None, false);
    assert!(Arc::ptr_eq(&clean, &paint(&mut f, None)));

    // Reuse the same composite with enough real rows to scroll, settle that
    // geometry, then require the next hover to use the newly realized identity.
    f.view_mut().set_tree_data(
        (0..30)
            .map(|id| NodeMeta {
                node_id: id,
                parent_index: None,
                depth: 0,
                has_children: false,
                label: format!("Row {id}"),
            })
            .collect(),
    );
    let owner = f.owner(f.tree_node);
    f.tree.mark_layout_dirty_by_widget_id(owner);
    f.frame();
    let position = f.row_point(1);
    f.move_to(position);
    f.frame();
    f.raw(RawInputEvent::PointerWheel {
        window_id: WindowId::PRIMARY,
        time: std::time::Instant::now(),
        device: PointerDevice::Mouse,
        delta: WheelDelta::Pixels(0.0, -84.0),
        position,
    });
    {
        let _update = crate::core::rect_authority::UpdateAuthorityGuard::enter();
        f.tree.update_scroll_animations(1.0 / 60.0);
    }
    pre_layout_sync_scroll_to_lists(&mut f.tree);
    f.frame();
    let offset = f.view().hts_handle().borrow().scroll_offset();
    assert!(offset > 0.0);
    let row = body(&f)
        .test_realized_row_probe_entries()
        .into_iter()
        .find(|row| row.rect.y > 0.0 && row.row_id != 1)
        .unwrap();
    let origin = f.tree.get_absolute_paint_rect(f.body).unwrap();
    f.move_to(Point::new(
        origin.x + 80.0,
        origin.y + row.rect.y + (offset as f32 * 0.5).min(row.rect.height * 0.25),
    ));
    projection(&f, Some(row.row_id), true);
    paint(&mut f, Some(row.row_id));
}

#[test]
#[cfg(debug_assertions)]
fn treeview_retained_scroll_reuses_realized_window() {
    use crate::core::event::{KeyCode, Modifiers, UiInteractionPolicy};
    use crate::core::math::Point;
    use crate::core::rect_authority::UpdateAuthorityGuard;
    use crate::core::scene::{Color, SceneNode};
    use crate::widgets::tree_view::TreeViewBody;
    use std::sync::Arc;

    let view = make_tree_view(
        (0..30)
            .map(|id| NodeMeta {
                node_id: id,
                parent_index: None,
                depth: 0,
                has_children: false,
                label: format!("Row {id}"),
            })
            .collect(),
    );
    let mut f = TreeActivationFixture::with_view(
        UiInteractionPolicy::NavigationAndCapture,
        true,
        view,
        ScrollArea::new_unwired()
            .enable_vertical_scrollbar(true)
            .enable_horizontal_scrollbar(true)
            .with_content_owned_vertical_translation(),
    );
    let scroll = f.tree.parent(f.body).unwrap();
    let owner = f.owner(f.body);
    let row_ids = |f: &TreeActivationFixture| {
        f.tree
            .node(f.body)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<TreeViewBody>()
            .unwrap()
            .test_visible_projection_probe_entries()
            .into_iter()
            .map(|row| row.node_id)
            .collect::<Vec<_>>()
    };
    let scroll_to = |f: &mut TreeActivationFixture, target: f32| {
        f.tree
            .node_mut(scroll)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .scroll_to_y(target);
        {
            let _update = UpdateAuthorityGuard::enter();
            f.tree.update_scroll_animations(1.0 / 60.0);
        }
        f.frame();
    };

    // The existing smooth path advances in fractions of a pixel and reverses
    // while the exact same HTS window remains visible.
    let rows = row_ids(&f);
    let mut previous = f.retained.local_fragment(owner).unwrap();
    let mut layout_frames = 0;
    let mut extra_hts_revisions = 0;
    let mut repaints = 0;
    for (target, expected) in [(5.0, 1.25), (5.0, 2.1875), (0.0, 1.640625)] {
        let epoch = f.tree.node(f.body).unwrap().layout_epoch;
        let revision = f.view().revision();
        scroll_to(&mut f, target);
        assert_eq!(f.view().hts().scroll_offset(), expected);
        assert_eq!(row_ids(&f), rows);
        let next = f.retained.local_fragment(owner).unwrap();
        layout_frames += usize::from(f.tree.node(f.body).unwrap().layout_epoch != epoch);
        extra_hts_revisions += f.view().revision() - revision - 1;
        repaints += usize::from(!Arc::ptr_eq(&previous, &next));
        previous = next;
    }
    assert_eq!(
        (layout_frames, extra_hts_revisions, repaints),
        (0, 0, 3),
        "three same-window frames: frames with layout, HTS realization passes, body paints"
    );

    // Paint and the real pointer route agree on the displayed identity.
    let p = f.row_point(2);
    f.move_to(p);
    f.frame();
    let hover = Color::from(Theme::dark().colors.list.row_hover);
    let painted = f.retained.local_fragment(owner).unwrap();
    assert!(painted.iter().any(|node| match node {
        SceneNode::Rect(rect) => rect.color == hover && rect.rect.contains(p),
        _ => false,
    }));
    f.click(2);
    f.key(KeyCode::Enter, true, false, Modifiers::default());
    assert_eq!(&*f.pending.borrow(), &[2, 2]);
    let viewport = f.tree.get_viewport(f.body).unwrap();
    f.move_to(Point::new(p.x, viewport.y + viewport.height + 1.0));
    f.frame();
    assert!(
        f.retained
            .local_fragment(owner)
            .unwrap()
            .iter()
            .all(|node| !matches!(node, SceneNode::Rect(rect) if rect.color == hover))
    );

    // Crossing an exact row boundary, and then changing/clamping the viewport,
    // still goes through ordinary realization.
    let epoch = f.tree.node(f.body).unwrap().layout_epoch;
    let revision = f.view().revision();
    scroll_to(&mut f, 140.0);
    assert_ne!(f.tree.node(f.body).unwrap().layout_epoch, epoch);
    assert!(f.view().revision() - revision >= 2);
    assert_eq!(row_ids(&f)[0], 1);
    f.tree.set_flex(f.tree_node, FlexParams::fixed(96.0));
    f.frame();
    assert!(row_ids(&f).len() < rows.len());
    f.tree
        .node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .jump_to_y(f32::MAX);
    f.frame();
    assert_eq!(row_ids(&f).last(), Some(&29));
    assert!(f.view().hts().scroll_offset() < f32::MAX as f64);
}

#[test]
fn hidden_treeview_header_moves_owned_body_to_top_edge() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree_view = make_tree_view(build_simple_tree());
    assert!(tree_view.header_visible());
    tree_view.set_header_visible(false);
    assert!(!tree_view.header_visible());
    let body = tree_view.create_body_widget();
    let footer = tree_view.create_footer_widget();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let tree_node = tree.add_child(root, Box::new(tree_view), None, 0);
    tree.set_flex(tree_node, FlexParams::fixed(240.0));
    let scroll = tree.add_child(tree_node, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.add_child(tree_node, Box::new(footer), None, 0);

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
fn treeview_scroll_area_reconstruction_clamps_retained_position_on_first_layout() {
    let _guard = LayoutAuthorityGuard::enter();
    let link = VerticalScrollLink::new();
    let constraints = Constraints::tight(Size::new(320.0, 120.0));
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let (mut old_tree, old_root, old_scroll) = build_retained_scroll_tree(link.clone(), 100);
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

    let (mut rebuilt_tree, rebuilt_root, rebuilt_scroll) = build_retained_scroll_tree(link, 20);
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
    let scroll_rect = rebuilt_tree.get_rect(rebuilt_scroll).unwrap();
    let expected = (scroll.content_extent_height() - scroll_rect.height).max(0.0);
    assert!(expected > 0.0);
    assert!(expected < 1200.0);
    assert!((scroll.logical_offset().y - expected).abs() < 0.001);
    assert!((scroll.visual_scroll_y() - expected).abs() < 0.001);
}

// =============================================================================
// Visible rows after HTS query
// =============================================================================

#[test]
fn test_treeview_visible_rows_all_expanded() {
    let mut tv = make_tree_view(build_simple_tree());

    // Force visible_rows computation via HTS.
    tv.hts_mut().set_viewport_height(500.0);
    tv.hts_mut().scroll_to(0.0);
    let visible = tv.hts_mut().visible_rows();

    // All 3 nodes visible (root expanded by default, children are leaves).
    assert_eq!(visible.len(), 3);
    assert_eq!(visible[0].row_index, 0);
    assert_eq!(visible[0].depth, 0);
    assert!(visible[0].has_children);
    assert!(visible[0].expanded);

    assert_eq!(visible[1].row_index, 1);
    assert_eq!(visible[1].depth, 1);
    assert!(!visible[1].has_children);

    assert_eq!(visible[2].row_index, 2);
    assert_eq!(visible[2].depth, 1);
}

// =============================================================================
// Expand/Collapse
// =============================================================================

#[test]
fn test_treeview_collapse_hides_children() {
    let mut tv = make_tree_view(build_simple_tree());

    // Collapse root.
    tv.collapse(0);

    tv.hts_mut().set_viewport_height(500.0);
    tv.hts_mut().scroll_to(0.0);
    let visible = tv.hts_mut().visible_rows();

    // Only root visible.
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].row_index, 0);
    assert!(!visible[0].expanded);
}

#[test]
fn test_treeview_toggle_expand_collapse() {
    let mut tv = make_tree_view(build_simple_tree());

    // Toggle root (expanded → collapsed).
    tv.toggle(0);
    tv.hts_mut().set_viewport_height(500.0);
    let v1 = tv.hts_mut().visible_rows();
    assert_eq!(v1.len(), 1);

    // Toggle root again (collapsed → expanded).
    tv.toggle(0);
    let v2 = tv.hts_mut().visible_rows();
    assert_eq!(v2.len(), 3);
}

// =============================================================================
// Deep tree: guide metadata
// =============================================================================

#[test]
fn test_treeview_deep_tree_guide_metadata() {
    let mut tv = make_tree_view(build_deep_tree());

    tv.hts_mut().set_viewport_height(500.0);
    tv.hts_mut().scroll_to(0.0);
    let visible = tv.hts_mut().visible_rows();

    assert_eq!(visible.len(), 3);

    // Root: depth 0, expanded, has children, stem_down=true.
    assert_eq!(visible[0].depth, 0);
    assert!(visible[0].stem_down);

    // Child: depth 1, expanded, has children, stem_down=true.
    assert_eq!(visible[1].depth, 1);
    assert!(visible[1].stem_down);

    // Grandchild: depth 2, leaf.
    assert_eq!(visible[2].depth, 2);
    assert!(!visible[2].has_children);
    assert!(!visible[2].stem_down);
}

#[test]
fn test_treeview_collapse_deep_hides_subtree() {
    let mut tv = make_tree_view(build_deep_tree());

    // Collapse child (index 1) → grandchild (index 2) hidden.
    tv.collapse(1);
    tv.hts_mut().set_viewport_height(500.0);
    let visible = tv.hts_mut().visible_rows();

    assert_eq!(visible.len(), 2);
    assert_eq!(visible[0].row_index, 0);
    assert_eq!(visible[1].row_index, 1);
    assert!(!visible[1].expanded);
}

// =============================================================================
// Part 6: Hit-testing structural validation
// =============================================================================

#[test]
fn test_hit_test_deep_tree_with_scroll_offset() {
    let mut tv = make_tree_view(build_hit_test_tree());

    // Set viewport and scroll.
    tv.hts_mut().set_viewport_height(72.0); // 3 rows visible at 24px each
    tv.hts_mut().scroll_to(0.0);

    // Populate visible_rows via direct HTS query (simulating layout).
    let visible = tv.hts_mut().visible_rows();
    assert_eq!(
        visible.len(),
        4,
        "viewport fits 4 rows at 24px (roots visible)"
    );

    // Scroll down by 1 row (24px).
    tv.hts_mut().scroll_to(24.0);
    let visible = tv.hts_mut().visible_rows();

    // After scrolling, we should see rows starting from index 1.
    assert!(visible.len() >= 3);
    // Row at visible index 0 should be physical row 1 (child of root).
    assert_eq!(visible[0].row_index, 1);
    assert!(visible[0].has_children);
}

#[test]
fn test_hit_test_control_zone_uses_visible_index() {
    let mut tv = make_tree_view(build_hit_test_tree());

    tv.hts_mut().set_viewport_height(500.0);
    tv.hts_mut().scroll_to(0.0);

    // Populate visible_rows (simulating layout).
    // This would normally happen in Widget::layout but we're testing internals.
    let visible = tv.hts_mut().visible_rows();
    // Manually set the cached visible_rows since we can't call layout() directly.
    // Access via the Vec directly: visible_rows[0] = root, [1] = child1, [2] = child2, etc.

    // Verify tree structure expectations.
    assert_eq!(visible[0].row_index, 0); // root
    assert!(visible[0].has_children);
    assert_eq!(visible[0].depth, 0);

    assert_eq!(visible[1].row_index, 1); // child 1
    assert!(visible[1].has_children);
    assert_eq!(visible[1].depth, 1);

    assert_eq!(visible[2].row_index, 2); // child 2
    assert!(visible[2].has_children);
    assert_eq!(visible[2].depth, 1);
}

#[test]
fn test_scroll_authority_is_hts() {
    let mut tv = make_tree_view(build_simple_tree());

    // Scroll offset stored in HTS, synced from ScrollArea via sync_scroll.
    tv.hts_mut().scroll_to(50.0);
    assert_eq!(tv.hts().scroll_offset(), 50.0);

    tv.hts_mut().scroll_to(0.0);
    assert_eq!(tv.hts().scroll_offset(), 0.0);
}

#[test]
fn test_row_height_from_hts() {
    let mut tv = make_tree_view(build_simple_tree());

    // Default row height.
    assert_eq!(tv.hts().row_height(), 24.0);

    // Change row height.
    tv.set_row_height(32.0);
    assert_eq!(tv.hts().row_height(), 32.0);
}
