// SPEC-HAL-128 Stage 3: Gesture Arbitration & Commit Tests
//
// These tests verify the gesture arbitration invariants:
//   INV-GA3 - Single Winner Rule
//   INV-GA4 - Deterministic Priority Resolution
//   INV-GA5 - Explicit Commit Only
//   INV-GA6 - Commit Causes Capture
//   INV-GA7 - Losers Are Cancelled
//
// Stage 3 scope: arbitration and commit, no axis locking or gesture mechanics.

use halcyon::core::event::{
    EventPhase, FusedInputEvent, GestureArbiter, GestureCommitError, GestureKind, GestureRegistry,
    InteractionDomainState, PointerButton, PointerDevice, PointerEventKind, PointerId, PointerType,
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
// Test Widget: Registers candidates and requests commit
// ============================================================================

/// Test widget state for tracking behavior
#[derive(Clone, Default)]
struct CommitTestState {
    /// Record of events received (kind_str, local_x, local_y)
    events: Vec<(&'static str, f32, f32)>,
    /// Whether commit was requested
    commit_requested: bool,
}

/// A test widget that registers gesture candidates and can request commit
struct CommitTestWidget {
    internal_id: WidgetInternalId,
    #[allow(dead_code)]
    name: &'static str,
    /// Shared state for verification
    state: Rc<RefCell<CommitTestState>>,
    /// Kind of gesture to register (if any)
    register_kind: Option<GestureKind>,
    /// If true, requests commit on PointerMove
    request_commit_on_move: bool,
    /// Movement threshold before requesting commit
    commit_threshold: f32,
    /// Start position for threshold calculation
    start_pos: RefCell<Option<(f32, f32)>>,
}

impl CommitTestWidget {
    fn new(
        name: &'static str,
        state: Rc<RefCell<CommitTestState>>,
        register_kind: Option<GestureKind>,
        request_commit_on_move: bool,
        commit_threshold: f32,
    ) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            name,
            state,
            register_kind,
            request_commit_on_move,
            commit_threshold,
            start_pos: RefCell::new(None),
        }
    }
}

impl WidgetDowncast for CommitTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CommitTestWidget {
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

    fn handle_event(
        &mut self,
        cx: &mut EventContext,
        event: &halcyon::core::event::Event,
        phase: halcyon::core::event::EventPhase,
    ) -> bool {
        // Only handle events in Target phase
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
            self.state
                .borrow_mut()
                .events
                .push((kind_str, pe.local_pos.0, pe.local_pos.1));

            // On PointerDown: record start position (candidates registered by system)
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
                        self.state.borrow_mut().commit_requested = true;
                    }
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

fn setup_single_widget(
    state: Rc<RefCell<CommitTestState>>,
    register_kind: Option<GestureKind>,
    request_commit_on_move: bool,
    commit_threshold: f32,
) -> (WidgetTree, NodeId, TreeWidgetId) {
    let mut tree = WidgetTree::new();

    let root = tree.create_root(
        Box::new(CommitTestWidget::new(
            "root",
            state,
            register_kind,
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

fn setup_two_widgets(
    state_a: Rc<RefCell<CommitTestState>>,
    state_b: Rc<RefCell<CommitTestState>>,
    kind_a: Option<GestureKind>,
    kind_b: Option<GestureKind>,
    commit_on_move_a: bool,
    commit_on_move_b: bool,
) -> (WidgetTree, NodeId, TreeWidgetId, TreeWidgetId) {
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(
        Box::new(CommitTestWidget::new(
            "root",
            Rc::new(RefCell::new(CommitTestState::default())),
            None,
            false,
            0.0,
        )),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

    // Widget A: left side (0-100)
    let widget_a = tree.add_child(
        root,
        Box::new(CommitTestWidget::new(
            "A",
            state_a,
            kind_a,
            commit_on_move_a,
            5.0, // 5px threshold
        )),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Widget B: right side (100-200)
    let widget_b = tree.add_child(
        root,
        Box::new(CommitTestWidget::new(
            "B",
            state_b,
            kind_b,
            commit_on_move_b,
            5.0,
        )),
        None,
        0,
    );
    tree.set_rect(widget_b, RectF::new(100.0, 0.0, 100.0, 100.0));

    halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
    halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let id_a = tree.node(widget_a).unwrap().id();
    let id_b = tree.node(widget_b).unwrap().id();

    (tree, root, id_a, id_b)
}

// ============================================================================
// Test: single_winner_rule (INV-GA3)
// ============================================================================

/// INV-GA3: At most one gesture may commit per PointerId.
#[test]
fn single_winner_rule() {
    let mut registry = GestureRegistry::new();
    let arbiter = GestureArbiter::new();

    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    // Register two candidates
    registry.register_candidate_request(PointerId::MOUSE, widget_a, GestureKind::Drag, None);
    registry.register_candidate_request(PointerId::MOUSE, widget_b, GestureKind::Scroll, None);

    let candidates = registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 2);

    let id_a = candidates[0].candidate_id;
    let id_b = candidates[1].candidate_id;

    // First commit succeeds
    let result = arbiter.try_commit(&mut registry, PointerId::MOUSE, id_a);
    assert!(result.is_ok(), "First commit should succeed");

    // Second commit fails with AlreadyCommitted
    let result = arbiter.try_commit(&mut registry, PointerId::MOUSE, id_b);
    assert!(
        matches!(result, Err(GestureCommitError::AlreadyCommitted { winner }) if winner == id_a),
        "Second commit must fail with AlreadyCommitted"
    );

    // Verify committed state
    assert!(registry.is_committed(PointerId::MOUSE));
    assert_eq!(registry.committed_gesture(PointerId::MOUSE), Some(id_a));
}

// ============================================================================
// Test: priority_resolution_is_deterministic (INV-GA4)
// ============================================================================

/// INV-GA4: Priority resolution order is Resize > Drag > Scroll > Custom.
#[test]
fn priority_resolution_is_deterministic() {
    let arbiter = GestureArbiter::new();

    // Create mock candidates with different kinds
    let resize_id = halcyon::core::event::GestureCandidateId {
        session: halcyon::core::event::GestureSessionId(1),
        local: 0,
    };
    let drag_id = halcyon::core::event::GestureCandidateId {
        session: halcyon::core::event::GestureSessionId(1),
        local: 1,
    };
    let scroll_id = halcyon::core::event::GestureCandidateId {
        session: halcyon::core::event::GestureSessionId(1),
        local: 2,
    };
    let custom_id = halcyon::core::event::GestureCandidateId {
        session: halcyon::core::event::GestureSessionId(1),
        local: 3,
    };

    let resize = halcyon::core::event::GestureCandidate::new_with_id(
        resize_id,
        TreeWidgetId(1),
        GestureKind::Resize,
    );
    let drag = halcyon::core::event::GestureCandidate::new_with_id(
        drag_id,
        TreeWidgetId(2),
        GestureKind::Drag,
    );
    let scroll = halcyon::core::event::GestureCandidate::new_with_id(
        scroll_id,
        TreeWidgetId(3),
        GestureKind::Scroll,
    );
    // SPEC-HAL-128 Stage 7: Use Select instead of Custom (Custom is not legal for Mouse)
    let select = halcyon::core::event::GestureCandidate::new_with_id(
        custom_id,
        TreeWidgetId(4),
        GestureKind::Select,
    );

    // SPEC-HAL-128 Stage 7: Drag is the PRIMARY gesture for Mouse, so it wins over all others
    // Priority order for Mouse: Drag (primary=0), Resize(1), Scroll(3), Select(4)
    let candidates: Vec<&halcyon::core::event::GestureCandidate> =
        vec![&drag, &scroll, &resize, &select];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);
    assert!(winner.is_some());
    assert_eq!(
        winner.unwrap().kind,
        GestureKind::Drag,
        "Drag is Mouse primary and wins over all others"
    );

    // Test: Resize wins against Scroll and Select (Drag not present)
    let candidates: Vec<&halcyon::core::event::GestureCandidate> = vec![&scroll, &resize, &select];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);
    assert!(winner.is_some());
    assert_eq!(
        winner.unwrap().kind,
        GestureKind::Resize,
        "Resize wins when Drag is not present"
    );

    // Test: Scroll wins against Select
    let candidates: Vec<&halcyon::core::event::GestureCandidate> = vec![&select, &scroll];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);
    assert!(winner.is_some());
    assert_eq!(winner.unwrap().kind, GestureKind::Scroll);

    // Test: Select alone
    let candidates: Vec<&halcyon::core::event::GestureCandidate> = vec![&select];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);
    assert!(winner.is_some());
    assert_eq!(winner.unwrap().kind, GestureKind::Select);

    // Test: Empty list
    let candidates: Vec<&halcyon::core::event::GestureCandidate> = vec![];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);
    assert!(winner.is_none());
}

// ============================================================================
// Test: commit_requires_explicit_request (INV-GA5)
// ============================================================================

/// INV-GA5: Commit only happens when explicitly requested.
/// No commit request = no commit, even if candidates exist.
#[test]
fn commit_requires_explicit_request() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(CommitTestState::default()));

    let (mut tree, root, _id) = setup_single_widget(
        state.clone(),
        Some(GestureKind::Drag), // Registers candidate
        false,                   // Does NOT request commit on move
        5.0,
    );

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down - registers candidate
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

    // Verify candidate registered
    assert!(
        domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Candidate should be registered"
    );

    // Move beyond threshold - but widget does NOT request commit
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(60.0, 60.0),
            delta: (10.0, 10.0),
            device: PointerDevice::Mouse,
            buttons: halcyon::core::event::PointerButtonsState::default(),
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // No commit should have happened
    assert!(
        !domain_state.gesture_registry.is_committed(PointerId::MOUSE),
        "No commit without explicit request"
    );
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "No capture without commit"
    );
    assert!(
        !state.borrow().commit_requested,
        "Widget should not have requested commit"
    );
}

// ============================================================================
// Test: commit_triggers_capture (INV-GA6)
// ============================================================================

/// INV-GA6: On commit, pointer is captured for the winning widget.
#[test]
fn commit_triggers_capture() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(CommitTestState::default()));

    let (mut tree, root, widget_id) = setup_single_widget(
        state.clone(),
        Some(GestureKind::Drag), // Registers candidate
        true,                    // Requests commit on move
        5.0,                     // 5px threshold
    );

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Verify no capture initially
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "No capture before commit"
    );

    // Pointer down - registers candidate
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

    // Still no capture (registration doesn't capture)
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "No capture from registration"
    );

    // Move beyond threshold - widget requests commit
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(60.0, 60.0),
            delta: (10.0, 10.0),
            device: PointerDevice::Mouse,
            buttons: halcyon::core::event::PointerButtonsState::default(),
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // INV-GA6: Capture should now be active
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Pointer should be captured after commit"
    );
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured_by(PointerId::MOUSE, widget_id),
        "Capture should be held by the committing widget"
    );
    assert!(
        state.borrow().commit_requested,
        "Widget should have requested commit"
    );
}

// ============================================================================
// Test: losers_are_cancelled (INV-GA7)
// ============================================================================

/// INV-GA7: On commit, all non-winning candidates are cancelled.
#[test]
fn losers_are_cancelled() {
    let mut registry = GestureRegistry::new();
    let arbiter = GestureArbiter::new();

    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);
    let widget_c = TreeWidgetId(3);

    // Register three candidates (all legal for Mouse per Stage 7)
    registry.register_candidate_request(PointerId::MOUSE, widget_a, GestureKind::Drag, None);
    registry.register_candidate_request(PointerId::MOUSE, widget_b, GestureKind::Scroll, None);
    registry.register_candidate_request(PointerId::MOUSE, widget_c, GestureKind::Select, None);

    let candidates = registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 3);

    let id_a = candidates[0].candidate_id;
    let id_b = candidates[1].candidate_id;
    let id_c = candidates[2].candidate_id;

    // Commit candidate A
    let result = arbiter.try_commit(&mut registry, PointerId::MOUSE, id_a);
    assert!(result.is_ok());

    let commit_result = result.unwrap();

    // Winner should be A
    assert_eq!(commit_result.winner, id_a);
    assert_eq!(commit_result.winner_widget, widget_a);

    // INV-GA7: B and C should be in the cancelled list
    assert_eq!(commit_result.cancelled.len(), 2);
    assert!(commit_result.cancelled.contains(&id_b));
    assert!(commit_result.cancelled.contains(&id_c));
}

// ============================================================================
// Test: no_commit_no_capture
// ============================================================================

/// No commit means no capture - candidates alone don't trigger capture.
#[test]
fn no_commit_no_capture() {
    let _guard = LayoutAuthorityGuard::enter();
    let state_a = Rc::new(RefCell::new(CommitTestState::default()));
    let state_b = Rc::new(RefCell::new(CommitTestState::default()));

    let (mut tree, root, _id_a, _id_b) = setup_two_widgets(
        state_a.clone(),
        state_b.clone(),
        Some(GestureKind::Drag),   // A registers Drag
        Some(GestureKind::Scroll), // B registers Scroll
        false,                     // A does NOT request commit
        false,                     // B does NOT request commit
    );

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down on A
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

    // Candidates registered, but no capture
    assert!(
        domain_state
            .gesture_registry
            .has_candidates(PointerId::MOUSE),
        "Candidates should exist"
    );
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "No capture without commit"
    );

    // Move around (no commit request)
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(100.0, 50.0),
            delta: (50.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: halcyon::core::event::PointerButtonsState::default(),
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Still no capture
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "No capture without explicit commit request"
    );
    assert!(
        !domain_state.gesture_registry.is_committed(PointerId::MOUSE),
        "No commit without explicit request"
    );
}

// ============================================================================
// Test: commit_fails_without_session
// ============================================================================

/// Commit fails if there's no active session.
#[test]
fn commit_fails_without_session() {
    let mut registry = GestureRegistry::new();
    let arbiter = GestureArbiter::new();

    // No session started
    let fake_id = halcyon::core::event::GestureCandidateId {
        session: halcyon::core::event::GestureSessionId(999),
        local: 0,
    };

    let result = arbiter.try_commit(&mut registry, PointerId::MOUSE, fake_id);
    assert!(
        matches!(result, Err(GestureCommitError::NoActiveSession)),
        "Commit should fail without session"
    );
}

// ============================================================================
// Test: commit_fails_for_unknown_candidate
// ============================================================================

/// Commit fails if the candidate ID is not found.
#[test]
fn commit_fails_for_unknown_candidate() {
    let mut registry = GestureRegistry::new();
    let arbiter = GestureArbiter::new();

    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);
    registry.register_candidate_request(PointerId::MOUSE, TreeWidgetId(1), GestureKind::Drag, None);

    // Try to commit with wrong candidate ID
    let fake_id = halcyon::core::event::GestureCandidateId {
        session: halcyon::core::event::GestureSessionId(1),
        local: 999, // Non-existent local ID
    };

    let result = arbiter.try_commit(&mut registry, PointerId::MOUSE, fake_id);
    assert!(
        matches!(result, Err(GestureCommitError::CandidateNotFound)),
        "Commit should fail for unknown candidate"
    );
}

// ============================================================================
// Test: session_cleared_on_pointer_up_clears_commit
// ============================================================================

/// When session is cleared on pointer up, commit state is also cleared.
#[test]
fn session_cleared_on_pointer_up_clears_commit() {
    let mut registry = GestureRegistry::new();
    let arbiter = GestureArbiter::new();

    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);
    registry.register_candidate_request(PointerId::MOUSE, TreeWidgetId(1), GestureKind::Drag, None);

    let candidate_id = registry.candidates(PointerId::MOUSE)[0].candidate_id;

    // Commit
    let result = arbiter.try_commit(&mut registry, PointerId::MOUSE, candidate_id);
    assert!(result.is_ok());
    assert!(registry.is_committed(PointerId::MOUSE));

    // Clear session (simulates pointer up)
    registry.clear_candidates(PointerId::MOUSE);

    // Verify everything is cleared
    assert!(!registry.has_candidates(PointerId::MOUSE));
    assert!(!registry.is_committed(PointerId::MOUSE));
    assert!(registry.committed_gesture(PointerId::MOUSE).is_none());
}

// ============================================================================
// Test: commit_priority_wins_even_if_lower_priority_requests_first
// ============================================================================

/// A test widget that registers on ALL phases (to test priority across tree structure)
struct PriorityTestWidget {
    internal_id: WidgetInternalId,
    #[allow(dead_code)]
    name: &'static str,
    state: Rc<RefCell<CommitTestState>>,
    register_kind: Option<GestureKind>,
    commit_threshold: f32,
    start_pos: RefCell<Option<(f32, f32)>>,
}

impl PriorityTestWidget {
    fn new(
        name: &'static str,
        state: Rc<RefCell<CommitTestState>>,
        register_kind: Option<GestureKind>,
        commit_threshold: f32,
    ) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            name,
            state,
            register_kind,
            commit_threshold,
            start_pos: RefCell::new(None),
        }
    }
}

impl WidgetDowncast for PriorityTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for PriorityTestWidget {
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

    fn handle_event(
        &mut self,
        cx: &mut EventContext,
        event: &halcyon::core::event::Event,
        phase: halcyon::core::event::EventPhase,
    ) -> bool {
        // Handle events in Target phase (for inner widget) and Bubble phase (for outer widgets)
        if phase != EventPhase::Target && phase != EventPhase::Bubble {
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
            self.state
                .borrow_mut()
                .events
                .push((kind_str, pe.local_pos.0, pe.local_pos.1));

            if matches!(pe.kind, PointerEventKind::Down) {
                *self.start_pos.borrow_mut() = Some(pe.local_pos);
            }

            if matches!(pe.kind, PointerEventKind::Move) {
                if let Some((start_x, start_y)) = *self.start_pos.borrow() {
                    let dx = pe.local_pos.0 - start_x;
                    let dy = pe.local_pos.1 - start_y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    if distance >= self.commit_threshold && self.register_kind.is_some() {
                        cx.request_gesture_commit();
                        self.state.borrow_mut().commit_requested = true;
                    }
                }
            }

            // Don't stop propagation to allow both widgets to handle
            false
        } else {
            false
        }
    }
}

/// SPEC-HAL-128 Stage 3 Fix: Priority resolution works regardless of dispatch order.
///
/// This test creates a nested tree structure:
///   Root
///    └─ Outer (Scroll - lower priority)
///        └─ Inner (Resize - higher priority)
///
/// Both widgets register candidates on Down and request commit on Move.
/// The Outer widget (Scroll) is processed FIRST (in Capture phase).
/// The Inner widget (Resize) is processed SECOND (in Target phase).
///
/// Despite Scroll requesting commit before Resize in dispatch order,
/// the priority-based arbitration must select Resize as the winner.
#[test]
fn commit_priority_wins_even_if_lower_priority_requests_first() {
    let _guard = LayoutAuthorityGuard::enter();
    let state_outer = Rc::new(RefCell::new(CommitTestState::default()));
    let state_inner = Rc::new(RefCell::new(CommitTestState::default()));

    let mut tree = WidgetTree::new();

    // Create tree: Root -> Outer (Scroll) -> Inner (Resize)
    // Dispatch order for Down: Outer sees Capture first, then Inner sees Target
    let root = tree.create_root(
        Box::new(PriorityTestWidget::new(
            "root",
            Rc::new(RefCell::new(CommitTestState::default())),
            None,
            5.0,
        )),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 200.0));

    // Outer: registers Scroll (priority 2) - will be processed first in capture phase
    let outer = tree.add_child(
        root,
        Box::new(PriorityTestWidget::new(
            "outer",
            state_outer.clone(),
            Some(GestureKind::Scroll),
            5.0,
        )),
        None,
        0,
    );
    tree.set_rect(outer, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Inner: registers Resize (priority 0) - will be processed second in target phase
    let inner = tree.add_child(
        outer,
        Box::new(PriorityTestWidget::new(
            "inner",
            state_inner.clone(),
            Some(GestureKind::Resize),
            5.0,
        )),
        None,
        0,
    );
    tree.set_rect(inner, RectF::new(0.0, 0.0, 100.0, 100.0));

    halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
    halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let inner_widget_id = tree.node(inner).unwrap().id();

    let mut router = EventRouter::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut domain_state = InteractionDomainState::new();

    // Pointer down at (50, 50) - hits Inner, but Outer sees it in Capture/Bubble
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

    // Both widgets should have registered candidates
    let candidates = domain_state.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(
        candidates.len(),
        2,
        "Both widgets should have registered candidates"
    );

    // Verify we have one Scroll and one Resize candidate
    let has_scroll = candidates.iter().any(|c| c.kind == GestureKind::Scroll);
    let has_resize = candidates.iter().any(|c| c.kind == GestureKind::Resize);
    assert!(
        has_scroll && has_resize,
        "Should have both Scroll and Resize candidates"
    );

    // Move pointer beyond threshold - both widgets request commit
    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(60.0, 60.0),
            delta: (10.0, 10.0),
            device: PointerDevice::Mouse,
            buttons: halcyon::core::event::PointerButtonsState::default(),
        },
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // INV-GA4: Resize (priority 0) must win over Scroll (priority 2)
    assert!(
        domain_state.gesture_registry.is_committed(PointerId::MOUSE),
        "A gesture should have committed"
    );

    // INV-GA7: Losers are structurally removed - only winner remains
    let remaining = domain_state.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(
        remaining.len(),
        1,
        "Only the winner should remain in the registry after commit"
    );
    assert_eq!(
        remaining[0].kind,
        GestureKind::Resize,
        "The winner should be Resize (higher priority)"
    );

    // INV-GA6: Pointer capture should be held by the Resize widget (inner)
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Pointer should be captured after commit"
    );
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured_by(PointerId::MOUSE, inner_widget_id),
        "Capture should be held by the Resize widget (inner)"
    );
}
