// SPEC-HAL-237: Multiroot Navigation Isolation and Delegation Contracts
// Integration tests proving multiple navigation roots coexist with strict isolation:
// no cross-root surface ownership, no cross-root focus/overlay leakage.

use halcyon::compositor::{Compositor, SurfaceVisibility};
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootPreparationError,
    NavigationRootRegistry, OverlayKind, SurfaceTerminationReason,
};
use halcyon::model::navigation_host::{NavigationHost, NavigationIntent, NavigationNodeId};

fn nid(id: u32) -> NavigationNodeId {
    NavigationNodeId(id)
}

fn rid(id: u32) -> NavigationRootId {
    NavigationRootId(id)
}

fn test_viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn default_allocator() -> Box<IncrementalSceneKeyAllocator> {
    Box::new(IncrementalSceneKeyAllocator::new())
}

// 1. Two roots can coexist: surface counts increase appropriately
#[test]
fn two_roots_coexist_surface_counts() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();

    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    assert_eq!(compositor.surface_count(), 1);

    registry.create_root(
        rid(2),
        NavigationHost::new(nid(10)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    assert_eq!(compositor.surface_count(), 2);

    assert_eq!(registry.root_count(), 2);
    assert!(registry.root(rid(1)).is_some());
    assert!(registry.root(rid(2)).is_some());
}

// 2. Dispatching root A does not change root B surface ids/counts
#[test]
fn dispatch_root_a_does_not_affect_root_b() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();

    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    registry.create_root(
        rid(2),
        NavigationHost::new(nid(10)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let root_b_surface = registry
        .root(rid(2))
        .unwrap()
        .surface_for_node(nid(10))
        .unwrap();
    let root_b_focused = registry.root(rid(2)).unwrap().focused_surface();

    // Dispatch on root A: navigate forward
    registry.dispatch(
        rid(1),
        NavigationIntent::NavigateTo(nid(2).into()),
        &mut compositor,
    );

    // Root B unchanged
    assert!(compositor.has_surface(root_b_surface));
    assert_eq!(
        registry.root(rid(2)).unwrap().focused_surface(),
        root_b_focused
    );
    assert_eq!(registry.root(rid(2)).unwrap().step_count(), 1);

    // Root A changed
    assert_eq!(registry.root(rid(1)).unwrap().step_count(), 2);
    assert_eq!(compositor.surface_count(), 3);
}

// 3. Overlays opened in root A do not affect root B focus/overlays
#[test]
fn overlays_root_a_do_not_affect_root_b() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();

    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    registry.create_root(
        rid(2),
        NavigationHost::new(nid(10)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let root_b_focused = registry.root(rid(2)).unwrap().focused_surface();

    // Open overlay on root A
    registry
        .root_mut(rid(1))
        .unwrap()
        .open_overlay(OverlayKind::Modal, &mut compositor);

    // Root B unaffected
    assert_eq!(registry.root(rid(2)).unwrap().overlay_stack_depth(), 0);
    assert_eq!(
        registry.root(rid(2)).unwrap().focused_surface(),
        root_b_focused
    );

    // Root A has overlay
    assert_eq!(registry.root(rid(1)).unwrap().overlay_stack_depth(), 1);
    assert_eq!(compositor.surface_count(), 3); // root_a + overlay_a + root_b
}

// 4. Root destruction terminates all surfaces it owns (including overlays) and leaves other root untouched
#[test]
fn root_destruction_terminates_owned_surfaces_only() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();

    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    registry.create_root(
        rid(2),
        NavigationHost::new(nid(10)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    // Navigate root A forward, then open overlay
    registry.dispatch(
        rid(1),
        NavigationIntent::NavigateTo(nid(2).into()),
        &mut compositor,
    );
    registry
        .root_mut(rid(1))
        .unwrap()
        .open_overlay(OverlayKind::Modal, &mut compositor);

    let root_b_surface = registry
        .root(rid(2))
        .unwrap()
        .surface_for_node(nid(10))
        .unwrap();
    assert_eq!(compositor.surface_count(), 4); // root_a(1) + root_a(2) + overlay + root_b(10)

    // Destroy root A
    let terminated = registry.destroy_root(rid(1), &mut compositor);

    // All 3 root A surfaces terminated with HostTeardown
    assert_eq!(terminated.len(), 3);
    for &(_, reason) in &terminated {
        assert_eq!(reason, SurfaceTerminationReason::HostTeardown);
    }

    // Root B intact
    assert!(compositor.has_surface(root_b_surface));
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(registry.root_count(), 1);
    assert!(registry.root(rid(1)).is_none());
    assert!(registry.root(rid(2)).is_some());
}

// 5. Navigation step change in root A auto-terminates overlays in root A only
#[test]
fn nav_step_change_auto_terminates_overlays_in_own_root_only() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();

    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    registry.create_root(
        rid(2),
        NavigationHost::new(nid(10)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    // Open overlay on both roots
    registry
        .root_mut(rid(1))
        .unwrap()
        .open_overlay(OverlayKind::Modal, &mut compositor);
    registry
        .root_mut(rid(2))
        .unwrap()
        .open_overlay(OverlayKind::Popover, &mut compositor);

    assert_eq!(compositor.surface_count(), 4); // 2 roots + 2 overlays
    assert_eq!(registry.root(rid(1)).unwrap().overlay_stack_depth(), 1);
    assert_eq!(registry.root(rid(2)).unwrap().overlay_stack_depth(), 1);

    // Navigate root A forward: auto-terminates root A overlay only
    let result = registry
        .dispatch(
            rid(1),
            NavigationIntent::NavigateTo(nid(2).into()),
            &mut compositor,
        )
        .unwrap();

    // Root A overlay terminated
    assert_eq!(registry.root(rid(1)).unwrap().overlay_stack_depth(), 0);

    // Root B overlay intact
    assert_eq!(registry.root(rid(2)).unwrap().overlay_stack_depth(), 1);

    // Verify via result
    let overlay_terminated: Vec<_> = result
        .terminated_surfaces
        .iter()
        .filter(|(_, reason)| *reason == SurfaceTerminationReason::OverlayReplaced)
        .collect();
    assert_eq!(overlay_terminated.len(), 1);
}

#[test]
fn prepared_root_abort_never_publishes_and_destroys_hidden_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();
    let prepared = registry
        .prepare_create_root(
            rid(1),
            NavigationHost::new(nid(1)),
            test_viewport(),
            &mut compositor,
            default_allocator(),
        )
        .unwrap();
    let staged_root = prepared.current_surface();

    assert_eq!(
        prepared.current_surface_visibility(),
        Some(SurfaceVisibility::Hidden)
    );
    prepared.abort();

    assert!(!compositor.has_surface(staged_root));
    assert_eq!(compositor.surface_count(), 0);
    assert_eq!(registry.root_count(), 0);
}

#[test]
fn duplicate_root_is_rejected_before_surface_allocation() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();
    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let count_before = compositor.surface_count();

    assert!(matches!(
        registry.prepare_create_root(
            rid(1),
            NavigationHost::new(nid(10)),
            test_viewport(),
            &mut compositor,
            default_allocator(),
        ),
        Err(NavigationRootPreparationError::DuplicateRoot(root)) if root == rid(1)
    ));
    assert_eq!(compositor.surface_count(), count_before);
    assert_eq!(registry.root_count(), 1);
    assert_eq!(registry.root(rid(1)).unwrap().current_node(), nid(1));
}

#[test]
fn root_registry_rejects_a_different_compositor_before_surface_allocation() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();
    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let mut other_compositor = Compositor::new(test_viewport());

    assert!(matches!(
        registry.prepare_create_root(
            rid(2),
            NavigationHost::new(nid(10)),
            test_viewport(),
            &mut other_compositor,
            default_allocator(),
        ),
        Err(NavigationRootPreparationError::WrongCompositor)
    ));

    assert_eq!(registry.root_count(), 1);
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(other_compositor.surface_count(), 0);
}

#[test]
fn prepared_whole_root_teardown_abort_preserves_root_current_and_overlay_surfaces() {
    let mut compositor = Compositor::new(test_viewport());
    let mut registry = NavigationRootRegistry::new();
    registry.create_root(
        rid(1),
        NavigationHost::new(nid(1)),
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    registry.dispatch(
        rid(1),
        NavigationIntent::NavigateTo(nid(2).into()),
        &mut compositor,
    );
    let overlay = registry
        .root_mut(rid(1))
        .unwrap()
        .open_overlay(OverlayKind::Modal, &mut compositor);
    let root_surface = registry
        .root(rid(1))
        .unwrap()
        .surface_for_node(nid(1))
        .unwrap();
    let current_surface = registry
        .root(rid(1))
        .unwrap()
        .surface_for_node(nid(2))
        .unwrap();
    let generation = registry.root(rid(1)).unwrap().orchestration_generation();
    let prepared = registry
        .prepare_root_surface_teardown(rid(1), generation, &mut compositor)
        .unwrap();
    let terminating: Vec<_> = prepared
        .terminating_surfaces()
        .iter()
        .map(|(surface, _)| *surface)
        .collect();
    assert_eq!(terminating, vec![overlay, current_surface, root_surface]);

    prepared.abort();

    assert!(compositor.has_surface(overlay));
    assert!(compositor.has_surface(current_surface));
    assert!(compositor.has_surface(root_surface));
    assert_eq!(registry.root_count(), 1);
    assert_eq!(
        registry.root(rid(1)).unwrap().orchestration_generation(),
        generation
    );
}
