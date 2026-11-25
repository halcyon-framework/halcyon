// SPEC-HAL-009 Step 8: Minimal Tests
// SPEC-HAL-087: Updated for type-safe widget identity model

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::*;
use halcyon::core::event::{
    FusedInputEvent, InteractionDomainState, PointerButton, PointerDevice, PointerEventKind,
    WindowId,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::rect_authority::LayoutAuthorityGuard;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::WidgetTree;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::utils::id::WidgetInternalId;
use std::time::Instant;

// ============================================================================
// Test Widget
// ============================================================================

struct TestWidget {
    // SPEC-HAL-087: Widget-internal ID (not the tree-managed TreeWidgetId)
    id: WidgetInternalId,
    events_received: std::cell::RefCell<Vec<(EventPhase, String)>>,
    capture_on_down: bool,
    request_focus_on_down: bool,
}

impl TestWidget {
    fn new(capture_on_down: bool, request_focus_on_down: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            events_received: std::cell::RefCell::new(Vec::new()),
            capture_on_down,
            request_focus_on_down,
        }
    }

    #[allow(dead_code)]
    fn get_events(&self) -> Vec<(EventPhase, String)> {
        self.events_received.borrow().clone()
    }

    #[allow(dead_code)]
    fn clear_events(&self) {
        self.events_received.borrow_mut().clear()
    }
}

impl WidgetDowncast for TestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TestWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[halcyon::core::widget_tree::NodeId],
    ) -> LayoutResult {
        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(
        &self,
        _cx: &mut PaintContext,
        _rect: RectF,
        _children: &[halcyon::core::widget_tree::NodeId],
    ) {
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let event_desc = match event {
            Event::Pointer(pe) => format!("Pointer({:?})", pe.kind),
            Event::Key(ke) => format!("Key({:?})", ke.key_code),
            Event::Focus(fe) => format!("Focus({:?})", fe),
            Event::Window(we) => format!("Window({:?})", we),
            Event::Text(te) => format!("Text({:?})", te.text),
            Event::Ime(ie) => format!("Ime({:?})", ie),
            Event::RelativePointerMotion(motion) => {
                format!("RelativePointerMotion({:?})", motion.delta)
            }
        };

        self.events_received
            .borrow_mut()
            .push((phase, event_desc.clone()));

        // Handle capture/focus requests
        if let Event::Pointer(pe) = event {
            if matches!(pe.kind, PointerEventKind::Down) {
                if self.capture_on_down {
                    if let Some(button) = pe.button {
                        cx.capture_pointer(button);
                    }
                }
                if self.request_focus_on_down {
                    cx.request_focus();
                }
            }
            if matches!(pe.kind, PointerEventKind::Up) && self.capture_on_down {
                cx.release_pointer();
            }
        }

        phase == EventPhase::Target
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

// ============================================================================
// Test 1: Hit-Testing with Nested Containers
// ============================================================================

#[test]
fn test_hit_testing_nested() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create root widget at (0, 0, 100, 100)
    let root_widget = Box::new(TestWidget::new(false, false));
    let root = tree.create_root(root_widget, None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));
    // SPEC-HAL-087: Get TreeWidgetId from tree (not from widget)
    let root_tree_id = tree.node(root).unwrap().id();

    // Create child widget at (10, 10, 50, 50)
    let child_widget = Box::new(TestWidget::new(false, false));
    let child = tree.add_child(root, child_widget, None, 1);
    tree.set_rect(child, RectF::new(10.0, 10.0, 50.0, 50.0));
    let child_tree_id = tree.node(child).unwrap().id();

    // SPEC-HAL-024 Stage 3B: Compute absolute_paint_rect for hit-testing
    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    // SPEC-HAL-088 Stage 3: Compute viewports for hit-testing
    tree.compute_viewports(root);

    // Hit test inside child
    let result = tree.hit_test(30.0, 30.0);
    assert!(result.is_some());
    let hit = result.unwrap();
    assert_eq!(hit.widget_id, child_tree_id); // Should hit child
    assert_eq!(hit.local_pos, (20.0, 20.0)); // Local to child

    // Hit test inside root but outside child
    let result = tree.hit_test(80.0, 80.0);
    assert!(result.is_some());
    let hit = result.unwrap();
    assert_eq!(hit.widget_id, root_tree_id); // Should hit root

    // Hit test outside root
    let result = tree.hit_test(150.0, 150.0);
    assert!(result.is_none());
}

// ============================================================================
// Test 2: Capture Overriding Hit-Testing
// ============================================================================

#[test]
fn test_pointer_capture() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut binding_registry = halcyon::runtime::binding_registry::BindingRegistry::new();

    // Create two widgets
    let widget1 = Box::new(TestWidget::new(true, false)); // Captures on down
    let root = tree.create_root(widget1, None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 50.0, 50.0));
    // SPEC-HAL-087: Get TreeWidgetId from tree
    let root_tree_id = tree.node(root).unwrap().id();

    let widget2 = Box::new(TestWidget::new(false, false));
    let child = tree.add_child(root, widget2, None, 1);
    tree.set_rect(child, RectF::new(60.0, 0.0, 50.0, 50.0));

    // SPEC-HAL-024 Stage 3B: Compute absolute_paint_rect for hit-testing
    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    // SPEC-HAL-088 Stage 3: Compute viewports for hit-testing
    tree.compute_viewports(root);

    // Click on widget1 (should capture)
    let down_event = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(25.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        down_event,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut binding_registry,
    );

    // Verify widget1 is captured
    assert!(domain_state.pointer_capture.is_captured());
    assert_eq!(
        domain_state.pointer_capture.captured_widget,
        Some(root_tree_id)
    );

    // Now move pointer over widget2 - should still go to widget1 due to capture
    let move_event = FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(75.0, 25.0), // Over widget2
        delta: (50.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState {
            primary: true,
            ..PointerButtonsState::default()
        },
    };
    router.route_input_event(
        &mut tree,
        move_event,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut binding_registry,
    );

    // Widget1 should receive the move event due to capture
    // (We'd need to inspect widget1's events_received to verify)

    // Release
    let up_event = FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(75.0, 25.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    };
    router.route_input_event(
        &mut tree,
        up_event,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut binding_registry,
    );

    // Capture should be released
    assert!(!domain_state.pointer_capture.is_captured());
}

// ============================================================================
// Test 3: Focus Switching
// ============================================================================

#[test]
fn test_focus_switching() {
    let mut tree = WidgetTree::new();
    let mut domain_state = InteractionDomainState::new();

    // Create widgets to get TreeWidgetIds
    let widget1 = Box::new(TestWidget::new(false, false));
    let root = tree.create_root(widget1, None, 0);
    let widget1_tree_id = tree.node(root).unwrap().id();

    let widget2 = Box::new(TestWidget::new(false, false));
    let child = tree.add_child(root, widget2, None, 1);
    let widget2_tree_id = tree.node(child).unwrap().id();

    // No widget focused initially
    assert_eq!(domain_state.focus_manager.focused(), None);

    // Set focus to widget 1
    domain_state.focus_manager.set_focus(Some(widget1_tree_id));
    assert_eq!(domain_state.focus_manager.focused(), Some(widget1_tree_id));

    // Switch focus to widget 2
    domain_state.focus_manager.set_focus(Some(widget2_tree_id));
    assert_eq!(domain_state.focus_manager.focused(), Some(widget2_tree_id));

    // Clear focus
    domain_state.focus_manager.set_focus(None);
    assert_eq!(domain_state.focus_manager.focused(), None);
}

// ============================================================================
// Test 4: DoubleClick Classification
// ============================================================================

#[test]
fn test_double_click_detection() {
    let mut tracker = ClickTracker::new();

    // First click
    let is_double = tracker.check_double_click(PointerButton::Primary, (100.0, 100.0));
    assert!(!is_double); // First click is not a double-click

    // Second click at same position within threshold
    let is_double = tracker.check_double_click(PointerButton::Primary, (101.0, 101.0));
    assert!(is_double); // Should be detected as double-click

    // Third click - should reset
    let is_double = tracker.check_double_click(PointerButton::Primary, (101.0, 101.0));
    assert!(!is_double); // After double-click, tracker resets

    // Click with different button
    tracker.check_double_click(PointerButton::Primary, (100.0, 100.0));
    let is_double = tracker.check_double_click(PointerButton::Secondary, (100.0, 100.0));
    assert!(!is_double); // Different button doesn't trigger double-click

    // Click too far away
    tracker.check_double_click(PointerButton::Primary, (100.0, 100.0));
    let is_double = tracker.check_double_click(PointerButton::Primary, (200.0, 200.0));
    assert!(!is_double); // Too far away

    // Click after long delay would also not be double-click (but hard to test without sleep)
}

// ============================================================================
// Test 5: Event Phase Propagation
// ============================================================================

#[test]
fn test_event_phases() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Create parent and child
    let parent = Box::new(TestWidget::new(false, false));
    let root = tree.create_root(parent, None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    let child = Box::new(TestWidget::new(false, false));
    let child_node = tree.add_child(root, child, None, 1);
    tree.set_rect(child_node, RectF::new(10.0, 10.0, 50.0, 50.0));

    // SPEC-HAL-024 Stage 3B: Compute absolute_paint_rect for hit-testing
    compute_absolute_positions(&mut tree, root, Point::zero());
    compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    // Send event to child using FusedInputEvent
    let event = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(30.0, 30.0), // Inside child
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    router.route_input_event(
        &mut tree,
        event,
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Both widgets should have received events
    // Parent: Capture phase, then Bubble phase (if not consumed)
    // Child: Target phase
    // (We'd need access to test_widget to verify the exact phases)
}
