// SPEC-HAL-038 Stage 9/10: Focus Persistence & Blur-on-Deactivation Rules
//
// Tests that verify focus lifecycle under window activation/deactivation:
// - Window deactivation blurs focused widget
// - Window activation auto-restores focus (Stage 10)
// - ESC clears focus when no scope is active
// - ESC does nothing when no widget is focused
// - ESC pops scope when scope is active (preserves focus outside scope)
// - Capture release does NOT blur

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomainState, KeyCode, Modifiers, PointerButton,
    PointerDevice, WindowId,
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
}

impl TrackingWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label,
            events,
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
        true
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
// Capturing Widget (for capture release test)
// ============================================================================

struct CapturingWidget {
    id: WidgetInternalId,
    label: &'static str,
    events: Rc<RefCell<Vec<String>>>,
}

impl CapturingWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label,
            events,
        }
    }
}

impl WidgetDowncast for CapturingWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CapturingWidget {
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
            .push(format!("{}: focus", self.label));
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(format!("{}: blur", self.label));
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        if let Event::Pointer(pe) = event {
            use halcyon::core::event::PointerEventKind;
            if matches!(pe.kind, PointerEventKind::Down) {
                if pe.button == Some(PointerButton::Primary) {
                    cx.capture_pointer(PointerButton::Primary);
                    self.events
                        .borrow_mut()
                        .push(format!("{}: capture", self.label));
                    return true;
                }
            }
        }
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
// Focus Scope Container (for scope tests)
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

fn setup_focus_test() -> (WidgetTree, NodeId, NodeId, Rc<RefCell<Vec<String>>>) {
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

    (tree, root, widget_a, events)
}

/// Helper: Send a complete click (PointerDown + PointerUp) to properly end gesture session.
/// SPEC-HAL-128 requires PointerUp to end gesture sessions before ESC can clear focus.
fn send_click(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    pos: Point,
    root: Option<NodeId>,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
) {
    let down = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        tree,
        down,
        root,
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    let up = FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    };
    router.route_input_event(
        tree,
        up,
        root,
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

// ============================================================================
// Test 1: Window deactivation blurs focused widget
// ============================================================================

#[test]
fn test_focus_blurs_on_window_deactivate() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, root, widget_a, events) = setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus widget A via left-click
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

    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Widget A should have focus"
    );
    assert!(
        events.borrow().contains(&"A: focus".to_string()),
        "Focus event should have fired"
    );
    events.borrow_mut().clear();

    // Simulate window losing focus
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

    // Widget A should no longer have focus
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Focus should be cleared after window deactivation"
    );

    // on_blur should have been called
    assert!(
        events.borrow().contains(&"A: blur".to_string()),
        "on_blur should have been called: {:?}",
        *events.borrow()
    );
}

// ============================================================================
// Test 2: Window activation auto-restores focus (Stage 10)
// ============================================================================

#[test]
fn test_focus_restored_on_window_activate() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, root, widget_a, events) = setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

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

    // Lose focus (window deactivate)
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

    events.borrow_mut().clear();

    // Gain focus (window activate)
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

    // SPEC-HAL-038 Stage 10: Focus should be auto-restored
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Focus should be auto-restored on window activation"
    );
    assert!(
        events.borrow().contains(&"A: focus".to_string()),
        "on_focus should be called on restore: {:?}",
        *events.borrow()
    );
}

// ============================================================================
// Test 3: ESC preserves focus when no scope is active
// ============================================================================

#[test]
fn test_esc_preserves_focused_widget() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, root, widget_a, events) = setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus widget A - SPEC-HAL-128: Use complete click (Down+Up) to end gesture session
    send_click(
        &mut router,
        &mut tree,
        Point::new(50.0, 30.0),
        Some(root),
        &mut text_engine,
        &mut domain_state,
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));
    events.borrow_mut().clear();

    // Press ESC
    let esc_down = FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Escape,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    };
    router.route_input_event(
        &mut tree,
        esc_down,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should be preserved at root scope.
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "ESC should preserve focus"
    );

    // on_blur should not have been called.
    assert!(
        !events.borrow().contains(&"A: blur".to_string()),
        "on_blur should not be called on root-scope ESC: {:?}",
        *events.borrow()
    );
}

// ============================================================================
// Test 4: ESC does nothing when no widget is focused
// ============================================================================

#[test]
fn test_esc_does_not_clear_when_no_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, root, _widget_a, events) = setup_focus_test();

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Verify no focus initially
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Press ESC
    let esc_down = FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Escape,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    };
    router.route_input_event(
        &mut tree,
        esc_down,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Still no focus (nothing happened)
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "ESC should do nothing when no widget is focused"
    );

    // No blur events (nothing to blur)
    assert!(
        !events.borrow().iter().any(|e| e.contains("blur")),
        "No blur events expected: {:?}",
        *events.borrow()
    );
}

// ============================================================================
// Test 5: ESC pops scope when scope is active (preserves focus outside scope)
// ============================================================================

#[test]
fn test_esc_pops_scope_preserves_outside_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let events = Rc::new(RefCell::new(Vec::new()));

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Widget A (outside scope) at (10, 10, 100, 50)
    let widget_a = tree.add_child(
        root,
        Box::new(TrackingWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    // Scope container at (150, 10, 200, 100)
    let scope = tree.add_child(root, Box::new(ScopeContainerWidget::new()), None, 0);
    tree.set_rect(scope, RectF::new(150.0, 10.0, 200.0, 100.0));

    // Widget B (inside scope) at (10, 10, 80, 40) relative to scope
    let _widget_b = tree.add_child(
        scope,
        Box::new(TrackingWidget::new("B", events.clone())),
        None,
        0,
    );
    tree.set_rect(_widget_b, RectF::new(10.0, 10.0, 80.0, 40.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();
    let id_a = tree.node(widget_a).unwrap().id();

    // Focus widget A (outside scope) - SPEC-HAL-128: Use complete click (Down+Up) to end gesture session
    send_click(
        &mut router,
        &mut tree,
        Point::new(50.0, 30.0),
        Some(root),
        &mut text_engine,
        &mut domain_state,
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Push the scope (simulating modal open)
    router.push_focus_scope(scope, &mut domain_state);
    assert!(domain_state.focus_manager.active_scope().is_some());

    events.borrow_mut().clear();

    // Press ESC - should pop scope, but preserve focus on A (which is outside scope)
    let esc_down = FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Escape,
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    };
    router.route_input_event(
        &mut tree,
        esc_down,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Scope should be popped
    assert!(
        domain_state.focus_manager.active_scope().is_none(),
        "ESC should pop the scope"
    );

    // Focus should STILL be on A (not cleared because A was outside the popped scope)
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Focus on widget outside scope should be preserved after ESC pops scope"
    );

    // No blur event on A
    assert!(
        !events.borrow().contains(&"A: blur".to_string()),
        "Widget A should not receive blur when scope is popped: {:?}",
        *events.borrow()
    );
}

// ============================================================================
// Test 6: Capture release does NOT blur
// ============================================================================

#[test]
fn test_capture_release_does_not_blur() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let events = Rc::new(RefCell::new(Vec::new()));

    // Root container
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Capturing widget at (10, 10, 100, 50)
    let widget_a = tree.add_child(
        root,
        Box::new(CapturingWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();
    let id_a = tree.node(widget_a).unwrap().id();

    // Click to focus and capture
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

    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Widget A should have focus"
    );
    assert!(
        domain_state.pointer_capture.is_captured(),
        "Widget A should have capture"
    );
    assert!(
        events.borrow().contains(&"A: focus".to_string()),
        "Focus event expected"
    );
    assert!(
        events.borrow().contains(&"A: capture".to_string()),
        "Capture event expected"
    );
    events.borrow_mut().clear();

    // Release capture via pointer up
    let release = FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    };
    router.route_input_event(
        &mut tree,
        release,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Capture should be released
    assert!(
        !domain_state.pointer_capture.is_captured(),
        "Capture should be released after pointer up"
    );

    // Focus should STILL be on A
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Focus should remain after capture release"
    );

    // No blur event
    assert!(
        !events.borrow().iter().any(|e| e.contains("blur")),
        "No blur expected after capture release: {:?}",
        *events.borrow()
    );
}
