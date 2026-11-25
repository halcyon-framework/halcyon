// SPEC-HAL-234: Surface Lifecycle and Termination Authority
// Integration tests proving all navigation-driven surface destruction is
// centralized, reasoned, and test-covered.

use halcyon::compositor::{
    Compositor, SceneKey, SurfaceBatchPreparationError, SurfaceMutationBatch, SurfaceVisibility,
};
use halcyon::core::math::RectF;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationProjectionError, NavigationSurfaceLifecycle,
    NavigationSurfaceOrchestrator, NavigationSurfacePreparationError, OverlayKind,
    SurfaceTerminationReason,
};
use halcyon::model::navigation_host::{
    GuardDecision, NavigationAuthorizationResult, NavigationDecision, NavigationEntry,
    NavigationGuard, NavigationHost, NavigationIntent, NavigationNodeId, NavigationState,
    PreparedNavigationCommit,
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

// 1. Retreat terminates exactly one surface with reason StepRetreat
#[test]
fn retreat_terminates_one_surface_with_step_retreat() {
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

    let surface_2 = orch.surface_for_node(nid(2)).unwrap();

    let result = orch.dispatch(NavigationIntent::NavigateUp, &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(result.terminated_surfaces.len(), 1);
    assert_eq!(result.terminated_surfaces[0].0, surface_2);
    assert_eq!(
        result.terminated_surfaces[0].1,
        SurfaceTerminationReason::StepRetreat
    );
    assert!(!compositor.has_surface(surface_2));
    assert_eq!(compositor.surface_count(), 1);
}

// 2. Replace terminates N surfaces with reason StepReplace
// Path [1, 2, 3, 4] → NavigateTo(2) → [1, 2]: removed=2, added=0, new_path_len=2
#[test]
fn replace_terminates_n_surfaces_with_step_replace() {
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
    assert_eq!(compositor.surface_count(), 4);

    let surface_3 = orch.surface_for_node(nid(3)).unwrap();
    let surface_4 = orch.surface_for_node(nid(4)).unwrap();

    // Jump to ancestor node 2: destroys surfaces for nodes 3 and 4
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(result.terminated_surfaces.len(), 2);

    // All terminated surfaces must have reason StepReplace
    for &(_, reason) in &result.terminated_surfaces {
        assert_eq!(reason, SurfaceTerminationReason::StepReplace);
    }

    // Verify the specific surfaces were terminated
    let terminated_ids: Vec<_> = result
        .terminated_surfaces
        .iter()
        .map(|&(id, _)| id)
        .collect();
    assert!(terminated_ids.contains(&surface_3));
    assert!(terminated_ids.contains(&surface_4));

    assert!(!compositor.has_surface(surface_3));
    assert!(!compositor.has_surface(surface_4));
    assert_eq!(compositor.surface_count(), 2);
}

// 3. NoOp causes zero termination
#[test]
fn noop_causes_zero_termination() {
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

    // NavigateTo(current) resolves to same path — no-op
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::NoOp);
    assert!(result.terminated_surfaces.is_empty());
    assert_eq!(compositor.surface_count(), 2);
}

// 4. Deny causes zero termination
#[test]
fn deny_causes_zero_termination() {
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

    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(2).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Deny);
    assert!(result.terminated_surfaces.is_empty());
    assert_eq!(compositor.surface_count(), count_before);
}

// 5. Root reset leaves exactly one surface alive, all terminated with RootReset
// Path [1, 2, 3] → NavigateTo(1) → [1]: removed=2, added=0, new_path_len=1
#[test]
fn root_reset_leaves_one_surface_alive() {
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
    assert_eq!(compositor.surface_count(), 3);

    let surface_1 = orch.surface_for_node(nid(1)).unwrap();
    let surface_2 = orch.surface_for_node(nid(2)).unwrap();
    let surface_3 = orch.surface_for_node(nid(3)).unwrap();

    // Jump to root: destroys all non-root surfaces
    let result = orch.dispatch(NavigationIntent::NavigateTo(nid(1).into()), &mut compositor);

    assert_eq!(result.decision, NavigationDecision::Allow);
    assert_eq!(result.terminated_surfaces.len(), 2);

    // All terminated surfaces must have reason RootReset
    for &(_, reason) in &result.terminated_surfaces {
        assert_eq!(reason, SurfaceTerminationReason::RootReset);
    }

    // Root surface survives, others destroyed
    assert!(compositor.has_surface(surface_1));
    assert!(!compositor.has_surface(surface_2));
    assert!(!compositor.has_surface(surface_3));
    assert_eq!(compositor.surface_count(), 1);
}

fn prepare_host_commit(
    orchestrator: &NavigationSurfaceOrchestrator,
    intent: NavigationIntent,
) -> PreparedNavigationCommit {
    let snapshot = orchestrator.nav_host().snapshot().unwrap();
    let authorization = match orchestrator
        .nav_host()
        .authorize(&snapshot, intent, None, None)
    {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        outcome => panic!("expected authorization, got {outcome:?}"),
    };
    orchestrator
        .nav_host()
        .prepare_commit(authorization, snapshot.generation(), None)
        .unwrap()
}

#[test]
fn prepared_surface_abort_destroys_only_hidden_staging_and_preserves_live_state() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orchestrator = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let root_surface = orchestrator.surface_for_node(nid(1)).unwrap();
    let generation = orchestrator.orchestration_generation();
    let prepared_host =
        prepare_host_commit(&orchestrator, NavigationIntent::NavigateTo(nid(2).into()));
    let prepared_surfaces = orchestrator
        .prepare_navigation_surface_transition(&prepared_host, generation, &mut compositor)
        .unwrap();
    let staged = prepared_surfaces.staged_surfaces()[0];

    assert_eq!(
        prepared_surfaces.staged_surface_visibility(staged),
        Some(SurfaceVisibility::Hidden)
    );
    assert_eq!(orchestrator.current_node(), nid(1));
    assert_eq!(orchestrator.project(), Err(NavigationProjectionError::Busy));
    assert_eq!(orchestrator.orchestration_generation(), generation);

    let mut unrelated_compositor = Compositor::new(test_viewport());
    assert_eq!(
        orchestrator.try_open_overlay(OverlayKind::Modal, &mut unrelated_compositor),
        Err(NavigationSurfacePreparationError::Busy)
    );

    prepared_surfaces.abort();
    drop(prepared_host);
    assert!(!compositor.has_surface(staged));
    assert!(compositor.has_surface(root_surface));
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(orchestrator.current_node(), nid(1));
    assert_eq!(orchestrator.orchestration_generation(), generation);
}

#[test]
fn stale_orchestration_generation_rejects_before_surface_staging() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orchestrator = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let stale_generation = orchestrator.orchestration_generation();
    let prepared_host =
        prepare_host_commit(&orchestrator, NavigationIntent::NavigateTo(nid(2).into()));
    orchestrator.open_overlay(OverlayKind::Modal, &mut compositor);
    let count_before = compositor.surface_count();

    assert!(matches!(
        orchestrator.prepare_navigation_surface_transition(
            &prepared_host,
            stale_generation,
            &mut compositor,
        ),
        Err(NavigationSurfacePreparationError::StaleGeneration)
    ));
    drop(prepared_host);
    assert_eq!(compositor.surface_count(), count_before);
    assert_eq!(orchestrator.current_node(), nid(1));
}

#[test]
fn navigation_surface_owner_rejects_a_different_compositor_without_mutation() {
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(nid(1));
    let mut orchestrator = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );
    let root_surface = orchestrator.surface_for_node(nid(1)).unwrap();
    let generation = orchestrator.orchestration_generation();
    let prepared_host =
        prepare_host_commit(&orchestrator, NavigationIntent::NavigateTo(nid(2).into()));
    let mut other_compositor = Compositor::new(test_viewport());

    assert_eq!(
        orchestrator.try_open_overlay(OverlayKind::Modal, &mut other_compositor),
        Err(NavigationSurfacePreparationError::WrongCompositor)
    );
    assert!(matches!(
        orchestrator.prepare_navigation_surface_transition(
            &prepared_host,
            generation,
            &mut other_compositor,
        ),
        Err(NavigationSurfacePreparationError::WrongCompositor)
    ));
    drop(prepared_host);

    assert_eq!(compositor.surface_count(), 1);
    assert!(compositor.has_surface(root_surface));
    assert_eq!(other_compositor.surface_count(), 0);
    assert_eq!(orchestrator.current_node(), nid(1));
    assert_eq!(orchestrator.orchestration_generation(), generation);
}

#[test]
fn complete_navigation_entries_own_distinct_surfaces_for_the_same_node_id() {
    let root = NavigationEntry::new(nid(1));
    let account_a = NavigationEntry::new(nid(2)).with_parameter("account", "a");
    let account_b = NavigationEntry::new(nid(2)).with_parameter("account", "b");
    let mut compositor = Compositor::new(test_viewport());
    let host = NavigationHost::new(root.clone());
    let mut orchestrator = NavigationSurfaceOrchestrator::new(
        host,
        test_viewport(),
        &mut compositor,
        default_allocator(),
    );

    let result = orchestrator.dispatch(
        NavigationIntent::NavigatePath(vec![root, account_a.clone(), account_b.clone()]),
        &mut compositor,
    );

    assert_eq!(result.decision, NavigationDecision::Allow);
    let surface_a = orchestrator.surface_for_entry(&account_a).unwrap();
    let surface_b = orchestrator.surface_for_entry(&account_b).unwrap();
    assert_ne!(surface_a, surface_b);
    assert_eq!(orchestrator.surface_for_node(nid(2)), Some(surface_b));
    assert_eq!(
        orchestrator.surface_lifecycle(surface_a),
        Some(NavigationSurfaceLifecycle::Inactive)
    );
    assert_eq!(
        orchestrator.surface_lifecycle(surface_b),
        Some(NavigationSurfaceLifecycle::Active)
    );
    assert_eq!(compositor.surface_count(), 3);
}

#[test]
fn mechanical_surface_batch_is_abortable_and_commits_exactly_once() {
    let mut compositor = Compositor::new(test_viewport());
    let surface = compositor.create_surface(0, test_viewport(), SceneKey::BASE);
    let mut batch = SurfaceMutationBatch::new();
    batch.set_visibility(surface, SurfaceVisibility::Hidden);

    let prepared = compositor.prepare_surface_batch(batch).unwrap();
    assert_eq!(
        prepared.surface_visibility(surface),
        Some(SurfaceVisibility::Visible)
    );
    let _ = prepared.abort();
    assert_eq!(
        compositor.surface_visibility(surface),
        Some(SurfaceVisibility::Visible)
    );

    let mut batch = SurfaceMutationBatch::new();
    batch.set_visibility(surface, SurfaceVisibility::Hidden);
    let prepared = compositor.prepare_surface_batch(batch).unwrap();
    let _ = prepared.commit();
    assert_eq!(
        compositor.surface_visibility(surface),
        Some(SurfaceVisibility::Hidden)
    );
}

#[test]
fn mechanical_batch_forbids_implicit_overlay_cascade() {
    let mut compositor = Compositor::new(test_viewport());
    let base = compositor.create_surface(0, test_viewport(), SceneKey::BASE);
    let overlay = compositor
        .create_overlay_surface(base, 10, test_viewport(), SceneKey::new(2))
        .unwrap();
    let mut invalid = SurfaceMutationBatch::new();
    invalid.destroy(base);

    assert_eq!(
        compositor.validate_surface_batch(&invalid),
        Err(SurfaceBatchPreparationError::ImplicitOverlayDestruction {
            parent: base,
            overlay,
        })
    );

    let mut exact = SurfaceMutationBatch::new();
    exact.destroy(overlay);
    exact.destroy(base);
    let prepared = compositor.prepare_surface_batch(exact).unwrap();
    let _ = prepared.commit();
    assert!(!compositor.has_surface(overlay));
    assert!(!compositor.has_surface(base));
}
