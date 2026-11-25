use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::core::app::{
    Screen, ScreenFrameContext, compute_absolute_paint_rects, compute_absolute_positions,
};
use crate::core::event::{
    CancellationReason, Event, EventPhase, FusedInputEvent, GestureKind, InteractionDomainId,
    InteractionDomainState, PointerButton, PointerButtonsState, PointerDevice, PointerEventKind,
    PointerId, RawButtonState, RawInputEvent, WindowId,
};
use crate::core::event_router::{EventRouter, RelativePointerWindowFacts};
use crate::core::input_fusion::InputFusion;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::relative_pointer::{
    CursorRestoreOutcome, CursorRestorePolicy, PointerMotionPolicy, RelativePointerAdmissionError,
    RelativePointerCancellationReason, RelativePointerLeaseBinding,
    RelativePointerNormalizationEpoch, RelativePointerTerminalOutcome,
};
use crate::core::relative_pointer_authority::RelativePointerLeaseAuthority;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::platform::relative_pointer::{
    RelativePointerPlatform, RelativePointerPlatformError, RelativePointerPlatformSupport,
};
use crate::runtime::binding_registry::BindingRegistry;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};

const BOUNDS: RectF = RectF::new(0.0, 0.0, 100.0, 100.0);
const DOWN: Point = Point { x: 20.0, y: 20.0 };

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlatformOperation {
    Support,
    Acquire,
    Hide,
    Restore(Point),
    Release,
    Show,
}

struct FakePlatform {
    operations: Rc<RefCell<Vec<PlatformOperation>>>,
    fail_acquire: bool,
    fail_release: bool,
    fail_show: bool,
}

impl RelativePointerPlatform for FakePlatform {
    fn support(&self, _: WindowId) -> RelativePointerPlatformSupport {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Support);
        RelativePointerPlatformSupport::Supported {
            normalization_epoch: RelativePointerNormalizationEpoch::new(7).unwrap(),
            restore_down_anchor: true,
        }
    }

    fn acquire(&mut self, _: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Acquire);
        if self.fail_acquire {
            Err(RelativePointerPlatformError::Denied)
        } else {
            Ok(())
        }
    }

    fn release(&mut self, _: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Release);
        if self.fail_release {
            Err(RelativePointerPlatformError::FailedPlatformOperation)
        } else {
            Ok(())
        }
    }

    fn hide_cursor(&mut self, _: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations.borrow_mut().push(PlatformOperation::Hide);
        Ok(())
    }

    fn show_cursor(&mut self, _: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations.borrow_mut().push(PlatformOperation::Show);
        if self.fail_show {
            Err(RelativePointerPlatformError::FailedPlatformOperation)
        } else {
            Ok(())
        }
    }

    fn restore_cursor(
        &mut self,
        _: WindowId,
        anchor: Point,
    ) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Restore(anchor));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OwnerEvent {
    Committed,
    RelativeMotion(f32, f32),
    Up,
    Terminal(RelativePointerTerminalOutcome),
}

struct ProbeWidget {
    id: WidgetInternalId,
    declares_drag: bool,
    release_on_motion: bool,
    events: Rc<RefCell<Vec<OwnerEvent>>>,
}

impl WidgetDowncast for ProbeWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ProbeWidget {
    fn layout(&mut self, _: &mut LayoutContext, _: Constraints, _: &[NodeId]) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 100.0))
    }

    fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn gesture_surface(&self) -> &'static [GestureKind] {
        if self.declares_drag {
            &[GestureKind::Drag]
        } else {
            &[]
        }
    }

    fn pointer_motion_policy(&self, _: GestureKind) -> PointerMotionPolicy {
        PointerMotionPolicy::RelativeLocked {
            restore: CursorRestorePolicy::DownAnchor,
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        match event {
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move => {
                cx.request_gesture_commit();
            }
            Event::Pointer(pointer) if pointer.kind == PointerEventKind::Up => {
                self.events.borrow_mut().push(OwnerEvent::Up);
            }
            Event::RelativePointerMotion(motion) => {
                self.events.borrow_mut().push(OwnerEvent::RelativeMotion(
                    motion.delta.x(),
                    motion.delta.y(),
                ));
                if self.release_on_motion {
                    cx.release_pointer();
                }
            }
            _ => {}
        }
        false
    }

    fn on_gesture_commit(&mut self, _: &mut EventContext, _: GestureKind) {
        self.events.borrow_mut().push(OwnerEvent::Committed);
    }

    fn on_relative_pointer_terminal(
        &mut self,
        _: &mut EventContext,
        _: GestureKind,
        outcome: RelativePointerTerminalOutcome,
    ) {
        self.events.borrow_mut().push(OwnerEvent::Terminal(outcome));
    }
}

struct Fixture {
    tree: WidgetTree,
    root: NodeId,
    owner: NodeId,
    router: EventRouter,
    domain: InteractionDomainState,
    text: FontManagerTextEngine,
    bindings: BindingRegistry,
    events: Rc<RefCell<Vec<OwnerEvent>>>,
    operations: Rc<RefCell<Vec<PlatformOperation>>>,
}

struct SemanticCancelScreen {
    owner: TreeWidgetId,
    widget: ProbeWidget,
}

impl SemanticCancelScreen {
    fn new(owner: TreeWidgetId) -> Self {
        Self {
            owner,
            widget: ProbeWidget {
                id: WidgetInternalId::new(),
                declares_drag: false,
                release_on_motion: false,
                events: Rc::new(RefCell::new(Vec::new())),
            },
        }
    }
}

impl Screen for SemanticCancelScreen {
    fn build<'a>(&'a mut self, _: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        &mut self.widget
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        tree.request_relative_pointer_semantic_cancel(self.owner, PointerId::MOUSE);
    }
}

impl Fixture {
    fn new(release_on_motion: bool, fail_cleanup: bool) -> Self {
        Self::with_lock_denial(release_on_motion, fail_cleanup, false)
    }

    fn with_lock_denial(release_on_motion: bool, fail_cleanup: bool, fail_acquire: bool) -> Self {
        let events = Rc::new(RefCell::new(Vec::new()));
        let operations = Rc::new(RefCell::new(Vec::new()));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(ProbeWidget {
                id: WidgetInternalId::new(),
                declares_drag: false,
                release_on_motion: false,
                events: Rc::new(RefCell::new(Vec::new())),
            }),
            None,
            0,
        );
        let owner = tree.add_child(
            root,
            Box::new(ProbeWidget {
                id: WidgetInternalId::new(),
                declares_drag: true,
                release_on_motion,
                events: events.clone(),
            }),
            None,
            1,
        );
        {
            let _layout = LayoutAuthorityGuard::enter();
            tree.set_rect(root, BOUNDS);
            tree.set_rect(owner, BOUNDS);
            compute_absolute_positions(&mut tree, root, Point::zero());
            compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }
        let mut domain = InteractionDomainState::new();
        domain.relative_pointer_authority = Some(Box::new(RelativePointerLeaseAuthority::new(
            WindowId::PRIMARY,
            InteractionDomainId::PRIMARY,
            FakePlatform {
                operations: operations.clone(),
                fail_acquire,
                fail_release: fail_cleanup,
                fail_show: fail_cleanup,
            },
        )));
        Self {
            tree,
            root,
            owner,
            router: EventRouter::new(),
            domain,
            text: FontManagerTextEngine::new(),
            bindings: BindingRegistry::new(),
            events,
            operations,
        }
    }

    fn route(&mut self, event: FusedInputEvent) {
        self.route_with_bounds(event, BOUNDS);
    }

    fn route_with_bounds(&mut self, event: FusedInputEvent, client_bounds: RectF) {
        self.router.route_input_event_with_relative_facts(
            &mut self.tree,
            event,
            Some(self.root),
            &mut self.text,
            None,
            &mut self.domain,
            &mut self.bindings,
            RelativePointerWindowFacts {
                window_id: WindowId::PRIMARY,
                interaction_domain_id: InteractionDomainId::PRIMARY,
                focused_window_id: Some(WindowId::PRIMARY),
                client_bounds,
            },
        );
    }

    fn activate(&mut self) -> RelativePointerLeaseBinding {
        self.route(FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: DOWN,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        });
        assert!(self.operations.borrow().is_empty());
        self.route(FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(30.0, 20.0),
            delta: (10.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState::new(),
        });
        let binding = self
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .unwrap()
            .binding();
        assert_eq!(self.events.borrow().as_slice(), &[OwnerEvent::Committed]);
        binding
    }

    fn drain_semantic_cancels(&mut self) -> bool {
        self.router.drain_relative_pointer_semantic_cancels(
            &mut self.tree,
            &mut self.domain,
            Some(RelativePointerWindowFacts {
                window_id: WindowId::PRIMARY,
                interaction_domain_id: InteractionDomainId::PRIMARY,
                focused_window_id: Some(WindowId::PRIMARY),
                client_bounds: BOUNDS,
            }),
        )
    }
}

#[test]
fn screen_update_semantic_cancel_terminates_without_another_input_event() {
    let mut fixture = Fixture::new(false, false);
    let binding = fixture.activate();
    let mut screen = SemanticCancelScreen::new(binding.capture_owner());

    screen.update(&mut fixture.tree);
    assert!(fixture.drain_semantic_cancels());

    assert_eq!(
        fixture.events.borrow().as_slice(),
        &[
            OwnerEvent::Committed,
            OwnerEvent::Terminal(RelativePointerTerminalOutcome::Cancelled {
                reason: RelativePointerCancellationReason::ExplicitSemanticCancel,
                restore: CursorRestoreOutcome::Restored,
            }),
        ]
    );
    assert!(fixture.operations.borrow().ends_with(&[
        PlatformOperation::Support,
        PlatformOperation::Restore(DOWN),
        PlatformOperation::Release,
        PlatformOperation::Show,
    ]));
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    assert!(
        !fixture
            .domain
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
    assert!(
        !fixture
            .domain
            .gesture_registry
            .has_session(PointerId::MOUSE)
    );
}

#[test]
fn wrong_and_duplicate_semantic_cancel_intents_do_not_touch_a_new_lease() {
    let mut fixture = Fixture::new(false, false);
    let binding = fixture.activate();
    let wrong_owner = fixture.tree.widget_id_for_node(fixture.root).unwrap();
    fixture
        .tree
        .request_relative_pointer_semantic_cancel(wrong_owner, PointerId::MOUSE);
    fixture
        .tree
        .request_relative_pointer_semantic_cancel(binding.capture_owner(), PointerId(9));
    assert!(!fixture.drain_semantic_cancels());
    assert_eq!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .unwrap()
            .binding(),
        binding
    );
    assert_eq!(fixture.events.borrow().as_slice(), &[OwnerEvent::Committed]);

    fixture
        .tree
        .request_relative_pointer_semantic_cancel(binding.capture_owner(), PointerId::MOUSE);
    fixture
        .tree
        .request_relative_pointer_semantic_cancel(binding.capture_owner(), PointerId::MOUSE);
    assert!(fixture.drain_semantic_cancels());
    assert!(!fixture.drain_semantic_cancels());
    assert_eq!(fixture.events.borrow().len(), 2);
    assert_eq!(
        fixture
            .operations
            .borrow()
            .iter()
            .filter(|op| **op == PlatformOperation::Release)
            .count(),
        1
    );

    fixture.events.borrow_mut().clear();
    fixture.operations.borrow_mut().clear();
    let next_binding = fixture.activate();
    assert_ne!(next_binding.lease_id(), binding.lease_id());
    assert!(!fixture.drain_semantic_cancels());
    assert_eq!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .unwrap()
            .binding(),
        next_binding
    );
}

#[test]
fn owner_unmount_precedes_queued_semantic_cancel_without_cursor_restore() {
    let mut fixture = Fixture::new(false, false);
    let binding = fixture.activate();
    fixture
        .tree
        .request_relative_pointer_semantic_cancel(binding.capture_owner(), PointerId::MOUSE);
    fixture.tree.remove_node(fixture.owner);
    let removed = fixture.tree.drain_pending_removals();
    fixture
        .router
        .on_widgets_destroyed(&removed, &mut fixture.domain);
    assert!(!fixture.drain_semantic_cancels());

    assert_eq!(fixture.events.borrow().as_slice(), &[OwnerEvent::Committed]);
    assert_eq!(
        fixture
            .operations
            .borrow()
            .iter()
            .filter(|op| **op == PlatformOperation::Release)
            .count(),
        1
    );
    assert!(
        !fixture
            .operations
            .borrow()
            .iter()
            .any(|op| matches!(op, PlatformOperation::Restore(_)))
    );
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    assert!(
        !fixture
            .domain
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
    assert!(
        !fixture
            .domain
            .gesture_registry
            .has_session(PointerId::MOUSE)
    );
}

#[test]
fn final_relative_delta_precedes_one_up_and_safe_restore() {
    let mut fixture = Fixture::new(false, false);
    let binding = fixture.activate();
    let gate = fixture
        .domain
        .relative_pointer_authority
        .as_ref()
        .unwrap()
        .active()
        .unwrap()
        .ingress_gate();
    let mut fusion = InputFusion::new();
    let mut fused = Vec::new();
    fusion.process_raw(
        RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state: RawButtonState::Pressed,
            position: DOWN,
        },
        &mut fused,
    );
    fused.clear();
    fusion.process_raw(
        RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            position: Point::new(30.0, 20.0),
            delta: (10.0, 0.0),
        },
        &mut fused,
    );
    fused.clear();
    for delta in [(2.0, 3.0), (4.0, 5.0)] {
        fusion.process_raw_with_relative_ingress(
            RawInputEvent::RelativePointerMotion {
                lease_id: binding.lease_id(),
                normalization_epoch: binding.normalization_epoch(),
                window_id: WindowId::PRIMARY,
                pointer_id: PointerId::MOUSE,
                time: Instant::now(),
                delta,
            },
            Some(&gate),
            &mut fused,
        );
    }
    assert!(fused.is_empty());
    fusion.process_raw_with_relative_ingress(
        RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state: RawButtonState::Released,
            position: Point::new(999.0, 999.0),
        },
        Some(&gate),
        &mut fused,
    );
    assert!(matches!(
        fused.as_slice(),
        [
            FusedInputEvent::RelativePointerMotion { .. },
            FusedInputEvent::DragEnd { .. },
            FusedInputEvent::PointerUp { .. }
        ]
    ));
    for event in fused {
        fixture.route(event);
    }
    assert_eq!(
        fixture.events.borrow().as_slice(),
        &[
            OwnerEvent::Committed,
            OwnerEvent::RelativeMotion(6.0, 8.0),
            OwnerEvent::Up,
            OwnerEvent::Terminal(RelativePointerTerminalOutcome::Completed {
                restore: CursorRestoreOutcome::Restored,
            }),
        ]
    );
    assert!(
        !fixture
            .domain
            .gesture_registry
            .has_session(PointerId::MOUSE)
    );
    assert!(
        !fixture
            .domain
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    let ops = fixture.operations.borrow();
    assert!(ops.ends_with(&[
        PlatformOperation::Support,
        PlatformOperation::Restore(DOWN),
        PlatformOperation::Release,
        PlatformOperation::Show,
    ]));
}

#[test]
fn widget_release_terminates_after_dispatch_with_typed_cancel() {
    let mut fixture = Fixture::new(true, false);
    let binding = fixture.activate();
    fixture.route(FusedInputEvent::RelativePointerMotion {
        lease_id: binding.lease_id(),
        window_id: WindowId::PRIMARY,
        pointer_id: PointerId::MOUSE,
        time: Instant::now(),
        delta: crate::core::relative_pointer::RelativeLogicalDelta::try_new(1.0, 2.0).unwrap(),
    });
    assert_eq!(fixture.events.borrow().len(), 3);
    assert_eq!(
        fixture.events.borrow()[1],
        OwnerEvent::RelativeMotion(1.0, 2.0)
    );
    assert_eq!(
        fixture.events.borrow()[2],
        OwnerEvent::Terminal(RelativePointerTerminalOutcome::Cancelled {
            reason: RelativePointerCancellationReason::ExplicitSemanticCancel,
            restore: CursorRestoreOutcome::Restored,
        })
    );
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    assert!(
        !fixture
            .domain
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
}

#[test]
fn forced_exits_drop_restore_and_clear_semantic_state_even_if_cleanup_fails() {
    for reason in [
        RelativePointerCancellationReason::FocusLost,
        RelativePointerCancellationReason::CaptureLost,
        RelativePointerCancellationReason::ApplicationShutdown,
        RelativePointerCancellationReason::NormalizationChanged,
    ] {
        let mut fixture = Fixture::new(false, true);
        fixture.activate();
        assert!(fixture.router.cancel_active_relative_with_tree(
            &mut fixture.tree,
            &mut fixture.domain,
            reason,
            None,
        ));
        assert!(!fixture.router.cancel_active_relative_with_tree(
            &mut fixture.tree,
            &mut fixture.domain,
            reason,
            None,
        ));
        assert_eq!(
            fixture.events.borrow()[1],
            OwnerEvent::Terminal(RelativePointerTerminalOutcome::Cancelled {
                reason,
                restore: CursorRestoreOutcome::SkippedUnsafeContext,
            })
        );
        assert!(
            !fixture
                .domain
                .gesture_registry
                .has_session(PointerId::MOUSE)
        );
        assert!(
            !fixture
                .domain
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE)
        );
        assert!(
            !fixture
                .operations
                .borrow()
                .iter()
                .any(|op| matches!(op, PlatformOperation::Restore(_)))
        );
        assert!(
            fixture
                .operations
                .borrow()
                .ends_with(&[PlatformOperation::Release, PlatformOperation::Show,])
        );
    }
}

#[test]
fn owner_unmount_and_unsafe_anchor_cleanup_are_distinct() {
    let mut unmounted = Fixture::new(false, false);
    unmounted.activate();
    unmounted.tree.remove_node(unmounted.owner);
    let removed = unmounted.tree.drain_pending_removals();
    unmounted
        .router
        .on_widgets_destroyed(&removed, &mut unmounted.domain);
    assert_eq!(
        unmounted.events.borrow().as_slice(),
        &[OwnerEvent::Committed]
    );
    assert!(
        unmounted
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    assert!(
        !unmounted
            .operations
            .borrow()
            .iter()
            .any(|op| matches!(op, PlatformOperation::Restore(_)))
    );

    let mut resized = Fixture::new(false, false);
    resized.activate();
    resized.route_with_bounds(
        FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(0.0, 0.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
        RectF::new(0.0, 0.0, 5.0, 5.0),
    );
    assert_eq!(
        resized.events.borrow().last().copied(),
        Some(OwnerEvent::Terminal(
            RelativePointerTerminalOutcome::Completed {
                restore: CursorRestoreOutcome::SkippedUnsafeContext,
            }
        ))
    );
    assert!(
        !resized
            .operations
            .borrow()
            .iter()
            .any(|op| matches!(op, PlatformOperation::Restore(_)))
    );
}

#[test]
fn fusion_discards_pending_relative_motion_on_focus_loss() {
    let mut fixture = Fixture::new(false, false);
    let binding = fixture.activate();
    let gate = fixture
        .domain
        .relative_pointer_authority
        .as_ref()
        .unwrap()
        .active()
        .unwrap()
        .ingress_gate();
    let mut fusion = InputFusion::new();
    let mut out = Vec::new();
    fusion.process_raw_with_relative_ingress(
        RawInputEvent::RelativePointerMotion {
            lease_id: binding.lease_id(),
            normalization_epoch: binding.normalization_epoch(),
            window_id: WindowId::PRIMARY,
            pointer_id: PointerId::MOUSE,
            time: Instant::now(),
            delta: (1.0, 1.0),
        },
        Some(&gate),
        &mut out,
    );
    fusion.process_raw_with_relative_ingress(
        RawInputEvent::FocusChanged {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            focused: false,
        },
        Some(&gate),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [FusedInputEvent::FocusChanged { focused: false, .. }]
    ));
    assert!(fixture.router.cancel_active_relative_with_tree(
        &mut fixture.tree,
        &mut fixture.domain,
        RelativePointerCancellationReason::FocusLost,
        None,
    ));
    fusion.reconcile_relative_ingress(None);
    out.clear();
    fusion.flush(&mut out);
    assert!(out.is_empty());
}

#[test]
fn relative_winner_without_authority_is_terminally_denied() {
    let mut fixture = Fixture::new(false, false);
    fixture.domain.relative_pointer_authority = None;
    fixture.route(FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: DOWN,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    });
    fixture.route(FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(30.0, 20.0),
        delta: (10.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState::new(),
    });
    assert_eq!(
        fixture.events.borrow().as_slice(),
        &[OwnerEvent::Terminal(
            RelativePointerTerminalOutcome::AdmissionDenied(
                RelativePointerAdmissionError::CapabilityUnsupported,
            )
        )]
    );
    assert!(
        !fixture
            .domain
            .gesture_registry
            .has_session(PointerId::MOUSE)
    );
    assert!(
        !fixture
            .domain
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
    assert!(fixture.operations.borrow().is_empty());
}

#[test]
fn denied_platform_lock_clears_the_gesture_without_hiding_the_cursor() {
    let mut fixture = Fixture::with_lock_denial(false, false, true);
    fixture.route(FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: DOWN,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    });
    fixture.route(FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(30.0, 20.0),
        delta: (10.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState::new(),
    });
    assert_eq!(
        fixture.events.borrow().as_slice(),
        &[OwnerEvent::Terminal(
            RelativePointerTerminalOutcome::AdmissionDenied(
                RelativePointerAdmissionError::PlatformLockDenied,
            )
        )]
    );
    assert_eq!(
        fixture.operations.borrow().as_slice(),
        &[PlatformOperation::Support, PlatformOperation::Acquire]
    );
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    assert!(
        !fixture
            .domain
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
    assert!(
        !fixture
            .domain
            .gesture_registry
            .has_session(PointerId::MOUSE)
    );
}

#[test]
fn stale_motion_is_ignored_and_capture_loss_cancels_the_live_lease() {
    let mut fixture = Fixture::new(false, false);
    let binding = fixture.activate();
    let delta = crate::core::relative_pointer::RelativeLogicalDelta::try_new(1.0, 2.0).unwrap();
    fixture.route(FusedInputEvent::RelativePointerMotion {
        lease_id: binding.lease_id(),
        window_id: WindowId(99),
        pointer_id: PointerId::MOUSE,
        time: Instant::now(),
        delta,
    });
    assert_eq!(fixture.events.borrow().as_slice(), &[OwnerEvent::Committed]);
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_some()
    );
    fixture
        .domain
        .pointer_capture_manager
        .cancel_capture(PointerId::MOUSE);
    fixture.route(FusedInputEvent::RelativePointerMotion {
        lease_id: binding.lease_id(),
        window_id: WindowId::PRIMARY,
        pointer_id: PointerId::MOUSE,
        time: Instant::now(),
        delta,
    });
    assert_eq!(
        fixture.events.borrow()[1],
        OwnerEvent::Terminal(RelativePointerTerminalOutcome::Cancelled {
            reason: RelativePointerCancellationReason::CaptureLost,
            restore: CursorRestoreOutcome::SkippedUnsafeContext,
        })
    );
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
}

#[test]
fn capture_loss_before_pointer_up_cancels_without_completing_or_routing_up() {
    let mut fixture = Fixture::new(false, false);
    fixture.activate();
    fixture
        .domain
        .pointer_capture_manager
        .cancel_capture(PointerId::MOUSE);
    fixture.route(FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: DOWN,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    });
    assert_eq!(
        fixture.events.borrow().as_slice(),
        &[
            OwnerEvent::Committed,
            OwnerEvent::Terminal(RelativePointerTerminalOutcome::Cancelled {
                reason: RelativePointerCancellationReason::CaptureLost,
                restore: CursorRestoreOutcome::SkippedUnsafeContext,
            }),
        ]
    );
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    assert!(
        !fixture
            .domain
            .gesture_registry
            .has_session(PointerId::MOUSE)
    );
    assert!(
        !fixture
            .operations
            .borrow()
            .iter()
            .any(|op| matches!(op, PlatformOperation::Restore(_)))
    );
}

#[test]
fn forced_cancel_revokes_lease_after_capture_and_session_records_disappear() {
    let mut fixture = Fixture::new(false, false);
    fixture.activate();
    fixture
        .domain
        .pointer_capture_manager
        .cancel_capture(PointerId::MOUSE);
    fixture
        .domain
        .gesture_registry
        .clear_candidates(PointerId::MOUSE);
    fixture.router.cancel_all_pointer_sessions_with_tree(
        &mut fixture.tree,
        CancellationReason::OverlayTakeover,
        &mut fixture.domain,
    );
    assert_eq!(
        fixture.events.borrow().as_slice(),
        &[
            OwnerEvent::Committed,
            OwnerEvent::Terminal(RelativePointerTerminalOutcome::Cancelled {
                reason: RelativePointerCancellationReason::CaptureLost,
                restore: CursorRestoreOutcome::SkippedUnsafeContext,
            }),
        ]
    );
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
    assert!(
        !fixture
            .operations
            .borrow()
            .iter()
            .any(|op| matches!(op, PlatformOperation::Restore(_)))
    );
}

#[test]
fn pointer_up_from_another_window_cannot_complete_or_reach_the_locked_owner() {
    let mut fixture = Fixture::new(false, false);
    fixture.activate();
    fixture.route(FusedInputEvent::PointerUp {
        window_id: WindowId(99),
        time: Instant::now(),
        pos: DOWN,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    });
    assert_eq!(fixture.events.borrow().as_slice(), &[OwnerEvent::Committed]);
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_some()
    );
    fixture.route(FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: DOWN,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    });
    assert_eq!(fixture.events.borrow().len(), 3);
    assert!(
        fixture
            .domain
            .relative_pointer_authority
            .as_ref()
            .unwrap()
            .active()
            .is_none()
    );
}
