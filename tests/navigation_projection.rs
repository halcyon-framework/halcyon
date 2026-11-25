// SPEC-HAL-246: SurfaceStack → Navigation Projection Contract
// Integration tests proving NavigationProjection is a pure, idempotent,
// zero-authority snapshot of orchestrator state.

use halcyon::compositor::{Compositor, SceneKey};
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationSurfaceOrchestrator, OverlayKind,
};
use halcyon::model::navigation_host::{NavigationHost, NavigationIntent, NavigationNodeId};

fn nid(id: u32) -> NavigationNodeId {
    NavigationNodeId(id)
}

fn test_viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn default_allocator() -> Box<IncrementalSceneKeyAllocator> {
    Box::new(IncrementalSceneKeyAllocator::new())
}

#[test]
fn projection_can_adopt_existing_root_surface_without_allocating() {
    let mut compositor = Compositor::new(test_viewport());
    let root_surface = compositor.create_surface(0, test_viewport(), SceneKey::BASE);
    let host = NavigationHost::new(nid(1));

    let orch = NavigationSurfaceOrchestrator::new_with_existing_root_surface(
        host,
        test_viewport(),
        root_surface,
        &mut compositor,
        default_allocator(),
    );

    let proj = orch.project().unwrap();

    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(orch.surface_for_node(nid(1)), Some(root_surface));
    assert_eq!(proj.navigation_surfaces, vec![root_surface]);
    assert_eq!(proj.active_surface, Some(root_surface));
}

// 1. Projection after initial construction: single root surface, focused
#[test]
fn projection_after_initial_construction() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let proj = orch.project().unwrap();
    let root_surface = orch.surface_for_node(nid(1)).unwrap();

    assert_eq!(proj.navigation_surfaces, vec![root_surface]);
    assert_eq!(proj.active_surface, Some(root_surface));
}

// 2. Projection after navigate forward: two navigation surfaces, focus on new
#[test]
fn projection_after_navigate_forward() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    let proj = orch.project().unwrap();
    let surface_1 = orch.surface_for_node(nid(1)).unwrap();
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();

    assert_eq!(proj.navigation_surfaces, vec![surface_1, surface_2]);
    assert_eq!(proj.active_surface, Some(surface_2));
}

// 3. Projection after navigate back: single surface, focus restored
#[test]
fn projection_after_navigate_back() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    let proj = orch.project().unwrap();
    let surface_1 = orch.surface_for_node(nid(1)).unwrap();

    assert_eq!(proj.navigation_surfaces, vec![surface_1]);
    assert_eq!(proj.active_surface, Some(surface_1));
}

// 4. Orchestrator-local overlays do not enter the navigation-only projection.
#[test]
fn projection_with_overlays() {
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

    let proj = orch.project().unwrap();
    let root_surface = orch.surface_for_node(nid(1)).unwrap();

    assert_eq!(proj.navigation_surfaces, vec![root_surface]);
    assert_eq!(proj.active_surface, Some(root_surface));
    assert!(compositor.has_surface(overlay_1));
    assert!(compositor.has_surface(overlay_2));
}

// 5. Projection after overlay close: overlay removed, focus restored
#[test]
fn projection_after_overlay_close() {
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

    orch.close_overlay(overlay_2, &mut compositor);

    let proj = orch.project().unwrap();
    let root_surface = orch.surface_for_node(nid(1)).unwrap();

    assert_eq!(proj.navigation_surfaces, vec![root_surface]);
    assert_eq!(proj.active_surface, Some(root_surface));
    assert!(compositor.has_surface(overlay_1));
}

// 6. Two consecutive calls without state change return identical projections (INV-246-05, INV-246-06)
#[test]
fn consecutive_calls_without_mutation_are_identical() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    orch.open_overlay(OverlayKind::Modal, &mut compositor);

    let proj_a = orch.project().unwrap();
    let proj_b = orch.project().unwrap();

    assert_eq!(proj_a, proj_b);
}

// 7. project() does not mutate orchestrator state (INV-246-06)
#[test]
fn project_does_not_mutate_state() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    let overlay = orch.open_overlay(OverlayKind::Modal, &mut compositor);

    // Snapshot state before project()
    let focused_before = orch.focused_surface();
    let active_overlay_before = orch.active_overlay();
    let step_count_before = orch.step_count();
    let surface_count_before = compositor.surface_count();

    // Call project() multiple times
    let _ = orch.project().unwrap();
    let _ = orch.project().unwrap();
    let _ = orch.project().unwrap();

    // State unchanged
    assert_eq!(orch.focused_surface(), focused_before);
    assert_eq!(orch.active_overlay(), active_overlay_before);
    assert_eq!(orch.step_count(), step_count_before);
    assert_eq!(compositor.surface_count(), surface_count_before);
    assert_eq!(orch.overlay_stack_depth(), 1);
    assert!(compositor.has_surface(overlay));
}

// 8. Projection after navigation auto-terminates overlays
#[test]
fn projection_after_nav_auto_terminates_overlays() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.open_overlay(OverlayKind::Modal, &mut compositor);
    orch.open_overlay(OverlayKind::Popover, &mut compositor);

    // Navigate forward: auto-terminates overlays
    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    let proj = orch.project().unwrap();
    let surface_1 = orch.surface_for_node(nid(1)).unwrap();
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();

    assert_eq!(proj.navigation_surfaces, vec![surface_1, surface_2]);
    assert_eq!(proj.active_surface, Some(surface_2));
}
