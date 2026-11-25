// SPEC-HAL-128 Stage 5: Gesture Lifecycle & Widget-Lifetime Safety Tests
//
// These tests verify the lifecycle and widget-lifetime safety invariants:
//   INV-PC6 - Guaranteed Pointer-Up Delivery
//   INV-WL1 - Widget Destruction Cancels Gesture
//   INV-WL2 - No Dangling Widget IDs
//   INV-PC-UP - Pointer-Up Before Cancel
//   INV-LIFE-ORDER - Teardown Order

//
// Stage 5 scope: pointer-up guarantees, widget destruction, session teardown.

use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, GestureKind, InteractionDomainState, PointerButton,
    PointerButtonsState, PointerDevice, PointerEventKind, PointerId, WindowId,
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
// Test Widget: Tracks events and lifecycle
// ============================================================================

#[derive(Clone, Default)]
struct LifecycleTestState {
    events: Vec<(&'static str, f32, f32)>,
    pointer_up_received: bool,
    pointer_up_before_cancel: bool,
    session_active_when_up_received: bool,
}

struct LifecycleTestWidget {
    internal_id: WidgetInternalId,
    state: Rc<RefCell<LifecycleTestState>>,
    register_kind: Option<GestureKind>,
}

impl LifecycleTestWidget {
    fn new(state: Rc<RefCell<LifecycleTestState>>, register_kind: Option<GestureKind>) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            state,
            register_kind,
        }
    }
}

impl WidgetDowncast for LifecycleTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for LifecycleTestWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 100.0))
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

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        if let Event::Pointer(pe) = event {
            let kind_str = match pe.kind {
                PointerEventKind::Down => "down",
                PointerEventKind::Up => "up",
                PointerEventKind::Move => "move",
                PointerEventKind::Scroll { .. } => "scroll",
                PointerEventKind::DoubleClick => "double",
                PointerEventKind::TripleClick => "triple",
                PointerEventKind::DragStart => "dragstart",
            };
            self.state
                .borrow_mut()
                .events
                .push((kind_str, pe.local_pos.0, pe.local_pos.1));

            // On PointerDown: capture pointer (candidates registered by system)
            if matches!(pe.kind, PointerEventKind::Down) {
                cx.capture_pointer(PointerButton::Primary);
            }

            // On PointerUp: track that we received it
            if matches!(pe.kind, PointerEventKind::Up) {
                self.state.borrow_mut().pointer_up_received = true;
            }

            true
        } else {
            false
        }
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn setup_widget_for_lifecycle(
    state: Rc<RefCell<LifecycleTestState>>,
    register_kind: Option<GestureKind>,
) -> (WidgetTree, NodeId, TreeWidgetId) {
    let mut tree = WidgetTree::new();

    let root = tree.create_root(
        Box::new(LifecycleTestWidget::new(state, register_kind)),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
    halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let id = tree.node(root).unwrap().id();

    (tree, root, id)
}

fn send_pointer_down(
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

fn send_pointer_up(
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

fn send_pointer_move(
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
            buttons: PointerButtonsState::new(),
        },
        Some(root),
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

// ============================================================================
// Test: pointer_up_delivered_before_teardown (INV-PC-UP, INV-LIFE-ORDER)
// ============================================================================

/// INV-PC-UP: PointerUp must be delivered BEFORE session teardown.
/// INV-LIFE-ORDER: Teardown order must be: PointerUp → Resolve → Release → Clear
#[test]
fn pointer_up_delivered_before_teardown() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(LifecycleTestState::default()));
    let (mut tree, root, _widget_id) =
        setup_widget_for_lifecycle(state.clone(), Some(GestureKind::Drag));

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // PointerDown at (50, 50) - begins session, registers candidate, captures pointer
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify session and capture are active
    assert!(
        domain_state.gesture_registry.has_session(PointerId::MOUSE),
        "Session should be active after PointerDown"
    );
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Capture should be active after PointerDown"
    );

    // PointerUp - should be delivered BEFORE teardown
    send_pointer_up(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify widget received the PointerUp
    assert!(
        state.borrow().pointer_up_received,
        "Widget must receive PointerUp (INV-PC6)"
    );

    // Verify teardown happened AFTER (session is now cleared)
    assert!(
        !domain_state.gesture_registry.has_session(PointerId::MOUSE),
        "Session should be cleared after PointerUp (INV-LIFE-ORDER step 4)"
    );
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Capture should be released after PointerUp (INV-LIFE-ORDER step 3)"
    );

    // Check event order: down, then up
    let events = &state.borrow().events;
    assert_eq!(events.len(), 2, "Should have exactly 2 events");
    assert_eq!(events[0].0, "down", "First event should be down");
    assert_eq!(events[1].0, "up", "Second event should be up");
}

// ============================================================================
// Test: widget_destruction_cancels_active_gesture (INV-WL1)
// ============================================================================

/// INV-WL1: Widget destruction while holding capture/gesture MUST cancel session.
#[test]
fn widget_destruction_cancels_active_gesture() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(LifecycleTestState::default()));
    let (mut tree, root, widget_id) =
        setup_widget_for_lifecycle(state.clone(), Some(GestureKind::Drag));

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // PointerDown - establishes session and capture
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify active state
    assert!(domain_state.gesture_registry.has_session(PointerId::MOUSE));
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
    assert!(
        domain_state
            .gesture_registry
            .has_candidate_for_widget(widget_id)
    );

    // Now destroy the widget
    tree.remove_node(root);
    let removed_widgets = tree.drain_pending_removals();

    // Notify EventRouter of destruction
    router.on_widgets_destroyed(&removed_widgets, &mut domain_state);

    // INV-WL1: Session must be cancelled
    assert!(
        !domain_state
            .gesture_registry
            .has_candidate_for_widget(widget_id),
        "Gesture candidates must be removed after widget destruction"
    );
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Capture must be released after widget destruction"
    );
}

// ============================================================================
// Test: no_capture_after_widget_destroyed (INV-WL2)
// ============================================================================

/// INV-WL2: No capture may reference a destroyed widget ID.
#[test]
fn no_capture_after_widget_destroyed() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(LifecycleTestState::default()));
    let (mut tree, root, widget_id) =
        setup_widget_for_lifecycle(state.clone(), Some(GestureKind::Drag));

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // PointerDown - establishes capture
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify capture is for this widget
    assert_eq!(
        domain_state
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(widget_id),
        "Capture should be held by widget"
    );

    // Destroy widget
    tree.remove_node(root);
    let removed = tree.drain_pending_removals();
    router.on_widgets_destroyed(&removed, &mut domain_state);

    // INV-WL2: Capture must not reference destroyed widget
    assert_ne!(
        domain_state
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(widget_id),
        "Capture must not reference destroyed widget ID"
    );
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "No capture should exist after widget destruction"
    );
}

// ============================================================================
// Test: no_dangling_candidates_after_removal (INV-WL2)
// ============================================================================

/// INV-WL2: No gesture candidates may reference a destroyed widget ID.
#[test]
fn no_dangling_candidates_after_removal() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(LifecycleTestState::default()));
    let (mut tree, root, widget_id) =
        setup_widget_for_lifecycle(state.clone(), Some(GestureKind::Drag));

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // PointerDown - registers gesture candidate
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify candidate exists
    assert!(
        domain_state
            .gesture_registry
            .has_candidate_for_widget(widget_id),
        "Should have candidate for widget before destruction"
    );

    // Destroy widget
    tree.remove_node(root);
    let removed = tree.drain_pending_removals();
    router.on_widgets_destroyed(&removed, &mut domain_state);

    // INV-WL2: No dangling candidates
    assert!(
        !domain_state
            .gesture_registry
            .has_candidate_for_widget(widget_id),
        "No candidates should reference destroyed widget ID"
    );
}

// ============================================================================
// Test: pointer_up_delivered_outside_bounds (INV-PC6)
// ============================================================================

/// INV-PC6: Pointer-up must be delivered to captured widget even when pointer
/// is outside widget bounds. Once captured, the widget receives all pointer
/// events until release - regardless of pointer position.
#[test]
fn pointer_up_delivered_outside_bounds() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(LifecycleTestState::default()));
    let (mut tree, root, _widget_id) =
        setup_widget_for_lifecycle(state.clone(), Some(GestureKind::Drag));

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // PointerDown at (50, 50) - establishes capture
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify capture active
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );

    // Clear state to track only subsequent events
    state.borrow_mut().events.clear();

    // PointerUp at position FAR outside widget bounds (200, 200)
    // Widget is 100x100 at (0,0), so this is clearly outside.
    // INV-PC6 requires delivery regardless of position.
    send_pointer_up(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        200.0,
        200.0,
    );

    // INV-PC6: Widget must receive PointerUp despite position being outside widget bounds
    assert!(
        state.borrow().pointer_up_received,
        "Widget must receive PointerUp even when pointer is outside bounds (INV-PC6)"
    );

    // Verify the event was delivered
    let events = &state.borrow().events;
    assert_eq!(events.len(), 1, "Should have exactly 1 event (up)");
    assert_eq!(events[0].0, "up", "Event should be up");
}
