use halcyon::compositor::{Compositor, SceneKey, SurfaceVisibility};
use halcyon::core::event::{InteractionDomainId, InteractionDomains};
use halcyon::core::focus_authority::{FocusDomainAddition, FocusDomainId};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::{
    NavigationApplicationCoordinator, NavigationDelegationError,
};
use halcyon::core::navigation_domain_router::{
    NavigationDomainRouterError, NavigationRootFocusBindingPortId, RoutedNavigationCandidate,
};
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
    SurfaceTerminationReason,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayDisposition, OverlayDomainCoordinator, OverlayFrameDescriptor, OverlayFrameId,
    OverlayOwnerId, OverlayOwnerLifecyclePort, OverlayOwnerLifecycleState, OverlayStackDescriptor,
    OverlayStackId,
};
use halcyon::model::navigation_host::{NavigationHost, NavigationNodeId};

const ROOT_ONE: NavigationRootId = NavigationRootId(1);
const ROOT_TWO: NavigationRootId = NavigationRootId(2);
const ROOT_THREE: NavigationRootId = NavigationRootId(3);
const OWNER_ONE: OverlayOwnerId = OverlayOwnerId(11);
const OWNER_TWO: OverlayOwnerId = OverlayOwnerId(12);
const OWNER_THREE: OverlayOwnerId = OverlayOwnerId(13);
const PORT_ONE: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(21);
const PORT_TWO: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(22);
const PORT_THREE: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(23);
const FOCUS_ONE: FocusDomainId = FocusDomainId(31);
const FOCUS_TWO: FocusDomainId = FocusDomainId(32);
const FOCUS_THREE: FocusDomainId = FocusDomainId(33);
const OVERLAY_FOCUS_ONE: FocusDomainId = FocusDomainId(41);
const OVERLAY_FOCUS_TWO: FocusDomainId = FocusDomainId(42);
const OVERLAY_FOCUS_THREE: FocusDomainId = FocusDomainId(43);

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

struct Setup {
    interaction_domains: InteractionDomains,
    application: NavigationApplicationCoordinator,
    registry: NavigationRootRegistry,
    compositor: Compositor,
}

impl Setup {
    fn new() -> Self {
        let interaction_domains = InteractionDomains::new();
        let overlay_domains = OverlayDomainCoordinator::new().unwrap();
        overlay_domains
            .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
            .unwrap();
        let application = NavigationApplicationCoordinator::new(overlay_domains);
        application
            .navigation_domains()
            .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
            .unwrap();
        Self {
            interaction_domains,
            application,
            registry: NavigationRootRegistry::new(),
            compositor: Compositor::new(viewport()),
        }
    }

    fn admit_root(
        &mut self,
        root_id: NavigationRootId,
        owner_id: OverlayOwnerId,
        port_id: NavigationRootFocusBindingPortId,
        base_focus_domain: FocusDomainId,
        overlay_focus_domain: Option<FocusDomainId>,
    ) -> OverlayOwnerLifecyclePort {
        let owner_port =
            OverlayOwnerLifecyclePort::new(owner_id, InteractionDomainId::PRIMARY, vec![]).unwrap();
        let mut additions = vec![FocusDomainAddition::new(base_focus_domain, None)];
        if let Some(overlay_focus_domain) = overlay_focus_domain {
            additions.push(FocusDomainAddition::new(
                overlay_focus_domain,
                Some(base_focus_domain),
            ));
        }
        self.application
            .prepare_root_admission(
                &mut self.interaction_domains,
                &mut self.registry,
                &mut self.compositor,
                InteractionDomainId::PRIMARY,
                root_id,
                NavigationHost::new(NavigationNodeId(root_id.0)),
                viewport(),
                Box::new(IncrementalSceneKeyAllocator::new()),
                owner_port.clone(),
                port_id,
                &additions,
                base_focus_domain,
            )
            .unwrap()
            .commit();
        owner_port
    }

    fn prime(&self, root_id: NavigationRootId) {
        let guard = self
            .application
            .overlay_domains()
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .unwrap();
        self.application
            .navigation_domains()
            .prime_root_guarded(&guard, root_id)
            .unwrap();
    }

    fn active_unscoped_root(&self) -> NavigationRootId {
        match self
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, None)
        {
            RoutedNavigationCandidate::Snapshot(snapshot) => snapshot.root_id,
            result => panic!("expected active unscoped root, got {result:?}"),
        }
    }

    fn activate_overlay(
        &mut self,
        root_id: NavigationRootId,
        owner_id: OverlayOwnerId,
        owner_port: &OverlayOwnerLifecyclePort,
        overlay_focus_domain: FocusDomainId,
        seed: u64,
    ) -> halcyon::compositor::SurfaceId {
        let route = match self
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(root_id))
        {
            RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
            result => panic!("expected scoped root route, got {result:?}"),
        };
        let surface = self
            .compositor
            .create_overlay_surface(
                route.focus_binding.navigation_surface_id,
                10_000 + seed as i32,
                viewport(),
                SceneKey::new(10_000 + seed),
            )
            .unwrap();
        let stack_id = OverlayStackId(100 + seed);
        owner_port
            .register_inactive_stack(OverlayStackDescriptor::new(
                stack_id,
                vec![OverlayFrameDescriptor::new(
                    OverlayFrameId(200 + seed),
                    overlay_focus_domain,
                    surface,
                )],
            ))
            .unwrap();
        let guard = self
            .application
            .overlay_domains()
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .unwrap();
        let eligibility = self
            .application
            .navigation_domains()
            .overlay_activation_eligibility_guarded(&guard, owner_id)
            .unwrap();
        let prepared = self
            .application
            .overlay_domains()
            .prepare_activation(
                &guard,
                &mut self.interaction_domains,
                owner_id,
                stack_id,
                Some(eligibility),
            )
            .unwrap();
        self.application
            .overlay_domains()
            .commit_activation(&guard, prepared);
        surface
    }
}

#[test]
fn navigation_delegation_and_return_switch_effective_root_and_focus_atomically() {
    let mut setup = Setup::new();
    setup.admit_root(ROOT_ONE, OWNER_ONE, PORT_ONE, FOCUS_ONE, None);
    setup.admit_root(ROOT_TWO, OWNER_TWO, PORT_TWO, FOCUS_TWO, None);
    setup.prime(ROOT_ONE);

    let committed = setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            ROOT_TWO,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    assert_eq!(committed.frame().source_root, ROOT_ONE);
    assert_eq!(committed.frame().target_root, ROOT_TWO);
    assert_eq!(setup.active_unscoped_root(), ROOT_TWO);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_TWO)
    );
    assert_eq!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_ONE)),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    let guard = setup
        .application
        .overlay_domains()
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .overlay_activation_eligibility_guarded(&guard, OWNER_ONE),
        Err(NavigationDomainRouterError::SuspendedRoot(ROOT_ONE))
    ));
    drop(guard);

    let prepared_return = setup
        .application
        .prepare_delegation_return(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            OverlayDisposition::Dismiss,
        )
        .unwrap();
    drop(prepared_return);
    assert_eq!(setup.active_unscoped_root(), ROOT_TWO);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_TWO)
    );
    setup
        .application
        .prepare_delegation_return(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    assert_eq!(setup.active_unscoped_root(), ROOT_ONE);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_ONE)
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_TWO)),
        RoutedNavigationCandidate::Snapshot(_)
    ));
}

#[test]
fn navigation_delegation_drop_releases_every_preparation_without_mutation() {
    let mut setup = Setup::new();
    setup.admit_root(ROOT_ONE, OWNER_ONE, PORT_ONE, FOCUS_ONE, None);
    setup.admit_root(ROOT_TWO, OWNER_TWO, PORT_TWO, FOCUS_TWO, None);
    setup.prime(ROOT_ONE);
    let prepared = setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            ROOT_TWO,
            OverlayDisposition::Dismiss,
        )
        .unwrap();
    drop(prepared);

    assert_eq!(setup.active_unscoped_root(), ROOT_ONE);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_ONE)
    );
    setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            ROOT_TWO,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
}

#[test]
fn navigation_delegation_suspends_source_overlay_before_suspending_source() {
    let mut setup = Setup::new();
    let source_port = setup.admit_root(
        ROOT_ONE,
        OWNER_ONE,
        PORT_ONE,
        FOCUS_ONE,
        Some(OVERLAY_FOCUS_ONE),
    );
    setup.admit_root(ROOT_TWO, OWNER_TWO, PORT_TWO, FOCUS_TWO, None);
    setup.prime(ROOT_ONE);
    let overlay = setup.activate_overlay(ROOT_ONE, OWNER_ONE, &source_port, OVERLAY_FOCUS_ONE, 1);

    let committed = setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            ROOT_TWO,
            OverlayDisposition::Suspend,
        )
        .unwrap()
        .commit();

    assert!(committed.terminated_surfaces().is_empty());
    assert!(setup.compositor.has_surface(overlay));
    assert_eq!(
        setup.compositor.surface_visibility(overlay),
        Some(SurfaceVisibility::Hidden)
    );
    assert_eq!(
        source_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Inactive
    );
    assert_eq!(setup.active_unscoped_root(), ROOT_TWO);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_TWO)
    );
}

#[test]
fn navigation_delegation_retains_target_overlay_and_return_dismisses_it() {
    let mut setup = Setup::new();
    setup.admit_root(ROOT_ONE, OWNER_ONE, PORT_ONE, FOCUS_ONE, None);
    let target_port = setup.admit_root(
        ROOT_TWO,
        OWNER_TWO,
        PORT_TWO,
        FOCUS_TWO,
        Some(OVERLAY_FOCUS_TWO),
    );
    setup.prime(ROOT_ONE);
    let overlay = setup.activate_overlay(ROOT_TWO, OWNER_TWO, &target_port, OVERLAY_FOCUS_TWO, 2);

    let delegation = setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            ROOT_TWO,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    assert!(delegation.terminated_surfaces().is_empty());
    assert_eq!(
        target_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active {
            stack_id: OverlayStackId(102)
        }
    );
    assert_eq!(
        setup.compositor.surface_visibility(overlay),
        Some(SurfaceVisibility::Visible)
    );
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(OVERLAY_FOCUS_TWO)
    );

    let prepared_return = setup
        .application
        .prepare_delegation_return(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            OverlayDisposition::Dismiss,
        )
        .unwrap();
    drop(prepared_return);
    assert_eq!(setup.active_unscoped_root(), ROOT_TWO);
    assert!(setup.compositor.has_surface(overlay));
    assert_eq!(
        setup.compositor.surface_visibility(overlay),
        Some(SurfaceVisibility::Visible)
    );
    assert_eq!(
        target_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active {
            stack_id: OverlayStackId(102)
        }
    );
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(OVERLAY_FOCUS_TWO)
    );
    let returned = setup
        .application
        .prepare_delegation_return(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    assert!(
        returned
            .terminated_surfaces()
            .contains(&(overlay, SurfaceTerminationReason::OverlayClosed))
    );
    assert!(!setup.compositor.has_surface(overlay));
    assert_eq!(
        target_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Inactive
    );
    assert_eq!(setup.active_unscoped_root(), ROOT_ONE);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_ONE)
    );
}

#[test]
fn navigation_delegation_rejects_unrelated_peer_overlay_without_mutation() {
    let mut setup = Setup::new();
    setup.admit_root(ROOT_ONE, OWNER_ONE, PORT_ONE, FOCUS_ONE, None);
    setup.admit_root(ROOT_TWO, OWNER_TWO, PORT_TWO, FOCUS_TWO, None);
    let peer_port = setup.admit_root(
        ROOT_THREE,
        OWNER_THREE,
        PORT_THREE,
        FOCUS_THREE,
        Some(OVERLAY_FOCUS_THREE),
    );
    setup.prime(ROOT_ONE);
    let overlay =
        setup.activate_overlay(ROOT_THREE, OWNER_THREE, &peer_port, OVERLAY_FOCUS_THREE, 3);

    let error = setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            ROOT_TWO,
            OverlayDisposition::Dismiss,
        )
        .map(drop)
        .expect_err("unrelated peer overlay must reject delegation");
    assert!(matches!(
        error,
        NavigationDelegationError::ActivePeerOverlay(binding)
            if binding.owner_id == OWNER_THREE
    ));
    assert_eq!(setup.active_unscoped_root(), ROOT_ONE);
    assert!(setup.compositor.has_surface(overlay));
    assert_eq!(
        peer_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active {
            stack_id: OverlayStackId(103)
        }
    );
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(OVERLAY_FOCUS_THREE)
    );
}

#[test]
fn navigation_delegation_supports_nested_frames_and_rejects_cycles() {
    let mut setup = Setup::new();
    setup.admit_root(ROOT_ONE, OWNER_ONE, PORT_ONE, FOCUS_ONE, None);
    setup.admit_root(ROOT_TWO, OWNER_TWO, PORT_TWO, FOCUS_TWO, None);
    setup.admit_root(ROOT_THREE, OWNER_THREE, PORT_THREE, FOCUS_THREE, None);
    setup.prime(ROOT_ONE);
    setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            ROOT_TWO,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_TWO,
            ROOT_THREE,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    assert_eq!(setup.active_unscoped_root(), ROOT_THREE);

    let error = setup
        .application
        .prepare_delegation(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            ROOT_THREE,
            ROOT_ONE,
            OverlayDisposition::Dismiss,
        )
        .map(drop)
        .expect_err("delegation cycle must reject");
    assert_eq!(
        error,
        NavigationDelegationError::Router(NavigationDomainRouterError::DelegationCycle(ROOT_ONE))
    );
    assert_eq!(setup.active_unscoped_root(), ROOT_THREE);

    setup
        .application
        .prepare_delegation_return(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    assert_eq!(setup.active_unscoped_root(), ROOT_TWO);
    setup
        .application
        .prepare_delegation_return(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            InteractionDomainId::PRIMARY,
            OverlayDisposition::Dismiss,
        )
        .unwrap()
        .commit();
    assert_eq!(setup.active_unscoped_root(), ROOT_ONE);
}
