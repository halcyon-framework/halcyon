use halcyon::compositor::Compositor;
use halcyon::core::event::{InteractionDomainId, InteractionDomains};
use halcyon::core::focus_authority::{FocusDomainAddition, FocusDomainId};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::{
    NavigationApplicationCoordinator, RoutedNavigationSnapshotResult,
};
use halcyon::core::navigation_domain_router::{
    NavigationRootFocusBindingPortId, RoutedNavigationCandidate,
};
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayDomainCoordinator, OverlayOwnerId, OverlayOwnerLifecyclePort,
};
use halcyon::model::navigation_host::{
    NavigationAuthorizationResult, NavigationHost, NavigationIntent, NavigationNodeId,
};

const ROOT: NavigationRootId = NavigationRootId(1);
const OWNER: OverlayOwnerId = OverlayOwnerId(2);
const PORT: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(3);
const OLD_PARENT: FocusDomainId = FocusDomainId(10);
const OLD_CHILD: FocusDomainId = FocusDomainId(11);
const NEW_PARENT: FocusDomainId = FocusDomainId(20);
const NEW_CHILD: FocusDomainId = FocusDomainId(21);

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn setup() -> (
    InteractionDomains,
    NavigationApplicationCoordinator,
    NavigationRootRegistry,
    Compositor,
) {
    let mut interaction_domains = InteractionDomains::new();
    let overlay = OverlayDomainCoordinator::new().unwrap();
    overlay
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let application = NavigationApplicationCoordinator::new(overlay);
    application
        .navigation_domains()
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let mut registry = NavigationRootRegistry::new();
    let mut compositor = Compositor::new(viewport());
    application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            ROOT,
            NavigationHost::new(NavigationNodeId(1)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(OWNER, InteractionDomainId::PRIMARY, vec![]).unwrap(),
            PORT,
            &[
                FocusDomainAddition::new(OLD_PARENT, None),
                FocusDomainAddition::new(OLD_CHILD, Some(OLD_PARENT)),
            ],
            OLD_PARENT,
        )
        .unwrap()
        .commit();
    (interaction_domains, application, registry, compositor)
}

fn snapshot_and_authorize(
    application: &NavigationApplicationCoordinator,
    registry: &NavigationRootRegistry,
) -> (
    halcyon::core::navigation_application::RoutedNavigationSnapshot,
    halcyon::model::navigation_host::NavigationGuardAuthorization,
) {
    let snapshot =
        match application.route_snapshot(registry, InteractionDomainId::PRIMARY, Some(ROOT)) {
            RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            result => panic!("expected routed graph snapshot, got {result:?}"),
        };
    let authorization = match application.authorize_routed_navigation(
        registry,
        &snapshot,
        NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
        None,
        None,
    ) {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        result => panic!("expected routed graph authorization, got {result:?}"),
    };
    (snapshot, authorization)
}

#[test]
fn navigation_routed_focus_graph_swaps_complete_membership_with_surface_and_history() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let (snapshot, authorization) = snapshot_and_authorize(&application, &registry);
    let prepared = application
        .prepare_routed_navigation_with_focus_graph(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            &[
                FocusDomainAddition::new(NEW_PARENT, None),
                FocusDomainAddition::new(NEW_CHILD, Some(NEW_PARENT)),
            ],
            &[OLD_PARENT, OLD_CHILD],
            Some(NEW_PARENT),
            NEW_PARENT,
        )
        .unwrap();
    assert_eq!(prepared.staged_surfaces().len(), 1);
    let next_surface = prepared.next_active_surface();

    prepared.commit();

    assert_eq!(
        registry
            .root(ROOT)
            .unwrap()
            .nav_host()
            .current_entry()
            .node_id(),
        NavigationNodeId(2)
    );
    assert_eq!(compositor.surface_count(), 2);
    let focus = &interaction_domains.primary_state().focus_manager;
    assert_eq!(focus.focus_domain_count(), 2);
    assert_eq!(focus.active_focus_domain(), Some(NEW_PARENT));
    let routed = match application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected post-graph route snapshot, got {result:?}"),
    };
    assert_eq!(routed.focus_binding.navigation_surface_id, next_surface);
    assert_eq!(routed.focus_binding.focus_domain_id, NEW_PARENT);
    assert_eq!(routed.focus_binding.owned_focus_domains.len(), 2);
    assert!(
        routed
            .focus_binding
            .owned_focus_domains
            .contains(&NEW_PARENT)
    );
    assert!(
        routed
            .focus_binding
            .owned_focus_domains
            .contains(&NEW_CHILD)
    );
    assert!(
        !routed
            .focus_binding
            .owned_focus_domains
            .contains(&OLD_PARENT)
    );
    assert!(
        !routed
            .focus_binding
            .owned_focus_domains
            .contains(&OLD_CHILD)
    );
}

#[test]
fn navigation_routed_focus_graph_drop_restores_old_graph_claims_host_and_surface() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let router_before = application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT));
    let (snapshot, authorization) = snapshot_and_authorize(&application, &registry);
    let prepared = application
        .prepare_routed_navigation_with_focus_graph(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            &[
                FocusDomainAddition::new(NEW_PARENT, None),
                FocusDomainAddition::new(NEW_CHILD, Some(NEW_PARENT)),
            ],
            &[OLD_PARENT, OLD_CHILD],
            Some(NEW_PARENT),
            NEW_PARENT,
        )
        .unwrap();
    let staged_surface = prepared.next_active_surface();

    drop(prepared);

    assert_eq!(compositor.surface_count(), 1);
    assert!(!compositor.has_surface(staged_surface));
    assert_eq!(
        registry
            .root(ROOT)
            .unwrap()
            .nav_host()
            .current_entry()
            .node_id(),
        NavigationNodeId(1)
    );
    let focus = &interaction_domains.primary_state().focus_manager;
    assert_eq!(focus.focus_domain_count(), 2);
    assert_eq!(focus.active_focus_domain(), Some(OLD_PARENT));
    assert_eq!(
        application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT)),
        router_before
    );
    assert!(
        application
            .overlay_domains()
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .is_ok()
    );
}
