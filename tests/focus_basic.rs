// SPEC-HAL-038 Stage 1: Basic Focus System Tests

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    FusedInputEvent, InteractionDomainState, KeyCode, Modifiers, PointerButton, PointerDevice,
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
// Focusable Test Widget
// ============================================================================

/// A test widget that tracks focus lifecycle events
struct FocusableWidget {
    id: WidgetInternalId,
    /// Shared storage for recording events across all widgets
    events: Rc<RefCell<Vec<FocusTestEvent>>>,
    /// Label to identify this widget in event log
    label: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
enum FocusTestEvent {
    OnFocus(&'static str),
    OnBlur(&'static str),
    KeyReceived(&'static str, KeyCode),
}

impl FocusableWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<FocusTestEvent>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            events,
            label,
        }
    }
}

impl WidgetDowncast for FocusableWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FocusableWidget {
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

    /// SPEC-HAL-038: This widget is focusable
    fn focusable(&self) -> bool {
        true
    }

    /// SPEC-HAL-038: Called when widget gains focus
    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(FocusTestEvent::OnFocus(self.label));
    }

    /// SPEC-HAL-038: Called when widget loses focus
    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.events
            .borrow_mut()
            .push(FocusTestEvent::OnBlur(self.label));
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        event: &halcyon::core::event::Event,
        phase: halcyon::core::event::EventPhase,
    ) -> bool {
        use halcyon::core::event::{Event, EventPhase};

        // Only record key events at target phase
        if phase == EventPhase::Target {
            if let Event::Key(key_event) = event {
                if key_event.is_down {
                    self.events
                        .borrow_mut()
                        .push(FocusTestEvent::KeyReceived(self.label, key_event.key_code));
                    return true;
                }
            }
        }
        false
    }
}

// ============================================================================
// Non-Focusable Test Widget (for comparison)
// ============================================================================

struct NonFocusableWidget {
    id: WidgetInternalId,
    events: Rc<RefCell<Vec<FocusTestEvent>>>,
    label: &'static str,
}

impl NonFocusableWidget {
    fn new(label: &'static str, events: Rc<RefCell<Vec<FocusTestEvent>>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            events,
            label,
        }
    }
}

impl WidgetDowncast for NonFocusableWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for NonFocusableWidget {
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

    /// Non-focusable widget
    fn focusable(&self) -> bool {
        false
    }

    fn handle_event(
        &mut self,
        _cx: &mut EventContext,
        event: &halcyon::core::event::Event,
        phase: halcyon::core::event::EventPhase,
    ) -> bool {
        use halcyon::core::event::{Event, EventPhase};

        if phase == EventPhase::Target {
            if let Event::Key(key_event) = event {
                if key_event.is_down {
                    self.events
                        .borrow_mut()
                        .push(FocusTestEvent::KeyReceived(self.label, key_event.key_code));
                    return true;
                }
            }
        }
        false
    }
}

// ============================================================================
// Helper: Setup tree with two focusable widgets side by side
// ============================================================================

fn setup_two_focusable_widgets(
    events: Rc<RefCell<Vec<FocusTestEvent>>>,
) -> (WidgetTree, NodeId, TreeWidgetId, TreeWidgetId) {
    let mut tree = WidgetTree::new();

    // Root container
    let root = tree.create_root(
        Box::new(NonFocusableWidget::new("root", events.clone())),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 300.0, 100.0));

    // Widget A at (0, 0, 100, 50)
    let widget_a = tree.add_child(
        root,
        Box::new(FocusableWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    // Widget B at (150, 0, 100, 50)
    let widget_b = tree.add_child(
        root,
        Box::new(FocusableWidget::new("B", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_b, RectF::new(150.0, 0.0, 100.0, 50.0));
    let id_b = tree.node(widget_b).unwrap().id();

    // Compute absolute positions for hit-testing
    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    (tree, root, id_a, id_b)
}

// ============================================================================
// Test 1: Click → Focus
// ============================================================================

#[test]
fn test_click_focus_switch() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, id_b) = setup_two_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Initially no focus
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Click on widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0), // Center of A
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

    // A should now be focused
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));
    assert!(domain_state.focus_manager.is_focused(id_a));
    assert!(!domain_state.focus_manager.is_focused(id_b));

    // Check that on_focus was called for A
    {
        let evts = events.borrow();
        assert!(
            evts.contains(&FocusTestEvent::OnFocus("A")),
            "Expected on_focus for A, got: {:?}",
            *evts
        );
    }

    // Clear events for next test
    events.borrow_mut().clear();

    // Click on widget B
    let click_b = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(200.0, 25.0), // Center of B
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

    // B should now be focused
    assert_eq!(domain_state.focus_manager.focused(), Some(id_b));
    assert!(!domain_state.focus_manager.is_focused(id_a));
    assert!(domain_state.focus_manager.is_focused(id_b));

    // Check that on_blur was called for A and on_focus for B
    {
        let evts = events.borrow();
        assert!(
            evts.contains(&FocusTestEvent::OnBlur("A")),
            "Expected on_blur for A, got: {:?}",
            *evts
        );
        assert!(
            evts.contains(&FocusTestEvent::OnFocus("B")),
            "Expected on_focus for B, got: {:?}",
            *evts
        );
    }
}

// ============================================================================
// Test 2: Keyboard Event Routing to Focused Widget
// ============================================================================

#[test]
fn test_keyboard_routing_to_focused() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, _id_b) = setup_two_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus widget A via click
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
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

    // Clear events from focus change
    events.borrow_mut().clear();

    // Send key event
    let key_event = FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Character('X'),
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    };
    router.route_input_event(
        &mut tree,
        key_event,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Only widget A should receive the key event
    {
        let evts = events.borrow();
        assert!(
            evts.contains(&FocusTestEvent::KeyReceived("A", KeyCode::Character('X'))),
            "Expected A to receive key X, got: {:?}",
            *evts
        );
        // B should NOT have received the key
        assert!(
            !evts.contains(&FocusTestEvent::KeyReceived("B", KeyCode::Character('X'))),
            "B should not receive key events when A is focused"
        );
    }
}

#[test]
fn test_keyboard_not_routed_without_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, _id_a, _id_b) = setup_two_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // No focus initially
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Send key event
    let key_event = FusedInputEvent::KeyDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        key_code: KeyCode::Character('Y'),
        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: Modifiers::default(),
        repeat: false,
    };
    router.route_input_event(
        &mut tree,
        key_event,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Neither widget should receive the key event
    {
        let evts = events.borrow();
        assert!(
            evts.is_empty(),
            "No widget should receive key events when nothing is focused, got: {:?}",
            *evts
        );
    }
}

// ============================================================================
// Test 3: Focus Clear
// ============================================================================

#[test]
fn test_focus_clear() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, _id_b) = setup_two_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
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

    // Clear events from focus
    events.borrow_mut().clear();

    // Clear focus
    router.clear_focus(&mut tree, &mut domain_state);

    // No widget should be focused
    assert_eq!(domain_state.focus_manager.focused(), None);

    // on_blur should have been called for A
    {
        let evts = events.borrow();
        assert!(
            evts.contains(&FocusTestEvent::OnBlur("A")),
            "Expected on_blur for A after clear_focus, got: {:?}",
            *evts
        );
    }
}

// ============================================================================
// Test 4: Click on non-focusable widget doesn't change focus
// ============================================================================

#[test]
fn test_click_nonfocusable_keeps_focus() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    // Root container (non-focusable)
    let root = tree.create_root(
        Box::new(NonFocusableWidget::new("root", events.clone())),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 300.0, 100.0));

    // Widget A (focusable) at (0, 0)
    let widget_a = tree.add_child(
        root,
        Box::new(FocusableWidget::new("A", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_a, RectF::new(0.0, 0.0, 100.0, 50.0));
    let id_a = tree.node(widget_a).unwrap().id();

    // Widget C (non-focusable) at (150, 0)
    let widget_c = tree.add_child(
        root,
        Box::new(NonFocusableWidget::new("C", events.clone())),
        None,
        0,
    );
    tree.set_rect(widget_c, RectF::new(150.0, 0.0, 100.0, 50.0));

    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
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

    events.borrow_mut().clear();

    // Click on non-focusable widget C
    // Per SPEC-HAL-038 Stage 1: "Do NOT clear focus when clicking empty space yet."
    // Clicking a non-focusable widget should NOT change focus.
    let click_c = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(200.0, 25.0), // Center of C
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_c,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // A should still be focused (clicking non-focusable doesn't clear focus)
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // No blur event should have been triggered
    {
        let evts = events.borrow();
        assert!(
            !evts.contains(&FocusTestEvent::OnBlur("A")),
            "A should not lose focus when clicking non-focusable widget"
        );
    }
}

// ============================================================================
// Test 5: Clicking already-focused widget doesn't re-trigger on_focus
// ============================================================================

#[test]
fn test_click_already_focused_no_retrigger() {
    let _guard = LayoutAuthorityGuard::enter();
    let events = Rc::new(RefCell::new(Vec::new()));
    let (mut tree, root, id_a, _id_b) = setup_two_focusable_widgets(events.clone());
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Focus widget A
    let click_a = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        click_a.clone(),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // Clear events
    events.borrow_mut().clear();

    // Click A again
    router.route_input_event(
        &mut tree,
        click_a,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Still focused on A
    assert_eq!(domain_state.focus_manager.focused(), Some(id_a));

    // No focus events should have been triggered (already focused)
    {
        let evts = events.borrow();
        assert!(
            !evts.contains(&FocusTestEvent::OnFocus("A")),
            "on_focus should not be called again when already focused"
        );
        assert!(
            !evts.contains(&FocusTestEvent::OnBlur("A")),
            "on_blur should not be called when clicking same widget"
        );
    }
}
