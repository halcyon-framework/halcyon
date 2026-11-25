use halcyon::compositor::{Compositor, SceneKey, SurfaceVisibility};
use halcyon::core::event::{InteractionDomainId, InteractionDomains};
use halcyon::core::focus_authority::{FocusDomainAddition, FocusDomainId};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::{
    NavigationApplicationCoordinator, RoutedNavigationSnapshot, RoutedNavigationSnapshotResult,
};
use halcyon::core::navigation_domain_router::{
    NavigationRootFocusBindingPortId, RoutedNavigationCandidate,
};
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
    SurfaceTerminationReason,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayDisposition, OverlayDomainCoordinator, OverlayDomainError, OverlayFrameDescriptor,
    OverlayFrameId, OverlayOwnerId, OverlayOwnerLifecyclePort, OverlayOwnerLifecycleState,
    OverlayStackDescriptor, OverlayStackId,
};
use halcyon::model::navigation_host::{
    NavigationAuthorizationResult, NavigationGuardAuthorization, NavigationHost, NavigationIntent,
    NavigationNodeId,
};

const ROOT: NavigationRootId = NavigationRootId(1);
const OWNER: OverlayOwnerId = OverlayOwnerId(2);
const PORT: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(3);
const BASE_FOCUS: FocusDomainId = FocusDomainId(10);
const OVERLAY_FOCUS: FocusDomainId = FocusDomainId(11);
const NEW_FOCUS: FocusDomainId = FocusDomainId(12);
const STACK: OverlayStackId = OverlayStackId(20);
const PEER_ROOT: NavigationRootId = NavigationRootId(101);
const PEER_OWNER: OverlayOwnerId = OverlayOwnerId(102);
const PEER_PORT: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(103);
const PEER_BASE_FOCUS: FocusDomainId = FocusDomainId(110);
const PEER_OVERLAY_FOCUS: FocusDomainId = FocusDomainId(111);
const PEER_STACK: OverlayStackId = OverlayStackId(120);

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

struct Setup {
    interaction_domains: InteractionDomains,
    application: NavigationApplicationCoordinator,
    registry: NavigationRootRegistry,
    compositor: Compositor,
    owner_port: OverlayOwnerLifecyclePort,
    overlay_surface: halcyon::compositor::SurfaceId,
}

fn setup_active_overlay() -> Setup {
    let mut interaction_domains = InteractionDomains::new();
    let overlay_domains = OverlayDomainCoordinator::new().unwrap();
    overlay_domains
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let application = NavigationApplicationCoordinator::new(overlay_domains);
    application
        .navigation_domains()
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let mut registry = NavigationRootRegistry::new();
    let mut compositor = Compositor::new(viewport());
    let owner_port =
        OverlayOwnerLifecyclePort::new(OWNER, InteractionDomainId::PRIMARY, vec![]).unwrap();
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
            owner_port.clone(),
            PORT,
            &[
                FocusDomainAddition::new(BASE_FOCUS, None),
                FocusDomainAddition::new(OVERLAY_FOCUS, Some(BASE_FOCUS)),
            ],
            BASE_FOCUS,
        )
        .unwrap()
        .commit();

    let routed = match application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected admitted route, got {result:?}"),
    };
    let overlay_surface = compositor
        .create_overlay_surface(
            routed.focus_binding.navigation_surface_id,
            100,
            viewport(),
            SceneKey::new(100),
        )
        .unwrap();
    owner_port
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(21),
                OVERLAY_FOCUS,
                overlay_surface,
            )],
        ))
        .unwrap();

    let guard = application
        .overlay_domains()
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let eligibility = application
        .navigation_domains()
        .overlay_activation_eligibility_guarded(&guard, OWNER)
        .unwrap();
    let prepared = application
        .overlay_domains()
        .prepare_activation(
            &guard,
            &mut interaction_domains,
            OWNER,
            STACK,
            Some(eligibility),
        )
        .unwrap();
    application
        .overlay_domains()
        .commit_activation(&guard, prepared);
    drop(guard);

    Setup {
        interaction_domains,
        application,
        registry,
        compositor,
        owner_port,
        overlay_surface,
    }
}

fn snapshot_and_authorize(
    application: &NavigationApplicationCoordinator,
    registry: &NavigationRootRegistry,
) -> (RoutedNavigationSnapshot, NavigationGuardAuthorization) {
    let snapshot =
        match application.route_snapshot(registry, InteractionDomainId::PRIMARY, Some(ROOT)) {
            RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            result => panic!("expected routed overlay snapshot, got {result:?}"),
        };
    let authorization = match application.authorize_routed_navigation(
        registry,
        &snapshot,
        NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
        None,
        None,
    ) {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        result => panic!("expected routed overlay authorization, got {result:?}"),
    };
    (snapshot, authorization)
}

#[test]
fn routed_overlay_owner_catalog_rejects_surface_aliases_across_inactive_stacks() {
    let mut compositor = Compositor::new(viewport());
    let base_surface = compositor.create_surface(0, viewport(), SceneKey::BASE);
    let overlay_surface = compositor
        .create_overlay_surface(base_surface, 100, viewport(), SceneKey::new(100))
        .unwrap();
    let stack = |stack_id, frame_id, focus_domain| {
        OverlayStackDescriptor::new(
            stack_id,
            vec![OverlayFrameDescriptor::new(
                frame_id,
                focus_domain,
                overlay_surface,
            )],
        )
    };

    assert!(matches!(
        OverlayOwnerLifecyclePort::new(
            OWNER,
            InteractionDomainId::PRIMARY,
            vec![
                stack(STACK, OverlayFrameId(21), OVERLAY_FOCUS),
                stack(OverlayStackId(30), OverlayFrameId(31), NEW_FOCUS),
            ],
        ),
        Err(OverlayDomainError::DuplicateSurface(surface)) if surface == overlay_surface
    ));

    let owner_port = OverlayOwnerLifecyclePort::new(
        OWNER,
        InteractionDomainId::PRIMARY,
        vec![stack(STACK, OverlayFrameId(21), OVERLAY_FOCUS)],
    )
    .unwrap();
    assert!(matches!(
        owner_port.register_inactive_stack(stack(
            OverlayStackId(30),
            OverlayFrameId(31),
            NEW_FOCUS,
        )),
        Err(OverlayDomainError::DuplicateSurface(surface)) if surface == overlay_surface
    ));
}

#[test]
fn routed_overlay_exit_dismisses_owner_focus_binding_history_and_surface_atomically() {
    let mut setup = setup_active_overlay();
    let (snapshot, authorization) = snapshot_and_authorize(&setup.application, &setup.registry);
    let prepared = setup
        .application
        .prepare_routed_navigation_with_overlay_exit(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &snapshot,
            authorization,
            None,
            OverlayDisposition::Dismiss,
            BASE_FOCUS,
        )
        .unwrap();
    let next_surface = prepared.next_active_surface();

    let committed = prepared.commit();

    assert_eq!(committed.active_surface(), Some(next_surface));
    assert!(committed.terminated_surfaces().contains(&(
        setup.overlay_surface,
        SurfaceTerminationReason::OverlayClosed
    )));
    assert!(!setup.compositor.has_surface(setup.overlay_surface));
    assert_eq!(setup.compositor.surface_count(), 2);
    assert_eq!(
        setup.owner_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Inactive
    );
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(BASE_FOCUS)
    );
    let guard = setup
        .application
        .overlay_domains()
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert_eq!(
        setup
            .application
            .overlay_domains()
            .active_binding_guarded(&guard)
            .unwrap(),
        None
    );
    drop(guard);
    assert_eq!(
        setup
            .registry
            .root(ROOT)
            .unwrap()
            .nav_host()
            .current_entry()
            .node_id(),
        NavigationNodeId(2)
    );
    let replacement_overlay = setup
        .compositor
        .create_overlay_surface(next_surface, 101, viewport(), SceneKey::new(101))
        .unwrap();
    setup
        .owner_port
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(22),
                OVERLAY_FOCUS,
                replacement_overlay,
            )],
        ))
        .unwrap();
}

#[test]
fn routed_overlay_exit_drop_restores_owner_tokens_and_preserves_overlay_surface() {
    let mut setup = setup_active_overlay();
    let route_before = setup
        .application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT));
    let (snapshot, authorization) = snapshot_and_authorize(&setup.application, &setup.registry);
    let prepared = setup
        .application
        .prepare_routed_navigation_with_overlay_exit(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &snapshot,
            authorization,
            None,
            OverlayDisposition::Dismiss,
            BASE_FOCUS,
        )
        .unwrap();
    let staged_surface = prepared.next_active_surface();

    drop(prepared);

    assert!(!setup.compositor.has_surface(staged_surface));
    assert!(setup.compositor.has_surface(setup.overlay_surface));
    assert_eq!(
        setup.compositor.surface_visibility(setup.overlay_surface),
        Some(SurfaceVisibility::Visible)
    );
    assert_eq!(setup.compositor.surface_count(), 2);
    assert_eq!(
        setup.owner_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active { stack_id: STACK }
    );
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(OVERLAY_FOCUS)
    );
    assert_eq!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT)),
        route_before
    );
}

#[test]
fn routed_overlay_suspend_hides_owner_surface_without_destroying_it() {
    let mut setup = setup_active_overlay();
    let (snapshot, authorization) = snapshot_and_authorize(&setup.application, &setup.registry);
    let prepared = setup
        .application
        .prepare_routed_navigation_with_overlay_exit(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &snapshot,
            authorization,
            None,
            OverlayDisposition::Suspend,
            BASE_FOCUS,
        )
        .unwrap();

    let committed = prepared.commit();

    assert!(
        !committed
            .terminated_surfaces()
            .iter()
            .any(|(surface, _)| *surface == setup.overlay_surface)
    );
    assert!(setup.compositor.has_surface(setup.overlay_surface));
    assert_eq!(
        setup.compositor.surface_visibility(setup.overlay_surface),
        Some(SurfaceVisibility::Hidden)
    );
    assert_eq!(setup.compositor.surface_count(), 3);
    assert_eq!(
        setup.owner_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Inactive
    );
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(BASE_FOCUS)
    );
    assert!(matches!(
        setup
            .owner_port
            .register_inactive_stack(OverlayStackDescriptor::new(
                STACK,
                vec![OverlayFrameDescriptor::new(
                    OverlayFrameId(22),
                    OVERLAY_FOCUS,
                    setup.overlay_surface,
                )],
            )),
        Err(OverlayDomainError::DuplicateStack(stack_id)) if stack_id == STACK
    ));
}

#[test]
fn routed_overlay_focus_graph_exit_replaces_membership_without_intermediate_restoration() {
    let mut setup = setup_active_overlay();
    let (snapshot, authorization) = snapshot_and_authorize(&setup.application, &setup.registry);
    let prepared = setup
        .application
        .prepare_routed_navigation_with_overlay_and_focus_graph(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &snapshot,
            authorization,
            None,
            OverlayDisposition::Dismiss,
            &[FocusDomainAddition::new(NEW_FOCUS, Some(BASE_FOCUS))],
            &[OVERLAY_FOCUS],
            Some(BASE_FOCUS),
            BASE_FOCUS,
        )
        .unwrap();
    let next_surface = prepared.next_active_surface();

    prepared.commit();

    assert!(!setup.compositor.has_surface(setup.overlay_surface));
    let focus = &setup.interaction_domains.primary_state().focus_manager;
    assert_eq!(focus.active_focus_domain(), Some(BASE_FOCUS));
    assert_eq!(focus.focus_domain_count(), 2);
    assert_eq!(focus.focus_domain_parent(OVERLAY_FOCUS), None);
    assert_eq!(focus.focus_domain_parent(NEW_FOCUS), Some(Some(BASE_FOCUS)));
    let routed = match setup
        .application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected routed overlay graph result, got {result:?}"),
    };
    assert_eq!(routed.focus_binding.navigation_surface_id, next_surface);
    assert_eq!(routed.focus_binding.focus_domain_id, BASE_FOCUS);
    assert!(
        routed
            .focus_binding
            .owned_focus_domains
            .contains(&BASE_FOCUS)
    );
    assert!(
        routed
            .focus_binding
            .owned_focus_domains
            .contains(&NEW_FOCUS)
    );
    assert!(
        !routed
            .focus_binding
            .owned_focus_domains
            .contains(&OVERLAY_FOCUS)
    );
}

#[test]
fn routed_overlay_focus_graph_drop_restores_owner_graph_claims_and_surface() {
    let mut setup = setup_active_overlay();
    let route_before = setup
        .application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT));
    let (snapshot, authorization) = snapshot_and_authorize(&setup.application, &setup.registry);
    let prepared = setup
        .application
        .prepare_routed_navigation_with_overlay_and_focus_graph(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &snapshot,
            authorization,
            None,
            OverlayDisposition::Dismiss,
            &[FocusDomainAddition::new(NEW_FOCUS, Some(BASE_FOCUS))],
            &[OVERLAY_FOCUS],
            Some(BASE_FOCUS),
            BASE_FOCUS,
        )
        .unwrap();
    let staged_surface = prepared.next_active_surface();

    drop(prepared);

    assert!(!setup.compositor.has_surface(staged_surface));
    assert!(setup.compositor.has_surface(setup.overlay_surface));
    assert_eq!(
        setup.compositor.surface_visibility(setup.overlay_surface),
        Some(SurfaceVisibility::Visible)
    );
    assert_eq!(
        setup.owner_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active { stack_id: STACK }
    );
    let focus = &setup.interaction_domains.primary_state().focus_manager;
    assert_eq!(focus.active_focus_domain(), Some(OVERLAY_FOCUS));
    assert_eq!(focus.focus_domain_count(), 2);
    assert_eq!(
        focus.focus_domain_parent(OVERLAY_FOCUS),
        Some(Some(BASE_FOCUS))
    );
    assert_eq!(focus.focus_domain_parent(NEW_FOCUS), None);
    assert_eq!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT)),
        route_before
    );
}

#[test]
fn routed_overlay_exit_rejects_active_peer_without_consuming_or_rewriting_it() {
    let mut interaction_domains = InteractionDomains::new();
    let overlay_domains = OverlayDomainCoordinator::new().unwrap();
    overlay_domains
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let application = NavigationApplicationCoordinator::new(overlay_domains);
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
            &[FocusDomainAddition::new(BASE_FOCUS, None)],
            BASE_FOCUS,
        )
        .unwrap()
        .commit();
    let peer_owner_port =
        OverlayOwnerLifecyclePort::new(PEER_OWNER, InteractionDomainId::PRIMARY, vec![]).unwrap();
    application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            PEER_ROOT,
            NavigationHost::new(NavigationNodeId(101)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            peer_owner_port.clone(),
            PEER_PORT,
            &[
                FocusDomainAddition::new(PEER_BASE_FOCUS, None),
                FocusDomainAddition::new(PEER_OVERLAY_FOCUS, Some(PEER_BASE_FOCUS)),
            ],
            PEER_BASE_FOCUS,
        )
        .unwrap()
        .commit();

    let peer_route = match application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(PEER_ROOT))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected peer route, got {result:?}"),
    };
    let peer_overlay_surface = compositor
        .create_overlay_surface(
            peer_route.focus_binding.navigation_surface_id,
            1100,
            viewport(),
            SceneKey::new(1100),
        )
        .unwrap();
    peer_owner_port
        .register_inactive_stack(OverlayStackDescriptor::new(
            PEER_STACK,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(121),
                PEER_OVERLAY_FOCUS,
                peer_overlay_surface,
            )],
        ))
        .unwrap();
    let guard = application
        .overlay_domains()
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let eligibility = application
        .navigation_domains()
        .overlay_activation_eligibility_guarded(&guard, PEER_OWNER)
        .unwrap();
    let activation = application
        .overlay_domains()
        .prepare_activation(
            &guard,
            &mut interaction_domains,
            PEER_OWNER,
            PEER_STACK,
            Some(eligibility),
        )
        .unwrap();
    application
        .overlay_domains()
        .commit_activation(&guard, activation);
    drop(guard);

    let route_before = application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT));
    let (snapshot, authorization) = snapshot_and_authorize(&application, &registry);
    let error = application
        .prepare_routed_navigation_with_overlay_exit(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            OverlayDisposition::Dismiss,
            BASE_FOCUS,
        )
        .map(drop)
        .expect_err("a routed root cannot unwind its peer's active overlay");
    assert!(matches!(
        error,
        halcyon::core::navigation_application::RoutedNavigationPreparationError::ActivePeerOverlay(
            binding
        ) if binding.owner_id == PEER_OWNER && binding.stack_id == PEER_STACK
    ));
    assert!(compositor.has_surface(peer_overlay_surface));
    assert_eq!(
        compositor.surface_visibility(peer_overlay_surface),
        Some(SurfaceVisibility::Visible)
    );
    assert_eq!(
        peer_owner_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active {
            stack_id: PEER_STACK
        }
    );
    assert_eq!(
        interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(PEER_OVERLAY_FOCUS)
    );
    assert_eq!(
        application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT)),
        route_before
    );
}
