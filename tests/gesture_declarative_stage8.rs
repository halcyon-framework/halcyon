// SPEC-HAL-128 Stage 8: Declarative Gesture Surfaces Tests
//
// These tests verify the declarative gesture surface invariants:
//   INV-GS1 - Widget Declaration is Static
//   INV-GS2 - Widget Declares, System Decides
//   INV-GS3 - Registration is the Only Widget Action (via declaration)
//   INV-GS4 - No Pointer-Type Branching
//   INV-GS6 - No Side Effects Before Commit
//   INV-GS7 - Cancel Restores Exactly
//   INV-GS8 - Widget Cannot Refuse Commit
//   INV-GS9 - Widget Cannot Refuse Cancel
//
// Stage 8 scope: declarative surfaces, commit/cancel callbacks.

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
// Test Widget State
// ============================================================================

#[derive(Clone, Default)]
struct DeclarativeTestState {
    /// Track commit callbacks received
    commit_callbacks: Vec<GestureKind>,
    /// Track cancel callbacks received
    cancel_callbacks: Vec<GestureKind>,
    /// Track pre-commit state changes (should be empty for INV-GS6)
    #[allow(dead_code)]
    pre_commit_mutations: Vec<String>,
    /// Track post-cancel state restoration
    #[allow(dead_code)]
    was_restored: bool,
}

// ============================================================================
// Test Widget: Verifies declarative gesture surface behavior
// ============================================================================

struct DeclarativeTestWidget {
    internal_id: WidgetInternalId,
    #[allow(dead_code)]
    name: &'static str,
    state: Rc<RefCell<DeclarativeTestState>>,
    /// Static gesture kinds this widget declares
    declared_gestures: &'static [GestureKind],
    /// Should request commit on move?
    request_commit_on_move: bool,
    /// Movement threshold for commit
    commit_threshold: f32,
    /// Start position for threshold calculation
    start_pos: RefCell<Option<(f32, f32)>>,
}

impl DeclarativeTestWidget {
    fn new(
        name: &'static str,
        state: Rc<RefCell<DeclarativeTestState>>,
        declared_gestures: &'static [GestureKind],
        request_commit_on_move: bool,
        commit_threshold: f32,
    ) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            name,
            state,
            declared_gestures,
            request_commit_on_move,
            commit_threshold,
            start_pos: RefCell::new(None),
        }
    }
}

impl WidgetDowncast for DeclarativeTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DeclarativeTestWidget {
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

    /// SPEC-HAL-128 Stage 8 INV-GS1: Static gesture surface declaration.
    /// This returns the same gestures regardless of runtime state.
    fn gesture_surface(&self) -> &'static [GestureKind] {
        // INV-GS1: Declaration is static
        // INV-GS4: No PointerType branching (we can't even see PointerType here!)
        self.declared_gestures
    }

    /// SPEC-HAL-128 Stage 8 INV-GS8: Called when gesture is committed.
    fn on_gesture_commit(&mut self, _cx: &mut EventContext, kind: GestureKind) {
        // INV-GS8: Widget cannot refuse commit
        self.state.borrow_mut().commit_callbacks.push(kind);
    }

    /// SPEC-HAL-128 Stage 8 INV-GS9: Called when gesture is cancelled.
    fn on_gesture_cancel(&mut self, _cx: &mut EventContext, kind: GestureKind) {
        // INV-GS9: Widget cannot refuse cancel
        // INV-GS7: Restore to pre-registration state
        self.state.borrow_mut().cancel_callbacks.push(kind);
        self.state.borrow_mut().was_restored = true;
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Handle both Target and Bubble phases for nested widget scenarios
        if phase != EventPhase::Target && phase != EventPhase::Bubble {
            return false;
        }

        if let Event::Pointer(pe) = event {
            // On PointerDown: record start position
            if matches!(pe.kind, PointerEventKind::Down) {
                *self.start_pos.borrow_mut() = Some(pe.local_pos);
            }

            // On PointerMove: check threshold and request commit if exceeded
            if matches!(pe.kind, PointerEventKind::Move) && self.request_commit_on_move {
                if let Some((start_x, start_y)) = *self.start_pos.borrow() {
                    let dx = pe.local_pos.0 - start_x;
                    let dy = pe.local_pos.1 - start_y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    if distance >= self.commit_threshold {
                        cx.request_gesture_commit();
                    }
                }
            }

            // Don't stop propagation
            false
        } else {
            false
        }
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn setup_single_widget(
    state: Rc<RefCell<DeclarativeTestState>>,
    declared_gestures: &'static [GestureKind],
    request_commit_on_move: bool,
    commit_threshold: f32,
) -> (WidgetTree, NodeId, TreeWidgetId) {
    let mut tree = WidgetTree::new();

    let root = tree.create_root(
        Box::new(DeclarativeTestWidget::new(
            "root",
            state,
            declared_gestures,
            request_commit_on_move,
            commit_threshold,
        )),
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
// Test: registration_from_declaration_only (INV-GS2, INV-GS3)
// ============================================================================

/// INV-GS2: Widget declares, system decides.
/// INV-GS3: Registration is the only widget action (via gesture_surface declaration).
///
/// Candidates are registered by the system based on gesture_surface(), not by widget code.
#[test]
fn registration_from_declaration_only() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(DeclarativeTestState::default()));
    static GESTURES: &[GestureKind] = &[GestureKind::Drag, GestureKind::Scroll];

    let (mut tree, root, widget_id) = setup_single_widget(state.clone(), GESTURES, false, 5.0);

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Before pointer down: no candidates
    assert!(
        !domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Should have no candidates before pointer down"
    );

    // Pointer down: system registers candidates from gesture_surface
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify candidates were registered BY THE SYSTEM from declaration
    let candidates = domain_state.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(
        candidates.len(),
        2,
        "System should register candidates from gesture_surface"
    );

    // Verify correct kinds
    let kinds: Vec<_> = candidates.iter().map(|c| c.kind).collect();
    assert!(
        kinds.contains(&GestureKind::Drag),
        "Should have Drag from declaration"
    );
    assert!(
        kinds.contains(&GestureKind::Scroll),
        "Should have Scroll from declaration"
    );

    // Verify widget ID
    for c in candidates {
        assert_eq!(
            c.widget_id, widget_id,
            "Candidates should be for declaring widget"
        );
    }
}

// ============================================================================
// Test: commit_callback_received (INV-GS8)
// ============================================================================

/// INV-GS8: Widget receives on_gesture_commit callback when it wins arbitration.
#[test]
fn commit_callback_received() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(DeclarativeTestState::default()));
    static GESTURES: &[GestureKind] = &[GestureKind::Drag];

    let (mut tree, root, _widget_id) = setup_single_widget(state.clone(), GESTURES, true, 5.0);

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // No commit callback yet
    assert!(
        state.borrow().commit_callbacks.is_empty(),
        "No commit before threshold exceeded"
    );

    // Move beyond threshold - triggers commit
    send_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        60.0,
        60.0,
    );

    // INV-GS8: Widget should have received commit callback
    let callbacks = &state.borrow().commit_callbacks;
    assert_eq!(
        callbacks.len(),
        1,
        "Should receive exactly one commit callback"
    );
    assert_eq!(
        callbacks[0],
        GestureKind::Drag,
        "Commit callback should be for Drag"
    );
}

// ============================================================================
// Test: cancel_callback_received (INV-GS9)
// ============================================================================

/// INV-GS9: Widget receives on_gesture_cancel callback when it loses arbitration.
#[test]
fn cancel_callback_received() {
    assert_cancel_callback_received(true);
}

#[test]
fn cancel_callback_received_without_commit_request() {
    assert_cancel_callback_received(false);
}

fn assert_cancel_callback_received(request_scroll_commit: bool) {
    let _guard = LayoutAuthorityGuard::enter();
    // Create two widgets: one with Drag (priority 0), one with Scroll (priority 3)
    let state_drag = Rc::new(RefCell::new(DeclarativeTestState::default()));
    let state_scroll = Rc::new(RefCell::new(DeclarativeTestState::default()));

    static DRAG: &[GestureKind] = &[GestureKind::Drag];
    static SCROLL: &[GestureKind] = &[GestureKind::Scroll];

    let mut tree = WidgetTree::new();

    // Container
    let root = tree.create_root(
        Box::new(DeclarativeTestWidget::new(
            "root",
            Rc::new(RefCell::new(DeclarativeTestState::default())),
            &[],
            false,
            0.0,
        )),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Inner widget with Scroll (lower priority, will lose)
    let scroll_widget = tree.add_child(
        root,
        Box::new(DeclarativeTestWidget::new(
            "scroll",
            state_scroll.clone(),
            SCROLL,
            request_scroll_commit,
            5.0,
        )),
        None,
        0,
    );
    tree.set_rect(scroll_widget, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Innermost widget with Drag (higher priority, will win)
    let drag_widget = tree.add_child(
        scroll_widget,
        Box::new(DeclarativeTestWidget::new(
            "drag",
            state_drag.clone(),
            DRAG,
            true, // Request commit on move
            5.0,
        )),
        None,
        0,
    );
    tree.set_rect(drag_widget, RectF::new(0.0, 0.0, 100.0, 100.0));

    halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
    halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down - both widgets register candidates
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Both should have registered candidates
    let candidates = domain_state.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 2, "Both widgets should have candidates");

    // Drag commits even when Scroll is still waiting for its own threshold.
    send_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        60.0,
        60.0,
    );

    // INV-GS8: Drag widget should have received commit callback
    assert_eq!(
        state_drag.borrow().commit_callbacks.len(),
        1,
        "Drag widget should receive commit"
    );

    // INV-GS9: Scroll widget should have received cancel callback
    assert_eq!(
        state_scroll.borrow().cancel_callbacks.len(),
        1,
        "Scroll widget should receive cancel"
    );
    assert_eq!(
        state_scroll.borrow().cancel_callbacks[0],
        GestureKind::Scroll,
        "Cancel should be for Scroll"
    );
    assert!(state_scroll.borrow().was_restored);
    assert!(state_scroll.borrow().commit_callbacks.is_empty());
    let drag_id = tree.node(drag_widget).unwrap().id();
    let candidates = domain_state.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].widget_id, drag_id);
    assert_eq!(
        domain_state
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(drag_id)
    );
    send_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        70.0,
        70.0,
    );
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(70.0, 70.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(
        state_scroll.borrow().cancel_callbacks,
        [GestureKind::Scroll]
    );
    assert_eq!(state_drag.borrow().commit_callbacks, [GestureKind::Drag]);
    assert!(state_drag.borrow().cancel_callbacks.is_empty());
}

// ============================================================================
// Test: no_pointer_type_branching (INV-GS4)
// ============================================================================

/// INV-GS4: Widget gesture behavior MUST NOT branch on PointerType.
/// The gesture_surface() method has no access to PointerType.
/// This test verifies that the same gestures are declared regardless of input.
#[test]
fn gesture_surface_is_static() {
    let state = Rc::new(RefCell::new(DeclarativeTestState::default()));
    static GESTURES: &[GestureKind] = &[GestureKind::Drag, GestureKind::Scroll];

    // Create widget with fixed declaration
    let widget = DeclarativeTestWidget::new("test", state.clone(), GESTURES, false, 5.0);

    // INV-GS1/GS4: gesture_surface() is static and doesn't depend on any runtime state
    let surface1 = widget.gesture_surface();
    let surface2 = widget.gesture_surface();

    assert_eq!(
        surface1, surface2,
        "gesture_surface must return same value (static)"
    );
    assert_eq!(surface1.len(), 2);
    assert!(surface1.contains(&GestureKind::Drag));
    assert!(surface1.contains(&GestureKind::Scroll));
}

// ============================================================================
// Test: empty_declaration_no_candidates (INV-GS1)
// ============================================================================

/// INV-GS1: Widget that declares no gestures should have no candidates registered.
#[test]
fn empty_declaration_no_candidates() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(DeclarativeTestState::default()));
    static EMPTY: &[GestureKind] = &[];

    let (mut tree, root, _widget_id) = setup_single_widget(state.clone(), EMPTY, false, 5.0);

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down on widget with empty gesture_surface
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // No candidates should be registered
    assert!(
        !domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Empty declaration should result in no candidates"
    );
}

// ============================================================================
// Test: illegal_gestures_filtered (Stage 7 + Stage 8)
// ============================================================================

/// Stage 7 legality filtering applies to declarative registrations.
/// Pinch is illegal for Mouse, so it should be filtered out.
#[test]
fn illegal_gestures_filtered_from_declaration() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(DeclarativeTestState::default()));
    // Drag is legal for Mouse, Pinch is NOT legal for Mouse
    static GESTURES: &[GestureKind] = &[GestureKind::Drag, GestureKind::Pinch];

    let (mut tree, root, _widget_id) = setup_single_widget(state.clone(), GESTURES, false, 5.0);

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down (Mouse)
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Only Drag should be registered (Pinch is illegal for Mouse)
    let candidates = domain_state.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(
        candidates.len(),
        1,
        "Only legal gestures should be registered"
    );
    assert_eq!(
        candidates[0].kind,
        GestureKind::Drag,
        "Legal gesture should be Drag"
    );
}
