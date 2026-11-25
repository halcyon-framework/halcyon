use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use halcyon::core::animation::{Animation, Easing};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomainState, PointerEventKind, ScrollSource,
    WindowId,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::transition::TransitionLayer;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::runtime::binding_registry::BindingRegistry;
use halcyon::utils::id::WidgetInternalId;

struct ScrollAnimationProbe {
    id: WidgetInternalId,
    animation_started: Rc<Cell<bool>>,
}

impl ScrollAnimationProbe {
    fn new(animation_started: Rc<Cell<bool>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            animation_started,
        }
    }
}

impl WidgetDowncast for ScrollAnimationProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ScrollAnimationProbe {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 100.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase == EventPhase::Target
            && matches!(
                event,
                Event::Pointer(pointer)
                    if matches!(pointer.kind, PointerEventKind::Scroll { .. })
            )
        {
            let animation = Animation::new(0.0_f32, 1.0, 0.2, Easing::Linear);
            self.animation_started
                .set(cx.start_animation(animation).is_some());
            return true;
        }

        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn wants_scroll(&self) -> bool {
        true
    }
}

#[test]
fn scroll_target_receives_animation_capability() {
    let animation_started = Rc::new(Cell::new(false));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ScrollAnimationProbe::new(animation_started.clone())),
        None,
        0,
    );

    {
        let _layout = LayoutAuthorityGuard::enter();
        tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));
        halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);
    }

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut binding_registry = BindingRegistry::new();
    let mut transition_layer = TransitionLayer::new();

    router.route_input_event(
        &mut tree,
        FusedInputEvent::Scroll {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(50.0, 50.0),
            delta: (0.0, 56.0),
            source: ScrollSource::Wheel,
        },
        Some(root),
        &mut text_engine,
        Some(transition_layer.animation_context()),
        &mut domain_state,
        &mut binding_registry,
    );

    assert!(
        animation_started.get(),
        "scroll target must receive the caller-provided animation capability"
    );
    assert!(transition_layer.has_active_animations());
}
