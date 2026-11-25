// SPEC-HAL-024 Stage 6: Scroll-Aware Input Projection Test
//
// Verifies that pointer events correctly target widgets inside scrolled content.
// Tests use a simple ScrollContainer widget that provides scroll_offset().
//
// INV-INPUT-PROJ-1: Input projection uses the same scroll offset as paint.
// INV-INPUT-PROJ-2: No widget outside viewport is unclickable if visible.
// INV-INPUT-PROJ-3: No double-application of scroll offset.

use halcyon::core::app::{compute_absolute_paint_rects, compute_absolute_positions};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomainState, PointerButton, PointerButtonsState,
    PointerDevice, PointerEventKind, WindowId,
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
use std::time::Instant;

// ============================================================================
// Test Widget: A simple scroll container that provides scroll_offset
// ============================================================================

struct ScrollContainer {
    id: WidgetInternalId,
    size: Size,
    scroll_y: f32,
}

impl ScrollContainer {
    fn new(width: f32, height: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            size: Size::new(width, height),
            scroll_y: 0.0,
        }
    }

    fn set_scroll_y(&mut self, y: f32) {
        self.scroll_y = y;
    }
}

impl WidgetDowncast for ScrollContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ScrollContainer {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(self.size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn scroll_offset(&self) -> Option<(f32, f32)> {
        // Return scroll offset for input projection
        Some((0.0, self.scroll_y))
    }

    fn hit_testable(&self) -> bool {
        true
    }
}

// ============================================================================
// Test Widget: Records local_pos from pointer events
// ============================================================================

struct TrackingWidget {
    id: WidgetInternalId,
    size: Size,
    received_local_pos: RefCell<Option<(f32, f32)>>,
    received_events: RefCell<Vec<PointerEventKind>>,
}

impl TrackingWidget {
    fn new(width: f32, height: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            size: Size::new(width, height),
            received_local_pos: RefCell::new(None),
            received_events: RefCell::new(Vec::new()),
        }
    }

    fn last_local_pos(&self) -> Option<(f32, f32)> {
        *self.received_local_pos.borrow()
    }

    fn event_count(&self) -> usize {
        self.received_events.borrow().len()
    }

    #[allow(dead_code)]
    fn clear(&self) {
        *self.received_local_pos.borrow_mut() = None;
        self.received_events.borrow_mut().clear();
    }
}

impl WidgetDowncast for TrackingWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TrackingWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(self.size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn handle_event(&mut self, _cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase == EventPhase::Target {
            if let Event::Pointer(pe) = event {
                *self.received_local_pos.borrow_mut() = Some(pe.local_pos);
                self.received_events.borrow_mut().push(pe.kind.clone());
            }
        }
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }
}

// ============================================================================
// Test 1: Click on widget inside scroll container with scroll offset
// ============================================================================

/// Test: Click on widget inside scroll container with scroll offset.
///
/// Setup:
/// - ScrollContainer at (0,0) with 200x100 viewport
/// - Child TrackingWidget at (0,0) in content space, 200x500 tall
/// - Scroll down by 250
/// - Click at screen position (100, 50)
///
/// Expected:
/// - Widget receives event (hit-test works)
/// - local_pos.y should be 50 + 250 = 300 (scroll-projected to content coords)
#[test]
fn test_scrolled_widget_receives_correct_local_pos() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Create scroll container as root (200x100 viewport)
    let scroll_container = Box::new(ScrollContainer::new(200.0, 100.0));
    let scroll_node = tree.create_root(scroll_container, None, 0);

    // Create tracking widget as child (200x500 content, larger than viewport)
    let tracking_widget = Box::new(TrackingWidget::new(200.0, 500.0));
    let tracking_node = tree.add_child(scroll_node, tracking_widget, None, 0);

    // Set positions
    tree.set_rect(scroll_node, RectF::new(0.0, 0.0, 200.0, 100.0));
    tree.set_rect(tracking_node, RectF::new(0.0, 0.0, 200.0, 500.0));

    // Compute absolute positions for hit testing
    compute_absolute_positions(&mut tree, scroll_node, Point::zero());
    compute_absolute_paint_rects(&mut tree, scroll_node, Point::zero());
    tree.compute_viewports(scroll_node);

    // Set scroll offset to 250
    {
        let node = tree.node_mut(scroll_node).unwrap();
        let scroll = node
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollContainer>()
            .unwrap();
        scroll.set_scroll_y(250.0);
    }

    // Send click at screen position (100, 50)
    // Content y = screen_y + scroll_offset = 50 + 250 = 300
    let event = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(100.0, 50.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };

    router.route_input_event(
        &mut tree,
        event,
        Some(scroll_node),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Verify the tracking widget received the event
    let tracking_data = tree.node(tracking_node).unwrap();
    let tracking = tracking_data
        .widget()
        .as_any()
        .downcast_ref::<TrackingWidget>()
        .unwrap();

    assert!(
        tracking.event_count() > 0,
        "Widget should have received pointer event"
    );

    let local = tracking.last_local_pos().expect("Should have local_pos");

    // INV-INPUT-PROJ-1: The local_pos should account for scroll offset.
    // Screen click at y=50, scroll offset is 250, so content y = 50 + 250 = 300
    // Tracking widget origin is at y=0 in content space
    // So local_y should be 300
    assert!(
        (local.1 - 300.0).abs() < 1.0,
        "local_pos.y should be ~300 (scroll-projected), got {}",
        local.1
    );
    assert!(
        (local.0 - 100.0).abs() < 1.0,
        "local_pos.x should be ~100, got {}",
        local.0
    );
}

// ============================================================================
// Test 2: Hover works on scrolled widget
// ============================================================================

/// Test: Hover works on scrolled widget.
///
/// Same setup as click test, but verifies Move events work correctly.
#[test]
fn test_scrolled_widget_hover() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Create scroll container
    let scroll_container = Box::new(ScrollContainer::new(200.0, 100.0));
    let scroll_node = tree.create_root(scroll_container, None, 0);

    // Create tracking widget as child
    let tracking_widget = Box::new(TrackingWidget::new(200.0, 500.0));
    let tracking_node = tree.add_child(scroll_node, tracking_widget, None, 0);

    // Set positions
    tree.set_rect(scroll_node, RectF::new(0.0, 0.0, 200.0, 100.0));
    tree.set_rect(tracking_node, RectF::new(0.0, 0.0, 200.0, 500.0));

    compute_absolute_positions(&mut tree, scroll_node, Point::zero());
    compute_absolute_paint_rects(&mut tree, scroll_node, Point::zero());
    tree.compute_viewports(scroll_node);

    // Set scroll offset to 200
    {
        let node = tree.node_mut(scroll_node).unwrap();
        let scroll = node
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollContainer>()
            .unwrap();
        scroll.set_scroll_y(200.0);
    }

    // Send hover at screen y=80
    let event = FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(50.0, 80.0),
        delta: (0.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState::new(),
    };

    router.route_input_event(
        &mut tree,
        event,
        Some(scroll_node),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    // Verify event received with correct local_pos
    let tracking_data = tree.node(tracking_node).unwrap();
    let tracking = tracking_data
        .widget()
        .as_any()
        .downcast_ref::<TrackingWidget>()
        .unwrap();

    assert!(
        tracking.event_count() > 0,
        "Widget should have received hover event"
    );

    let local = tracking.last_local_pos().expect("Should have local_pos");

    // Screen y=80, scroll=200, content y = 280
    assert!(
        (local.1 - 280.0).abs() < 1.0,
        "local_pos.y should be ~280, got {}",
        local.1
    );
}

// ============================================================================
// Test 3: Zero scroll offset works correctly (no regression)
// ============================================================================

/// Test: Zero scroll offset works correctly (no regression).
#[test]
fn test_unscrolled_widget() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    // Create scroll container (no scroll)
    let scroll_container = Box::new(ScrollContainer::new(200.0, 100.0));
    let scroll_node = tree.create_root(scroll_container, None, 0);

    // Create tracking widget as child
    let tracking_widget = Box::new(TrackingWidget::new(200.0, 500.0));
    let tracking_node = tree.add_child(scroll_node, tracking_widget, None, 0);

    // Set positions
    tree.set_rect(scroll_node, RectF::new(0.0, 0.0, 200.0, 100.0));
    tree.set_rect(tracking_node, RectF::new(0.0, 0.0, 200.0, 500.0));

    compute_absolute_positions(&mut tree, scroll_node, Point::zero());
    compute_absolute_paint_rects(&mut tree, scroll_node, Point::zero());
    tree.compute_viewports(scroll_node);

    // No scroll - click at screen y=30
    let event = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(100.0, 30.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };

    router.route_input_event(
        &mut tree,
        event,
        Some(scroll_node),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    let tracking_data = tree.node(tracking_node).unwrap();
    let tracking = tracking_data
        .widget()
        .as_any()
        .downcast_ref::<TrackingWidget>()
        .unwrap();

    assert!(
        tracking.event_count() > 0,
        "Widget should have received event"
    );

    let local = tracking.last_local_pos().expect("Should have local_pos");

    // No scroll, so local_y = screen_y = 30
    assert!(
        (local.1 - 30.0).abs() < 1.0,
        "local_pos.y should be ~30, got {}",
        local.1
    );
}

// ============================================================================
// Test 4: Small scroll offset (regression test for tiny scrolls)
// ============================================================================

/// Test: Small scroll offset (5-20px) doesn't break interactivity.
#[test]
fn test_small_scroll_offset() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();

    let scroll_container = Box::new(ScrollContainer::new(200.0, 100.0));
    let scroll_node = tree.create_root(scroll_container, None, 0);

    let tracking_widget = Box::new(TrackingWidget::new(200.0, 500.0));
    let tracking_node = tree.add_child(scroll_node, tracking_widget, None, 0);

    tree.set_rect(scroll_node, RectF::new(0.0, 0.0, 200.0, 100.0));
    tree.set_rect(tracking_node, RectF::new(0.0, 0.0, 200.0, 500.0));

    compute_absolute_positions(&mut tree, scroll_node, Point::zero());
    compute_absolute_paint_rects(&mut tree, scroll_node, Point::zero());
    tree.compute_viewports(scroll_node);

    // Small scroll: 15px
    {
        let node = tree.node_mut(scroll_node).unwrap();
        let scroll = node
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollContainer>()
            .unwrap();
        scroll.set_scroll_y(15.0);
    }

    // Click at screen y=50
    let event = FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(100.0, 50.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };

    router.route_input_event(
        &mut tree,
        event,
        Some(scroll_node),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );

    let tracking_data = tree.node(tracking_node).unwrap();
    let tracking = tracking_data
        .widget()
        .as_any()
        .downcast_ref::<TrackingWidget>()
        .unwrap();

    assert!(
        tracking.event_count() > 0,
        "Widget should have received event after small scroll"
    );

    let local = tracking.last_local_pos().expect("Should have local_pos");

    // screen y=50, scroll=15, content y = 65
    assert!(
        (local.1 - 65.0).abs() < 1.0,
        "local_pos.y should be ~65, got {}",
        local.1
    );
}
