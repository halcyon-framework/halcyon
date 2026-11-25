use halcyon::compositor::{Compositor, ProjectedSurfaceView, SceneKey};
use halcyon::core::event::{InteractionDomainId, InteractionDomains, WindowId};
use halcyon::core::focus_authority::{FocusAuthorityError, FocusDomainAddition, FocusDomainId};
use halcyon::core::frame_projection_binding_registry::{
    FrameProjectionAuthorityAccess, FrameProjectionBinding, FrameProjectionBindingRegistry,
    FrameProjectionWindowAccess, NavigationProjectionSource, ScreenHostProjectionKey,
};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::{
    FrameProjectionRootConsumerPlan, NavigationApplicationCoordinator,
    NavigationRootRetirementError,
};
use halcyon::core::navigation_domain_router::{
    NavigationRootFocusBindingPortId, RoutedNavigationCandidate,
};
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry, OverlayKind,
    SurfaceTerminationReason,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayDisposition, OverlayDomainCoordinator, OverlayDomainError, OverlayFrameDescriptor,
    OverlayFrameId, OverlayOwnerId, OverlayOwnerLifecyclePort, OverlayOwnerLifecycleState,
    OverlayStackDescriptor, OverlayStackId,
};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::model::navigation_host::{NavigationHost, NavigationNodeId};
use halcyon::widgets::screen_host::ScreenHost;

const ROOT_ONE: NavigationRootId = NavigationRootId(1);
const OWNER_ONE: OverlayOwnerId = OverlayOwnerId(2);
const PORT_ONE: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(3);
const FOCUS_ONE: FocusDomainId = FocusDomainId(4);

const ROOT_TWO: NavigationRootId = NavigationRootId(11);
const OWNER_TWO: OverlayOwnerId = OverlayOwnerId(12);
const PORT_TWO: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(13);
const FOCUS_TWO: FocusDomainId = FocusDomainId(14);
const OVERLAY_FOCUS_TWO: FocusDomainId = FocusDomainId(15);
const STACK_TWO: OverlayStackId = OverlayStackId(16);

const ROOT_THREE: NavigationRootId = NavigationRootId(21);
const OWNER_THREE: OverlayOwnerId = OverlayOwnerId(22);
const PORT_THREE: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(23);
const FOCUS_THREE: FocusDomainId = FocusDomainId(24);
const OVERLAY_FOCUS_THREE: FocusDomainId = FocusDomainId(25);
const STACK_THREE: OverlayStackId = OverlayStackId(26);

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

struct Setup {
    interaction_domains: InteractionDomains,
    overlay_domains: OverlayDomainCoordinator,
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
        let application = NavigationApplicationCoordinator::new(overlay_domains.clone());
        application
            .navigation_domains()
            .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
            .unwrap();
        Self {
            interaction_domains,
            overlay_domains,
            application,
            registry: NavigationRootRegistry::new(),
            compositor: Compositor::new(viewport()),
        }
    }

    fn admit_root(
        &mut self,
        root_id: NavigationRootId,
        owner_port: OverlayOwnerLifecyclePort,
        port_id: NavigationRootFocusBindingPortId,
        focus_domains: &[FocusDomainAddition],
        root_focus_domain: FocusDomainId,
    ) {
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
                owner_port,
                port_id,
                focus_domains,
                root_focus_domain,
            )
            .unwrap()
            .commit();
    }

    fn delegate(&mut self, source_root: NavigationRootId, target_root: NavigationRootId) {
        self.application
            .prepare_delegation(
                &mut self.interaction_domains,
                &mut self.registry,
                &mut self.compositor,
                InteractionDomainId::PRIMARY,
                source_root,
                target_root,
                OverlayDisposition::Dismiss,
            )
            .unwrap()
            .commit();
    }

    fn prime(&self, root_id: NavigationRootId) {
        let guard = self
            .overlay_domains
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
}

fn owner_port(
    owner_id: OverlayOwnerId,
    stacks: Vec<OverlayStackDescriptor>,
) -> OverlayOwnerLifecyclePort {
    OverlayOwnerLifecyclePort::new(owner_id, InteractionDomainId::PRIMARY, stacks).unwrap()
}

fn tree_with_screen_host() -> (WidgetTree, NodeId) {
    let mut tree = WidgetTree::new();
    let node = tree.create_root(Box::new(ScreenHost::new()), None, 0);
    (tree, node)
}

fn screen_host(tree: &WidgetTree, node: NodeId) -> &ScreenHost {
    tree.node(node)
        .unwrap()
        .widget()
        .as_any()
        .downcast_ref::<ScreenHost>()
        .unwrap()
}

fn screen_host_mut(tree: &mut WidgetTree, node: NodeId) -> &mut ScreenHost {
    tree.node_mut(node)
        .unwrap()
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<ScreenHost>()
        .unwrap()
}

fn projected_view() -> ProjectedSurfaceView {
    ProjectedSurfaceView {
        navigation_contexts: Vec::new(),
        overlay_surfaces: Vec::new(),
        focused_surface: None,
    }
}

fn admit_two_roots(setup: &mut Setup) {
    setup.admit_root(
        ROOT_ONE,
        owner_port(OWNER_ONE, vec![]),
        PORT_ONE,
        &[FocusDomainAddition::new(FOCUS_ONE, None)],
        FOCUS_ONE,
    );
    setup.admit_root(
        ROOT_TWO,
        owner_port(OWNER_TWO, vec![]),
        PORT_TWO,
        &[FocusDomainAddition::new(FOCUS_TWO, None)],
        FOCUS_TWO,
    );
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    setup
        .application
        .navigation_domains()
        .prime_root_guarded(&guard, ROOT_ONE)
        .unwrap();
}

fn install_two_root_projection(
    setup: &Setup,
    projection_bindings: &FrameProjectionBindingRegistry,
    tree: &mut WidgetTree,
    key: ScreenHostProjectionKey,
) -> halcyon::core::frame_projection_binding_registry::FrameProjectionEpoch {
    let focus_resolver = halcyon::core::focus_authority::FocusResolver::new();
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    projection_bindings
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &setup.interaction_domains,
                &setup.overlay_domains,
                &focus_resolver,
            )
            .with_registered_roots(&setup.registry, setup.application.navigation_domains()),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &setup.compositor,
                tree,
            ),
            key,
            FrameProjectionBinding::new(
                InteractionDomainId::PRIMARY,
                vec![
                    NavigationProjectionSource::registered_root(ROOT_ONE),
                    NavigationProjectionSource::registered_root(ROOT_TWO),
                ],
            ),
            &guard,
        )
        .unwrap()
}

#[test]
fn root_retirement_replaces_projection_and_commits_every_authority_atomically() {
    let mut setup = Setup::new();
    admit_two_roots(&mut setup);
    let retiring_surface = setup
        .registry
        .root(ROOT_ONE)
        .unwrap()
        .project()
        .unwrap()
        .navigation_surfaces[0];
    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    let original_epoch = install_two_root_projection(&setup, &projection_bindings, &mut tree, key);
    screen_host_mut(&mut tree, screen_host_node).set_projected_view(projected_view());

    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &setup.compositor,
        &mut tree,
    );
    let prepared = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![window],
            vec![FrameProjectionRootConsumerPlan::replace(
                key,
                FrameProjectionBinding::new(
                    InteractionDomainId::PRIMARY,
                    vec![NavigationProjectionSource::registered_root(ROOT_TWO)],
                ),
            )],
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            Some(ROOT_TWO),
        )
        .unwrap();
    let committed = prepared.commit();

    assert_eq!(committed.root_id, ROOT_ONE);
    assert_eq!(
        committed.terminated_surfaces,
        vec![(retiring_surface, SurfaceTerminationReason::HostTeardown)]
    );
    assert!(setup.registry.root(ROOT_ONE).is_none());
    assert!(setup.registry.root(ROOT_TWO).is_some());
    assert!(!setup.compositor.has_surface(retiring_surface));
    assert_eq!(setup.compositor.surface_count(), 1);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        1
    );
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
            .overlay_domains
            .owner_registration_generation(OWNER_ONE),
        Err(OverlayDomainError::UnknownOwner(OWNER_ONE))
    );
    assert_eq!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_ONE)),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_TWO)),
        RoutedNavigationCandidate::Snapshot(_)
    ));
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let binding = projection_bindings
        .binding_guarded(&guard, key)
        .unwrap()
        .unwrap();
    assert_ne!(binding.epoch(), original_epoch);
    assert_eq!(
        binding.binding().navigation_sources(),
        &[NavigationProjectionSource::registered_root(ROOT_TWO)]
    );
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_none()
    );
}

#[test]
fn dropping_prepared_root_retirement_aborts_every_reserved_authority() {
    let mut setup = Setup::new();
    admit_two_roots(&mut setup);
    let surface_count = setup.compositor.surface_count();
    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    let original_epoch = install_two_root_projection(&setup, &projection_bindings, &mut tree, key);
    screen_host_mut(&mut tree, screen_host_node).set_projected_view(projected_view());

    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &setup.compositor,
        &mut tree,
    );
    let prepared = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![window],
            vec![FrameProjectionRootConsumerPlan::replace(
                key,
                FrameProjectionBinding::new(
                    InteractionDomainId::PRIMARY,
                    vec![NavigationProjectionSource::registered_root(ROOT_TWO)],
                ),
            )],
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            Some(ROOT_TWO),
        )
        .unwrap();
    drop(prepared);

    assert_eq!(setup.registry.root_count(), 2);
    assert_eq!(setup.compositor.surface_count(), surface_count);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        2
    );
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_ONE)
    );
    assert!(
        setup
            .overlay_domains
            .owner_registration_generation(OWNER_ONE)
            .is_ok()
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_ONE)),
        RoutedNavigationCandidate::Snapshot(_)
    ));
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let binding = projection_bindings
        .binding_guarded(&guard, key)
        .unwrap()
        .unwrap();
    assert_eq!(binding.epoch(), original_epoch);
    assert_eq!(binding.binding().navigation_sources().len(), 2);
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_some()
    );
}

#[test]
fn root_retirement_priming_replacement_preserves_a_surviving_peer_focus_domain() {
    let mut setup = Setup::new();
    setup.admit_root(
        ROOT_TWO,
        owner_port(OWNER_TWO, vec![]),
        PORT_TWO,
        &[FocusDomainAddition::new(FOCUS_TWO, None)],
        FOCUS_TWO,
    );
    setup.admit_root(
        ROOT_ONE,
        owner_port(OWNER_ONE, vec![]),
        PORT_ONE,
        &[FocusDomainAddition::new(FOCUS_ONE, None)],
        FOCUS_ONE,
    );
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    setup
        .application
        .navigation_domains()
        .prime_root_guarded(&guard, ROOT_ONE)
        .unwrap();
    drop(guard);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_TWO)
    );

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            Some(ROOT_TWO),
        )
        .unwrap()
        .commit();

    assert_eq!(setup.registry.root_count(), 1);
    assert!(setup.registry.root(ROOT_TWO).is_some());
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_TWO)
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, None),
        RoutedNavigationCandidate::Snapshot(snapshot) if snapshot.root_id == ROOT_TWO
    ));
}

#[test]
fn root_retirement_deactivates_owner_overlay_and_tears_down_its_surfaces() {
    let mut setup = Setup::new();
    let owner_port = owner_port(OWNER_ONE, vec![]);
    setup.admit_root(
        ROOT_ONE,
        owner_port.clone(),
        PORT_ONE,
        &[
            FocusDomainAddition::new(FOCUS_ONE, None),
            FocusDomainAddition::new(OVERLAY_FOCUS_TWO, Some(FOCUS_ONE)),
        ],
        FOCUS_ONE,
    );
    let overlay_surface = setup
        .registry
        .root_mut(ROOT_ONE)
        .unwrap()
        .open_overlay(OverlayKind::Modal, &mut setup.compositor);
    owner_port
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK_TWO,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(17),
                OVERLAY_FOCUS_TWO,
                overlay_surface,
            )],
        ))
        .unwrap();
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let eligibility = setup
        .application
        .navigation_domains()
        .overlay_activation_eligibility_guarded(&guard, OWNER_ONE)
        .unwrap();
    let prepared = setup
        .overlay_domains
        .prepare_activation(
            &guard,
            &mut setup.interaction_domains,
            OWNER_ONE,
            STACK_TWO,
            Some(eligibility),
        )
        .unwrap();
    setup.overlay_domains.commit_activation(&guard, prepared);
    drop(guard);
    assert_eq!(
        owner_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active {
            stack_id: STACK_TWO
        }
    );

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let committed = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            None,
        )
        .unwrap()
        .commit();

    assert!(
        committed
            .terminated_surfaces
            .iter()
            .any(|(surface, _)| *surface == overlay_surface)
    );
    assert!(!setup.compositor.has_surface(overlay_surface));
    assert_eq!(setup.compositor.surface_count(), 0);
    assert_eq!(setup.registry.root_count(), 0);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        0
    );
    assert_eq!(
        setup
            .overlay_domains
            .owner_registration_generation(OWNER_ONE),
        Err(OverlayDomainError::UnknownOwner(OWNER_ONE))
    );
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert_eq!(
        setup.overlay_domains.active_binding_guarded(&guard),
        Ok(None)
    );
}

#[test]
fn peer_overlay_restoration_dependency_rejects_root_retirement_without_mutation() {
    let mut setup = Setup::new();
    setup.admit_root(
        ROOT_ONE,
        owner_port(OWNER_ONE, vec![]),
        PORT_ONE,
        &[FocusDomainAddition::new(FOCUS_ONE, None)],
        FOCUS_ONE,
    );
    let peer_owner_port = owner_port(OWNER_TWO, vec![]);
    setup.admit_root(
        ROOT_TWO,
        peer_owner_port.clone(),
        PORT_TWO,
        &[
            FocusDomainAddition::new(FOCUS_TWO, None),
            FocusDomainAddition::new(OVERLAY_FOCUS_TWO, Some(FOCUS_TWO)),
        ],
        FOCUS_TWO,
    );
    let overlay_surface = setup
        .registry
        .root_mut(ROOT_TWO)
        .unwrap()
        .open_overlay(OverlayKind::Modal, &mut setup.compositor);
    peer_owner_port
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK_TWO,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(17),
                OVERLAY_FOCUS_TWO,
                overlay_surface,
            )],
        ))
        .unwrap();
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let eligibility = setup
        .application
        .navigation_domains()
        .overlay_activation_eligibility_guarded(&guard, OWNER_TWO)
        .unwrap();
    let prepared = setup
        .overlay_domains
        .prepare_activation(
            &guard,
            &mut setup.interaction_domains,
            OWNER_TWO,
            STACK_TWO,
            Some(eligibility),
        )
        .unwrap();
    setup.overlay_domains.commit_activation(&guard, prepared);
    drop(guard);
    let surface_count = setup.compositor.surface_count();
    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);

    let error = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            None,
        )
        .map(drop)
        .expect_err("peer restoration dependency must block retirement");

    assert_eq!(
        error,
        NavigationRootRetirementError::Focus(FocusAuthorityError::PeerRestorationDependency {
            activated_domain: OVERLAY_FOCUS_TWO,
            superseded_domain: FOCUS_ONE,
        })
    );
    assert_eq!(setup.registry.root_count(), 2);
    assert_eq!(setup.compositor.surface_count(), surface_count);
    assert!(setup.compositor.has_surface(overlay_surface));
    assert_eq!(
        peer_owner_port.lifecycle_state(),
        OverlayOwnerLifecycleState::Active {
            stack_id: STACK_TWO
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
    assert!(
        setup
            .overlay_domains
            .owner_registration_generation(OWNER_ONE)
            .is_ok()
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_ONE)),
        RoutedNavigationCandidate::Snapshot(_)
    ));
    assert!(
        setup
            .overlay_domains
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .is_ok()
    );
}

#[test]
fn root_retirement_of_delegated_target_unwinds_to_its_source_atomically() {
    let mut setup = Setup::new();
    admit_two_roots(&mut setup);
    setup.delegate(ROOT_ONE, ROOT_TWO);

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let committed = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_TWO,
            None,
        )
        .unwrap()
        .commit();

    assert_eq!(
        committed.unwound_delegation_frames,
        vec![halcyon::core::navigation_domain_router::DelegationFrame {
            source_root: ROOT_ONE,
            target_root: ROOT_TWO,
        }]
    );
    assert!(setup.registry.root(ROOT_TWO).is_none());
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
fn root_retirement_of_nested_delegation_member_unwinds_dependent_frames_top_first() {
    let mut setup = Setup::new();
    admit_two_roots(&mut setup);
    setup.admit_root(
        ROOT_THREE,
        owner_port(OWNER_THREE, vec![]),
        PORT_THREE,
        &[FocusDomainAddition::new(FOCUS_THREE, None)],
        FOCUS_THREE,
    );
    setup.delegate(ROOT_ONE, ROOT_TWO);
    setup.delegate(ROOT_TWO, ROOT_THREE);

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let committed = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_TWO,
            None,
        )
        .unwrap()
        .commit();

    assert_eq!(
        committed.unwound_delegation_frames,
        vec![
            halcyon::core::navigation_domain_router::DelegationFrame {
                source_root: ROOT_TWO,
                target_root: ROOT_THREE,
            },
            halcyon::core::navigation_domain_router::DelegationFrame {
                source_root: ROOT_ONE,
                target_root: ROOT_TWO,
            },
        ]
    );
    assert_eq!(setup.active_unscoped_root(), ROOT_ONE);
    assert!(setup.registry.root(ROOT_TWO).is_none());
    assert!(setup.registry.root(ROOT_THREE).is_some());
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_THREE)),
        RoutedNavigationCandidate::Snapshot(_)
    ));
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
fn root_retirement_of_primed_delegation_source_can_prime_the_surviving_target() {
    let mut setup = Setup::new();
    admit_two_roots(&mut setup);
    setup.delegate(ROOT_ONE, ROOT_TWO);

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_ONE,
            Some(ROOT_TWO),
        )
        .unwrap()
        .commit();

    assert!(setup.registry.root(ROOT_ONE).is_none());
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
fn delegation_linked_root_retirement_dismisses_effective_peer_overlay_in_one_surface_batch() {
    let mut setup = Setup::new();
    admit_two_roots(&mut setup);
    let peer_owner = owner_port(OWNER_THREE, vec![]);
    setup.admit_root(
        ROOT_THREE,
        peer_owner.clone(),
        PORT_THREE,
        &[
            FocusDomainAddition::new(FOCUS_THREE, None),
            FocusDomainAddition::new(OVERLAY_FOCUS_THREE, Some(FOCUS_THREE)),
        ],
        FOCUS_THREE,
    );
    setup.delegate(ROOT_ONE, ROOT_TWO);
    setup.delegate(ROOT_TWO, ROOT_THREE);

    let route = match setup
        .application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_THREE))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected current delegated root, got {result:?}"),
    };
    let overlay_surface = setup
        .compositor
        .create_overlay_surface(
            route.focus_binding.navigation_surface_id,
            10_000,
            viewport(),
            SceneKey::new(10_000),
        )
        .unwrap();
    peer_owner
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK_THREE,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(27),
                OVERLAY_FOCUS_THREE,
                overlay_surface,
            )],
        ))
        .unwrap();
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let eligibility = setup
        .application
        .navigation_domains()
        .overlay_activation_eligibility_guarded(&guard, OWNER_THREE)
        .unwrap();
    let prepared_overlay = setup
        .overlay_domains
        .prepare_activation(
            &guard,
            &mut setup.interaction_domains,
            OWNER_THREE,
            STACK_THREE,
            Some(eligibility),
        )
        .unwrap();
    setup
        .overlay_domains
        .commit_activation(&guard, prepared_overlay);
    drop(guard);

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let committed = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_TWO,
            None,
        )
        .unwrap()
        .commit();

    assert!(
        committed
            .terminated_surfaces
            .iter()
            .any(|(surface, reason)| {
                *surface == overlay_surface && *reason == SurfaceTerminationReason::OverlayClosed
            })
    );
    assert!(!setup.compositor.has_surface(overlay_surface));
    assert_eq!(
        peer_owner.lifecycle_state(),
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
fn delegated_target_root_retirement_dismisses_its_external_owner_overlay() {
    let mut setup = Setup::new();
    setup.admit_root(
        ROOT_ONE,
        owner_port(OWNER_ONE, vec![]),
        PORT_ONE,
        &[FocusDomainAddition::new(FOCUS_ONE, None)],
        FOCUS_ONE,
    );
    let retiring_owner = owner_port(OWNER_TWO, vec![]);
    setup.admit_root(
        ROOT_TWO,
        retiring_owner.clone(),
        PORT_TWO,
        &[
            FocusDomainAddition::new(FOCUS_TWO, None),
            FocusDomainAddition::new(OVERLAY_FOCUS_TWO, Some(FOCUS_TWO)),
        ],
        FOCUS_TWO,
    );
    setup.prime(ROOT_ONE);
    setup.delegate(ROOT_ONE, ROOT_TWO);

    let route = match setup
        .application
        .navigation_domains()
        .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_TWO))
    {
        RoutedNavigationCandidate::Snapshot(snapshot) => snapshot,
        result => panic!("expected delegated target, got {result:?}"),
    };
    let overlay_surface = setup
        .compositor
        .create_overlay_surface(
            route.focus_binding.navigation_surface_id,
            10_001,
            viewport(),
            SceneKey::new(10_001),
        )
        .unwrap();
    retiring_owner
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK_TWO,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(28),
                OVERLAY_FOCUS_TWO,
                overlay_surface,
            )],
        ))
        .unwrap();
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let eligibility = setup
        .application
        .navigation_domains()
        .overlay_activation_eligibility_guarded(&guard, OWNER_TWO)
        .unwrap();
    let prepared_overlay = setup
        .overlay_domains
        .prepare_activation(
            &guard,
            &mut setup.interaction_domains,
            OWNER_TWO,
            STACK_TWO,
            Some(eligibility),
        )
        .unwrap();
    setup
        .overlay_domains
        .commit_activation(&guard, prepared_overlay);
    drop(guard);

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let committed = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_TWO,
            None,
        )
        .unwrap()
        .commit();

    assert!(
        committed
            .terminated_surfaces
            .iter()
            .any(|(surface, _)| *surface == overlay_surface)
    );
    assert!(!setup.compositor.has_surface(overlay_surface));
    assert_eq!(
        setup
            .overlay_domains
            .owner_registration_generation(OWNER_TWO),
        Err(OverlayDomainError::UnknownOwner(OWNER_TWO))
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
fn dropping_prepared_delegation_linked_root_retirement_preserves_the_complete_chain() {
    let mut setup = Setup::new();
    admit_two_roots(&mut setup);
    setup.admit_root(
        ROOT_THREE,
        owner_port(OWNER_THREE, vec![]),
        PORT_THREE,
        &[FocusDomainAddition::new(FOCUS_THREE, None)],
        FOCUS_THREE,
    );
    setup.delegate(ROOT_ONE, ROOT_TWO);
    setup.delegate(ROOT_TWO, ROOT_THREE);
    let surface_count = setup.compositor.surface_count();

    let projection_bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let prepared = setup
        .application
        .prepare_root_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &projection_bindings,
            vec![],
            vec![],
            InteractionDomainId::PRIMARY,
            ROOT_TWO,
            None,
        )
        .unwrap();
    drop(prepared);

    assert_eq!(setup.registry.root_count(), 3);
    assert_eq!(setup.compositor.surface_count(), surface_count);
    assert_eq!(setup.active_unscoped_root(), ROOT_THREE);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .active_focus_domain(),
        Some(FOCUS_THREE)
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, Some(ROOT_TWO)),
        RoutedNavigationCandidate::RoutingUnavailable
    ));
    assert!(
        setup
            .overlay_domains
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .is_ok()
    );
}
