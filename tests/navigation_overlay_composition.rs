// SPEC-HAL-236: Overlay Layer Composition and Hit-Test Authority
// Integration tests proving overlay surfaces are explicitly layered, LIFO-ordered,
// and auto-terminated by navigation path changes.

use halcyon::compositor::Compositor;
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationSurfaceOrchestrator, OverlayKind,
    SurfaceTerminationReason,
};
use halcyon::model::navigation_host::{
    GuardDecision, NavigationDecision, NavigationGuard, NavigationHost, NavigationIntent,
    NavigationNodeId, NavigationState,
};
use std::any::Any;

fn nid(id: u32) -> NavigationNodeId {
    NavigationNodeId(id)
}

fn test_viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn default_allocator() -> Box<IncrementalSceneKeyAllocator> {
    Box::new(IncrementalSceneKeyAllocator::new())
}

struct DenyAllGuard;

impl NavigationGuard for DenyAllGuard {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        _intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        GuardDecision::Deny
    }
}

// 1. Opening an overlay creates exactly one new surface above active
#[test]
fn open_overlay_creates_one_surface_above_active() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    assert_eq!(compositor.surface_count(), 1);

    let overlay_id = orch.open_overlay(OverlayKind::Modal, &mut compositor);

    assert_eq!(compositor.surface_count(), 2);
    assert!(compositor.has_surface(overlay_id));
    assert_eq!(orch.overlay_stack_depth(), 1);
    assert_eq!(orch.active_overlay(), Some(overlay_id));
    assert_eq!(orch.focused_surface(), Some(overlay_id));
}

// 2. Multiple overlays stack correctly (LIFO)
#[test]
fn multiple_overlays_stack_lifo() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let overlay_1 = orch.open_overlay(OverlayKind::Modal, &mut compositor);
    let overlay_2 = orch.open_overlay(OverlayKind::Popover, &mut compositor);
    let overlay_3 = orch.open_overlay(OverlayKind::ContextMenu, &mut compositor);

    assert_eq!(compositor.surface_count(), 4); // root + 3 overlays
    assert_eq!(orch.overlay_stack_depth(), 3);
    // Top of stack is the last opened
    assert_eq!(orch.active_overlay(), Some(overlay_3));
    assert_eq!(orch.focused_surface(), Some(overlay_3));

    assert!(compositor.has_surface(overlay_1));
    assert!(compositor.has_surface(overlay_2));
    assert!(compositor.has_surface(overlay_3));
}

// 3. Closing top overlay restores focus correctly
#[test]
fn close_top_overlay_restores_focus() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let overlay_1 = orch.open_overlay(OverlayKind::Modal, &mut compositor);
    let overlay_2 = orch.open_overlay(OverlayKind::Popover, &mut compositor);

    assert_eq!(orch.focused_surface(), Some(overlay_2));

    let terminated = orch.close_overlay(overlay_2, &mut compositor);
    assert!(terminated.is_some());
    let (tid, reason) = terminated.unwrap();
    assert_eq!(tid, overlay_2);
    assert_eq!(reason, SurfaceTerminationReason::OverlayClosed);

    assert!(!compositor.has_surface(overlay_2));
    assert_eq!(orch.overlay_stack_depth(), 1);
    // Focus falls back to previous overlay
    assert_eq!(orch.active_overlay(), Some(overlay_1));
    assert_eq!(orch.focused_surface(), Some(overlay_1));
}

// 4. Closing all overlays restores focus to navigation surface
#[test]
fn close_all_overlays_restores_focus_to_nav_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let root_surface = orch.surface_for_node(nid(1)).unwrap();
    let overlay_1 = orch.open_overlay(OverlayKind::Modal, &mut compositor);

    assert_eq!(orch.focused_surface(), Some(overlay_1));

    orch.close_overlay(overlay_1, &mut compositor);

    assert_eq!(orch.overlay_stack_depth(), 0);
    assert_eq!(orch.active_overlay(), None);
    assert_eq!(orch.focused_surface(), Some(root_surface));
}

// 5. Navigation step change auto-terminates all overlays
#[test]
fn navigation_step_change_auto_terminates_overlays() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let overlay_1 = orch.open_overlay(OverlayKind::Modal, &mut compositor);
    let overlay_2 = orch.open_overlay(OverlayKind::Popover, &mut compositor);
    assert_eq!(compositor.surface_count(), 3); // root + 2 overlays

    // Navigate forward: auto-terminates overlays
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(orch.overlay_stack_depth(), 0);
    assert_eq!(orch.active_overlay(), None);

    // Both overlays terminated with OverlayReplaced
    let overlay_terminated: Vec<_> = result
        .terminated_surfaces
        .iter()
        .filter(|(_, reason)| *reason == SurfaceTerminationReason::OverlayReplaced)
        .collect();
    assert_eq!(overlay_terminated.len(), 2);

    // Overlay surfaces destroyed in compositor
    assert!(!compositor.has_surface(overlay_1));
    assert!(!compositor.has_surface(overlay_2));

    // Focus on new navigation surface, not overlay
    let new_surface = orch.surface_for_node(nid(2)).unwrap();
    assert_eq!(orch.focused_surface(), Some(new_surface));
    assert_eq!(compositor.surface_count(), 2); // root + new
}

// 6. Deny navigation does not mutate overlay stack
#[test]
fn deny_navigation_does_not_mutate_overlay_stack() {
    let mut compositor = Compositor::new(test_viewport());
    let mut host = NavigationHost::new(nid(1));
    host.set_guard(Box::new(DenyAllGuard));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let overlay_1 = orch.open_overlay(OverlayKind::Modal, &mut compositor);

    assert_eq!(orch.overlay_stack_depth(), 1);
    assert_eq!(orch.focused_surface(), Some(overlay_1));

    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Deny);
    assert_eq!(orch.overlay_stack_depth(), 1);
    assert_eq!(orch.active_overlay(), Some(overlay_1));
    assert_eq!(orch.focused_surface(), Some(overlay_1));
    assert!(compositor.has_surface(overlay_1));
}
