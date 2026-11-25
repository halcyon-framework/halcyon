use halcyon::compositor::{Compositor, ProjectedSurfaceView};
use halcyon::core::event::{InteractionDomainId, InteractionDomains, WindowId};
use halcyon::core::focus_authority::{FocusDomainAddition, FocusDomainId, FocusResolver};
use halcyon::core::frame_projection_binding_registry::{
    FrameProjectionAuthorityAccess, FrameProjectionBinding, FrameProjectionBindingError,
    FrameProjectionBindingRegistry, FrameProjectionWindowAccess, NavigationProjectionSource,
    ScreenHostProjectionKey,
};
use halcyon::core::math::RectF;
use halcyon::core::navigation_application::{
    NavigationApplicationCoordinator, RoutedNavigationSnapshotResult,
    StandaloneNavigationSnapshotResult,
};
use halcyon::core::navigation_domain_router::NavigationRootFocusBindingPortId;
use halcyon::core::navigation_orchestrator::{
    IncrementalSceneKeyAllocator, NavigationRootId, NavigationRootRegistry,
    NavigationSurfaceOrchestrator, StepTransition,
};
use halcyon::core::overlay_domain_coordinator::{
    OverlayDomainCoordinator, OverlayOwnerId, OverlayOwnerLifecyclePort,
};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::model::navigation_host::{
    NavigationAuthorizationResult, NavigationDecision, NavigationExecutionError, NavigationHost,
    NavigationIntent, NavigationNodeId, NavigationOutcome,
};
use halcyon::widgets::screen_host::ScreenHost;

const ROOT: NavigationRootId = NavigationRootId(1);
const OWNER: OverlayOwnerId = OverlayOwnerId(2);
const PORT: NavigationRootFocusBindingPortId = NavigationRootFocusBindingPortId(3);
const FOCUS: FocusDomainId = FocusDomainId(4);

fn viewport() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
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

fn empty_view() -> ProjectedSurfaceView {
    ProjectedSurfaceView {
        navigation_contexts: Vec::new(),
        overlay_surfaces: Vec::new(),
        focused_surface: None,
    }
}

#[test]
fn frame_projection_binding_registry_replacement_abort_commit_and_remove_are_atomic() {
    let interaction_domains = InteractionDomains::new();
    let coordinator = OverlayDomainCoordinator::new().unwrap();
    coordinator
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let focus_resolver = FocusResolver::new();
    let registry = FrameProjectionBindingRegistry::new(&coordinator);
    let application = NavigationApplicationCoordinator::new(coordinator.clone());
    let mut compositor = Compositor::new(viewport());
    let mut orchestrator = NavigationSurfaceOrchestrator::new(
        NavigationHost::new(NavigationNodeId(1)),
        viewport(),
        &mut compositor,
        Box::new(IncrementalSceneKeyAllocator::new()),
    );
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    let initial_binding = FrameProjectionBinding::new(
        InteractionDomainId::PRIMARY,
        vec![NavigationProjectionSource::standalone(
            InteractionDomainId::PRIMARY,
            orchestrator.projection_handle(),
        )],
    );

    let host_snapshot = orchestrator.nav_host().snapshot().unwrap();
    let host_authorization = match orchestrator.nav_host().authorize(
        &host_snapshot,
        NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
        None,
        None,
    ) {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        result => panic!("expected host authorization, got {result:?}"),
    };
    let prepared_host = orchestrator
        .nav_host()
        .prepare_commit(host_authorization, host_snapshot.generation(), None)
        .unwrap();
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert_eq!(
        registry.install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &coordinator,
                &focus_resolver,
            ),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
            key,
            initial_binding.clone(),
            &guard,
        ),
        Err(FrameProjectionBindingError::Participation(
            halcyon::core::navigation_orchestrator::FrameProjectionParticipationError::Busy,
        ))
    );
    drop(guard);
    drop(prepared_host);

    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let first_epoch = registry
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &coordinator,
                &focus_resolver,
            ),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
            key,
            initial_binding.clone(),
            &guard,
        )
        .unwrap();
    assert_eq!(
        registry
            .domain_consumers(&guard, InteractionDomainId::PRIMARY)
            .unwrap(),
        [key].into()
    );
    drop(guard);

    assert_eq!(
        orchestrator.nav_host().execute(
            NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
            None,
            None,
        ),
        Err(NavigationExecutionError::Retry)
    );
    let direct = orchestrator.dispatch(
        NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
        &mut compositor,
    );
    assert_eq!(direct.decision, NavigationDecision::Retry);
    assert_eq!(direct.step_transition, StepTransition::NoOp);
    assert_eq!(orchestrator.current_node(), NavigationNodeId(1));

    let snapshot =
        match application.standalone_snapshot(&orchestrator, InteractionDomainId::PRIMARY) {
            StandaloneNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
            result => panic!("expected standalone snapshot, got {result:?}"),
        };
    let authorization = match application.authorize_standalone_navigation(
        &orchestrator,
        &snapshot,
        NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
        None,
        None,
    ) {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        result => panic!("expected standalone authorization, got {result:?}"),
    };
    let committed = application
        .prepare_standalone_navigation(
            &mut orchestrator,
            &mut compositor,
            &snapshot,
            authorization,
            None,
        )
        .unwrap()
        .commit();
    assert_eq!(
        committed.outcome,
        NavigationOutcome::Committed { redirected: false }
    );
    assert_eq!(orchestrator.current_node(), NavigationNodeId(2));

    screen_host_mut(&mut tree, screen_host_node).set_projected_view(empty_view());
    let mut other_compositor = Compositor::new(viewport());
    let other_orchestrator = NavigationSurfaceOrchestrator::new(
        NavigationHost::new(NavigationNodeId(2)),
        viewport(),
        &mut other_compositor,
        Box::new(IncrementalSceneKeyAllocator::new()),
    );
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &compositor,
        &mut tree,
    );
    assert_eq!(
        registry
            .prepare_remove_or_replace_guarded(
                FrameProjectionAuthorityAccess::new(
                    &interaction_domains,
                    &coordinator,
                    &focus_resolver,
                ),
                &window,
                key,
                first_epoch,
                Some(FrameProjectionBinding::new(
                    InteractionDomainId::PRIMARY,
                    vec![NavigationProjectionSource::standalone(
                        InteractionDomainId::PRIMARY,
                        other_orchestrator.projection_handle(),
                    )],
                )),
                &guard,
            )
            .unwrap_err(),
        FrameProjectionBindingError::WrongCompositor
    );
    drop(window);
    assert_eq!(
        registry
            .binding_guarded(&guard, key)
            .unwrap()
            .unwrap()
            .epoch(),
        first_epoch
    );
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_some()
    );

    let unavailable_handle = {
        let unavailable_orchestrator = NavigationSurfaceOrchestrator::new(
            NavigationHost::new(NavigationNodeId(3)),
            viewport(),
            &mut compositor,
            Box::new(IncrementalSceneKeyAllocator::new()),
        );
        unavailable_orchestrator.projection_handle()
    };
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &compositor,
        &mut tree,
    );
    assert_eq!(
        registry
            .prepare_remove_or_replace_guarded(
                FrameProjectionAuthorityAccess::new(
                    &interaction_domains,
                    &coordinator,
                    &focus_resolver,
                ),
                &window,
                key,
                first_epoch,
                Some(FrameProjectionBinding::new(
                    InteractionDomainId::PRIMARY,
                    vec![NavigationProjectionSource::standalone(
                        InteractionDomainId::PRIMARY,
                        unavailable_handle,
                    )],
                )),
                &guard,
            )
            .unwrap_err(),
        FrameProjectionBindingError::SourceUnavailable
    );
    drop(window);
    assert_eq!(
        registry
            .binding_guarded(&guard, key)
            .unwrap()
            .unwrap()
            .epoch(),
        first_epoch
    );
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_some()
    );

    screen_host_mut(&mut tree, screen_host_node).set_projected_view(empty_view());
    let replacement = FrameProjectionBinding::new(InteractionDomainId::PRIMARY, Vec::new());
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &compositor,
        &mut tree,
    );
    let prepared = registry
        .prepare_remove_or_replace_guarded(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &coordinator,
                &focus_resolver,
            ),
            &window,
            key,
            first_epoch,
            Some(replacement.clone()),
            &guard,
        )
        .unwrap();
    drop(prepared);
    drop(window);
    assert_eq!(
        registry
            .binding_guarded(&guard, key)
            .unwrap()
            .unwrap()
            .epoch(),
        first_epoch
    );
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_some()
    );

    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &compositor,
        &mut tree,
    );
    let prepared = registry
        .prepare_remove_or_replace_guarded(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &coordinator,
                &focus_resolver,
            ),
            &window,
            key,
            first_epoch,
            Some(replacement.clone()),
            &guard,
        )
        .unwrap();
    let mut window = window;
    let replacement_epoch = registry
        .commit_remove_or_replace_guarded(&mut window, &guard, prepared)
        .unwrap();
    assert_ne!(replacement_epoch, first_epoch);
    assert_eq!(
        registry
            .binding_guarded(&guard, key)
            .unwrap()
            .unwrap()
            .binding(),
        &replacement
    );
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_none()
    );
    let direct = orchestrator.dispatch(
        NavigationIntent::NavigateTo(NavigationNodeId(3).into()),
        &mut compositor,
    );
    assert_eq!(direct.decision, NavigationDecision::Allow);
    assert_eq!(orchestrator.current_node(), NavigationNodeId(3));

    screen_host_mut(&mut tree, screen_host_node).set_projected_view(empty_view());
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    registry
        .remove(
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
            key,
            &guard,
        )
        .unwrap();
    assert_eq!(registry.binding_guarded(&guard, key).unwrap(), None);
    assert!(
        registry
            .domain_consumers(&guard, InteractionDomainId::PRIMARY)
            .unwrap()
            .is_empty()
    );
    drop(guard);
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_none()
    );
}

#[test]
fn frame_projection_binding_registry_tracks_registered_root_consumers() {
    let mut interaction_domains = InteractionDomains::new();
    let overlay_domains = OverlayDomainCoordinator::new().unwrap();
    overlay_domains
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let focus_resolver = FocusResolver::new();
    let application = NavigationApplicationCoordinator::new(overlay_domains.clone());
    application
        .navigation_domains()
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let mut navigation_roots = NavigationRootRegistry::new();
    let mut compositor = Compositor::new(viewport());
    application
        .prepare_root_admission(
            &mut interaction_domains,
            &mut navigation_roots,
            &mut compositor,
            InteractionDomainId::PRIMARY,
            ROOT,
            NavigationHost::new(NavigationNodeId(1)),
            viewport(),
            Box::new(IncrementalSceneKeyAllocator::new()),
            OverlayOwnerLifecyclePort::new(OWNER, InteractionDomainId::PRIMARY, vec![]).unwrap(),
            PORT,
            &[FocusDomainAddition::new(FOCUS, None)],
            FOCUS,
        )
        .unwrap()
        .commit();

    let projection_bindings = FrameProjectionBindingRegistry::new(&overlay_domains);
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let second_screen_host_node =
        tree.add_child(screen_host_node, Box::new(ScreenHost::new()), None, 0);
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    let second_key = ScreenHostProjectionKey::new(WindowId::PRIMARY, second_screen_host_node);
    let binding = FrameProjectionBinding::new(
        InteractionDomainId::PRIMARY,
        vec![NavigationProjectionSource::registered_root(ROOT)],
    );
    let guard = overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    let epoch = projection_bindings
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &overlay_domains,
                &focus_resolver,
            )
            .with_registered_roots(&navigation_roots, application.navigation_domains()),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
            key,
            binding.clone(),
            &guard,
        )
        .unwrap();
    let second_epoch = projection_bindings
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &overlay_domains,
                &focus_resolver,
            )
            .with_registered_roots(&navigation_roots, application.navigation_domains()),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
            second_key,
            binding,
            &guard,
        )
        .unwrap();
    assert_eq!(
        projection_bindings.root_consumers(&guard, ROOT).unwrap(),
        [key, second_key].into()
    );
    drop(guard);

    let direct = navigation_roots
        .dispatch(
            ROOT,
            NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
            &mut compositor,
        )
        .unwrap();
    assert_eq!(direct.decision, NavigationDecision::Retry);
    assert_eq!(
        navigation_roots.root(ROOT).unwrap().current_node(),
        NavigationNodeId(1)
    );

    let snapshot = match application.route_snapshot(
        &navigation_roots,
        InteractionDomainId::PRIMARY,
        Some(ROOT),
    ) {
        RoutedNavigationSnapshotResult::Snapshot(snapshot) => snapshot,
        result => panic!("expected routed snapshot, got {result:?}"),
    };
    let authorization = match application.authorize_routed_navigation(
        &navigation_roots,
        &snapshot,
        NavigationIntent::NavigateTo(NavigationNodeId(2).into()),
        None,
        None,
    ) {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        result => panic!("expected routed authorization, got {result:?}"),
    };
    application
        .prepare_routed_navigation(
            &mut interaction_domains,
            &mut navigation_roots,
            &mut compositor,
            &snapshot,
            authorization,
            None,
            FOCUS,
        )
        .unwrap()
        .commit();
    assert_eq!(
        navigation_roots.root(ROOT).unwrap().current_node(),
        NavigationNodeId(2)
    );

    let guard = overlay_domains
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();

    screen_host_mut(&mut tree, screen_host_node).set_projected_view(empty_view());
    screen_host_mut(&mut tree, second_screen_host_node).set_projected_view(empty_view());
    let replacement = FrameProjectionBinding::new(InteractionDomainId::PRIMARY, Vec::new());
    let prepared = {
        let window = FrameProjectionWindowAccess::new(
            WindowId::PRIMARY,
            InteractionDomainId::PRIMARY,
            &compositor,
            &mut tree,
        );
        projection_bindings
            .prepare_remove_or_replace_guarded(
                FrameProjectionAuthorityAccess::new(
                    &interaction_domains,
                    &overlay_domains,
                    &focus_resolver,
                )
                .with_registered_roots(&navigation_roots, application.navigation_domains()),
                &window,
                key,
                epoch,
                Some(replacement.clone()),
                &guard,
            )
            .unwrap()
    };
    let second_prepared = {
        let window = FrameProjectionWindowAccess::new(
            WindowId::PRIMARY,
            InteractionDomainId::PRIMARY,
            &compositor,
            &mut tree,
        );
        projection_bindings
            .prepare_remove_or_replace_guarded(
                FrameProjectionAuthorityAccess::new(
                    &interaction_domains,
                    &overlay_domains,
                    &focus_resolver,
                )
                .with_registered_roots(&navigation_roots, application.navigation_domains()),
                &window,
                second_key,
                second_epoch,
                Some(replacement),
                &guard,
            )
            .unwrap()
    };
    assert_eq!(
        projection_bindings.root_consumers(&guard, ROOT).unwrap(),
        [key, second_key].into()
    );
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_some()
    );
    assert!(
        screen_host(&tree, second_screen_host_node)
            .projected_view()
            .is_some()
    );

    let mut window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &compositor,
        &mut tree,
    );
    projection_bindings.commit_remove_or_replace_guarded(&mut window, &guard, prepared);
    drop(window);
    assert_eq!(
        projection_bindings.root_consumers(&guard, ROOT).unwrap(),
        [second_key].into()
    );
    let direct = navigation_roots
        .dispatch(
            ROOT,
            NavigationIntent::NavigateTo(NavigationNodeId(3).into()),
            &mut compositor,
        )
        .unwrap();
    assert_eq!(direct.decision, NavigationDecision::Retry);
    let mut window = FrameProjectionWindowAccess::new(
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        &compositor,
        &mut tree,
    );
    projection_bindings.commit_remove_or_replace_guarded(&mut window, &guard, second_prepared);
    drop(window);
    assert!(
        projection_bindings
            .root_consumers(&guard, ROOT)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        projection_bindings
            .domain_consumers(&guard, InteractionDomainId::PRIMARY)
            .unwrap(),
        [key, second_key].into()
    );
    assert!(
        screen_host(&tree, screen_host_node)
            .projected_view()
            .is_none()
    );
    assert!(
        screen_host(&tree, second_screen_host_node)
            .projected_view()
            .is_none()
    );
    drop(guard);
    let direct = navigation_roots
        .dispatch(
            ROOT,
            NavigationIntent::NavigateTo(NavigationNodeId(3).into()),
            &mut compositor,
        )
        .unwrap();
    assert_eq!(direct.decision, NavigationDecision::Retry);
    assert_eq!(
        navigation_roots.root(ROOT).unwrap().current_node(),
        NavigationNodeId(2)
    );
}

#[test]
fn frame_projection_binding_registry_orphan_invalidation_removes_all_indexes() {
    let interaction_domains = InteractionDomains::new();
    let coordinator = OverlayDomainCoordinator::new().unwrap();
    coordinator
        .register_domain(&interaction_domains, InteractionDomainId::PRIMARY)
        .unwrap();
    let focus_resolver = FocusResolver::new();
    let registry = FrameProjectionBindingRegistry::new(&coordinator);
    let compositor = Compositor::new(viewport());
    let (mut tree, screen_host_node) = tree_with_screen_host();
    let key = ScreenHostProjectionKey::new(WindowId::PRIMARY, screen_host_node);
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    registry
        .install_or_replace(
            FrameProjectionAuthorityAccess::new(
                &interaction_domains,
                &coordinator,
                &focus_resolver,
            ),
            FrameProjectionWindowAccess::new(
                WindowId::PRIMARY,
                InteractionDomainId::PRIMARY,
                &compositor,
                &mut tree,
            ),
            key,
            FrameProjectionBinding::new(InteractionDomainId::PRIMARY, Vec::new()),
            &guard,
        )
        .unwrap();
    drop(guard);

    tree.remove_node(screen_host_node);
    let guard = coordinator
        .begin_domain_transition(InteractionDomainId::PRIMARY)
        .unwrap();
    assert_eq!(
        registry.invalidate_missing_screen_host(key, &guard),
        Err(FrameProjectionBindingError::MissingScreenHostInvalidated(
            key
        ))
    );
    assert_eq!(registry.binding_guarded(&guard, key).unwrap(), None);
    assert!(
        registry
            .domain_consumers(&guard, InteractionDomainId::PRIMARY)
            .unwrap()
            .is_empty()
    );
}
