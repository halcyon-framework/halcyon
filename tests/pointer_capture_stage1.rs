// SPEC-HAL-128 Stage 1: Pointer Capture Core Tests
//
// These tests verify the pointer capture bookkeeping invariants:
//   INV-PC1 - Explicit Capture Only
//   INV-PC2 - Per-Pointer Exclusivity
//   INV-PC3 - Capture Lifetime
//   INV-PC6 - Guaranteed Pointer-Up Delivery
//
// Stage 1 scope: capture manager logic only, no gesture arbitration.

use halcyon::core::event::{
    CaptureError, EventPhase, FusedInputEvent, InteractionDomainState, PointerButton,
    PointerButtonsState, PointerCaptureManager, PointerDevice, PointerEventKind, PointerId,
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
// Test Widget: Records events for verification
// ============================================================================

/// A test widget that records received events and can request capture
struct CaptureTestWidget {
    internal_id: WidgetInternalId,
    #[allow(dead_code)]
    name: &'static str,
    /// Records (local_x, local_y) of received pointer events
    received_events: Rc<RefCell<Vec<(f32, f32, &'static str)>>>,
    /// If true, requests capture on PointerDown
    request_capture_on_down: bool,
    /// If true, releases capture on PointerUp
    release_on_up: bool,
}

impl CaptureTestWidget {
    fn new(
        name: &'static str,
        events: Rc<RefCell<Vec<(f32, f32, &'static str)>>>,
        request_capture_on_down: bool,
        release_on_up: bool,
    ) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            name,
            received_events: events,
            request_capture_on_down,
            release_on_up,
        }
    }
}

impl WidgetDowncast for CaptureTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CaptureTestWidget {
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

            // INV-PC1: Explicit capture request (not implicit on down)
            if matches!(pe.kind, PointerEventKind::Down) && self.request_capture_on_down {
                cx.capture_pointer(PointerButton::Primary);
            }

            if matches!(pe.kind, PointerEventKind::Up) && self.release_on_up {
                cx.release_pointer();
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
    widget_a_captures: bool,
    widget_b_captures: bool,
) -> (WidgetTree, NodeId, TreeWidgetId, TreeWidgetId) {
    let mut tree = WidgetTree::new();
    let _guard = LayoutAuthorityGuard::enter();

    // Root container: 200x100
    let root = tree.create_root(
        Box::new(CaptureTestWidget::new(
            "root",
            Rc::new(RefCell::new(Vec::new())),
            false,
            false,
        )),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

    // Widget A: left side (0-100, 0-100)
    let widget_a = tree.add_child(
        root,
        Box::new(CaptureTestWidget::new(
            "A",
            events_a,
            widget_a_captures,
            true,
        )),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Widget B: right side (100-200, 0-100)
    let widget_b = tree.add_child(
        root,
        Box::new(CaptureTestWidget::new(
            "B",
            events_b,
            widget_b_captures,
            true,
        )),
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

fn route_pointer_down(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    root: NodeId,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
    x: f32,
    y: f32,
) {
    router.route_input_event(
        tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(x, y),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        Some(root),
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

fn route_pointer_move(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    root: NodeId,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
    x: f32,
    y: f32,
) {
    router.route_input_event(
        tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(x, y),
            delta: (0.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState {
                primary: true,
                ..PointerButtonsState::default()
            },
        },
        Some(root),
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

fn route_pointer_up(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    root: NodeId,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
    x: f32,
    y: f32,
) {
    router.route_input_event(
        tree,
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(x, y),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
        Some(root),
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

// ============================================================================
// Test: capture_exclusive_per_pointer (INV-PC2)
// ============================================================================

/// INV-PC2: At most one widget may capture a PointerId.
/// If Widget A captures, Widget B's request must fail.
#[test]
fn capture_exclusive_per_pointer() {
    let mut manager = PointerCaptureManager::new();

    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    // Widget A captures mouse
    let result = manager.request_capture(PointerId::MOUSE, widget_a);
    assert!(result.is_ok(), "First capture should succeed");
    assert_eq!(manager.current_capture(PointerId::MOUSE), Some(widget_a));

    // Widget B tries to capture same pointer - must fail (INV-PC2)
    let result = manager.request_capture(PointerId::MOUSE, widget_b);
    assert!(
        matches!(result, Err(CaptureError::AlreadyCaptured { by }) if by == widget_a),
        "Second widget capture must fail with AlreadyCaptured"
    );

    // Widget A still has capture
    assert_eq!(manager.current_capture(PointerId::MOUSE), Some(widget_a));
    assert!(manager.is_captured_by(PointerId::MOUSE, widget_a));
    assert!(!manager.is_captured_by(PointerId::MOUSE, widget_b));

    // Widget A requesting again is idempotent
    let result = manager.request_capture(PointerId::MOUSE, widget_a);
    assert!(result.is_ok(), "Re-capture by same widget should succeed");
}

// ============================================================================
// Test: capture_blocks_other_widgets
// ============================================================================

/// When Widget A has capture, pointer events must route to A
/// even when pointer is over Widget B (routing bypass).
#[test]
fn capture_blocks_other_widgets() {
    let events_a = Rc::new(RefCell::new(Vec::new()));
    let events_b = Rc::new(RefCell::new(Vec::new()));

    let (mut tree, root, _id_a, _id_b) = setup_two_widgets(
        events_a.clone(),
        events_b.clone(),
        true,  // A captures
        false, // B does not capture
    );

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // 1. Press down on Widget A (x=50, y=50) - A receives and captures
    route_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "A should have capture after down"
    );

    // Clear events to track only move
    events_a.borrow_mut().clear();
    events_b.borrow_mut().clear();

    // 2. Move to Widget B's area (x=150, y=50) - should still go to A
    route_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        150.0,
        50.0,
    );

    // Widget A should receive the move event (routing bypass)
    let a_events = events_a.borrow();
    let b_events = events_b.borrow();

    assert!(
        !a_events.is_empty(),
        "Widget A should receive move event during capture"
    );
    assert!(
        b_events.is_empty(),
        "Widget B should NOT receive events when A has capture"
    );

    // The local_pos for A should be relative to A's position
    // A is at (0,0) with size 100x100
    // Global pos is (150, 50), so local_pos = (150-0, 50-0) = (150, 50)
    let (x, _y, kind) = a_events[0];
    assert_eq!(kind, "move");
    // Note: local_pos may be outside widget bounds when captured
    assert!((x - 150.0).abs() < 1.0, "local_x should be ~150, got {}", x);
}

// ============================================================================
// Test: pointer_up_delivered_after_leave (INV-PC6)
// ============================================================================

/// INV-PC6: If a widget captures a pointer, it MUST receive pointer-up,
/// even if pointer leaves window bounds.
#[test]
fn pointer_up_delivered_after_leave() {
    let events_a = Rc::new(RefCell::new(Vec::new()));
    let events_b = Rc::new(RefCell::new(Vec::new()));

    let (mut tree, root, _id_a, _id_b) = setup_two_widgets(
        events_a.clone(),
        events_b.clone(),
        true,  // A captures
        false, // B does not capture
    );

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // 1. Press down on Widget A - captures
    route_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );

    events_a.borrow_mut().clear();

    // 2. Pointer leaves window bounds (simulate with position outside tree)
    // 3. PointerUp at position outside window - A must still receive it
    route_pointer_up(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        500.0,
        500.0,
    );

    // INV-PC6: Widget A must receive pointer-up
    let a_events = events_a.borrow();
    assert!(
        a_events.iter().any(|(_, _, kind)| *kind == "up"),
        "Widget A must receive pointer-up even when pointer is outside bounds"
    );
}

// ============================================================================
// Test: capture_released_on_up (INV-PC3)
// ============================================================================

/// INV-PC3: Capture ends on pointer up.
#[test]
fn capture_released_on_up() {
    let events_a = Rc::new(RefCell::new(Vec::new()));
    let events_b = Rc::new(RefCell::new(Vec::new()));

    let (mut tree, root, _id_a, _id_b) = setup_two_widgets(
        events_a.clone(),
        events_b.clone(),
        true, // A captures
        true, // A releases on up
    );

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // 1. Press down on Widget A - captures
    route_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Should have capture after down"
    );

    // 2. Release - INV-PC3: capture ends
    route_pointer_up(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Capture should be released after pointer up"
    );
    assert_eq!(
        domain_state
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
}

// ============================================================================
// Test: capture_released_on_cancel (INV-PC3)
// ============================================================================

/// INV-PC3: Capture ends on explicit cancel.
#[test]
fn capture_released_on_cancel() {
    let mut manager = PointerCaptureManager::new();

    let widget_a = TreeWidgetId(1);

    // Widget A captures
    assert!(manager.request_capture(PointerId::MOUSE, widget_a).is_ok());
    assert!(manager.is_captured(PointerId::MOUSE));

    // Cancel capture explicitly (INV-PC3)
    manager.cancel_capture(PointerId::MOUSE);

    assert!(!manager.is_captured(PointerId::MOUSE));
    assert_eq!(manager.current_capture(PointerId::MOUSE), None);

    // Now another widget can capture
    let widget_b = TreeWidgetId(2);
    assert!(manager.request_capture(PointerId::MOUSE, widget_b).is_ok());
    assert_eq!(manager.current_capture(PointerId::MOUSE), Some(widget_b));
}

// ============================================================================
// Test: capture_released_on_widget_destruction (INV-PC3)
// ============================================================================

/// INV-PC3: Capture ends on widget destruction.
#[test]
fn capture_released_on_widget_destruction() {
    let mut manager = PointerCaptureManager::new();

    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    // Both widgets capture different pointers (hypothetical multi-touch)
    let pointer_0 = PointerId(0);
    let pointer_1 = PointerId(1);

    assert!(manager.request_capture(pointer_0, widget_a).is_ok());
    assert!(manager.request_capture(pointer_1, widget_b).is_ok());

    assert_eq!(manager.current_capture(pointer_0), Some(widget_a));
    assert_eq!(manager.current_capture(pointer_1), Some(widget_b));

    // Widget A is destroyed - all its captures are released
    manager.release_all_for_widget(widget_a);

    assert_eq!(manager.current_capture(pointer_0), None);
    assert_eq!(
        manager.current_capture(pointer_1),
        Some(widget_b),
        "Widget B's capture should be unaffected"
    );
}
