use halcyon::compositor::{Compositor, ProjectedSurfaceView, SceneKey};
use halcyon::core::event::{InteractionDomainId, InteractionDomains, WindowId};
use halcyon::core::focus_authority::{FocusDomainAddition, FocusDomainId};
use halcyon::core::frame_projection_binding_registry::{
    FrameProjectionAuthorityAccess, FrameProjectionBinding, FrameProjectionBindingRegistry,
    FrameProjectionWindowAccess, NavigationProjectionSource, ScreenHostProjectionKey,
};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::NavigationApplicationCoordinator;
use halcyon::core::navigation_domain_router::{
    DelegationFrame, NavigationRootFocusBindingPortId, RoutedNavigationCandidate,
};
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
    SurfaceTerminationReason,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayActivationAdmission, OverlayDomainCoordinator, OverlayDomainError,
    OverlayFrameDescriptor, OverlayFrameId, OverlayOwnerId, OverlayOwnerLifecyclePort,
    OverlayStackDescriptor, OverlayStackId,
};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::model::navigation_host::{NavigationHost, NavigationNodeId};
use halcyon::widgets::screen_host::ScreenHost;

const ROOT_ONE: NavigationRootId = NavigationRootId(1);
const ROOT_TWO: NavigationRootId = NavigationRootId(2);
const OWNER_ONE: OverlayOwnerId = OverlayOwnerId(11);
const OWNER_TWO: OverlayOwnerId = OverlayOwnerId(12);
const PORT_ONE: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(21);
const PORT_TWO: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(22);
const FOCUS_ONE: FocusDomainId = FocusDomainId(31);
const FOCUS_TWO: FocusDomainId = FocusDomainId(32);
const OVERLAY_FOCUS_TWO: FocusDomainId = FocusDomainId(33);
const STACK_TWO: OverlayStackId = OverlayStackId(41);
const DOMAIN_LOCAL_OWNER: OverlayOwnerId = OverlayOwnerId(51);

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

struct Setup {
    interaction_domains: InteractionDomains,
    overlay_domains: OverlayDomainCoordinator,
    application: NavigationApplicationCoordinator,
    registry: NavigationRootRegistry,
    compositor: Compositor,
    owner_two: OverlayOwnerLifecyclePort,
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
        let owner_two =
            OverlayOwnerLifecyclePort::new(OWNER_TWO, InteractionDomainId::PRIMARY, vec![])
                .unwrap();
        Self {
            interaction_domains,
            overlay_domains,
            application,
            registry: NavigationRootRegistry::new(),
            compositor: Compositor::new(viewport()),
            owner_two,
        }
    }

    fn admit_roots(&mut self) {
        self.application
            .prepare_root_admission(
                &mut self.interaction_domains,
                &mut self.registry,
                &mut self.compositor,
                InteractionDomainId::PRIMARY,
                ROOT_ONE,
                NavigationHost::new(NavigationNodeId(101)),
                viewport(),
                Box::new(IncrementalSceneKeyAllocator::new()),
                OverlayOwnerLifecyclePort::new(OWNER_ONE, InteractionDomainId::PRIMARY, vec![])
                    .unwrap(),
                PORT_ONE,
                &[FocusDomainAddition::new(FOCUS_ONE, None)],
                FOCUS_ONE,
            )
            .unwrap()
            .commit();
        self.application
            .prepare_root_admission(
                &mut self.interaction_domains,
                &mut self.registry,
                &mut self.compositor,
                InteractionDomainId::PRIMARY,
                ROOT_TWO,
                NavigationHost::new(NavigationNodeId(102)),
                viewport(),
                Box::new(IncrementalSceneKeyAllocator::new()),
                self.owner_two.clone(),
                PORT_TWO,
                &[
                    FocusDomainAddition::new(FOCUS_TWO, None),
                    FocusDomainAddition::new(OVERLAY_FOCUS_TWO, Some(FOCUS_TWO)),
                ],
                FOCUS_TWO,
            )
            .unwrap()
            .commit();
        let guard = self
            .overlay_domains
            .begin_domain_transition(InteractionDomainId::PRIMARY)
            .unwrap();
        self.application
            .navigation_domains()
            .prime_root_guarded(&guard, ROOT_ONE)
            .unwrap();
    }

    fn delegate_to_root_two(&mut self) {
        self.application
            .prepare_delegation(
                &mut self.interaction_domains,
                &mut self.registry,
                &mut self.compositor,
                InteractionDomainId::PRIMARY,
                ROOT_ONE,
                ROOT_TWO,
                halcyon::core::overlay_domain_coordinator::OverlayDisposition::Dismiss,
            )
            .unwrap()
            .commit();
    }
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

fn install_projection(
    setup: &Setup,
    bindings: &FrameProjectionBindingRegistry,
    tree: &mut WidgetTree,
    key: ScreenHostProjectionKey,
) {
    let resolver = halcyon::core::focus_authority::FocusResolver::new();
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    bindings
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &setup.interaction_domains,
                &setup.overlay_domains,
                &resolver,
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
        .unwrap();
}

#[test]
fn navigation_domain_retirement_prepared_drop_restores_every_reserved_authority() {
    let mut setup = Setup::new();
    setup.admit_roots();
    assert_eq!(
        setup.overlay_domains.unregister_owner(OWNER_TWO),
        Err(OverlayDomainError::ApplicationLifecycleRequired(OWNER_TWO))
    );
    let original_surface_count = setup.compositor.surface_count();
    let bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    install_projection(&setup, &bindings, &mut tree, key);
    screen_host_mut(&mut tree, screen_host_node).set_projected_view(ProjectedSurfaceView {
        navigation_contexts: vec![],
        overlay_surfaces: vec![],
        focused_surface: None,
    });

    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &setup.compositor,
        &mut tree,
    );
    let prepared = setup
        .application
        .prepare_domain_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &bindings,
            vec![window],
            InteractionDomainId::PRIMARY,
        )
        .unwrap();
    assert_eq!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, None),
        RoutedNavigationCandidate::Retry
    );
    assert!(matches!(
        setup
            .overlay_domains
            .begin_domain_transition(InteractionDomainId::PRIMARY),
        Err(OverlayDomainError::Busy(InteractionDomainId::PRIMARY))
    ));
    drop(prepared);

    assert_eq!(setup.registry.root_count(), 2);
    assert_eq!(setup.compositor.surface_count(), original_surface_count);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        3
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, None),
        RoutedNavigationCandidate::Snapshot(snapshot) if snapshot.root_id == ROOT_ONE
    ));
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(bindings.binding_guarded(&guard, key).unwrap().is_some());
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_some()
    );
}

#[test]
fn navigation_domain_retirement_missing_projection_window_rejects_without_mutation() {
    let mut setup = Setup::new();
    setup.admit_roots();
    let surface_count = setup.compositor.surface_count();
    let bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    install_projection(&setup, &bindings, &mut tree, key);

    let error = setup
        .application
        .prepare_domain_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &bindings,
            vec![],
            InteractionDomainId::PRIMARY,
        )
        .map(drop)
        .expect_err("complete domain projection access is mandatory");
    assert!(matches!(
        error,
        halcyon::core::navigation_application::NavigationDomainRetirementError::MissingProjectionWindow(
            WindowId::PRIMARY
        )
    ));
    assert_eq!(setup.registry.root_count(), 2);
    assert_eq!(setup.compositor.surface_count(), surface_count);
    assert_eq!(
        setup
            .interaction_domains
            .primary_state()
            .focus_manager
            .focus_domain_count(),
        3
    );
    assert!(matches!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, None),
        RoutedNavigationCandidate::Snapshot(snapshot) if snapshot.root_id == ROOT_ONE
    ));
    let guard = setup
        .overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert!(bindings.binding_guarded(&guard, key).unwrap().is_some());
}

#[test]
fn navigation_domain_retirement_commits_every_authority_and_domain_removal() {
    let mut setup = Setup::new();
    setup.admit_roots();
    setup.delegate_to_root_two();
    let root_two_surface = setup
        .registry
        .root(ROOT_TWO)
        .unwrap()
        .project()
        .unwrap()
        .navigation_surfaces[0];
    let overlay_surface = setup
        .compositor
        .create_overlay_surface(root_two_surface, 10_000, viewport(), SceneKey::new(10_000))
        .unwrap();
    setup
        .owner_two
        .register_inactive_stack(OverlayStackDescriptor::new(
            STACK_TWO,
            vec![OverlayFrameDescriptor::new(
                OverlayFrameId(42),
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

    let bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    install_projection(&setup, &bindings, &mut tree, key);
    screen_host_mut(&mut tree, screen_host_node).set_projected_view(ProjectedSurfaceView {
        navigation_contexts: vec![],
        overlay_surfaces: vec![overlay_surface],
        focused_surface: Some(overlay_surface),
    });
    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &setup.compositor,
        &mut tree,
    );
    let committed = setup
        .application
        .prepare_domain_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &bindings,
            vec![window],
            InteractionDomainId::PRIMARY,
        )
        .unwrap()
        .commit();

    assert_eq!(
        committed.root_ids,
        vec![NavigationRootId(1), NavigationRootId(2)]
    );
    assert_eq!(
        committed.unwound_delegation_frames,
        vec![DelegationFrame {
            source_root: ROOT_ONE,
            target_root: ROOT_TWO,
        }]
    );
    assert!(
        committed
            .terminated_surfaces
            .contains(&(overlay_surface, SurfaceTerminationReason::OverlayClosed))
    );
    assert_eq!(setup.registry.root_count(), 0);
    assert_eq!(setup.compositor.surface_count(), 0);
    assert!(
        setup
            .interaction_domains
            .state(InteractionDomainId::PRIMARY)
            .is_none()
    );
    assert_eq!(
        setup
            .interaction_domains
            .domain_for_window(WindowId::PRIMARY),
        None
    );
    assert_eq!(
        setup
            .application
            .navigation_domains()
            .route_snapshot(InteractionDomainId::PRIMARY, None),
        RoutedNavigationCandidate::RoutingUnavailable
    );
    assert!(matches!(
        setup
            .overlay_domains
            .begin_domain_transition(InteractionDomainId::PRIMARY),
        Err(OverlayDomainError::UnknownInteractionDomain(
            InteractionDomainId::PRIMARY
        ))
    ));
    assert_eq!(
        setup
            .overlay_domains
            .owner_registration_generation(OWNER_ONE),
        Err(OverlayDomainError::UnknownOwner(OWNER_ONE))
    );
    assert_eq!(
        setup
            .overlay_domains
            .owner_registration_generation(OWNER_TWO),
        Err(OverlayDomainError::UnknownOwner(OWNER_TWO))
    );
    assert!(bindings.guard_identity(key).is_none());
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_none()
    );
}

#[test]
fn navigation_domain_retirement_handles_empty_secondary_domain() {
    let mut interaction_domains = InteractionDomains::new();
    let window = WindowId(9);
    let domain = interaction_domains.register_window(window);
    let overlay_domains = OverlayDomainCoordinator::new().unwrap();
    overlay_domains
        .register_domain(&interaction_domains, domain)
        .unwrap();
    let application = NavigationApplicationCoordinator::new(overlay_domains.clone());
    application
        .navigation_domains()
        .register_domain(&interaction_domains, domain)
        .unwrap();
    let mut registry = NavigationRootRegistry::new();
    let mut compositor = Compositor::new(viewport());
    let bindings = FrameProjectionBindingRegistry::new(&overlay_domains);

    let committed = application
        .prepare_domain_retirement(
            &mut interaction_domains,
            &mut registry,
            &mut compositor,
            &bindings,
            vec![],
            domain,
        )
        .unwrap()
        .commit();

    assert_eq!(committed.interaction_domain_id, domain);
    assert!(committed.root_ids.is_empty());
    assert!(committed.terminated_surfaces.is_empty());
    assert!(committed.unwound_delegation_frames.is_empty());
    assert!(interaction_domains.state(domain).is_none());
    assert_eq!(interaction_domains.domain_for_window(window), None);
    assert!(matches!(
        overlay_domains.begin_domain_transition(domain),
        Err(OverlayDomainError::UnknownInteractionDomain(actual)) if actual == domain
    ));
}

#[test]
fn navigation_domain_retirement_unregisters_inactive_domain_local_owners() {
    let mut setup = Setup::new();
    setup.admit_roots();
    setup
        .overlay_domains
        .register_owner(
            OverlayOwnerLifecyclePort::new(
                DOMAIN_LOCAL_OWNER,
                InteractionDomainId::PRIMARY,
                vec![],
            )
            .unwrap(),
            OverlayActivationAdmission::DomainLocal,
        )
        .unwrap();
    let bindings = FrameProjectionBindingRegistry::new(&setup.overlay_domains);

    setup
        .application
        .prepare_domain_retirement(
            &mut setup.interaction_domains,
            &mut setup.registry,
            &mut setup.compositor,
            &bindings,
            vec![],
            InteractionDomainId::PRIMARY,
        )
        .unwrap()
        .commit();

    assert_eq!(
        setup
            .overlay_domains
            .owner_registration_generation(DOMAIN_LOCAL_OWNER),
        Err(OverlayDomainError::UnknownOwner(DOMAIN_LOCAL_OWNER))
    );
}
