use halcyon::compositor::{Compositor, SurfaceVisibility};
use halcyon::core::event::{InteractionDomainId, InteractionDomains};
use halcyon::core::focus_authority::{FocusDomainAddition, FocusDomainId};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::{
    NavigationApplicationCoordinator, NavigationRootAdmissionError,
};
use halcyon::core::navigation_domain_router::{
    NavigationDomainRouterError, NavigationRootFocusBindingError, NavigationRootFocusBindingPortId,
    RoutedNavigationCandidate,
};
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayDomainCoordinator, OverlayDomainError, OverlayOwnerId, OverlayOwnerLifecyclePort,
};
use halcyon::model::navigation_host::{
    NavigationDecision, NavigationEntry, NavigationHost, NavigationIntent, NavigationNodeId,
};

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn setup() -> (
    InteractionDomains,
    NavigationApplicationCoordinator,
    NavigationRootRegistry,
    Compositor,
) {
    let interaction_domains = InteractionDomains::new();
    let overlay = OverlayDomainCoordinator::new().unwrap();
    overlay
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let application = NavigationApplicationCoordinator::new(overlay);
    application
        .navigation_domains()
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    (
        interaction_domains,
        application,
        NavigationRootRegistry::new(),
        Compositor::new(viewport()),
    )
}

#[test]
fn navigation_root_admission_publishes_registry_router_owner_focus_and_surface_together() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let root = NavigationRootId(1);
    let owner = OverlayOwnerId(2);
    let port = NavigationRootFocusBindingPortId(3);
    let focus_root = FocusDomainId(4);
    let focus_child = FocusDomainId(5);
    let prepared = application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            root,
            NavigationHost::new(NavigationNodeId(10)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(owner, InteractionDomainId::PRIMARY, vec![]).unwrap(),
            port,
            &[
                FocusDomainAddition::new(focus_root, None),
                FocusDomainAddition::new(focus_child, Some(focus_root)),
            ],
            focus_root,
        )
        .unwrap();
    assert_eq!(
        prepared.prepared_surface_visibility(),
        Some(SurfaceVisibility::Hidden)
    );
    assert_eq!(
        application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(root)),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    assert!(matches!(
        application
            .overlay_domains()
            .begin_domain_transition(InteractionDomainId::PRIMARY),
        Err(OverlayDomainError::Busy(InteractionDomainId::PRIMARY))
    ));

    let committed = prepared.commit();
    assert_eq!(committed.root_id, root);
    assert_eq!(committed.focus_binding.port_id(), port);
    assert_eq!(registry.root_count(), 1);
    assert!(registry.root(root).is_some());
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        2
    );
    assert!(
        application
            .overlay_domains()
            .owner_registration_generation(owner)
            .is_ok()
    );
    assert!(matches!(
        application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(root)),
        RoutedNavigationCandidate::Snapshot(_)
    ));
    let direct = registry
        .dispatch(
            root,
            NavigationIntent::NavigateTo(NavigationEntry::new(NavigationNodeId(52))),
            &mut compositor,
        )
        .expect("admitted root remains resident");
    assert_eq!(direct.decision, NavigationDecision::Retry);
}

#[test]
fn navigation_root_admission_late_rejection_aborts_every_reservation() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let existing_parent = FocusDomainId(40);
    let child = FocusDomainId(41);
    application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            NavigationRootId(100),
            NavigationHost::new(NavigationNodeId(1000)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(
                OverlayOwnerId(200),
                InteractionDomainId::PRIMARY,
                vec![],
            )
            .unwrap(),
            NavigationRootFocusBindingPortId(300),
            &[FocusDomainAddition::new(existing_parent, None)],
            existing_parent,
        )
        .unwrap()
        .commit();

    let root = NavigationRootId(1);
    let owner = OverlayOwnerId(2);
    let result = application.prepare_root_admission(
        &mut interaction_domains,
        &mut registry,
        &mut compositor,
        InteractionDomainId::PRIMARY,
        root,
        NavigationHost::new(NavigationNodeId(10)),
        viewport(),
        Box::new(IncrementalSceneKeyAllocator::new()),
        OverlayOwnerLifecyclePort::new(owner, InteractionDomainId::PRIMARY, vec![]).unwrap(),
        NavigationRootFocusBindingPortId(3),
        &[FocusDomainAddition::new(child, Some(existing_parent))],
        child,
    );
    let error = result
        .map(drop)
        .expect_err("partial focus subtree admission must fail");
    assert_eq!(
        error,
        NavigationRootAdmissionError::FocusBinding(
            NavigationRootFocusBindingError::IncompleteOwnedFocusTree(child)
        )
    );
    assert_eq!(registry.root_count(), 1);
    assert_eq!(compositor.surface_count(), 1);
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        1
    );
    assert_eq!(
        application
            .overlay_domains()
            .owner_registration_generation(owner),
        Err(OverlayDomainError::UnknownOwner(owner))
    );
    assert!(
        application
            .overlay_domains()
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .is_ok()
    );
    assert_eq!(
        application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(root)),
        RoutedNavigationCandidate::RoutingUnavailable
    );
}

#[test]
fn navigation_root_admission_drop_aborts_hidden_state_and_releases_guard() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let root = NavigationRootId(1);
    let owner = OverlayOwnerId(2);
    let focus = FocusDomainId(4);
    let prepared = application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            root,
            NavigationHost::new(NavigationNodeId(10)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(owner, InteractionDomainId::PRIMARY, vec![]).unwrap(),
            NavigationRootFocusBindingPortId(3),
            &[FocusDomainAddition::new(focus, None)],
            focus,
        )
        .unwrap();
    let hidden_surface = prepared.prepared_surface();
    assert_eq!(
        prepared.prepared_surface_visibility(),
        Some(SurfaceVisibility::Hidden)
    );

    drop(prepared);

    assert_eq!(registry.root_count(), 0);
    assert_eq!(compositor.surface_count(), 0);
    assert!(!compositor.has_surface(hidden_surface));
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        0
    );
    assert_eq!(
        application
            .overlay_domains()
            .owner_registration_generation(owner),
        Err(OverlayDomainError::UnknownOwner(owner))
    );
    assert_eq!(
        application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(root)),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    assert!(
        application
            .overlay_domains()
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .is_ok()
    );
}

#[test]
fn navigation_root_admission_router_duplicate_preflight_precedes_surface_allocation() {
    let (mut interaction_domains, application, mut registry, mut compositor) = setup();
    let root = NavigationRootId(1);
    let first_focus = FocusDomainId(4);
    application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            root,
            NavigationHost::new(NavigationNodeId(10)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(OverlayOwnerId(2), InteractionDomainId::PRIMARY, vec![])
                .unwrap(),
            NavigationRootFocusBindingPortId(3),
            &[FocusDomainAddition::new(first_focus, None)],
            first_focus,
        )
        .unwrap()
        .commit();

    let mut unrelated_registry = NavigationRootRegistry::new();
    let second_focus = FocusDomainId(14);
    let surface_count_before = compositor.surface_count();
    let error = application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut unrelated_registry,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            root,
            NavigationHost::new(NavigationNodeId(20)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(
                OverlayOwnerId(12),
                InteractionDomainId::PRIMARY,
                vec![],
            )
            .unwrap(),
            NavigationRootFocusBindingPortId(13),
            &[FocusDomainAddition::new(second_focus, None)],
            second_focus,
        )
        .map(drop)
        .expect_err("router duplicate root must reject before registry allocation");

    assert_eq!(
        error,
        NavigationRootAdmissionError::Router(NavigationDomainRouterError::DuplicateRoot(root))
    );
    assert_eq!(unrelated_registry.root_count(), 0);
    assert_eq!(compositor.surface_count(), surface_count_before);
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        1
    );
}
