// SPEC-HAL-038 Stage 8: Pointer Capture × Focus Coordination
//
// Tests that verify focus and pointer capture work together correctly:
// - Primary press captures AND focuses
// - Capture prevents focus-steal from other widgets
// - Right-click during capture does nothing
// - Drag outside does not blur
// - Release ends capture but keeps focus

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomainState, PointerButton, PointerDevice,
    PointerEventKind, WindowId,
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
// Focusable Widget that captures pointer on Primary press
// ============================================================================

struct CapturingWidget {
    id: WidgetInternalId,
    label: &'static str,
    bounds: Size,
    events: Rc<RefCell<Vec<String>>>,
}

impl CapturingWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label,
            bounds: Size::new(100.0, 50.0),
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
        LayoutResult::fit(self.bounds)
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

        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down => {
                    // Capture on Primary button press (like Button widget)
                    if pe.button == Some(PointerButton::Primary) {
                        self.events
                            .borrow_mut()
                            .push(format!("{}: capture", self.label));
                        cx.capture_pointer(PointerButton::Primary);
                        return true;
                    }
                    false
                }
                PointerEventKind::Up => {
                    if pe.button == Some(PointerButton::Primary) {
                        self.events
                            .borrow_mut()
                            .push(format!("{}: release", self.label));
                        // Capture auto-released by router after Up
                        return true;
                    }
                    false
                }
                PointerEventKind::Move => {
                    // Track move events during capture
                    self.events
                        .borrow_mut()
                        .push(format!("{}: move", self.label));
                    true
                }
                _ => false,
            },
            _ => false,
        }
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
// Helper: Setup tree with two capturing widgets
// ============================================================================

fn setup_capture_test() -> (
    WidgetTree,
    EventRouter,
    InteractionDomainState,
    FontManagerTextEngine,
    NodeId,
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
        Box::new(CapturingWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    // Widget B at (150, 10, 100, 50)
    let widget_b = tree.add_child(
        root,
        Box::new(CapturingWidget::new("B", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_b, RectF::new(150.0, 10.0, 100.0, 50.0));

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
        widget_b,
        events,
    )
}

// ============================================================================
// Test 1: Primary press captures AND focuses
// ============================================================================

#[test]
fn test_primary_press_captures_and_focuses() {
    let _guard = LayoutAuthorityGuard::enter();
    let (
        mut tree,
        mut router,
        mut domain_state,
        mut text_engine,
        root,
        widget_a,
        _widget_b,
        events,
    ) = setup_capture_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Initially no focus, no capture
    assert_eq!(domain_state.focus_manager.focused(), None);
    assert!(!domain_state.pointer_capture.is_captured());

    // Primary click on widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0), // Inside A
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

    // Widget A should have focus
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Primary press should assign focus"
    );

    // Widget A should have capture
    assert!(
        domain_state.pointer_capture.is_captured(),
        "Primary press should trigger capture"
    );
    assert_eq!(
        domain_state.pointer_capture.captured_widget,
        Some(id_a),
        "Capture should be on widget A"
    );

    // Events should show focus then capture
    let evts = events.borrow();
    assert!(
        evts.contains(&"A: focus".to_string()),
        "Focus event expected"
    );
    assert!(
        evts.contains(&"A: capture".to_string()),
        "Capture event expected"
    );
}

// ============================================================================
// Test 2: Capture prevents focus-steal
// ============================================================================

#[test]
fn test_capture_prevents_focus_steal() {
    let _guard = LayoutAuthorityGuard::enter();
    let (
        mut tree,
        mut router,
        mut domain_state,
        mut text_engine,
        root,
        widget_a,
        widget_b,
        _events,
    ) = setup_capture_test();
    let id_a = tree.node(widget_a).unwrap().id();
    let id_b = tree.node(widget_b).unwrap().id();

    // Primary click on A to capture
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
    assert!(domain_state.pointer_capture.is_captured());

    // Now try to click on B while A has capture
    let click_b = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(200.0, 30.0), // Inside B
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

    // Focus should STILL be on A (capture prevents focus-steal)
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Focus should stay on A while it has capture"
    );

    // Capture should still be on A
    assert_eq!(
        domain_state.pointer_capture.captured_widget,
        Some(id_a),
        "Capture should remain on A"
    );

    // B should NOT have focus
    assert_ne!(
        domain_state.focus_manager.focused(),
        Some(id_b),
        "B should not steal focus during A's capture"
    );
}

// ============================================================================
// Test 3: Right-click during capture does nothing
// ============================================================================

#[test]
fn test_right_click_during_capture_does_nothing() {
    let _guard = LayoutAuthorityGuard::enter();
    let (
        mut tree,
        mut router,
        mut domain_state,
        mut text_engine,
        root,
        widget_a,
        _widget_b,
        events,
    ) = setup_capture_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Primary click on A to capture
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
    events.borrow_mut().clear();

    // Right-click on B while A has capture
    let right_click_b = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(200.0, 30.0), // Inside B
        device: PointerDevice::Mouse,
        button: PointerButton::Secondary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        right_click_b,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus still on A
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Right-click should not change focus during capture"
    );

    // No focus events should have occurred
    let evts = events.borrow();
    assert!(
        !evts
            .iter()
            .any(|e| e.contains("focus") || e.contains("blur")),
        "No focus events expected from right-click: {:?}",
        *evts
    );
}

// ============================================================================
// Test 4: Drag outside does not blur
// ============================================================================

#[test]
fn test_drag_outside_does_not_blur() {
    let _guard = LayoutAuthorityGuard::enter();
    let (
        mut tree,
        mut router,
        mut domain_state,
        mut text_engine,
        root,
        widget_a,
        _widget_b,
        events,
    ) = setup_capture_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Primary click on A to capture
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
    events.borrow_mut().clear();

    // Drag to outside A's bounds (but still inside container)
    let drag_outside = FusedInputEvent::DragUpdate {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        origin: Point::new(50.0, 30.0),
        current: Point::new(300.0, 100.0), // Far outside A
        delta: (250.0, 70.0),
        button: PointerButton::Primary,
    };
    router.route_input_event(
        &mut tree,
        drag_outside,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should still be on A
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Focus should stay on A during drag outside"
    );

    // No blur event should have occurred
    let evts = events.borrow();
    assert!(
        !evts.iter().any(|e| e.contains("blur")),
        "No blur expected during drag outside: {:?}",
        *evts
    );
}

// ============================================================================
// Test 5: Release ends capture but keeps focus
// ============================================================================

#[test]
fn test_release_ends_capture_but_keeps_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (
        mut tree,
        mut router,
        mut domain_state,
        mut text_engine,
        root,
        widget_a,
        _widget_b,
        events,
    ) = setup_capture_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Primary click on A to capture
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
    assert!(domain_state.pointer_capture.is_captured());
    events.borrow_mut().clear();

    // Release the mouse button
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
        "Focus should remain on A after capture release"
    );

    // No blur event should have occurred
    let evts = events.borrow();
    assert!(
        !evts.iter().any(|e| e.contains("blur")),
        "No blur expected after release: {:?}",
        *evts
    );
    assert!(
        evts.contains(&"A: release".to_string()),
        "Release event expected"
    );
}
