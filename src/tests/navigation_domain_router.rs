use crate::compositor::SurfaceId;
use crate::core::event::{InteractionDomainId, InteractionDomains};
use crate::core::focus_authority::{FocusDomainAddition, FocusDomainId, FocusResolver};
use crate::core::navigation_domain_router::{
    NavigationDomainRouter, NavigationDomainRouterError, NavigationRootFocusBindingError,
    NavigationRootFocusBindingPort, NavigationRootFocusBindingPortId,
    NavigationRootFocusBindingReplacement, RoutedNavigationCandidate,
};
use crate::core::navigation_orchestrator::NavigationRootId;
use crate::core::overlay_domain_coordinator::{
    OverlayActivationAdmission, OverlayDomainCoordinator, OverlayOwnerId, OverlayOwnerLifecyclePort,
};

const ROOT: NavigationRootId = NavigationRootId(10);
const OWNER: OverlayOwnerId = OverlayOwnerId(20);
const PORT: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(30);
const ROOT_FOCUS: FocusDomainId = FocusDomainId(40);
const CHILD_FOCUS: FocusDomainId = FocusDomainId(41);
const PEER_ROOT_FOCUS: FocusDomainId = FocusDomainId(42);
const PEER_CHILD_FOCUS: FocusDomainId = FocusDomainId(43);
const DOMAIN_LOCAL_FOCUS: FocusDomainId = FocusDomainId(44);

fn initial_surface() -> SurfaceId {
    SurfaceId::test_id(50)
}

fn setup() -> (
    InteractionDomains,
    OverlayDomainCoordinator,
    NavigationDomainRouter,
) {
    let interaction_domains = InteractionDomains::new();
    let coordinator = OverlayDomainCoordinator::new().unwrap();
    coordinator
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let router = NavigationDomainRouter::new(coordinator.clone());
    router
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    (interaction_domains, coordinator, router)
}

fn admit_root(
    interaction_domains: &mut InteractionDomains,
    coordinator: &OverlayDomainCoordinator,
    router: &NavigationDomainRouter,
) -> NavigationRootFocusBindingPort {
    coordinator
        .register_owner(
            OverlayOwnerLifecyclePort::new(OWNER, InteractionDomainId::PRIMARY, vec![]).unwrap(),
            OverlayActivationAdmission::RouterEligibilityRequired,
        )
        .unwrap();
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let resolver = FocusResolver::new();
    let graph = resolver
        .prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            &mut interaction_domains.primary_state_mut().focus_manager,
            &[
                FocusDomainAddition::new(ROOT_FOCUS, None),
                FocusDomainAddition::new(CHILD_FOCUS, Some(ROOT_FOCUS)),
            ],
            &[],
            vec![],
            Some(ROOT_FOCUS),
        )
        .unwrap();
    let port = NavigationRootFocusBindingPort::prepare_create_guarded(
        &guard,
        ROOT,
        PORT,
        initial_surface(),
        ROOT_FOCUS,
        graph.testimony(),
    )
    .unwrap();
    let mut association = router
        .prepare_root_association_guarded(&guard, ROOT, OWNER, port.testimony(), graph.testimony())
        .unwrap();

    router.begin_root_association_commit_guarded(&guard, &mut association);
    resolver.commit_focus_domain_graph_mutation(guard.focus_capability(), graph);
    let port = NavigationRootFocusBindingPort::commit_create_guarded(&guard, port);
    router.apply_root_association_commit_guarded(&guard, &mut association, port.clone());
    router.finish_root_association_commit_guarded(&guard, association);
    port
}

#[test]
fn association_is_atomic_claimed_and_routable_only_after_publication() {
    let (mut interaction_domains, coordinator, router) = setup();
    let port = admit_root(&mut interaction_domains, &coordinator, &router);

    assert_eq!(
        router.route_snapshot(InteractionDomainId::PRIMARY, None),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    router.prime_root_guarded(&guard, ROOT).unwrap();
    drop(guard);

    let RoutedNavigationCandidate::Snapshot(snapshot) =
        router.route_snapshot(InteractionDomainId::PRIMARY, None)
    else {
        panic!("primed root must produce one coherent routing snapshot");
    };
    assert_eq!(snapshot.root_id, ROOT);
    assert_eq!(snapshot.overlay_owner_id, OWNER);
    assert_eq!(snapshot.focus_binding.focus_binding_port_id, PORT);
    assert_eq!(
        snapshot.focus_binding.navigation_surface_id,
        initial_surface()
    );
    assert_eq!(snapshot.focus_binding.focus_domain_id, ROOT_FOCUS);
    assert_eq!(
        snapshot.focus_binding.owned_focus_domains,
        [ROOT_FOCUS, CHILD_FOCUS].into_iter().collect()
    );
    assert_eq!(
        port.candidate_identity(),
        crate::core::navigation_domain_router::NavigationRootFocusBindingCandidate::Stable(
            crate::core::navigation_domain_router::NavigationRootFocusBindingCandidateIdentity {
                focus_binding_port_id: PORT,
                generation: snapshot.focus_binding.generation,
            }
        )
    );
}

#[test]
fn binding_port_rejects_partial_subtree_and_foreign_coordinator_guard() {
    let (mut interaction_domains, coordinator, _) = setup();
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let resolver = FocusResolver::new();
    let parent = resolver
        .prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            &mut interaction_domains.primary_state_mut().focus_manager,
            &[FocusDomainAddition::new(ROOT_FOCUS, None)],
            &[],
            vec![],
            Some(ROOT_FOCUS),
        )
        .unwrap();
    resolver.commit_focus_domain_graph_mutation(guard.focus_capability(), parent);
    let child = resolver
        .prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            &mut interaction_domains.primary_state_mut().focus_manager,
            &[FocusDomainAddition::new(CHILD_FOCUS, Some(ROOT_FOCUS))],
            &[],
            vec![],
            None,
        )
        .unwrap();
    assert_eq!(
        NavigationRootFocusBindingPort::prepare_create_guarded(
            &guard,
            ROOT,
            PORT,
            initial_surface(),
            CHILD_FOCUS,
            child.testimony(),
        )
        .unwrap_err(),
        NavigationRootFocusBindingError::IncompleteOwnedFocusTree(CHILD_FOCUS)
    );
    drop(child);
    drop(guard);

    let (_, coordinator, router) = setup();
    let mut fresh_domains = InteractionDomains::new();
    let port = admit_root(&mut fresh_domains, &coordinator, &router);
    let foreign = OverlayDomainCoordinator::new().unwrap();
    foreign
        .register_domain(&fresh_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let foreign_guard = foreign
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert_eq!(
        port.snapshot_guarded(&foreign_guard),
        Err(NavigationRootFocusBindingError::GuardMismatch)
    );
}

#[test]
fn batch_membership_change_commits_focus_port_and_claims_as_one_generation() {
    let (mut interaction_domains, coordinator, router) = setup();
    let port = admit_root(&mut interaction_domains, &coordinator, &router);
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let before = port.snapshot_guarded(&guard).unwrap();
    let RoutedNavigationCandidate::Snapshot(routing_before) =
        router.route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    else {
        panic!("resident scoped root must route before membership change");
    };
    let resolver = FocusResolver::new();
    let graph = resolver
        .prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            &mut interaction_domains.primary_state_mut().focus_manager,
            &[
                FocusDomainAddition::new(PEER_ROOT_FOCUS, None),
                FocusDomainAddition::new(PEER_CHILD_FOCUS, Some(PEER_ROOT_FOCUS)),
            ],
            &[],
            vec![],
            None,
        )
        .unwrap();
    let port_change = port
        .prepare_owned_domain_change_guarded(&guard, before.generation, graph.testimony(), None)
        .unwrap();
    let mut router_change = router
        .prepare_root_focus_membership_change_guarded(
            &guard,
            ROOT,
            routing_before.router_generation,
            port_change.testimony(),
            graph.testimony(),
        )
        .unwrap();

    router.begin_root_focus_membership_commit_guarded(&guard, &mut router_change);
    resolver.commit_focus_domain_graph_mutation(guard.focus_capability(), graph);
    port.commit_owned_domain_change_guarded(&guard, port_change);
    router.apply_root_focus_membership_commit_guarded(&guard, &mut router_change);
    router.finish_root_focus_membership_commit_guarded(&guard, router_change);
    drop(guard);

    let RoutedNavigationCandidate::Snapshot(after) =
        router.route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    else {
        panic!("membership commit must restore stable routing publication");
    };
    assert!(after.router_generation > routing_before.router_generation);
    assert!(after.focus_binding.generation > before.generation);
    assert_eq!(
        after.focus_binding.owned_focus_domains,
        [ROOT_FOCUS, CHILD_FOCUS, PEER_ROOT_FOCUS, PEER_CHILD_FOCUS]
            .into_iter()
            .collect()
    );
}

#[test]
fn eligibility_exclusively_leases_router_and_binding_generation() {
    let (mut interaction_domains, coordinator, router) = setup();
    admit_root(&mut interaction_domains, &coordinator, &router);
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let eligibility = router
        .overlay_activation_eligibility_guarded(&guard, OWNER)
        .unwrap();
    assert_eq!(
        router.prime_root_guarded(&guard, ROOT),
        Err(NavigationDomainRouterError::Busy)
    );
    drop(eligibility);
    router.prime_root_guarded(&guard, ROOT).unwrap();
}

#[test]
fn retirement_clears_focus_claims_routing_owner_and_association_in_order() {
    let (mut interaction_domains, coordinator, router) = setup();
    let port = admit_root(&mut interaction_domains, &coordinator, &router);
    let RoutedNavigationCandidate::Snapshot(routing_before) =
        router.route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    else {
        panic!("resident root must route before retirement");
    };
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let delegation_plan = router
        .root_delegation_retirement_plan_guarded(&guard, ROOT, None)
        .unwrap();
    let resolver = FocusResolver::new();
    let graph = resolver
        .prepare_focus_domain_retirement(
            guard.focus_capability(),
            &mut interaction_domains.primary_state_mut().focus_manager,
            &[ROOT_FOCUS, CHILD_FOCUS],
            vec![],
            None,
        )
        .unwrap();
    let port_retirement = port
        .prepare_retire_guarded(&guard, graph.testimony())
        .unwrap();
    let mut router_retirement = router
        .prepare_root_retirement_guarded(
            &guard,
            ROOT,
            routing_before.router_generation,
            port_retirement.testimony(),
            graph.testimony(),
            &delegation_plan,
        )
        .unwrap();
    let owner_retirement = coordinator
        .prepare_owner_unregistration(&guard, OWNER)
        .unwrap();

    router.begin_root_retirement_commit_guarded(&guard, &mut router_retirement);
    resolver.commit_focus_domain_retirement(guard.focus_capability(), graph);
    port.commit_retire_guarded(&guard, port_retirement);
    router.apply_root_retirement_claims_guarded(&guard, &mut router_retirement);
    router.apply_root_routing_retirement_guarded(&guard, &mut router_retirement);
    coordinator.commit_owner_unregistration(&guard, owner_retirement);
    router.remove_root_association_guarded(&guard, &mut router_retirement);
    router.finish_root_retirement_commit_guarded(&guard, router_retirement);
    drop(guard);

    assert_eq!(
        router.route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT)),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        0
    );
}

#[test]
fn domain_retirement_accepts_complete_focus_graph_with_unclaimed_domain_local_tree() {
    let (mut interaction_domains, coordinator, router) = setup();
    let port = admit_root(&mut interaction_domains, &coordinator, &router);
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let resolver = FocusResolver::new();
    let domain_local_graph = resolver
        .prepare_focus_domain_graph_mutation(
            guard.focus_capability(),
            &mut interaction_domains.primary_state_mut().focus_manager,
            &[FocusDomainAddition::new(DOMAIN_LOCAL_FOCUS, None)],
            &[],
            vec![],
            None,
        )
        .unwrap();
    resolver.commit_focus_domain_graph_mutation(guard.focus_capability(), domain_local_graph);

    let snapshot = router.domain_retirement_snapshot_guarded(&guard).unwrap();
    let focus_retirement = resolver
        .prepare_complete_focus_domain_retirement(
            guard.focus_capability(),
            &interaction_domains.primary_state().focus_manager,
            &[ROOT_FOCUS, CHILD_FOCUS, DOMAIN_LOCAL_FOCUS],
            &[],
        )
        .unwrap();
    let port_retirement = port
        .prepare_retire_for_domain_guarded(&guard, focus_retirement.testimony())
        .unwrap();
    let mut router_retirement = router
        .prepare_domain_retirement_guarded(
            &guard,
            snapshot.router_generation(),
            &[port_retirement.testimony()],
            focus_retirement.testimony(),
        )
        .unwrap();
    let owner_retirement = coordinator
        .prepare_owner_unregistration(&guard, OWNER)
        .unwrap();

    router.begin_domain_retirement_commit_guarded(&guard, &mut router_retirement);
    resolver.commit_complete_focus_domain_retirement(
        guard.focus_capability(),
        &mut interaction_domains.primary_state_mut().focus_manager,
        focus_retirement,
    );
    port.commit_retire_guarded(&guard, port_retirement);
    router.apply_domain_retirement_claims_guarded(&guard, &mut router_retirement);
    router.apply_domain_routing_retirement_guarded(&guard, &mut router_retirement);
    coordinator.commit_owner_unregistration(&guard, owner_retirement);
    router.remove_domain_root_associations_guarded(&guard, &mut router_retirement);
    router.finish_domain_retirement_commit_guarded(&guard, router_retirement);
    drop(guard);

    assert_eq!(
        router.route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT)),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        0
    );
}
