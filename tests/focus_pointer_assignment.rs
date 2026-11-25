// SPEC-HAL-038 Stage 7: Focus Assignment from Pointer Events
//
// Tests that verify focus is only assigned on Primary (left) mouse button,
// not on Secondary (right), Auxiliary (middle), or other buttons.

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomainState, PointerButton, PointerDevice,
    WindowId,
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
// Test Widget: Focusable
// ============================================================================

struct FocusableTestWidget {
    id: WidgetInternalId,
    label: &'static str,
}

impl FocusableTestWidget {
    fn new(label: &'static str) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label,
        }
    }
}

impl WidgetDowncast for FocusableTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FocusableTestWidget {
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
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        tracing::debug!("FocusableTestWidget '{}' gained focus", self.label);
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        tracing::debug!("FocusableTestWidget '{}' lost focus", self.label);
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }
}

// ============================================================================
// Test Widget: Non-Focusable Container
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

struct SelectableFocusWidget {
    id: WidgetInternalId,
    cleared_count: Rc<RefCell<u32>>,
    has_selection: bool,
}

impl SelectableFocusWidget {
    fn new(cleared_count: Rc<RefCell<u32>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            cleared_count,
            has_selection: true,
        }
    }
}

impl WidgetDowncast for SelectableFocusWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SelectableFocusWidget {
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
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn clear_read_only_text_selection(&mut self) -> bool {
        if !self.has_selection {
            return false;
        }
        self.has_selection = false;
        *self.cleared_count.borrow_mut() += 1;
        true
    }
}

#[test]
fn test_outside_down_default_hook_delegates_to_original_clear_once() {
    let count = Rc::new(RefCell::new(0));
    let mut widget = SelectableFocusWidget::new(count.clone());
    let ordinary: &mut dyn Widget = &mut widget;
    assert!(ordinary.clear_read_only_text_selection_on_outside_down());
    assert_eq!(*count.borrow(), 1);
    assert!(!ordinary.clear_read_only_text_selection_on_outside_down());
    assert_eq!(*count.borrow(), 1);
    assert!(!ordinary.clear_read_only_text_selection());
}

// ============================================================================
// Helper: Setup tree with two focusable widgets
// ============================================================================

fn setup_focus_test() -> (
    WidgetTree,
    EventRouter,
    InteractionDomainState,
    FontManagerTextEngine,
    NodeId,
    NodeId,
    NodeId,
) {
    let mut tree = WidgetTree::new();

    // Root container (non-focusable)
    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    // Widget A (focusable) at (10, 10, 100, 50)
    let widget_a = tree.add_child(root, Box::new(FocusableTestWidget::new("A")), None, 0);
    tree.set_rect(widget_a, RectF::new(10.0, 10.0, 100.0, 50.0));

    // Widget B (focusable) at (150, 10, 100, 50)
    let widget_b = tree.add_child(root, Box::new(FocusableTestWidget::new("B")), None, 0);
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
    )
}

// ============================================================================
// Test 1: Left-click assigns focus
// ============================================================================

#[test]
fn test_left_click_assigns_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, widget_a, _widget_b) =
        setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // Initially no focus
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Left-click on widget A
    let left_click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0), // Inside widget A (10-110, 10-60)
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        left_click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Widget A should now have focus
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Left-click should assign focus to clicked widget"
    );
}

// ============================================================================
// Test 2: Right-click does NOT assign focus
// ============================================================================

#[test]
fn test_right_click_does_not_assign_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, _widget_a, _widget_b) =
        setup_focus_test();

    // Initially no focus
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Right-click on widget A
    let right_click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0), // Inside widget A
        device: PointerDevice::Mouse,
        button: PointerButton::Secondary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        right_click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should still be None
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Right-click should NOT assign focus"
    );
}

// ============================================================================
// Test 3: Middle-click (Auxiliary) does NOT assign focus
// ============================================================================

#[test]
fn test_auxiliary_click_does_not_assign_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, _widget_a, _widget_b) =
        setup_focus_test();

    // Initially no focus
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Middle-click on widget A
    let middle_click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0), // Inside widget A
        device: PointerDevice::Mouse,
        button: PointerButton::Auxiliary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        middle_click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Focus should still be None
    assert_eq!(
        domain_state.focus_manager.focused(),
        None,
        "Middle-click (Auxiliary) should NOT assign focus"
    );
}

// ============================================================================
// Test 4: Right-click elsewhere does NOT remove existing focus
// ============================================================================

#[test]
fn test_right_click_elsewhere_keeps_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, widget_a, _widget_b) =
        setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // First, left-click on widget A to give it focus
    let left_click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0), // Inside widget A
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        left_click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Now right-click on widget B
    let right_click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(200.0, 30.0), // Inside widget B (150-250, 10-60)
        device: PointerDevice::Mouse,
        button: PointerButton::Secondary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        right_click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Widget A should STILL have focus (right-click doesn't transfer or clear)
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Right-click elsewhere should NOT remove existing focus"
    );
}

// ============================================================================
// Test 5: Left-click on non-focusable does NOT clear existing focus
// ============================================================================

#[test]
fn test_left_click_on_non_focusable_keeps_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, widget_a, _widget_b) =
        setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();

    // First, left-click on widget A to give it focus
    let left_click = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0), // Inside widget A
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        left_click,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Now left-click on the non-focusable container (outside both widgets)
    let click_container = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(350.0, 150.0), // Inside container but outside focusable widgets
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_container,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Widget A should still have focus (clicking non-focusable doesn't clear)
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_a),
        "Left-click on non-focusable widget should NOT clear existing focus"
    );
}

#[test]
fn test_left_click_on_non_focusable_clears_focused_read_only_selection() {
    let _guard = LayoutAuthorityGuard::enter();
    let cleared_count = Rc::new(RefCell::new(0_u32));
    let mut tree = WidgetTree::new();

    let root = tree.create_root(Box::new(ContainerWidget::new()), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 200.0));

    let selectable = tree.add_child(
        root,
        Box::new(SelectableFocusWidget::new(cleared_count.clone())),
        None,
        0,
    );
    tree.set_rect(selectable, RectF::new(10.0, 10.0, 100.0, 50.0));
    let selectable_id = tree.node(selectable).unwrap().id();

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(50.0, 30.0),
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
    assert_eq!(domain_state.focus_manager.focused(), Some(selectable_id));
    assert_eq!(*cleared_count.borrow(), 0);

    router.route_input_event(
        &mut tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(350.0, 150.0),
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

    assert_eq!(domain_state.focus_manager.focused(), Some(selectable_id));
    assert_eq!(*cleared_count.borrow(), 1);
}

// ============================================================================
// Test 6: Left-click transfers focus between focusable widgets
// ============================================================================

#[test]
fn test_left_click_transfers_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let (mut tree, mut router, mut domain_state, mut text_engine, root, widget_a, widget_b) =
        setup_focus_test();
    let id_a = tree.node(widget_a).unwrap().id();
    let id_b = tree.node(widget_b).unwrap().id();

    // Left-click on widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 30.0), // Inside widget A
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

    // Left-click on widget B
    let click_b = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(200.0, 30.0), // Inside widget B
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

    // Focus should transfer to widget B
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(id_b),
        "Left-click should transfer focus to new focusable widget"
    );
}
