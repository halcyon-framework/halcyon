use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use halcyon::compositor::Compositor;
use halcyon::core::event::{InteractionDomainId, InteractionDomains};
use halcyon::core::focus_authority::{FocusDomainAddition, FocusDomainId};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::{
    NavigationApplicationCoordinator, RoutedNavigationPreparationError,
    RoutedNavigationSnapshotResult,
};
use halcyon::core::navigation_domain_router::{
    NavigationDomainRouterError, NavigationRootFocusBindingPortId, RoutedNavigationCandidate,
};
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayDomainCoordinator, OverlayOwnerId, OverlayOwnerLifecyclePort,
};
use halcyon::model::navigation_host::{
    GuardDecision, NavigationAuthorizationResult, NavigationGuard,
    NavigationGuardContextGeneration, NavigationHost, NavigationIntent, NavigationNodeId,
    NavigationOutcome, NavigationState,
};

const ROOT: NavigationRootId = NavigationRootId(1);
const OWNER: OverlayOwnerId = OverlayOwnerId(2);
const PORT: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(3);
const FOCUS: FocusDomainId = FocusDomainId(4);

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
            NavigationHost::new(NavigationNodeId(10)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(OWNER, InteractionDomainId::PRIMARY, vec![]).unwrap(),
            PORT,
            &[FocusDomainAddition::new(FOCUS, None)],
            FOCUS,
        )
        .unwrap()
        .commit();
    (interaction_domains, application, registry, compositor)
}

struct GuardMustRunOutsideDomainToken {
    coordinator: OverlayDomainCoordinator,
    called: Rc<Cell<bool>>,
}

impl NavigationGuard for GuardMustRunOutsideDomainToken {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        _intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        let token = self
            .coordinator
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .expect("application guard must execute before the domain token is acquired");
        drop(token);
        self.called.set(true);
        GuardDecision::Allow
    }
}

fn authorize(
    application: &NavigationApplicationCoordinator,
    registry: &NavigationRootRegistry,
    snapshot: &halcyon::core::navigation_application::RoutedNavigationSnapshot,
    target: NavigationNodeId,
    guard_context_generation: Option<NavigationGuardContextGeneration>,
) -> halcyon::model::navigation_host::NavigationGuardAuthorization {
    match application.authorize_routed_navigation(
        registry,
        snapshot,
        NavigationIntent::NavigateTo(target.into()),
        None,
        guard_context_generation,
    ) {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        result => panic!("expected routed authorization, got {result:?}"),
    }
}

#[test]
fn navigation_routed_transaction_runs_guard_outside_token_and_publishes_atomically() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let guard_called = Rc::new(Cell::new(false));
    registry
        .root_mut(ROOT)
        .unwrap()
        .nav_host_mut()
        .set_guard(Box::new(GuardMustRunOutsideDomainToken {
            coordinator: application.overlay_domains().clone(),
            called: guard_called.clone(),
        }));
    let snapshot =
        match application.route_snapshot(&registry, InteractionDomainId::PRIMARY, Some(ROOT)) {
            RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            result => panic!("expected routed snapshot, got {result:?}"),
        };
    let router_before = match application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected router snapshot, got {result:?}"),
    };
    let authorization = authorize(
        &application,
        &registry,
        &snapshot,
        NavigationNodeId(11),
        None,
    );
    assert!(guard_called.get());

    let prepared = application
        .prepare_routed_navigation(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            FOCUS,
        )
        .unwrap();
    assert_eq!(prepared.staged_surfaces().len(), 1);
    let next_surface = prepared.next_active_surface();
    assert!(
        application
            .overlay_domains()
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .is_err()
    );

    let committed = prepared.commit();
    assert_eq!(
        committed.outcome,
        NavigationOutcome::Committed { redirected: false }
    );
    assert_eq!(committed.active_surface(), Some(next_surface));
    assert_eq!(
        registry
            .root(ROOT)
            .unwrap()
            .nav_host()
            .current_entry()
            .node_id(),
        NavigationNodeId(11)
    );
    assert_eq!(compositor.surface_count(), 2);
    assert_eq!(
        compositor.surface_visibility(next_surface),
        Some(halcyon::compositor::SurfaceVisibility::Visible)
    );
    let router_after = match application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected router snapshot, got {result:?}"),
    };
    assert_ne!(
        router_after.router_generation,
        router_before.router_generation
    );
    assert_ne!(
        router_after.focus_binding.generation,
        router_before.focus_binding.generation
    );
    assert_eq!(
        router_after.focus_binding.navigation_surface_id,
        next_surface
    );
    assert_eq!(router_after.focus_binding.focus_domain_id, FOCUS);
}

#[test]
fn navigation_routed_transaction_drop_aborts_every_prepared_participant() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let snapshot =
        match application.route_snapshot(&registry, InteractionDomainId::PRIMARY, Some(ROOT)) {
            RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            result => panic!("expected routed snapshot, got {result:?}"),
        };
    let router_before = application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT));
    let authorization = authorize(
        &application,
        &registry,
        &snapshot,
        NavigationNodeId(11),
        None,
    );
    let prepared = application
        .prepare_routed_navigation(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            FOCUS,
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
        NavigationNodeId(10)
    );
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

#[test]
fn navigation_routed_transaction_rejects_stale_router_before_host_or_surface_preparation() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let snapshot =
        match application.route_snapshot(&registry, InteractionDomainId::PRIMARY, Some(ROOT)) {
            RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            result => panic!("expected routed snapshot, got {result:?}"),
        };
    let authorization = authorize(
        &application,
        &registry,
        &snapshot,
        NavigationNodeId(11),
        None,
    );
    let guard = application
        .overlay_domains()
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    application
        .navigation_domains()
        .prime_root_guarded(&guard, ROOT)
        .unwrap();
    drop(guard);

    let error = application
        .prepare_routed_navigation(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            FOCUS,
        )
        .map(drop)
        .expect_err("stale router snapshot must reject before preparation");
    assert!(matches!(
        error,
        RoutedNavigationPreparationError::Router(NavigationDomainRouterError::StaleGeneration)
    ));
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(
        registry
            .root(ROOT)
            .unwrap()
            .nav_host()
            .current_entry()
            .node_id(),
        NavigationNodeId(10)
    );
}

#[test]
fn navigation_routed_transaction_rejects_stale_guard_context_without_mutation() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let snapshot =
        match application.route_snapshot(&registry, InteractionDomainId::PRIMARY, Some(ROOT)) {
            RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            result => panic!("expected routed snapshot, got {result:?}"),
        };
    let authorization = authorize(
        &application,
        &registry,
        &snapshot,
        NavigationNodeId(11),
        Some(NavigationGuardContextGeneration::new(1)),
    );
    let error = application
        .prepare_routed_navigation(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            Some(NavigationGuardContextGeneration::new(2)),
            FOCUS,
        )
        .map(drop)
        .expect_err("stale guard context must reject before surface preparation");
    assert!(matches!(
        error,
        RoutedNavigationPreparationError::Host(
            halcyon::model::navigation_host::NavigationPreparationError::StaleGuardContext
        )
    ));
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(
        registry
            .root(ROOT)
            .unwrap()
            .nav_host()
            .current_entry()
            .node_id(),
        NavigationNodeId(10)
    );
}

#[test]
fn navigation_routed_transaction_unscoped_route_uses_the_primed_root() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let guard = application
        .overlay_domains()
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    application
        .navigation_domains()
        .prime_root_guarded(&guard, ROOT)
        .unwrap();
    drop(guard);
    let snapshot = match application.route_snapshot(&registry, InteractionDomainId::PRIMARY, None) {
        RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
        result => panic!("expected unscoped routed snapshot, got {result:?}"),
    };
    let authorization = authorize(
        &application,
        &registry,
        &snapshot,
        NavigationNodeId(12),
        None,
    );

    application
        .prepare_routed_navigation(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            FOCUS,
        )
        .unwrap()
        .commit();

    assert_eq!(
        registry
            .root(ROOT)
            .unwrap()
            .nav_host()
            .current_entry()
            .node_id(),
        NavigationNodeId(12)
    );
}
