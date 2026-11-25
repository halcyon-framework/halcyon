// SPEC-HAL-235: Focus Resolution and Routing Authority
// Integration tests proving focus routing is explicit, deterministic, and navigation-owned.

use halcyon::compositor::Compositor;
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    FocusTransition, IncrementalSceneKeyAllocator, NavigationSurfaceOrchestrator,
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

// 1. Initial navigation assigns focus to root surface
#[test]
fn initial_focus_assigned_to_root_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let root_surface = orch.surface_for_node(nid(1)).unwrap();
    assert_eq!(orch.focused_surface(), Some(root_surface));
}

// 2. Advance transfers focus to newly created surface
#[test]
fn advance_transfers_focus_to_new_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let root_surface = orch.surface_for_node(nid(1)).unwrap();
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    let new_surface = orch.surface_for_node(nid(2)).unwrap();
    assert_eq!(
        result.focus_transition,
        Some(FocusTransition::Transferred {
            from: root_surface,
            to: new_surface,
        })
    );
    assert_eq!(orch.focused_surface(), Some(new_surface));
}

// 3. Retreat transfers focus back to previous surface
#[test]
fn retreat_transfers_focus_back() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();
    let surface_1 = orch.surface_for_node(nid(1)).unwrap();

    let result = orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    assert_eq!(
        result.focus_transition,
        Some(FocusTransition::Transferred {
            from: surface_2,
            to: surface_1,
        })
    );
    assert_eq!(orch.focused_surface(), Some(surface_1));
}

// 4. Replace transfers focus correctly (multi-surface destruction)
#[test]
fn replace_transfers_focus_correctly() {
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
    orch.dispatch(NavigationIntent::NavigateTo(nid(4).into()), &mut compositor);

    let surface_4 = orch.surface_for_node(nid(4)).unwrap();
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();

    // Jump to ancestor node 2: [1,2,3,4] → [1,2]
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(
        result.focus_transition,
        Some(FocusTransition::Transferred {
            from: surface_4,
            to: surface_2,
        })
    );
    assert_eq!(orch.focused_surface(), Some(surface_2));
}

// 5. Deny produces no focus transition
#[test]
fn deny_produces_no_focus_transition() {
    let mut compositor = Compositor::new(test_viewport());
    let mut host = NavigationHost::new(nid(1));
    host.set_guard(Box::new(DenyAllGuard));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let root_surface = orch.surface_for_node(nid(1)).unwrap();

    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Deny);
    assert_eq!(result.focus_transition, None);
    // Focus unchanged
    assert_eq!(orch.focused_surface(), Some(root_surface));
}

// 6. Root reset transfers focus to root surface
#[test]
fn root_reset_transfers_focus_to_root() {
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

    let surface_3 = orch.surface_for_node(nid(3)).unwrap();
    let surface_1 = orch.surface_for_node(nid(1)).unwrap();

    // Root reset: [1,2,3] → [1]
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(1).into()), &mut compositor);

    assert_eq!(
        result.focus_transition,
        Some(FocusTransition::Transferred {
            from: surface_3,
            to: surface_1,
        })
    );
    assert_eq!(orch.focused_surface(), Some(surface_1));
}

// 7. Terminated surface never remains focused
#[test]
fn terminated_surface_never_remains_focused() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();
    assert_eq!(orch.focused_surface(), Some(surface_2));

    // Navigate back: surface_2 is terminated
    let result = orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    // Terminated surface must not remain focused
    assert_ne!(orch.focused_surface(), Some(surface_2));
    // Focus should be on root
    let surface_1 = orch.surface_for_node(nid(1)).unwrap();
    assert_eq!(orch.focused_surface(), Some(surface_1));

    // Verify via terminated list
    let terminated_ids: Vec<_> = result
        .terminated_surfaces
        .iter()
        .map(|&(id, _)| id)
        .collect();
    assert!(terminated_ids.contains(&surface_2));
    // Focused surface is NOT in terminated list
    assert!(!terminated_ids.contains(&orch.focused_surface().unwrap()));
}
