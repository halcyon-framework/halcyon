// SPEC-HAL-038 Stage 2: Tab Navigation Tests

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    FusedInputEvent, InteractionDomainState, KeyCode, Modifiers, PointerButton, PointerDevice,
    WindowId,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::utils::id::{TreeWidgetId, WidgetInternalId};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

// ============================================================================
// Focusable Test Widget
// ============================================================================

struct FocusableWidget {
    id: WidgetInternalId,
    events: Rc<RefCell<Vec<TabTestEvent>>>,
    label: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
enum TabTestEvent {
    OnFocus(&'static str),
    OnBlur(&'static str),
}

impl FocusableWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<TabTestEvent>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            events,
            label,
        }
    }
}

impl WidgetDowncast for FocusableWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FocusableWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 50.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(TabTestEvent::OnFocus(self.label));
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(TabTestEvent::OnBlur(self.label));
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

// ============================================================================
// Non-Focusable Container Widget
// ============================================================================

struct ContainerWidget {
    id: WidgetInternalId,
}

impl ContainerWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for ContainerWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ContainerWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(400.0, 200.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        false
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        _event: &halcyon::core::event::Event,
        _phase: halcyon::core::event::EventPhase,
    ) -> bool {
        false
    }
}

// ============================================================================
// Helper: Create Tab key event
// ============================================================================

fn tab_key_event(shift: bool) -> FusedInputEvent {
    FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Tab,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers {
            shift,
            ctrl: false,
            alt: false,
            meta: false,
        },
        repeat: false,
    }
}

// ============================================================================
// Helper: Setup tree with three focusable widgets
// ============================================================================

fn setup_three_focusable_widgets(
    events: Rc<RefCell<Vec<TabTestEvent>>>,
) -> (WidgetTree, NodeId, TreeWidgetId, TreeWidgetId, TreeWidgetId) {
    let mut tree = WidgetTree::new();

    // Root container (non-focusable)
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Widget A
    let widget_a = tree.add_child(
        root,
        Box::new(FocusableWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    // Widget B
    let widget_b = tree.add_child(
        root,
        Box::new(FocusableWidget::new("B", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_b, RectF::new(110.0, 0.0, 100.0, 50.0));
    let id_b = tree.node(widget_b).unwrap().id();

    // Widget C
    let widget_c = tree.add_child(
        root,
        Box::new(FocusableWidget::new("C", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_c, RectF::new(220.0, 0.0, 100.0, 50.0));
    let id_c = tree.node(widget_c).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    (tree, root, id_a, id_b, id_c)
}

// ============================================================================
// Test 1: Tab Focus Forward
// ============================================================================

#[test]
fn test_tab_focus_forward() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, id_b, id_c) = setup_three_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus on A via click
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_a,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    events.borrow_mut().clear();

    // Tab: A -> B
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_b));

    // Tab: B -> C
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_c));

    // Tab: C -> A (wrap around)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Verify lifecycle events
    {
        let evts = events.borrow();
        assert!(
            evts.contains(&TabTestEvent::OnBlur("A")),
            "A should blur when moving to B"
        );
        assert!(
            evts.contains(&TabTestEvent::OnFocus("B")),
            "B should focus after Tab"
        );
        assert!(
            evts.contains(&TabTestEvent::OnBlur("B")),
            "B should blur when moving to C"
        );
        assert!(
            evts.contains(&TabTestEvent::OnFocus("C")),
            "C should focus after Tab"
        );
        assert!(
            evts.contains(&TabTestEvent::OnBlur("C")),
            "C should blur when wrapping to A"
        );
        // A focuses again after wrap
    }
}

// ============================================================================
// Test 2: Tab Focus Backward (Shift+Tab)
// ============================================================================

#[test]
fn test_tab_focus_backward() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, id_b, id_c) = setup_three_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus on C via click
    let click_c = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(270.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_c,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_c));

    events.borrow_mut().clear();

    // Shift+Tab: C -> B
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_b));

    // Shift+Tab: B -> A
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Shift+Tab: A -> C (wrap around)
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_c));
}

// ============================================================================
// Test 3: Tab When None Focused
// ============================================================================

#[test]
fn test_tab_when_none_focused() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, _id_b, id_c) = setup_three_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // No focus initially
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Tab with no focus -> first focusable (A)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Clear focus
    router.clear_focus(&mut tree, &mut domain_state);
    assert_eq!(domain_state.focus_manager.focused(), None);

    events.borrow_mut().clear();

    // Shift+Tab with no focus -> last focusable (C)
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_c));
}

// ============================================================================
// Test 4: Tab Single Focusable
// ============================================================================

#[test]
fn test_tab_single_focusable() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

    // Single focusable widget
    let widget_a = tree.add_child(
        root,
        Box::new(FocusableWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_a,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    events.borrow_mut().clear();

    // Tab on single focusable -> stays on A (no-op, but still valid)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // No blur/focus events because we're moving to the same widget
    // (request_focus detects same widget and returns (None, None))
    {
        let evts = events.borrow();
        assert!(
            evts.is_empty(),
            "No events when tabbing to same widget: {:?}",
            *evts
        );
    }
}

// ============================================================================
// Test 5: Focus Removed Node
// ============================================================================

#[test]
fn test_focus_removed_node() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, id_b, _id_c) = setup_three_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus on B
    let click_b = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(160.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_b,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_b));

    // Find B's NodeId
    let b_node_id = tree.find_node_by_widget_id(id_b).unwrap();

    // Remove B from tree
    tree.remove_node(b_node_id);

    // Rebuild focus chain - should detect B is gone and clear focus
    domain_state.focus_manager.rebuild_focus_chain(&tree, root);
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should be cleared when focused widget is removed"
    );

    // Tab should now focus first available (A)
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));
}

// ============================================================================
// Test 6: Empty Focus Chain
// ============================================================================

#[test]
fn test_tab_empty_focus_chain() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Root container only (non-focusable)
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Tab with no focusable widgets -> nothing happens
    router.route_input_event(
        &mut tree,
        tab_key_event(false),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Shift+Tab with no focusable widgets -> nothing happens
    router.route_input_event(
        &mut tree,
        tab_key_event(true),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), None);
}
