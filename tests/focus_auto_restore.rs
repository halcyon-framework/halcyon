// SPEC-HAL-038 Stage 10: Auto-Restore Focus on Window Activation
//
// Tests that verify focus is automatically restored when window regains focus:
// - Normal restoration after deactivation/reactivation
// - Restoration fails if widget deleted
// - Restoration fails if widget no longer focusable
// - Restoration blocked by active focus scope
// - Restoration skipped if user already assigned focus
// - Restoration memory consumed (only restores once)

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomainState, PointerButton, PointerDevice,
    WindowId,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::utils::id::WidgetInternalId;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

// ============================================================================
// Focusable Widget that tracks focus/blur events
// ============================================================================

struct TrackingWidget {
    id: WidgetInternalId,
    label: &'static str,
    events: Rc<RefCell<Vec<String>>>,
    is_focusable: Rc<RefCell<bool>>,
}

impl TrackingWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label,
            events,
            is_focusable: Rc::new(RefCell::new(true)),
        }
    }

    fn with_focusable_control(
        label: &'static str,
        events: Rc<RefCell<Vec<String>>>,
        focusable: Rc<RefCell<bool>>,
    ) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label,
            events,
            is_focusable: focusable,
        }
    }
}

impl WidgetDowncast for TrackingWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TrackingWidget {
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
        *self.is_focusable.borrow()
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(format!("{}: focus", self.label));
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(format!("{}: blur", self.label));
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }
}

// ============================================================================
// Non-Focusable Container
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

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }
}

// ============================================================================
// Focus Scope Container
// ============================================================================

struct ScopeContainerWidget {
    id: WidgetInternalId,
}

impl ScopeContainerWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for ScopeContainerWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ScopeContainerWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(200.0, 100.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        false
    }

    fn is_focus_scope(&self) -> bool {
        true
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }
}

// ============================================================================
// Helper: Setup tree with focusable widget
// ============================================================================

fn setup_focus_test() -> (
    WidgetTree,
    EventRouter,
    InteractionDomainState,
    FontManagerTextEngine,
    NodeId,
    NodeId,
    Rc<RefCell<Vec<String>>>,
) {
    let mut tree = WidgetTree::new();
    let events = Rc::new(RefCell::new(Vec::new()));

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Widget A at (10, 10, 100, 50)
    let widget_a = tree.add_child(
        root,
        Box::new(TrackingWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let router = EventRouter::new();
    let domain_state = InteractionDomainState::new();
    let text_engine = FontManagerTextEngine::new();
    (
        tree,
        router,
        domain_state,
        text_engine,
        root,
        widget_a,
        events,
    )
}

// ============================================================================
// Test 1: Normal restoration after window reactivate
// ============================================================================

#[test]
fn test_restore_after_window_reactivate() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, widget_a, events) =
        setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Focus widget A via click
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0),
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
    assert!(events.borrow().contains(&"A: focus".to_string()));
    events.borrow_mut().clear();

    // Window deactivates
    let focus_lost = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: false,
    };
    router.route_input_event(
        &mut tree,
        focus_lost,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus cleared, blur fired
    assert_eq!(domain_state.focus_manager.focused(), None);
    assert!(
        events.borrow().contains(&"A: blur".to_string()),
        "Expected blur on deactivation: {:?}",
        *events.borrow()
    );
    events.borrow_mut().clear();

    // Window reactivates
    let focus_gained = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: true,
    };
    router.route_input_event(
        &mut tree,
        focus_gained,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus restored to A
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Focus should be restored after window reactivation"
    );
    assert!(
        events.borrow().contains(&"A: focus".to_string()),
        "Expected focus restored: {:?}",
        *events.borrow()
    );
}

// ============================================================================
// Test 2: Restoration fails if widget deleted
// ============================================================================

#[test]
fn test_restore_fails_if_widget_deleted() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, widget_a, _events) =
        setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Focus widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0),
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

    // Window deactivates
    let focus_lost = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: false,
    };
    router.route_input_event(
        &mut tree,
        focus_lost,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Delete widget A from tree
    tree.remove_node(widget_a);

    // Window reactivates
    let focus_gained = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: true,
    };
    router.route_input_event(
        &mut tree,
        focus_gained,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should NOT be restored (widget deleted)
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should not be restored when widget was deleted"
    );
}

// ============================================================================
// Test 3: Restoration fails if widget no longer focusable
// ============================================================================

#[test]
fn test_restore_fails_if_widget_not_focusable_anymore() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let focusable_control = Rc::new(RefCell::new(true));

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Widget A with controllable focusable state
    let widget_a = tree.add_child(
        root,
        Box::new(TrackingWidget::with_focusable_control(
            "A",
            events.clone(),
            focusable_control.clone(),
        )),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let id_a = tree.node(widget_a).unwrap().id();

    // Focus widget A
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0),
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

    // Window deactivates
    let focus_lost = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: false,
    };
    router.route_input_event(
        &mut tree,
        focus_lost,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Make widget A no longer focusable
    *focusable_control.borrow_mut() = false;

    // Window reactivates
    let focus_gained = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: true,
    };
    router.route_input_event(
        &mut tree,
        focus_gained,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should NOT be restored (widget no longer focusable)
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should not be restored when widget is no longer focusable"
    );
}

// ============================================================================
// Test 4: Restoration blocked by active focus scope
// ============================================================================

#[test]
fn test_restore_blocked_by_focus_scope() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let events = Rc::new(RefCell::new(Vec::new()));

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Widget A (outside scope)
    let widget_a = tree.add_child(
        root,
        Box::new(TrackingWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    // Scope container
    let scope = tree.add_child(root, Box::new(ScopeContainerWidget::new()), None, 0);
    tree.set_rect(scope, RectF::new(150.0, 10.0, 200.0, 100.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();
    let id_a = tree.node(widget_a).unwrap().id();

    // Focus widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0),
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

    // Window deactivates
    let focus_lost = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: false,
    };
    router.route_input_event(
        &mut tree,
        focus_lost,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Simulate modal opening: push focus scope
    router.push_focus_scope(scope, &mut domain_state);
    assert!(domain_state.focus_manager.active_scope().is_some());

    // Window reactivates
    let focus_gained = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: true,
    };
    router.route_input_event(
        &mut tree,
        focus_gained,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should NOT be restored (scope is active)
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should not be restored when focus scope is active"
    );

    // SPEC-HAL-089 Stage 3: Memory must be cleared when scope blocks restore
    assert!(
        !domain_state.focus_manager.has_remembered_focus(),
        "Memory must be cleared when scope blocks restore"
    );
}

// ============================================================================
// Test 5: Restoration skipped if user already assigned focus
// ============================================================================

#[test]
fn test_restore_skipped_if_user_reassigned_focus_early() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let events = Rc::new(RefCell::new(Vec::new()));

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Widget A
    let widget_a = tree.add_child(
        root,
        Box::new(TrackingWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    // Widget B
    let widget_b = tree.add_child(
        root,
        Box::new(TrackingWidget::new("B", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_b, RectF::new(150.0, 10.0, 100.0, 50.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();
    let id_a = tree.node(widget_a).unwrap().id();
    let id_b = tree.node(widget_b).unwrap().id();

    // Focus widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0),
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

    // Window deactivates
    let focus_lost = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: false,
    };
    router.route_input_event(
        &mut tree,
        focus_lost,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), None);

    // User focuses widget B before window reactivation
    // (simulating user clicking while window is being activated)
    router.request_focus_for_widget(&mut tree, id_b, &mut domain_state);
    assert_eq!(domain_state.focus_manager.focused(), Some(id_b));

    events.borrow_mut().clear();

    // Window reactivates
    let focus_gained = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: true,
    };
    router.route_input_event(
        &mut tree,
        focus_gained,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should STILL be on B (not overwritten by restore)
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_b),
        "Focus should not be overwritten when user already assigned focus"
    );

    // No focus event on A (restoration was skipped)
    assert!(
        !events.borrow().contains(&"A: focus".to_string()),
        "A should not receive focus when B already focused: {:?}",
        *events.borrow()
    );

    // SPEC-HAL-089 Stage 2: Memory must be cleared when user overrides focus
    assert!(
        !domain_state.focus_manager.has_remembered_focus(),
        "Memory must be cleared when user overrides focus"
    );
}

// ============================================================================
// Test 6: Restoration memory consumed (only restores once)
// ============================================================================

#[test]
fn test_restore_once_only() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, widget_a, events) =
        setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Focus widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0),
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

    // Window deactivates
    let focus_lost = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: false,
    };
    router.route_input_event(
        &mut tree,
        focus_lost,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Window reactivates (first time) - focus restored
    let focus_gained = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: true,
    };
    router.route_input_event(
        &mut tree,
        focus_gained,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Clear focus manually (simulating user pressing ESC)
    router.clear_focus(&mut tree, &mut domain_state);
    assert_eq!(domain_state.focus_manager.focused(), None);
    events.borrow_mut().clear();

    // Window reactivates again (without prior deactivation that remembers focus)
    let focus_gained2 = FusedInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: true,
    };
    router.route_input_event(
        &mut tree,
        focus_gained2,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should NOT be restored (memory was consumed on first restore)
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should not be restored a second time - memory was consumed"
    );

    // No focus events (nothing happened)
    assert!(
        !events.borrow().iter().any(|e| e.contains("focus")),
        "No focus events expected on second activation: {:?}",
        *events.borrow()
    );
}
