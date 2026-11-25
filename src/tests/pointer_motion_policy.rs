use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use crate::core::event::{
    CaptureMode, Event, EventPhase, FusedInputEvent, GestureKind, InteractionDomainState,
    PointerButton, PointerButtonsState, PointerDevice, PointerEventKind, PointerId, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::relative_pointer::{CursorRestorePolicy, PointerMotionPolicy};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::binding_registry::BindingRegistry;
use crate::utils::id::WidgetInternalId;

const ALL_GESTURE_KINDS: &[GestureKind] = &[
    GestureKind::Drag,
    GestureKind::Resize,
    GestureKind::Scroll,
    GestureKind::Custom,
    GestureKind::Select,
    GestureKind::Pinch,
    GestureKind::Swipe,
    GestureKind::Draw,
];

struct DefaultDeclarationWidget {
    id: WidgetInternalId,
}

impl DefaultDeclarationWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for DefaultDeclarationWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DefaultDeclarationWidget {
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
        self.id
    }

    fn gesture_surface(&self) -> &'static [GestureKind] {
        ALL_GESTURE_KINDS
    }
}

#[derive(Default)]
struct ArbitrationProbeState {
    commits: Vec<GestureKind>,
    cancels: Vec<GestureKind>,
}

struct ArbitrationProbeWidget {
    id: WidgetInternalId,
    gestures: &'static [GestureKind],
    motion_policy: PointerMotionPolicy,
    state: Rc<RefCell<ArbitrationProbeState>>,
}

impl ArbitrationProbeWidget {
    fn new(
        gestures: &'static [GestureKind],
        motion_policy: PointerMotionPolicy,
        state: Rc<RefCell<ArbitrationProbeState>>,
    ) -> Self {
        Self {
            id: WidgetInternalId::new(),
            gestures,
            motion_policy,
            state,
        }
    }
}

impl WidgetDowncast for ArbitrationProbeWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ArbitrationProbeWidget {
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
        self.id
    }

    fn gesture_surface(&self) -> &'static [GestureKind] {
        self.gestures
    }

    fn pointer_motion_policy(&self, _kind: GestureKind) -> PointerMotionPolicy {
        self.motion_policy
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if matches!(phase, EventPhase::Target | EventPhase::Bubble)
            && matches!(event, Event::Pointer(pointer) if pointer.kind == PointerEventKind::Move)
        {
            cx.request_gesture_commit();
        }
        false
    }

    fn on_gesture_commit(&mut self, _cx: &mut EventContext, kind: GestureKind) {
        self.state.borrow_mut().commits.push(kind);
    }

    fn on_gesture_cancel(&mut self, _cx: &mut EventContext, kind: GestureKind) {
        self.state.borrow_mut().cancels.push(kind);
    }
}

#[test]
fn existing_gesture_declarations_default_to_absolute_motion() {
    let widget = DefaultDeclarationWidget::new();

    for &kind in widget.gesture_surface() {
        assert_eq!(
            widget.pointer_motion_policy(kind),
            PointerMotionPolicy::Absolute
        );
    }
}

#[test]
fn relative_policy_without_platform_authority_is_denied_without_absolute_fallback() {
    static SCROLL: &[GestureKind] = &[GestureKind::Scroll];
    static DRAG: &[GestureKind] = &[GestureKind::Drag];

    let scroll_state = Rc::new(RefCell::new(ArbitrationProbeState::default()));
    let drag_state = Rc::new(RefCell::new(ArbitrationProbeState::default()));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ArbitrationProbeWidget::new(
            SCROLL,
            PointerMotionPolicy::Absolute,
            scroll_state.clone(),
        )),
        None,
        0,
    );
    let drag_node = tree.add_child(
        root,
        Box::new(ArbitrationProbeWidget::new(
            DRAG,
            PointerMotionPolicy::RelativeLocked {
                restore: CursorRestorePolicy::DownAnchor,
            },
            drag_state.clone(),
        )),
        None,
        0,
    );
    {
        let _layout = LayoutAuthorityGuard::enter();
        tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));
        tree.set_rect(drag_node, RectF::new(0.0, 0.0, 100.0, 100.0));
        compute_absolute_positions(&mut tree, root, Point::zero());
        compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);
    }

    let drag_widget_id = tree.widget_id_for_node(drag_node).unwrap();
    let mut router = EventRouter::new();
    let mut text = FontManagerTextEngine::new();
    let mut domain = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();

    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(20.0, 20.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        Some(root),
        &mut text,
        None,
        &mut domain,
        &mut bindings,
    );

    let candidates = domain.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 2);
    let drag_candidate = candidates
        .iter()
        .find(|candidate| candidate.widget_id == drag_widget_id)
        .copied()
        .unwrap();
    assert_eq!(
        drag_candidate.motion_policy,
        PointerMotionPolicy::RelativeLocked {
            restore: CursorRestorePolicy::DownAnchor,
        }
    );

    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(30.0, 30.0),
            delta: (10.0, 10.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState::new(),
        },
        Some(root),
        &mut text,
        None,
        &mut domain,
        &mut bindings,
    );

    assert!(!domain.gesture_registry.has_session(PointerId::MOUSE));
    assert!(!domain.pointer_capture_manager.is_captured(PointerId::MOUSE));
    assert_eq!(domain.pointer_capture.captured_widget, None);
    assert!(drag_state.borrow().commits.is_empty());
    assert_eq!(drag_state.borrow().cancels.as_slice(), &[GestureKind::Drag]);
    assert_eq!(
        scroll_state.borrow().cancels.as_slice(),
        &[GestureKind::Scroll]
    );
}
