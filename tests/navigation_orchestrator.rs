use halcyon::compositor::Compositor;
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationSurfaceOrchestrator,
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

// --- Guard implementations ---

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

struct RedirectGuard {
    source: NavigationNodeId,
    target: NavigationNodeId,
}

impl NavigationGuard for RedirectGuard {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        match intent {
            NavigationIntent::NavigateTo(entry) if entry.node_id() == self.source => {
                GuardDecision::ReplaceWith(NavigationIntent::NavigateTo(self.target.into()))
            }
            _ => GuardDecision::Allow,
        }
    }
}

// --- Tests ---

// 1. Deny guard — zero compositor mutation, zero host state change
#[test]
fn deny_guard_zero_compositor_mutation() {
    let mut compositor = Compositor::new(test_viewport());
    let mut host = NavigationHost::new(nid(1));
    host.set_guard(Box::new(DenyAllGuard));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let count_before = compositor.surface_count();
    let node_before = orch.current_node();

    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Deny);
    assert_eq!(compositor.surface_count(), count_before);
    assert!(result.active_surface.is_none());
    // NavigationHost state unchanged (semantic-only, no surface remnants)
    assert_eq!(orch.nav_host().current(), node_before);
}

// 2. Allow NavigateTo — creates compositor surface
#[test]
fn allow_navigate_to_creates_compositor_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    assert_eq!(compositor.surface_count(), 1); // root

    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(compositor.surface_count(), 2);
    assert!(result.active_surface.is_some());
    assert!(compositor.has_surface(result.active_surface.unwrap()));
}

// 3. NavigateUp — destroys correct surface
#[test]
fn navigate_back_destroys_correct_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    // Navigate to node 2, then node 3
    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    orch.dispatch(NavigationIntent::NavigateTo(nid(3).into()), &mut compositor);
    assert_eq!(compositor.surface_count(), 3);

    // Remember surface for node 3 before going back
    let surface_3 = orch.surface_for_node(nid(3)).unwrap();

    let result = orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(compositor.surface_count(), 2);
    assert!(!compositor.has_surface(surface_3)); // node 3's surface destroyed
    assert_eq!(result.current_node, nid(2));
}

// 4. ReplaceWith — redirect, no ghost surfaces
#[test]
fn replacewith_redirect_no_ghost_surfaces() {
    let mut compositor = Compositor::new(test_viewport());
    let mut host = NavigationHost::new(nid(1));
    host.set_guard(Box::new(RedirectGuard {
        source: nid(5),
        target: nid(10),
    }));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(5).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Redirect);
    assert_eq!(result.current_node, nid(10));
    assert!(orch.surface_for_node(nid(10)).is_some()); // redirected target has surface
    assert!(orch.surface_for_node(nid(5)).is_none()); // no ghost surface for original
    assert_eq!(compositor.surface_count(), 2); // root + redirected target
}
