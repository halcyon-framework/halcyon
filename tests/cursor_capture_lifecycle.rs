use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    CancellationReason, CursorIcon, Event, EventPhase, FusedInputEvent, InteractionDomainState,
    PointerButton, PointerDevice, PointerEventKind, PointerId, WindowId,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::runtime::binding_registry::BindingRegistry;
use halcyon::utils::id::{TreeWidgetId, WidgetInternalId};
use std::time::Instant;

struct CursorCaptureWidget {
    id: WidgetInternalId,
    capture_on_down: bool,
}

impl CursorCaptureWidget {
    fn new(capture_on_down: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            capture_on_down,
        }
    }

    fn restore_cursor(cx: &mut EventContext) {
        cx.request_cursor(CursorIcon::Default);
        cx.release_pointer();
    }
}

impl WidgetDowncast for CursorCaptureWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CursorCaptureWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        let Event::Pointer(pointer) = event else {
            return false;
        };

        match (pointer.kind, pointer.button) {
            (PointerEventKind::Down, Some(PointerButton::Secondary)) => {
                cx.request_focus();
                cx.request_keyboard_capture();
                if self.capture_on_down {
                    cx.capture_pointer(PointerButton::Secondary);
                }
                cx.request_cursor(CursorIcon::Hidden);
                true
            }
            (PointerEventKind::Up, Some(PointerButton::Secondary)) => {
                Self::restore_cursor(cx);
                true
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        Self::restore_cursor(cx);
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        Self::restore_cursor(cx);
    }
}

struct Harness {
    tree: WidgetTree,
    root: NodeId,
    widget_id: TreeWidgetId,
    router: EventRouter,
    domain_state: InteractionDomainState,
    text_engine: FontManagerTextEngine,
    binding_registry: BindingRegistry,
}

impl Harness {
    fn new(capture_on_down: bool) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(CursorCaptureWidget::new(capture_on_down)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));
        let widget_id = tree.node(root).expect("root widget must exist").id();
        compute_absolute_positions(&mut tree, root, Point::zero());
        compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        Self {
            tree,
            root,
            widget_id,
            router: EventRouter::new(),
            domain_state: InteractionDomainState::new(),
            text_engine: FontManagerTextEngine::new(),
            binding_registry: BindingRegistry::new(),
        }
    }

    fn secondary_down(&mut self) {
        self.router.route_input_event(
            &mut self.tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(50.0, 50.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Secondary,
                click_count: 1,
            },
            Some(self.root),
            &mut self.text_engine,
            None,
            &mut self.domain_state,
            &mut self.binding_registry,
        );
    }

    fn secondary_up(&mut self, pos: Point) {
        self.router.route_input_event(
            &mut self.tree,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Secondary,
            },
            Some(self.root),
            &mut self.text_engine,
            None,
            &mut self.domain_state,
            &mut self.binding_registry,
        );
    }

    fn window_focus_lost(&mut self) {
        self.router.route_input_event(
            &mut self.tree,
            FusedInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                focused: false,
            },
            Some(self.root),
            &mut self.text_engine,
            None,
            &mut self.domain_state,
            &mut self.binding_registry,
        );
    }

    fn tick(&mut self) {
        self.router.route_input_event(
            &mut self.tree,
            FusedInputEvent::Tick {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                dt: std::time::Duration::from_millis(16),
            },
            Some(self.root),
            &mut self.text_engine,
            None,
            &mut self.domain_state,
            &mut self.binding_registry,
        );
    }

    fn is_captured(&self) -> bool {
        self.domain_state
            .pointer_capture_manager
            .is_captured_by(PointerId::MOUSE, self.widget_id)
    }
}

#[test]
fn cursor_hidden_request_requires_same_widget_pointer_capture() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut harness = Harness::new(false);

    harness.secondary_down();

    assert!(!harness.is_captured());
    assert_eq!(harness.router.take_requested_cursor(), None);
}

#[test]
fn cursor_capture_hides_until_normal_up_outside_bounds() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut harness = Harness::new(true);

    harness.secondary_down();
    assert!(harness.is_captured());
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Hidden)
    );

    harness.secondary_up(Point::new(500.0, 500.0));

    assert!(!harness.is_captured());
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Default)
    );
}

#[test]
fn cursor_capture_leave_honors_owner_restore_and_release_requests() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut harness = Harness::new(true);

    harness.secondary_down();
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Hidden)
    );

    harness.router.notify_capture_leave(
        &mut harness.tree,
        &mut harness.domain_state,
        &mut harness.text_engine,
    );

    assert!(!harness.is_captured());
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Default)
    );
}

#[test]
fn cursor_forced_cancel_restoration_survives_frame_reset() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut harness = Harness::new(true);

    harness.secondary_down();
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Hidden)
    );

    harness
        .router
        .cancel_all_pointer_sessions(CancellationReason::SystemCancel, &mut harness.domain_state);
    harness.router.reset_cursor_request();

    assert!(!harness.is_captured());
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Default)
    );
}

#[test]
fn cursor_window_focus_loss_restores_and_releases_capture() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut harness = Harness::new(true);

    harness.secondary_down();
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Hidden)
    );

    harness.window_focus_lost();

    assert!(!harness.is_captured());
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Default)
    );
}

#[test]
fn cursor_owner_destruction_restoration_survives_frame_reset() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut harness = Harness::new(true);

    harness.secondary_down();
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Hidden)
    );

    harness.tree.remove_node(harness.root);
    let removed = harness.tree.drain_pending_removals();
    harness
        .router
        .on_widgets_destroyed(&removed, &mut harness.domain_state);
    harness.router.reset_cursor_request();

    assert!(!harness.is_captured());
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Default)
    );
}

#[test]
fn cursor_invalid_capture_target_is_cancelled_and_restored() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut harness = Harness::new(true);

    harness.secondary_down();
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Hidden)
    );

    harness.tree.remove_node(harness.root);
    let _ = harness.tree.drain_pending_removals();
    harness.router.reset_cursor_request();
    harness.tick();

    assert!(!harness.is_captured());
    assert_eq!(
        harness.router.take_requested_cursor(),
        Some(CursorIcon::Default)
    );
}
