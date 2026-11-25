// SPEC-HAL-038: Focus Entry Scroll Neutrality Regression Test
//
// Engine-internal test verifying INV-FOCUS-ENTRY-SCROLL-NEUTRALITY:
// "Focus acquisition from None (first focus) MUST NOT trigger scroll reveal."
//
// This test MUST live in src/tests/ because it accesses internal EventRouter state.
//
// Bug scenario (prior to fix):
// 1. User scrolls List down to see item at position Y
// 2. User clicks on visible item (focus is None -> focus becomes Some(item))
// 3. BUG: post_layout_reveal_focused_widget scrolled the item back to top
//
// Fix: request_focus_for_widget only sets pending_focus_reveal when old_focus.is_some()

use crate::core::event::InteractionDomainState;
use crate::core::event_router::EventRouter;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::prelude::ScrollArea;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};

/// Focusable test widget for verifying focus behavior.
struct FocusableWidget {
    internal_id: WidgetInternalId,
    focusable: bool,
}

impl FocusableWidget {
    fn new(focusable: bool) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            focusable,
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
        self.internal_id
    }

    fn focusable(&self) -> bool {
        self.focusable
    }
}

/// INV-FOCUS-ENTRY-SCROLL-NEUTRALITY: First focus must not trigger scroll reveal.
///
/// When focus transitions from None to Some(widget), pending_focus_reveal must NOT be set.
/// This prevents pointer-originated focus entry from causing unwanted scroll jumps.
#[test]
fn test_first_focus_does_not_set_pending_reveal() {
    let _guard = LayoutAuthorityGuard::enter();
    // 1. Create EventRouter, InteractionDomainState, and WidgetTree
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut tree = WidgetTree::new();

    // 2. Create ScrollArea with focusable child (simulates List with ListItem)
    let scroll_area = ScrollArea::new(NodeId(1)).enable_vertical_scrollbar(true);
    let root_id = tree.create_root(Box::new(scroll_area), None, 0);
    tree.set_rect(root_id, RectF::new(0.0, 0.0, 400.0, 300.0));

    // Add focusable child widget
    let child_widget = FocusableWidget::new(true);
    let child_id = tree.add_child(root_id, Box::new(child_widget), None, 0);
    tree.set_rect(child_id, RectF::new(0.0, 0.0, 400.0, 50.0));

    // Compute absolute positions for proper tree traversal
    crate::core::app::compute_absolute_positions(&mut tree, root_id, Point::zero());
    crate::core::app::compute_absolute_paint_rects(&mut tree, root_id, Point::zero());

    // 3. Programmatically scroll down (simulate user has scrolled)
    if let Some(node) = tree.node_mut(root_id) {
        if let Some(container) = node.widget.scroll_container_mut() {
            container.scroll_by(0.0, 200.0);
        }
    }

    // 4. Verify focus is None (precondition)
    assert!(
        domain_state.focus_manager.focused().is_none(),
        "Precondition: focus must be None before first focus"
    );

    // 5. Get the TreeWidgetId of the child widget
    let child_widget_id = tree.node(child_id).unwrap().widget.id();
    let child_tree_widget_id = TreeWidgetId(child_widget_id.0);

    // 6. Simulate focus request (as if pointer clicked the widget)
    router.request_focus_for_widget(&mut tree, child_tree_widget_id, &mut domain_state);

    // 7. CRITICAL ASSERTION: pending_focus_reveal must be None
    // This is the invariant that prevents scroll jumps on first focus
    let pending = router.take_pending_focus_reveal();
    assert!(
        pending.is_none(),
        "INV-FOCUS-ENTRY-SCROLL-NEUTRALITY violated: pending_focus_reveal was {:?} \
         but must be None on first focus (old_focus was None). \
         This would cause scroll jump when user clicks visible item.",
        pending
    );

    // 8. Verify focus was actually assigned
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(child_tree_widget_id),
        "Focus must be assigned to the widget"
    );
}

/// Verify that focus CHANGE (old_focus.is_some()) DOES set pending_reveal.
/// This is the expected behavior for keyboard navigation between items.
#[test]
fn test_focus_change_does_set_pending_reveal() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut tree = WidgetTree::new();

    // Create ScrollArea with two focusable children
    let scroll_area = ScrollArea::new(NodeId(1)).enable_vertical_scrollbar(true);
    let root_id = tree.create_root(Box::new(scroll_area), None, 0);
    tree.set_rect(root_id, RectF::new(0.0, 0.0, 400.0, 300.0));

    let child1 = FocusableWidget::new(true);
    let child1_id = tree.add_child(root_id, Box::new(child1), None, 0);
    tree.set_rect(child1_id, RectF::new(0.0, 0.0, 400.0, 50.0));

    let child2 = FocusableWidget::new(true);
    let child2_id = tree.add_child(root_id, Box::new(child2), None, 0);
    tree.set_rect(child2_id, RectF::new(0.0, 50.0, 400.0, 50.0));

    crate::core::app::compute_absolute_positions(&mut tree, root_id, Point::zero());
    crate::core::app::compute_absolute_paint_rects(&mut tree, root_id, Point::zero());

    // Get widget IDs
    let child1_widget_id = TreeWidgetId(tree.node(child1_id).unwrap().widget.id().0);
    let child2_widget_id = TreeWidgetId(tree.node(child2_id).unwrap().widget.id().0);

    // First focus - should NOT set pending_reveal
    router.request_focus_for_widget(&mut tree, child1_widget_id, &mut domain_state);
    let pending = router.take_pending_focus_reveal();
    assert!(
        pending.is_none(),
        "First focus should not set pending_reveal"
    );

    // Change focus - SHOULD set pending_reveal (for keyboard navigation scroll-into-view)
    router.request_focus_for_widget(&mut tree, child2_widget_id, &mut domain_state);
    let pending = router.take_pending_focus_reveal();
    assert_eq!(
        pending,
        Some(child2_widget_id),
        "Focus change (old_focus.is_some()) MUST set pending_focus_reveal \
         for keyboard navigation scroll-into-view"
    );
}

/// Verify the invariant holds even with realistic tree setup.
/// The key assertion is that pending_focus_reveal is None on first focus.
#[test]
fn test_first_focus_scroll_offset_unchanged() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut tree = WidgetTree::new();

    // Create ScrollArea with focusable child
    let scroll_area = ScrollArea::new(NodeId(1)).enable_vertical_scrollbar(true);
    let root_id = tree.create_root(Box::new(scroll_area), None, 0);
    tree.set_rect(root_id, RectF::new(0.0, 0.0, 400.0, 300.0));

    // Focusable child
    let child = FocusableWidget::new(true);
    let child_id = tree.add_child(root_id, Box::new(child), None, 0);
    tree.set_rect(child_id, RectF::new(0.0, 0.0, 400.0, 50.0));

    crate::core::app::compute_absolute_positions(&mut tree, root_id, Point::zero());
    crate::core::app::compute_absolute_paint_rects(&mut tree, root_id, Point::zero());

    // Record scroll offset before focus
    let offset_before = tree
        .node(root_id)
        .unwrap()
        .widget
        .scroll_container()
        .map(|c| c.visual_offset())
        .unwrap_or((0.0, 0.0));

    // Focus the child (first focus, from None)
    let child_widget_id = TreeWidgetId(tree.node(child_id).unwrap().widget.id().0);
    router.request_focus_for_widget(&mut tree, child_widget_id, &mut domain_state);

    // CRITICAL: pending_focus_reveal must be None on first focus
    // This is the invariant that prevents scroll jumps
    let pending = router.take_pending_focus_reveal();
    assert!(
        pending.is_none(),
        "INV-FOCUS-ENTRY-SCROLL-NEUTRALITY: First focus must not set pending_reveal"
    );

    // Verify scroll offset unchanged (no side effects from focus request)
    let offset_after = tree
        .node(root_id)
        .unwrap()
        .widget
        .scroll_container()
        .map(|c| c.visual_offset())
        .unwrap_or((0.0, 0.0));

    assert_eq!(
        offset_before, offset_after,
        "Scroll offset must not change from focus request alone. \
         Before: {:?}, After: {:?}",
        offset_before, offset_after
    );
}

/// Negative test: Programmatic focus assignment on an unfocused, scrolled container.
///
/// This test verifies that programmatic focus assignment (not just pointer-originated)
/// also respects INV-FOCUS-ENTRY-SCROLL-NEUTRALITY. The scroll offset must remain
/// unchanged when focus is programmatically assigned to a widget inside a scrolled
/// container that previously had no focus.
#[test]
fn test_programmatic_focus_on_scrolled_unfocused_container() {
    let _guard = LayoutAuthorityGuard::enter();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut tree = WidgetTree::new();

    // Create ScrollArea with focusable child
    let scroll_area = ScrollArea::new(NodeId(1)).enable_vertical_scrollbar(true);
    let root_id = tree.create_root(Box::new(scroll_area), None, 0);
    tree.set_rect(root_id, RectF::new(0.0, 0.0, 400.0, 300.0));

    // Focusable child positioned at y=500 (below initial viewport)
    let child = FocusableWidget::new(true);
    let child_id = tree.add_child(root_id, Box::new(child), None, 0);
    tree.set_rect(child_id, RectF::new(0.0, 500.0, 400.0, 50.0));

    crate::core::app::compute_absolute_positions(&mut tree, root_id, Point::zero());
    crate::core::app::compute_absolute_paint_rects(&mut tree, root_id, Point::zero());

    // Programmatically scroll down to y=400 (child now visible at y=100 in viewport)
    if let Some(node) = tree.node_mut(root_id) {
        if let Some(container) = node.widget.scroll_container_mut() {
            container.scroll_by(0.0, 400.0);
        }
    }

    // Record scroll offset after scrolling
    let offset_before = tree
        .node(root_id)
        .unwrap()
        .widget
        .scroll_container()
        .map(|c| c.visual_offset())
        .unwrap_or((0.0, 0.0));

    // Precondition: focus is None
    assert!(
        domain_state.focus_manager.focused().is_none(),
        "Precondition: focus must be None before programmatic focus"
    );

    // Programmatically focus the child (simulates any code path that calls request_focus_for_widget)
    let child_widget_id = TreeWidgetId(tree.node(child_id).unwrap().widget.id().0);
    router.request_focus_for_widget(&mut tree, child_widget_id, &mut domain_state);

    // ASSERTION 1: pending_focus_reveal must be None (invariant check)
    let pending = router.take_pending_focus_reveal();
    assert!(
        pending.is_none(),
        "INV-FOCUS-ENTRY-SCROLL-NEUTRALITY: Programmatic focus on unfocused container \
         must not set pending_focus_reveal. Got: {:?}",
        pending
    );

    // ASSERTION 2: Scroll offset must be unchanged
    let offset_after = tree
        .node(root_id)
        .unwrap()
        .widget
        .scroll_container()
        .map(|c| c.visual_offset())
        .unwrap_or((0.0, 0.0));

    assert_eq!(
        offset_before, offset_after,
        "Programmatic focus on unfocused scrolled container must not change scroll offset. \
         Before: {:?}, After: {:?}",
        offset_before, offset_after
    );

    // Verify focus was assigned
    assert_eq!(
        domain_state.focus_manager.focused(),
        Some(child_widget_id),
        "Focus must be assigned to the widget after programmatic focus"
    );
}
