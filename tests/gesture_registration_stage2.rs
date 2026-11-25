// SPEC-HAL-128 Stage 2: Gesture Candidate Registration Tests
//
// These tests verify the gesture candidate registration invariants:
//   INV-GA1 - Candidate Registration on Pointer Down
//   INV-GA2 - Movement Threshold Commit (no capture on registration)
//
// Stage 2 scope: registration only, no arbitration or capture.

use halcyon::core::event::{
    EventPhase, FusedInputEvent, GestureKind, GestureRegistry, InteractionDomainState,
    PointerButton, PointerDevice, PointerEventKind, PointerId, PointerType, WindowId,
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
// Test Widget: Registers gesture candidates on pointer down
// ============================================================================

/// A test widget that registers gesture candidates on pointer down
struct GestureTestWidget {
    internal_id: WidgetInternalId,
    #[allow(dead_code)]
    name: &'static str,
    /// Records (local_x, local_y, event_kind) of received pointer events
    received_events: Rc<RefCell<Vec<(f32, f32, &'static str)>>>,
    /// Kind of gesture to register (if any)
    register_kind: Option<GestureKind>,
    /// If true, requests capture on PointerDown (to test that registration doesn't capture)
    request_capture: bool,
}

impl GestureTestWidget {
    fn new(
        name: &'static str,
        events: Rc<RefCell<Vec<(f32, f32, &'static str)>>>,
        register_kind: Option<GestureKind>,
        request_capture: bool,
    ) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            name,
            received_events: events,
            register_kind,
            request_capture,
        }
    }
}

impl WidgetDowncast for GestureTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for GestureTestWidget {
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
        self.internal_id
    }

    /// SPEC-HAL-128 Stage 8: Static gesture surface declaration.
    fn gesture_surface(&self) -> &'static [GestureKind] {
        match self.register_kind {
            Some(GestureKind::Drag) => &[GestureKind::Drag],
            Some(GestureKind::Scroll) => &[GestureKind::Scroll],
            Some(GestureKind::Resize) => &[GestureKind::Resize],
            Some(GestureKind::Select) => &[GestureKind::Select],
            Some(GestureKind::Pinch) => &[GestureKind::Pinch],
            Some(GestureKind::Swipe) => &[GestureKind::Swipe],
            Some(GestureKind::Draw) => &[GestureKind::Draw],
            Some(GestureKind::Custom) => &[GestureKind::Custom],
            None => &[],
        }
    }

    fn handle_event(
        &mut self,
        cx: &mut EventContext,
        event: &halcyon::core::event::Event,
        phase: halcyon::core::event::EventPhase,
    ) -> bool {
        // Only handle events in Target phase to avoid consuming during Capture/Bubble
        if phase != EventPhase::Target {
            return false;
        }

        if let halcyon::core::event::Event::Pointer(pe) = event {
            let kind_str = match pe.kind {
                PointerEventKind::Down => "down",
                PointerEventKind::Up => "up",
                PointerEventKind::Move => "move",
                PointerEventKind::Scroll { .. } => "scroll",
                PointerEventKind::DoubleClick => "double",
                PointerEventKind::TripleClick => "triple",
                PointerEventKind::DragStart => "dragstart",
            };
            self.received_events
                .borrow_mut()
                .push((pe.local_pos.0, pe.local_pos.1, kind_str));

            // On PointerDown: optionally request capture (candidates registered by system)
            if matches!(pe.kind, PointerEventKind::Down) {
                if self.request_capture {
                    cx.capture_pointer(PointerButton::Primary);
                }
            }

            true // Handled
        } else {
            false
        }
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn setup_two_widgets(
    events_a: Rc<RefCell<Vec<(f32, f32, &'static str)>>>,
    events_b: Rc<RefCell<Vec<(f32, f32, &'static str)>>>,
    widget_a_kind: Option<GestureKind>,
    widget_b_kind: Option<GestureKind>,
    widget_a_captures: bool,
) -> (WidgetTree, NodeId, TreeWidgetId, TreeWidgetId) {
    let mut tree = WidgetTree::new();

    // Root container: 200x100
    let root = tree.create_root(
        Box::new(GestureTestWidget::new(
            "root",
            Rc::new(RefCell::new(Vec::new())),
            None,
            false,
        )),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

    // Widget A: left side (0-100, 0-100)
    let widget_a = tree.add_child(
        root,
        Box::new(GestureTestWidget::new(
            "A",
            events_a,
            widget_a_kind,
            widget_a_captures,
        )),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Widget B: right side (100-200, 0-100)
    let widget_b = tree.add_child(
        root,
        Box::new(GestureTestWidget::new("B", events_b, widget_b_kind, false)),
        None,
        0,
    );
    tree.set_rect(widget_b, RectF::new(100.0, 0.0, 100.0, 100.0));

    // Compute positions for hit-testing
    halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
    halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let id_a = tree.node(widget_a).unwrap().id();
    let id_b = tree.node(widget_b).unwrap().id();

    (tree, root, id_a, id_b)
}

// ============================================================================
// Test: candidates_registered_on_pointer_down (INV-GA1)
// ============================================================================

/// INV-GA1: Candidates are registered on pointer down.
#[test]
fn candidates_registered_on_pointer_down() {
    let _guard = LayoutAuthorityGuard::enter();
    let events_a = Rc::new(RefCell::new(Vec::new()));
    let events_b = Rc::new(RefCell::new(Vec::new()));

    let (mut tree, root, id_a, _id_b) = setup_two_widgets(
        events_a.clone(),
        events_b.clone(),
        Some(GestureKind::Drag), // A registers Drag
        None,                    // B does not register
        false,                   // A does not capture
    );

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Verify no candidates initially
    assert!(
        !domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Should have no candidates before pointer down"
    );

    // Pointer down on Widget A
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(50.0, 50.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Verify Widget A received the event
    assert!(
        !events_a.borrow().is_empty(),
        "Widget A should have received the pointer down event"
    );

    // Verify candidate was registered
    let candidates = domain_state.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 1, "Should have exactly 1 candidate");
    assert_eq!(
        candidates[0].widget_id, id_a,
        "Candidate should be from Widget A"
    );
    assert_eq!(
        candidates[0].kind,
        GestureKind::Drag,
        "Candidate should be Drag kind"
    );
}

// ============================================================================
// Test: multiple_widgets_can_register_candidates (INV-GA1)
// ============================================================================

/// INV-GA1: Multiple widgets can register candidates (non-exclusive).
/// SPEC-HAL-128 Stage 2.1: Updated for session-based API.
#[test]
fn multiple_widgets_can_register_candidates() {
    let mut registry = GestureRegistry::new();
    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    registry.register_candidate_request(PointerId::MOUSE, widget_a, GestureKind::Drag, None);
    registry.register_candidate_request(PointerId::MOUSE, widget_b, GestureKind::Scroll, None);

    let candidates = registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 2);

    assert_eq!(candidates[0].widget_id, widget_a);
    assert_eq!(candidates[0].kind, GestureKind::Drag);

    assert_eq!(candidates[1].widget_id, widget_b);
    assert_eq!(candidates[1].kind, GestureKind::Scroll);

    // Session-scoped IDs must match the active session
    let sid = registry.session_id(PointerId::MOUSE).unwrap();
    assert_eq!(candidates[0].candidate_id.session, sid);
    assert_eq!(candidates[1].candidate_id.session, sid);
    assert_eq!(candidates[0].candidate_id.local, 0);
    assert_eq!(candidates[1].candidate_id.local, 1);
}

// ============================================================================
// Test: candidates_cleared_on_pointer_up
// ============================================================================

/// Candidates are cleared when pointer up occurs.
#[test]
fn candidates_cleared_on_pointer_up() {
    let _guard = LayoutAuthorityGuard::enter();
    let events_a = Rc::new(RefCell::new(Vec::new()));
    let events_b = Rc::new(RefCell::new(Vec::new()));

    let (mut tree, root, _id_a, _id_b) = setup_two_widgets(
        events_a.clone(),
        events_b.clone(),
        Some(GestureKind::Drag), // A registers Drag
        None,
        false, // A does not capture
    );

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // 1. Pointer down - candidate should be registered
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(50.0, 50.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    assert!(
        domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Should have candidates after pointer down"
    );

    // 2. Pointer up - candidates should be cleared
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(50.0, 50.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    assert!(
        !domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Candidates should be cleared after pointer up"
    );
}

// ============================================================================
// Test: candidates_cleared_on_cancel
// ============================================================================

/// Candidates are cleared when gesture is cancelled.
/// SPEC-HAL-128 Stage 2.1: Updated for session-based API.
#[test]
fn candidates_cleared_on_cancel() {
    let mut registry = GestureRegistry::new();
    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    let widget_a = TreeWidgetId(1);
    registry.register_candidate_request(PointerId::MOUSE, widget_a, GestureKind::Resize, None);

    assert!(registry.has_candidates(PointerId::MOUSE));

    registry.clear_candidates(PointerId::MOUSE);

    assert!(!registry.has_candidates(PointerId::MOUSE));
    assert_eq!(registry.candidates(PointerId::MOUSE).len(), 0);
    assert!(registry.session_id(PointerId::MOUSE).is_none());
}

// ============================================================================
// Test: registration_does_not_capture_pointer (INV-GA2)
// ============================================================================

/// INV-GA2: Registration does not cause pointer capture.
/// Capture only happens on explicit request, not on candidate registration.
#[test]
fn registration_does_not_capture_pointer() {
    let _guard = LayoutAuthorityGuard::enter();
    let events_a = Rc::new(RefCell::new(Vec::new()));
    let events_b = Rc::new(RefCell::new(Vec::new()));

    let (mut tree, root, _id_a, _id_b) = setup_two_widgets(
        events_a.clone(),
        events_b.clone(),
        Some(GestureKind::Scroll), // A registers Scroll candidate
        None,
        false, // A does NOT request capture
    );

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down on Widget A - registers candidate but does NOT capture
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(50.0, 50.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Verify candidate was registered
    assert!(
        domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Should have candidates after pointer down"
    );

    // Verify pointer is NOT captured (INV-GA2)
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Pointer should NOT be captured from registration alone"
    );

    // Clear events
    events_a.borrow_mut().clear();
    events_b.borrow_mut().clear();

    // Move to Widget B area - should hit Widget B, not A (no capture)
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(150.0, 50.0),
            delta: (100.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: halcyon::core::event::PointerButtonsState::default(),
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Widget B should receive the move event (no capture blocking it)
    let b_events = events_b.borrow();
    let a_events = events_a.borrow();
    assert!(
        !b_events.is_empty(),
        "Widget B should receive move event when pointer is not captured"
    );
    assert!(
        a_events.is_empty(),
        "Widget A should NOT receive move event (no capture)"
    );
}

// ============================================================================
// Test: candidate_ids_are_session_scoped (SPEC-HAL-128 Stage 2.1)
// ============================================================================

/// SPEC-HAL-128 Stage 2.1: Candidate IDs are scoped to sessions.
/// New sessions get new session IDs, local counters reset.
#[test]
fn candidate_ids_are_session_scoped() {
    let mut registry = GestureRegistry::new();

    let widget = TreeWidgetId(1);

    let sid1 = registry.begin_session(PointerId::MOUSE, PointerType::Mouse);
    registry.register_candidate_request(PointerId::MOUSE, widget, GestureKind::Drag, None);
    let c1 = registry.candidates(PointerId::MOUSE)[0].candidate_id;
    assert_eq!(c1.session, sid1);
    assert_eq!(c1.local, 0);

    registry.clear_candidates(PointerId::MOUSE);

    let sid2 = registry.begin_session(PointerId::MOUSE, PointerType::Mouse);
    registry.register_candidate_request(PointerId::MOUSE, widget, GestureKind::Drag, None);
    let c2 = registry.candidates(PointerId::MOUSE)[0].candidate_id;

    assert_eq!(c2.session, sid2);
    assert_eq!(c2.local, 0);
    assert_ne!(c1.session, c2.session);
}
