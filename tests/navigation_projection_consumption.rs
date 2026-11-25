// SPEC-HAL-246: SurfaceStack Navigation Projection Consumption
// Integration tests proving SurfaceStack is a pure consumer of projection data.
// No inference. No reordering. No recomputation.

use halcyon::compositor::{
    Compositor, FrameProjectionError, NavigationFrameProjection, NavigationProjectionContext,
    ProjectedSurfaceView, SceneKey, SurfaceId,
};
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationProjection, NavigationSurfaceOrchestrator,
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

fn create_overlay(
    compositor: &mut Compositor,
    parent: SurfaceId,
    z_index: i32,
    scene_key: u64,
) -> SurfaceId {
    compositor
        .create_overlay_surface(parent, z_index, test_viewport(), SceneKey::new(scene_key))
        .unwrap()
}

fn apply(
    compositor: &Compositor,
    projection: NavigationProjection,
    overlay_surfaces: Vec<SurfaceId>,
    focused_surface: Option<SurfaceId>,
) -> ProjectedSurfaceView {
    compositor
        .apply_frame_projection(&NavigationFrameProjection::new(
            compositor.identity(),
            vec![NavigationProjectionContext {
                navigation_surfaces: projection.navigation_surfaces,
                active_surface: projection.active_surface,
            }],
            overlay_surfaces,
            focused_surface,
        ))
        .unwrap()
}

// 1. Different projections produce different views
#[test]
fn different_projections_produce_different_views() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    // Projection 1: single root
    let proj1 = orch.project().unwrap();
    let view1 = apply(&compositor, proj1, Vec::new(), None);

    // Navigate forward
    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    // Projection 2: root + child
    let proj2 = orch.project().unwrap();
    let view2 = apply(&compositor, proj2, Vec::new(), None);

    // Views must differ: different surface count, different active surface
    assert_ne!(
        view1.navigation_contexts[0].navigation_surfaces.len(),
        view2.navigation_contexts[0].navigation_surfaces.len()
    );
    assert_ne!(
        view1.navigation_contexts[0].active_surface,
        view2.navigation_contexts[0].active_surface
    );
}

// 2. Navigation surfaces are in exact projection order, not z-index order
#[test]
fn navigation_surfaces_preserve_projection_order() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    orch.dispatch(NavigationIntent::NavigateTo(nid(3).into()), &mut compositor);

    let proj = orch.project().unwrap();
    let view = apply(&compositor, proj.clone(), Vec::new(), None);

    // IDs in view match IDs in projection, in exact order
    let proj_ids: Vec<_> = proj.navigation_surfaces.iter().copied().collect();
    let view_ids: Vec<_> = view.navigation_contexts[0]
        .navigation_surfaces
        .iter()
        .map(|d| d.id)
        .collect();
    assert_eq!(view_ids, proj_ids);
}

// 3. Active surface is passed through verbatim
#[test]
fn active_surface_verbatim() {
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
    let view = apply(&compositor, proj.clone(), Vec::new(), None);

    assert_eq!(
        view.navigation_contexts[0].active_surface,
        proj.active_surface
    );
}

// 4. Overlay surfaces are passed through verbatim
#[test]
fn overlay_surfaces_verbatim() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let proj = orch.project().unwrap();
    let parent = proj.active_surface.unwrap();
    let first = create_overlay(&mut compositor, parent, 100, 100);
    let second = create_overlay(&mut compositor, parent, 101, 101);
    let overlays = vec![first, second];
    let view = apply(&compositor, proj, overlays.clone(), None);

    assert_eq!(view.overlay_surfaces, overlays);
}

// 5. Focused surface is passed through verbatim
#[test]
fn focused_surface_verbatim() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let proj = orch.project().unwrap();
    let overlay = create_overlay(&mut compositor, proj.active_surface.unwrap(), 100, 100);
    let view = apply(&compositor, proj, vec![overlay], Some(overlay));

    // Focus is on overlay, not on active navigation surface
    assert_eq!(view.focused_surface, Some(overlay));
    assert_ne!(
        view.focused_surface,
        view.navigation_contexts[0].active_surface
    );
}

// 6. No field recomputed or reordered — full round-trip
#[test]
fn no_field_recomputed_or_reordered() {
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
    let overlay = create_overlay(&mut compositor, proj.active_surface.unwrap(), 100, 100);
    let view = apply(&compositor, proj.clone(), vec![overlay], Some(overlay));

    // Every field matches projection exactly
    let view_nav_ids: Vec<_> = view.navigation_contexts[0]
        .navigation_surfaces
        .iter()
        .map(|d| d.id)
        .collect();
    assert_eq!(view_nav_ids, proj.navigation_surfaces);
    assert_eq!(
        view.navigation_contexts[0].active_surface,
        proj.active_surface
    );
    assert_eq!(view.overlay_surfaces, vec![overlay]);
    assert_eq!(view.focused_surface, Some(overlay));
}

// 7. Same projection applied twice produces identical output
#[test]
fn same_projection_produces_identical_output() {
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
    let overlay = create_overlay(&mut compositor, proj.active_surface.unwrap(), 100, 100);

    let view_a = apply(&compositor, proj.clone(), vec![overlay], Some(overlay));
    let view_b = apply(&compositor, proj, vec![overlay], Some(overlay));

    // Identical navigation surface IDs
    let ids_a: Vec<_> = view_a.navigation_contexts[0]
        .navigation_surfaces
        .iter()
        .map(|d| d.id)
        .collect();
    let ids_b: Vec<_> = view_b.navigation_contexts[0]
        .navigation_surfaces
        .iter()
        .map(|d| d.id)
        .collect();
    assert_eq!(ids_a, ids_b);
    assert_eq!(
        view_a.navigation_contexts[0].active_surface,
        view_b.navigation_contexts[0].active_surface
    );
    assert_eq!(view_a.overlay_surfaces, view_b.overlay_surfaces);
    assert_eq!(view_a.focused_surface, view_b.focused_surface);
}

// 8. apply_frame_projection does not mutate compositor state
#[test]
fn apply_frame_projection_does_not_mutate_compositor() {
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
    let overlay = create_overlay(&mut compositor, proj.active_surface.unwrap(), 100, 100);
    let surface_count_before = compositor.surface_count();

    let _ = apply(&compositor, proj.clone(), vec![overlay], Some(overlay));
    let _ = apply(&compositor, proj.clone(), vec![overlay], Some(overlay));
    let _ = apply(&compositor, proj, vec![overlay], Some(overlay));

    assert_eq!(compositor.surface_count(), surface_count_before);
}

#[test]
fn frame_projection_rejects_a_different_compositor_identity() {
    let mut source = Compositor::new(test_viewport());
    let orchestrator = NavigationSurfaceOrchestrator::new(
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut source,
        default_allocator(),
    );
    let projection = orchestrator.project().unwrap();
    let frame = NavigationFrameProjection::new(
        source.identity(),
        vec![NavigationProjectionContext {
            navigation_surfaces: projection.navigation_surfaces,
            active_surface: projection.active_surface,
        }],
        Vec::new(),
        None,
    );
    let target = Compositor::new(test_viewport());

    assert!(matches!(
        target.apply_frame_projection(&frame),
        Err(FrameProjectionError::WrongCompositor { expected, actual })
            if expected == source.identity() && actual == target.identity()
    ));
}
