// SPEC-HAL-248: ScreenHost → NavigationRoot Projection Pull
// Integration tests proving ScreenHost is a pure, pull-only consumer of projection data.
// Navigation state changes are invisible to ScreenHost until an explicit projection pull.

use halcyon::compositor::{Compositor, NavigationFrameProjection, NavigationProjectionContext};
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationSurfaceOrchestrator,
};
use halcyon::model::navigation_host::{NavigationHost, NavigationIntent, NavigationNodeId};
use halcyon::widgets::screen_host::ScreenHost;

fn nid(id: u32) -> NavigationNodeId {
    NavigationNodeId(id)
}

fn test_viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn default_allocator() -> Box<IncrementalSceneKeyAllocator> {
    Box::new(IncrementalSceneKeyAllocator::new())
}

/// Mechanical sink helper; the full guarded runtime pull is covered separately.
fn pull(orch: &NavigationSurfaceOrchestrator, compositor: &Compositor, sh: &mut ScreenHost) {
    let proj = orch.project().unwrap();
    let frame = NavigationFrameProjection::new(
        compositor.identity(),
        vec![NavigationProjectionContext {
            navigation_surfaces: proj.navigation_surfaces,
            active_surface: proj.active_surface,
        }],
        Vec::new(),
        None,
    );
    let view = compositor.apply_frame_projection(&frame).unwrap();
    sh.set_projected_view(view);
}

// 1. Navigation state change is invisible until projection pull
#[test]
fn nav_change_invisible_until_pull() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let mut sh = ScreenHost::new();

    // Initial pull: single root surface
    pull(&orch, &compositor, &mut sh);
    let initial_nav_count = sh.projected_view().unwrap().navigation_contexts[0]
        .navigation_surfaces
        .len();
    let initial_active = sh.projected_view().unwrap().navigation_contexts[0].active_surface;
    assert_eq!(initial_nav_count, 1);

    // Navigate forward — but do NOT pull yet
    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    // ScreenHost still shows old state (single surface, same active)
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0]
            .navigation_surfaces
            .len(),
        initial_nav_count
    );
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0].active_surface,
        initial_active
    );

    // NOW pull
    pull(&orch, &compositor, &mut sh);

    // ScreenHost now shows new state (two surfaces, different active)
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0]
            .navigation_surfaces
            .len(),
        2
    );
    assert_ne!(
        sh.projected_view().unwrap().navigation_contexts[0].active_surface,
        initial_active
    );
}

// 2. Two frames with no navigation mutation produce identical render input
#[test]
fn two_pulls_without_mutation_produce_identical_input() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let mut sh = ScreenHost::new();

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    // Frame 1 pull
    pull(&orch, &compositor, &mut sh);
    let frame1_nav_ids: Vec<_> = sh.projected_view().unwrap().navigation_contexts[0]
        .navigation_surfaces
        .iter()
        .map(|d| d.id)
        .collect();
    let frame1_active = sh.projected_view().unwrap().navigation_contexts[0].active_surface;
    let frame1_overlays = sh.projected_view().unwrap().overlay_surfaces.clone();
    let frame1_focused = sh.projected_view().unwrap().focused_surface;

    // Frame 2 pull (no mutation between)
    pull(&orch, &compositor, &mut sh);
    let frame2_nav_ids: Vec<_> = sh.projected_view().unwrap().navigation_contexts[0]
        .navigation_surfaces
        .iter()
        .map(|d| d.id)
        .collect();
    let frame2_active = sh.projected_view().unwrap().navigation_contexts[0].active_surface;
    let frame2_overlays = sh.projected_view().unwrap().overlay_surfaces.clone();
    let frame2_focused = sh.projected_view().unwrap().focused_surface;

    assert_eq!(frame1_nav_ids, frame2_nav_ids);
    assert_eq!(frame1_active, frame2_active);
    assert_eq!(frame1_overlays, frame2_overlays);
    assert_eq!(frame1_focused, frame2_focused);
}

// 3. ScreenHost starts with no projected view
#[test]
fn starts_with_no_projected_view() {
    let sh = ScreenHost::new();
    assert!(sh.projected_view().is_none());
}

// 4. Pull replaces previous projected view (no accumulation)
#[test]
fn pull_replaces_previous_view() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let mut sh = ScreenHost::new();

    // First pull: single root
    pull(&orch, &compositor, &mut sh);
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0]
            .navigation_surfaces
            .len(),
        1
    );

    // Navigate forward
    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    // Second pull: root + child
    pull(&orch, &compositor, &mut sh);
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0]
            .navigation_surfaces
            .len(),
        2
    );

    // Navigate back
    orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    // Third pull: back to single root
    pull(&orch, &compositor, &mut sh);
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0]
            .navigation_surfaces
            .len(),
        1
    );
}

// 5. Resolved context grouping and within-context order are retained.
#[test]
fn rendering_contract_fields_present() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let mut sh = ScreenHost::new();

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    pull(&orch, &compositor, &mut sh);

    let view = sh.projected_view().unwrap();

    // navigation_surfaces: ordered descriptors with IDs
    assert_eq!(view.navigation_contexts[0].navigation_surfaces.len(), 2);
    assert_ne!(
        view.navigation_contexts[0].navigation_surfaces[0].id,
        view.navigation_contexts[0].navigation_surfaces[1].id
    );
    assert!(view.focused_surface.is_none());
}

// 6. Multiple navigations between pulls — only final state visible
#[test]
fn multiple_navigations_between_pulls() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let mut sh = ScreenHost::new();

    // Initial pull
    pull(&orch, &compositor, &mut sh);

    // Multiple navigations without pulling
    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    orch.dispatch(NavigationIntent::NavigateTo(nid(3).into()), &mut compositor);
    orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    // ScreenHost still shows initial state (1 surface)
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0]
            .navigation_surfaces
            .len(),
        1
    );

    // Pull captures final state: root(1) + node(2)
    pull(&orch, &compositor, &mut sh);
    assert_eq!(
        sh.projected_view().unwrap().navigation_contexts[0]
            .navigation_surfaces
            .len(),
        2
    );
}
