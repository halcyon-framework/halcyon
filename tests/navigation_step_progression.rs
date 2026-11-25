// SPEC-HAL-233: Step-Oriented Surface Progression Contract
// Integration tests proving step semantics are mechanically derived from path diffs.

use halcyon::compositor::Compositor;
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationSurfaceOrchestrator, StepTransition,
};
use halcyon::model::navigation_host::{
    NavigationDecision, NavigationHost, NavigationIntent, NavigationNodeId,
};

fn nid(id: u32) -> NavigationNodeId {
    NavigationNodeId(id)
}

fn test_viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn default_allocator() -> Box<IncrementalSceneKeyAllocator> {
    Box::new(IncrementalSceneKeyAllocator::new())
}

// 1. Forward step creates exactly one new surface
#[test]
fn forward_step_creates_one_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(orch.step_index(), 0);
    assert_eq!(orch.step_count(), 1);

    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(result.step_transition, StepTransition::Advance);
    assert_eq!(compositor.surface_count(), 2);
    assert_eq!(orch.step_index(), 1);
    assert_eq!(orch.step_count(), 2);
}

// 2. Backward step destroys exactly one surface
#[test]
fn backward_step_destroys_one_surface() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    assert_eq!(compositor.surface_count(), 2);
    assert_eq!(orch.step_index(), 1);

    let surface_2 = orch.surface_for_node(nid(2)).unwrap();

    let result = orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(result.step_transition, StepTransition::Retreat);
    assert_eq!(compositor.surface_count(), 1);
    assert!(!compositor.has_surface(surface_2));
    assert_eq!(orch.step_index(), 0);
    assert_eq!(orch.step_count(), 1);
}

// 3. Replace step destroys + creates correctly
// Multi-step ancestor jump: [1, 2, 3] → NavigateTo(1) → [1]
// prefix=[1], removed=2, added=0 → Replace
#[test]
fn replace_step_destroys_correctly() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    // Build path [1, 2, 3]
    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    orch.dispatch(NavigationIntent::NavigateTo(nid(3).into()), &mut compositor);
    assert_eq!(compositor.surface_count(), 3);
    assert_eq!(orch.step_index(), 2);

    let surface_1 = orch.surface_for_node(nid(1)).unwrap();
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();
    let surface_3 = orch.surface_for_node(nid(3)).unwrap();

    // Jump to root: [1, 2, 3] → [1]
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(1).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(
        result.step_transition,
        StepTransition::Replace {
            removed: 2,
            added: 0
        }
    );
    assert_eq!(compositor.surface_count(), 1);
    assert!(compositor.has_surface(surface_1)); // root survives
    assert!(!compositor.has_surface(surface_2)); // destroyed
    assert!(!compositor.has_surface(surface_3)); // destroyed
    assert_eq!(orch.step_index(), 0);
    assert_eq!(orch.step_count(), 1);
    assert_eq!(result.current_node, nid(1));
}

// 4. No-op navigation does not mutate surfaces
#[test]
fn noop_navigation_no_surface_mutation() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));

    let mut orch = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);
    assert_eq!(compositor.surface_count(), 2);

    let surface_1 = orch.surface_for_node(nid(1)).unwrap();
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();

    // NavigateUp from path [1, 2] → [1]: that's Retreat, not NoOp.
    // NavigateTo(current) from [1, 2] where current=2: resolve truncates to [1, 2] (same). NoOp.
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::NoOp);
    assert_eq!(result.step_transition, StepTransition::NoOp);
    assert_eq!(compositor.surface_count(), 2);
    assert!(compositor.has_surface(surface_1));
    assert!(compositor.has_surface(surface_2));
    assert_eq!(orch.step_index(), 1);
    assert_eq!(orch.step_count(), 2);
}
